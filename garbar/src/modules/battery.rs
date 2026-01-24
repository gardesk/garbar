use std::fs;
use std::path::PathBuf;

use crate::config::BatteryConfig;
use crate::render::{Block, BlockStyle, Color, Padding};

use super::{Module, ModuleOutput};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryStatus {
    Charging,
    Discharging,
    Full,
    NotCharging,
    Unknown,
}

pub struct BatteryModule {
    config: BatteryConfig,
    device_path: Option<PathBuf>,
    percent: u32,
    status: BatteryStatus,
}

impl BatteryModule {
    pub fn new(config: &BatteryConfig) -> Self {
        let mut module = Self {
            config: config.clone(),
            device_path: None,
            percent: 0,
            status: BatteryStatus::Unknown,
        };
        module.find_battery();
        // Don't call update() here - let the async event loop handle it
        module
    }

    fn find_battery(&mut self) {
        let power_supply = PathBuf::from("/sys/class/power_supply");

        if self.config.device != "auto" {
            // Use specific device
            let path = power_supply.join(&self.config.device);
            if path.exists() {
                self.device_path = Some(path);
            }
            return;
        }

        // Auto-detect: look for devices with type=Battery that have capacity file
        // Prefer devices with "battery" in the name (e.g., macsmc-battery over apple_mfi_fastcharge)
        let mut candidates: Vec<PathBuf> = Vec::new();

        if let Ok(entries) = fs::read_dir(&power_supply) {
            for entry in entries.flatten() {
                let path = entry.path();
                // Check if this is a battery by reading its type
                let type_path = path.join("type");
                if let Ok(device_type) = fs::read_to_string(&type_path) {
                    if device_type.trim() == "Battery" {
                        // Must have capacity file to be useful
                        if path.join("capacity").exists() {
                            candidates.push(path);
                        }
                    }
                }
            }
        }

        // Sort candidates: prefer devices with "battery" in name
        candidates.sort_by(|a, b| {
            let a_name = a.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
            let b_name = b.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
            let a_has_battery = a_name.contains("battery");
            let b_has_battery = b_name.contains("battery");
            b_has_battery.cmp(&a_has_battery) // true (has battery) comes first
        });

        if let Some(path) = candidates.into_iter().next() {
            tracing::info!("Battery: auto-detected {}", path.display());
            self.device_path = Some(path);
        }
    }

    fn read_file(&self, filename: &str) -> Option<String> {
        let path = self.device_path.as_ref()?.join(filename);
        fs::read_to_string(path).ok().map(|s| s.trim().to_string())
    }

    fn read_battery(&mut self) {
        if self.device_path.is_none() {
            return;
        }

        // Read capacity (percentage)
        if let Some(cap_str) = self.read_file("capacity") {
            self.percent = cap_str.parse().unwrap_or(0);
        }

        // Read status
        if let Some(status_str) = self.read_file("status") {
            self.status = match status_str.as_str() {
                "Charging" => BatteryStatus::Charging,
                "Discharging" => BatteryStatus::Discharging,
                "Full" => BatteryStatus::Full,
                "Not charging" => BatteryStatus::NotCharging,
                _ => BatteryStatus::Unknown,
            };
        }
    }

    fn get_icon(&self) -> &str {
        for icon_cfg in self.config.icons.iter().rev() {
            if self.percent <= icon_cfg.threshold {
                return &icon_cfg.icon;
            }
        }
        // Fallback to last icon
        self.config.icons.last().map(|i| i.icon.as_str()).unwrap_or("")
    }

    fn format_output(&self) -> String {
        let format = match self.status {
            BatteryStatus::Charging => &self.config.format_charging,
            BatteryStatus::Full => &self.config.format_full,
            _ => &self.config.format_discharging,
        };

        // Use fixed-width formatting (3 chars for 0-100%) to prevent bar from shifting
        format
            .replace("{percent}", &format!("{:3}", self.percent))
            .replace("{icon}", self.get_icon())
    }

    fn get_color(&self) -> Color {
        if self.status == BatteryStatus::Charging {
            Color::from_hex("#98c379").unwrap_or(Color::green())
        } else if self.percent <= self.config.low_threshold {
            Color::from_hex(&self.config.low_foreground).unwrap_or(Color::red())
        } else {
            Color::from_hex("#e5c07b").unwrap_or(Color::white())
        }
    }
}

impl Module for BatteryModule {
    fn name(&self) -> &'static str {
        "battery"
    }

    fn output(&self) -> ModuleOutput {
        if self.device_path.is_none() {
            // No battery found, show nothing
            return ModuleOutput::empty();
        }

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
        30000 // 30 seconds
    }

    fn update(&mut self) {
        self.read_battery();
    }
}
