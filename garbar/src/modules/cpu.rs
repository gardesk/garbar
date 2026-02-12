use std::fs;

use crate::config::CpuConfig;
use crate::render::{Block, BlockStyle, Color, Padding};

use super::{Module, ModuleOutput};

/// CPU time values from /proc/stat
#[derive(Debug, Clone, Copy, Default)]
struct CpuTimes {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
}

impl CpuTimes {
    fn total(&self) -> u64 {
        self.user + self.nice + self.system + self.idle + self.iowait + self.irq + self.softirq + self.steal
    }

    fn active(&self) -> u64 {
        self.total() - self.idle - self.iowait
    }
}

pub struct CpuModule {
    config: CpuConfig,
    usage: u32,
    smoothed_usage: f64,  // EMA smoothed value
    prev_times: CpuTimes,
}

// Smoothing factor for exponential moving average (0.0-1.0)
// Lower = smoother but slower to respond, higher = more responsive but jumpier
const EMA_ALPHA: f64 = 0.1;

// Only update displayed value if it changes by more than this threshold
const UPDATE_THRESHOLD: u32 = 3;

impl CpuModule {
    pub fn new(config: &CpuConfig) -> Self {
        let mut module = Self {
            config: config.clone(),
            usage: 0,
            smoothed_usage: 0.0,
            prev_times: CpuTimes::default(),
        };
        // Initial read to establish baseline
        module.prev_times = module.read_cpu_times();
        module
    }

    fn read_cpu_times(&self) -> CpuTimes {
        let contents = match fs::read_to_string("/proc/stat") {
            Ok(c) => c,
            Err(_) => return CpuTimes::default(),
        };

        // First line is aggregate CPU stats
        let first_line = match contents.lines().next() {
            Some(l) => l,
            None => return CpuTimes::default(),
        };

        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() < 8 || parts[0] != "cpu" {
            return CpuTimes::default();
        }

        CpuTimes {
            user: parts[1].parse().unwrap_or(0),
            nice: parts[2].parse().unwrap_or(0),
            system: parts[3].parse().unwrap_or(0),
            idle: parts[4].parse().unwrap_or(0),
            iowait: parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0),
            irq: parts.get(6).and_then(|s| s.parse().ok()).unwrap_or(0),
            softirq: parts.get(7).and_then(|s| s.parse().ok()).unwrap_or(0),
            steal: parts.get(8).and_then(|s| s.parse().ok()).unwrap_or(0),
        }
    }

    fn calculate_usage(&mut self) -> u32 {
        let current = self.read_cpu_times();

        let total_diff = current.total().saturating_sub(self.prev_times.total());
        let active_diff = current.active().saturating_sub(self.prev_times.active());

        self.prev_times = current;

        if total_diff == 0 {
            return 0;
        }

        ((active_diff as f64 / total_diff as f64) * 100.0) as u32
    }

    fn format_output(&self) -> String {
        // Use fixed-width formatting (3 chars for 0-100%) to prevent bar from shifting
        self.config.format.replace("{usage}", &format!("{:3}", self.usage))
    }

    fn get_color(&self) -> Color {
        if self.usage >= self.config.critical_threshold {
            Color::from_hex(&self.config.critical_foreground).unwrap_or(Color::red())
        } else if self.usage >= self.config.warning_threshold {
            Color::from_hex(&self.config.warning_foreground).unwrap_or(Color::yellow())
        } else {
            Color::from_hex("#5294e2").unwrap_or(Color::white())
        }
    }
}

impl Module for CpuModule {
    fn name(&self) -> &'static str {
        "cpu"
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

    fn update(&mut self) -> bool {
        let raw_usage = self.calculate_usage() as f64;
        // Apply exponential moving average smoothing
        self.smoothed_usage = EMA_ALPHA * raw_usage + (1.0 - EMA_ALPHA) * self.smoothed_usage;

        // Only update displayed value if change exceeds threshold (hysteresis)
        let new_usage = self.smoothed_usage.round() as u32;
        if new_usage.abs_diff(self.usage) >= UPDATE_THRESHOLD {
            self.usage = new_usage;
            true
        } else {
            false
        }
    }
}
