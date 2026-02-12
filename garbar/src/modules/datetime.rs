use chrono::Local;

use crate::config::DatetimeConfig;
use crate::render::{Block, BlockStyle, Color, Padding};

use super::{Module, ModuleOutput};

pub struct DatetimeModule {
    config: DatetimeConfig,
    current_text: String,
    use_alt_format: bool,
}

impl DatetimeModule {
    pub fn new(config: &DatetimeConfig) -> Self {
        // Don't call update() here - let the async event loop handle it
        Self {
            config: config.clone(),
            current_text: String::new(),
            use_alt_format: false,
        }
    }

    fn format_time(&self) -> String {
        let now = Local::now();
        let format = if self.use_alt_format {
            &self.config.format_alt
        } else {
            &self.config.format
        };
        now.format(format).to_string()
    }
}

impl Module for DatetimeModule {
    fn name(&self) -> &'static str {
        "datetime"
    }

    fn output(&self) -> ModuleOutput {
        let foreground = Color::from_hex("#e0e0e0").unwrap_or(Color::white());

        ModuleOutput::single(
            Block::new(&self.current_text).with_style(
                BlockStyle::new()
                    .with_foreground(foreground)
                    .with_padding(Padding::horizontal(8.0)),
            ),
        )
    }

    fn interval(&self) -> u64 {
        (self.config.interval as u64) * 1000
    }

    fn update(&mut self) -> bool {
        let new_text = self.format_time();
        if new_text != self.current_text {
            self.current_text = new_text;
            true
        } else {
            false
        }
    }

    fn on_click(&mut self, button: u8, _block_index: usize, _x: i16, _y: i16) {
        if button == 1 {
            // Left click toggles alt format
            self.use_alt_format = !self.use_alt_format;
            self.update();
        }
    }
}
