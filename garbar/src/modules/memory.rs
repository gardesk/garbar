use std::fs;

use crate::config::MemoryConfig;
use crate::render::{Block, BlockStyle, Color, Padding};

use super::{Module, ModuleOutput};

pub struct MemoryModule {
    config: MemoryConfig,
    total_kb: u64,
    available_kb: u64,
    used_kb: u64,
    percent: u32,
    use_alt_format: bool,
}

impl MemoryModule {
    pub fn new(config: &MemoryConfig) -> Self {
        // Don't call update() here - let the async event loop handle it
        Self {
            config: config.clone(),
            total_kb: 0,
            available_kb: 0,
            used_kb: 0,
            percent: 0,
            use_alt_format: false,
        }
    }

    fn read_meminfo(&mut self) {
        let contents = match fs::read_to_string("/proc/meminfo") {
            Ok(c) => c,
            Err(_) => return,
        };

        let mut total = 0u64;
        let mut available = 0u64;
        let mut free = 0u64;
        let mut buffers = 0u64;
        let mut cached = 0u64;

        for line in contents.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            let value: u64 = parts[1].parse().unwrap_or(0);

            match parts[0] {
                "MemTotal:" => total = value,
                "MemAvailable:" => available = value,
                "MemFree:" => free = value,
                "Buffers:" => buffers = value,
                "Cached:" => cached = value,
                _ => {}
            }
        }

        self.total_kb = total;
        // MemAvailable is preferred (kernel 3.14+), fall back to calculation
        self.available_kb = if available > 0 {
            available
        } else {
            free + buffers + cached
        };
        self.used_kb = total.saturating_sub(self.available_kb);

        self.percent = if total > 0 {
            ((self.used_kb as f64 / total as f64) * 100.0) as u32
        } else {
            0
        };
    }

    fn format_bytes(kb: u64) -> String {
        if kb >= 1024 * 1024 {
            format!("{:.1}G", kb as f64 / (1024.0 * 1024.0))
        } else if kb >= 1024 {
            format!("{:.0}M", kb as f64 / 1024.0)
        } else {
            format!("{}K", kb)
        }
    }

    fn format_output(&self) -> String {
        let format = if self.use_alt_format {
            &self.config.format_alt
        } else {
            &self.config.format
        };

        // Use fixed-width formatting (3 chars for 0-100%) to prevent bar from shifting
        format
            .replace("{percent}", &format!("{:3}", self.percent))
            .replace("{used}", &Self::format_bytes(self.used_kb))
            .replace("{total}", &Self::format_bytes(self.total_kb))
            .replace("{available}", &Self::format_bytes(self.available_kb))
    }

    fn get_color(&self) -> Color {
        if self.percent >= self.config.critical_threshold {
            Color::from_hex(&self.config.critical_foreground).unwrap_or(Color::red())
        } else if self.percent >= self.config.warning_threshold {
            Color::from_hex(&self.config.warning_foreground).unwrap_or(Color::yellow())
        } else {
            Color::from_hex("#98c379").unwrap_or(Color::white())
        }
    }
}

impl Module for MemoryModule {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn output(&self) -> ModuleOutput {
        ModuleOutput::single(
            Block::new(&self.format_output())
                .with_style(
                    BlockStyle::new()
                        .with_foreground(self.get_color())
                        .with_padding(Padding::horizontal(8.0)),
                )
                // Reserve space for "100%" to prevent shifting
                .with_min_width(70.0),
        )
    }

    fn interval(&self) -> u64 {
        (self.config.interval as u64) * 1000
    }

    fn update(&mut self) {
        self.read_meminfo();
    }

    fn on_click(&mut self, button: u8, _block_index: usize, _x: i16, _y: i16) {
        if button == 1 {
            self.use_alt_format = !self.use_alt_format;
        }
    }
}
