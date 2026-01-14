//! IPC server for garbar
//!
//! Listens on a Unix socket for commands from garbarctl.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use tracing::{debug, error, info, warn};

use super::protocol::{Command, Response};

/// IPC server that listens for commands
pub struct IpcServer {
    socket_path: PathBuf,
    command_tx: Sender<Command>,
    listener_handle: Option<thread::JoinHandle<()>>,
}

impl IpcServer {
    /// Create and start a new IPC server
    pub fn new() -> (Self, Receiver<Command>) {
        let socket_path = Self::socket_path();
        let (command_tx, command_rx) = mpsc::channel();

        let server = Self {
            socket_path,
            command_tx,
            listener_handle: None,
        };

        (server, command_rx)
    }

    /// Get the socket path
    pub fn socket_path() -> PathBuf {
        std::env::var("XDG_RUNTIME_DIR")
            .map(|dir| PathBuf::from(dir).join("garbar.sock"))
            .unwrap_or_else(|_| PathBuf::from("/tmp/garbar.sock"))
    }

    /// Start listening for connections
    pub fn start(&mut self) -> std::io::Result<()> {
        // Remove existing socket if present
        if self.socket_path.exists() {
            fs::remove_file(&self.socket_path)?;
        }

        // Create listener
        let listener = UnixListener::bind(&self.socket_path)?;
        info!("IPC server listening on {:?}", self.socket_path);

        // Set non-blocking to allow periodic checking
        listener.set_nonblocking(true)?;

        let socket_path = self.socket_path.clone();
        let command_tx = self.command_tx.clone();

        let handle = thread::spawn(move || {
            Self::listener_loop(listener, command_tx, socket_path);
        });

        self.listener_handle = Some(handle);
        Ok(())
    }

    /// Main listener loop (runs in background thread)
    fn listener_loop(listener: UnixListener, command_tx: Sender<Command>, _socket_path: PathBuf) {
        loop {
            match listener.accept() {
                Ok((stream, _addr)) => {
                    debug!("IPC client connected");
                    let tx = command_tx.clone();
                    thread::spawn(move || {
                        Self::handle_client(stream, tx);
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No connection pending, sleep briefly
                    thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    error!("Error accepting IPC connection: {}", e);
                    thread::sleep(Duration::from_millis(100));
                }
            }

            // Check if channel is closed (daemon shutting down)
            if command_tx.send(Command::Status).is_err() {
                info!("IPC server shutting down (channel closed)");
                break;
            }
        }
    }

    /// Handle a single client connection
    fn handle_client(mut stream: UnixStream, command_tx: Sender<Command>) {
        // Set read timeout
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));

        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();

        match reader.read_line(&mut line) {
            Ok(0) => {
                debug!("IPC client disconnected");
                return;
            }
            Ok(_) => {
                let line = line.trim();
                debug!("IPC received: {}", line);

                let response = match serde_json::from_str::<Command>(line) {
                    Ok(cmd) => {
                        // Don't send Status commands through the channel
                        // (they're just for checking if channel is alive)
                        match &cmd {
                            Command::Status => Response::ok(),
                            _ => {
                                if command_tx.send(cmd).is_ok() {
                                    Response::ok()
                                } else {
                                    Response::err("Failed to send command to daemon")
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Invalid IPC command: {}", e);
                        Response::err(format!("Invalid command: {}", e))
                    }
                };

                // Send response
                if let Ok(json) = serde_json::to_string(&response) {
                    let _ = writeln!(stream, "{}", json);
                }
            }
            Err(e) => {
                warn!("Error reading from IPC client: {}", e);
            }
        }
    }

    /// Stop the server
    pub fn stop(&mut self) {
        // Drop the sender to signal shutdown
        // The listener thread will detect this and exit

        // Remove socket file
        if self.socket_path.exists() {
            let _ = fs::remove_file(&self.socket_path);
        }

        // Wait for thread to finish
        if let Some(handle) = self.listener_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        // Clean up socket file
        if self.socket_path.exists() {
            let _ = fs::remove_file(&self.socket_path);
            debug!("Removed IPC socket file");
        }
    }
}
