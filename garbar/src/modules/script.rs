//! Script module - runs arbitrary commands and displays output
//!
//! This is the core of garbar's extensibility. Users can define any number
//! of script modules that run shell commands and display their output.
//!
//! Example configs:
//! ```toml
//! [modules.script.weather]
//! exec = "curl -s 'wttr.in?format=1'"
//! interval = 300
//!
//! [modules.script.music]
//! exec = "playerctl metadata --format '{{artist}} - {{title}}'"
//! interval = 1
//!
//! [modules.script.volume]
//! exec = "pamixer --get-volume-human"
//! interval = 0
//! tail = false
//! ```

use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::config::ScriptConfig;
use crate::render::{Block, BlockStyle, Color, Padding};

use super::{Module, ModuleOutput};

pub struct ScriptModule {
    name: String,
    config: ScriptConfig,
    output: Arc<Mutex<String>>,
    foreground: Color,
    pending: Arc<Mutex<bool>>,
}

impl ScriptModule {
    pub fn new(name: &str, config: &ScriptConfig) -> Self {
        Self {
            name: format!("script:{}", name),
            config: config.clone(),
            output: Arc::new(Mutex::new(String::new())),
            foreground: Color::white(),
            pending: Arc::new(Mutex::new(false)),
        }
    }

    /// Spawn async command execution - doesn't block
    fn spawn_command(&self) {
        // Don't spawn if already pending
        {
            let mut pending = self.pending.lock().unwrap();
            if *pending {
                return;
            }
            *pending = true;
        }

        let exec = self.config.exec.clone();
        let format = self.config.format.clone();
        let output = Arc::clone(&self.output);
        let pending = Arc::clone(&self.pending);
        let name = self.name.clone();

        thread::spawn(move || {
            let result = Command::new("sh")
                .arg("-c")
                .arg(&exec)
                .output();

            let new_output = match result {
                Ok(cmd_output) if cmd_output.status.success() => {
                    String::from_utf8(cmd_output.stdout)
                        .ok()
                        .map(|s| {
                            let trimmed = s.trim().to_string();
                            if format.is_empty() {
                                trimmed
                            } else {
                                format.replace("{output}", &trimmed)
                            }
                        })
                }
                Ok(cmd_output) => {
                    tracing::debug!(
                        "Script '{}' failed: {}",
                        name,
                        String::from_utf8_lossy(&cmd_output.stderr)
                    );
                    None
                }
                Err(e) => {
                    tracing::debug!("Script '{}' exec error: {}", name, e);
                    None
                }
            };

            // Update output if we got a result
            if let Some(text) = new_output {
                if let Ok(mut out) = output.lock() {
                    *out = text;
                }
            }

            // Mark as not pending
            if let Ok(mut p) = pending.lock() {
                *p = false;
            }
        });
    }
}

impl Module for ScriptModule {
    fn name(&self) -> &'static str {
        // This is a bit of a hack - we leak the string to get a static lifetime
        // In practice this is fine since modules live for the program duration
        Box::leak(self.name.clone().into_boxed_str())
    }

    fn output(&self) -> ModuleOutput {
        let output = self.output.lock().unwrap();
        if output.is_empty() {
            return ModuleOutput::empty();
        }

        let mut style = BlockStyle::new()
            .with_foreground(self.foreground.clone())
            .with_padding(Padding::horizontal(8.0));

        // Apply font size if configured
        if let Some(size) = self.config.font_size {
            style = style.with_font_size(size);
        }

        ModuleOutput::single(Block::new(&*output).with_style(style))
    }

    fn interval(&self) -> u64 {
        (self.config.interval as u64) * 1000
    }

    fn update(&mut self) {
        // Spawn command asynchronously - doesn't block
        self.spawn_command();
    }

    fn on_click(&mut self, button: u8, _block_index: usize, _x: i16, _y: i16) {
        let cmd = match button {
            1 => &self.config.click_left,
            2 => &self.config.click_middle,
            3 => &self.config.click_right,
            _ => return,
        };

        if !cmd.is_empty() {
            // Run click handler in background
            let cmd = cmd.clone();
            thread::spawn(move || {
                let _ = Command::new("sh").arg("-c").arg(&cmd).status();
            });
        }
    }

    fn on_scroll(&mut self, up: bool, _block_index: usize, _x: i16, _y: i16) {
        let cmd = if up {
            &self.config.scroll_up
        } else {
            &self.config.scroll_down
        };

        if !cmd.is_empty() {
            // Run scroll handler in background
            let cmd = cmd.clone();
            thread::spawn(move || {
                let _ = Command::new("sh").arg("-c").arg(&cmd).status();
            });
            // Trigger refresh after scroll action
            self.spawn_command();
        }
    }
}
