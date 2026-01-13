use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

use crate::daemon::signals::SignalHandler;
use crate::x11::{Connection, BarWindow};

/// Get the path to the PID file
fn pid_file_path() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("garbar.pid")
}

/// Check if an existing daemon is running
fn check_existing_daemon() -> Result<()> {
    let pid_path = pid_file_path();

    if pid_path.exists() {
        let pid_str = fs::read_to_string(&pid_path)?;
        let pid: i32 = pid_str.trim().parse()?;

        // Check if process is still running
        let proc_path = format!("/proc/{}", pid);
        if std::path::Path::new(&proc_path).exists() {
            anyhow::bail!(
                "garbar daemon already running (PID {}). \
                 If this is incorrect, remove {}",
                pid,
                pid_path.display()
            );
        } else {
            warn!("Removing stale PID file for PID {}", pid);
            fs::remove_file(&pid_path)?;
        }
    }

    Ok(())
}

/// Write the current process PID to the PID file
fn write_pid_file() -> Result<()> {
    let pid_path = pid_file_path();
    let pid = std::process::id();

    let mut file = fs::File::create(&pid_path)?;
    writeln!(file, "{}", pid)?;

    debug!("Wrote PID {} to {}", pid, pid_path.display());
    Ok(())
}

/// Remove the PID file
fn remove_pid_file() {
    let pid_path = pid_file_path();
    if let Err(e) = fs::remove_file(&pid_path) {
        warn!("Failed to remove PID file: {}", e);
    } else {
        debug!("Removed PID file");
    }
}

/// Main daemon state
pub struct DaemonState {
    conn: Connection,
    bar: BarWindow,
    signal_handler: SignalHandler,
    running: bool,
}

impl DaemonState {
    /// Create a new daemon state
    pub async fn new() -> Result<Self> {
        // Connect to X11
        let conn = Connection::new().context("Failed to connect to X11 display")?;
        info!("Connected to X11 display");

        // Create bar window
        let bar = BarWindow::new(&conn).context("Failed to create bar window")?;
        info!(
            "Created bar window: {}x{} at ({}, {})",
            bar.width, bar.height, bar.x, bar.y
        );

        // Set up signal handler
        let signal_handler = SignalHandler::new()?;

        Ok(Self {
            conn,
            bar,
            signal_handler,
            running: true,
        })
    }

    /// Run the main event loop
    pub async fn run(&mut self) -> Result<()> {
        info!("Entering main event loop");

        // Map the window to make it visible
        self.bar.map(&self.conn)?;
        self.conn.flush()?;

        while self.running {
            tokio::select! {
                // Handle signals
                signal = self.signal_handler.recv() => {
                    match signal {
                        Some(signals::Signal::Terminate) => {
                            info!("Received termination signal, shutting down");
                            self.running = false;
                        }
                        Some(signals::Signal::Reload) => {
                            info!("Received reload signal");
                            self.handle_reload().await?;
                        }
                        None => {
                            warn!("Signal handler closed unexpectedly");
                            self.running = false;
                        }
                    }
                }

                // Handle X11 events
                event = self.conn.next_event() => {
                    match event {
                        Ok(Some(event)) => {
                            self.handle_x11_event(event)?;
                        }
                        Ok(None) => {
                            // No event available, continue
                        }
                        Err(e) => {
                            error!("X11 event error: {}", e);
                            self.running = false;
                        }
                    }
                }
            }
        }

        info!("Event loop terminated");
        Ok(())
    }

    /// Handle X11 events
    fn handle_x11_event(&mut self, event: x11rb::protocol::Event) -> Result<()> {
        use x11rb::protocol::Event;

        match event {
            Event::Expose(e) if e.window == self.bar.window => {
                debug!("Expose event on bar window");
                self.bar.draw(&self.conn)?;
            }
            Event::ButtonPress(e) if e.event == self.bar.window => {
                debug!(
                    "Button {} pressed at ({}, {})",
                    e.detail, e.event_x, e.event_y
                );
                // TODO: Route to modules
            }
            Event::ConfigureNotify(e) if e.window == self.bar.window => {
                debug!("Configure notify: {}x{}", e.width, e.height);
            }
            Event::DestroyNotify(e) if e.window == self.bar.window => {
                warn!("Bar window destroyed externally");
                self.running = false;
            }
            _ => {
                // Ignore other events for now
            }
        }

        Ok(())
    }

    /// Handle configuration reload
    async fn handle_reload(&mut self) -> Result<()> {
        info!("Reloading configuration...");
        // TODO: Re-read config file and update modules
        Ok(())
    }
}

impl Drop for DaemonState {
    fn drop(&mut self) {
        debug!("DaemonState dropped, cleaning up");
    }
}

/// Run the daemon
pub async fn run(_config: Option<String>, _foreground: bool) -> Result<()> {
    // Check for existing daemon
    check_existing_daemon()?;

    // Write PID file
    write_pid_file()?;

    // Ensure PID file is removed on exit
    struct PidGuard;
    impl Drop for PidGuard {
        fn drop(&mut self) {
            remove_pid_file();
        }
    }
    let _pid_guard = PidGuard;

    // Create and run daemon
    let mut daemon = DaemonState::new().await?;
    daemon.run().await?;

    info!("Daemon shutdown complete");
    Ok(())
}

use super::signals;
