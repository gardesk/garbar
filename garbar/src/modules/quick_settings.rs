//! Quick settings trigger module
//!
//! Displays a settings icon that triggers gartray's quick settings panel via IPC.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use crate::config::QuickSettingsConfig;
use crate::render::{Block, BlockStyle, Color, Padding};

use super::{Module, ModuleOutput};

/// Get gartray socket path
fn gartray_socket_path() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("gartray.sock")
}

/// Check if gartray daemon is running by testing socket connectivity
fn is_gartray_running() -> bool {
    let path = gartray_socket_path();
    if !path.exists() {
        return false;
    }
    // Actually try to connect - socket file can exist without daemon
    UnixStream::connect(&path).is_ok()
}

/// Send command to gartray daemon with position
fn send_gartray_command_at(command: &str, x: i32, y: i32) -> Result<(), String> {
    let path = gartray_socket_path();

    if !path.exists() {
        return Err("gartray daemon not running".to_string());
    }

    let mut stream = UnixStream::connect(&path)
        .map_err(|e| format!("Failed to connect: {}", e))?;

    // Send JSON command with coordinates
    let cmd_json = match command {
        "toggle" => format!(r#"{{"command":"toggle","x":{},"y":{}}}"#, x, y),
        "show" => format!(r#"{{"command":"show","x":{},"y":{}}}"#, x, y),
        "hide" => r#"{"command":"hide"}"#.to_string(),
        _ => return Err(format!("Unknown command: {}", command)),
    };

    writeln!(stream, "{}", cmd_json)
        .map_err(|e| format!("Failed to send: {}", e))?;
    stream.flush()
        .map_err(|e| format!("Failed to flush: {}", e))?;

    // Read response
    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line)
        .map_err(|e| format!("Failed to read response: {}", e))?;

    // Parse response
    if response_line.contains("\"success\":true") {
        Ok(())
    } else {
        Err("Command failed".to_string())
    }
}

/// Quick settings trigger module
pub struct QuickSettingsModule {
    config: QuickSettingsConfig,
    panel_visible: bool,
}

impl QuickSettingsModule {
    pub fn new(config: &QuickSettingsConfig) -> Self {
        Self {
            config: config.clone(),
            panel_visible: false,
        }
    }
}

impl Module for QuickSettingsModule {
    fn name(&self) -> &'static str {
        "quick_settings"
    }

    fn output(&self) -> ModuleOutput {
        if !self.config.enabled {
            return ModuleOutput::empty();
        }

        // Check if gartray is running
        let available = is_gartray_running();

        // Use the configured icon or default
        let icon = if self.panel_visible {
            &self.config.icon_active
        } else {
            &self.config.icon
        };

        // Dim the icon if gartray isn't running
        let foreground = if available {
            if self.panel_visible {
                Color::from_hex(&self.config.active_foreground).unwrap_or(Color::rgb(0.38, 0.68, 0.93)) // cyan-ish
            } else {
                Color::from_hex(&self.config.foreground).unwrap_or(Color::white())
            }
        } else {
            Color::from_hex("#555555").unwrap_or(Color::rgb(0.33, 0.33, 0.33)) // gray
        };

        ModuleOutput::single(
            Block::new(icon)
                .with_style(
                    BlockStyle::new()
                        .with_foreground(foreground)
                        .with_padding(Padding::horizontal(8.0)),
                )
                .with_min_width(32.0), // Ensure clickable area
        )
    }

    fn interval(&self) -> u64 {
        // Check gartray status every 5 seconds
        5000
    }

    fn update(&mut self) {
        // We could check panel state here via IPC status command
        // For now, just track local state
    }

    fn on_click(&mut self, button: u8, _block_index: usize, x: i16, y: i16) {
        // x, y are now root (absolute screen) coordinates from ButtonPress event
        let screen_x = x as i32;
        let screen_y = y as i32;

        tracing::info!("Quick settings on_click: button={}, screen=({}, {})", button, screen_x, screen_y);

        match button {
            1 => {
                // Left click - toggle panel at click position
                match send_gartray_command_at("toggle", screen_x, screen_y) {
                    Ok(()) => {
                        self.panel_visible = !self.panel_visible;
                        tracing::info!("Toggled quick settings panel at ({}, {})", screen_x, screen_y);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to toggle panel: {}", e);
                    }
                }
            }
            3 => {
                // Right click - same as left click for now
                if let Err(e) = send_gartray_command_at("toggle", screen_x, screen_y) {
                    tracing::warn!("Failed to toggle panel: {}", e);
                }
            }
            _ => {}
        }
    }
}
