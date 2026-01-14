//! garbarctl - CLI control tool for garbar
//!
//! Communicates with the garbar daemon via Unix socket IPC.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "garbarctl")]
#[command(author, version, about = "Control tool for garbar")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show the bar
    Show,
    /// Hide the bar
    Hide,
    /// Toggle bar visibility
    Toggle,
    /// Reload configuration
    Reload,
    /// Gracefully quit the daemon
    Quit,
    /// Get bar status
    Status,
    /// Force update a specific module
    Update {
        /// Module name to update
        module: String,
    },
}

/// Command to send to garbar
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "command", rename_all = "snake_case")]
enum IpcCommand {
    Show,
    Hide,
    Toggle,
    Reload,
    Quit,
    Status,
    UpdateModule { module: String },
}

/// Response from garbar
#[derive(Debug, Clone, Deserialize)]
struct IpcResponse {
    success: bool,
    error: Option<String>,
    data: Option<serde_json::Value>,
}

/// Get the garbar socket path
fn socket_path() -> PathBuf {
    std::env::var("XDG_RUNTIME_DIR")
        .map(|dir| PathBuf::from(dir).join("garbar.sock"))
        .unwrap_or_else(|_| PathBuf::from("/tmp/garbar.sock"))
}

/// Send a command to the daemon and get the response
fn send_command(cmd: IpcCommand) -> Result<IpcResponse> {
    let path = socket_path();

    // Connect to socket
    let mut stream = UnixStream::connect(&path)
        .with_context(|| format!("Failed to connect to garbar at {:?}. Is garbar running?", path))?;

    // Set timeouts
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    // Send command as JSON line
    let json = serde_json::to_string(&cmd)?;
    writeln!(stream, "{}", json)?;
    stream.flush()?;

    // Read response
    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader
        .read_line(&mut response_line)
        .context("Failed to read response from garbar")?;

    // Parse response
    let response: IpcResponse = serde_json::from_str(&response_line)
        .context("Failed to parse response from garbar")?;

    Ok(response)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let cmd = match cli.command {
        Commands::Show => IpcCommand::Show,
        Commands::Hide => IpcCommand::Hide,
        Commands::Toggle => IpcCommand::Toggle,
        Commands::Reload => IpcCommand::Reload,
        Commands::Quit => IpcCommand::Quit,
        Commands::Status => IpcCommand::Status,
        Commands::Update { module } => IpcCommand::UpdateModule { module },
    };

    match send_command(cmd) {
        Ok(response) => {
            if response.success {
                if let Some(data) = response.data {
                    println!("{}", serde_json::to_string_pretty(&data)?);
                } else {
                    println!("OK");
                }
            } else {
                let error = response.error.unwrap_or_else(|| "Unknown error".to_string());
                eprintln!("Error: {}", error);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
