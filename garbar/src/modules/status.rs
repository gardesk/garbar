use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::config::StatusConfig;
use crate::render::{Block, BlockStyle, Color, Padding};

use super::{Module, ModuleOutput};

#[derive(Debug, Clone, Default)]
struct RawStatus {
    load_1m: f64,
    load_5m: f64,
    load_15m: f64,
    mem_total_kb: u64,
    mem_available_kb: u64,
    disk_percent: u32,
    disk_free: String,
}

pub struct StatusModule {
    config: StatusConfig,
    load_1m: f64,
    load_5m: f64,
    load_15m: f64,
    mem_percent: u32,
    mem_used_kb: u64,
    mem_total_kb: u64,
    disk_percent: u32,
    disk_free: String,
    use_alt_format: bool,
    pending: Arc<Mutex<bool>>,
    cached_output: Arc<Mutex<Option<RawStatus>>>,
}

impl StatusModule {
    pub fn new(config: &StatusConfig) -> Self {
        Self {
            config: config.clone(),
            load_1m: 0.0,
            load_5m: 0.0,
            load_15m: 0.0,
            mem_percent: 0,
            mem_used_kb: 0,
            mem_total_kb: 0,
            disk_percent: 0,
            disk_free: String::new(),
            use_alt_format: false,
            pending: Arc::new(Mutex::new(false)),
            cached_output: Arc::new(Mutex::new(None)),
        }
    }

    fn spawn_ssh(&self) {
        {
            let mut pending = self.pending.lock().unwrap();
            if *pending {
                return;
            }
            *pending = true;
        }

        let host = self.config.host.clone();
        let mountpoint = self.config.mountpoint.clone();
        let pending = Arc::clone(&self.pending);
        let cached = Arc::clone(&self.cached_output);

        thread::spawn(move || {
            let remote_cmd = format!(
                "awk '{{print $1,$2,$3}}' /proc/loadavg; \
                 awk '/MemTotal/{{t=$2}}/MemAvailable/{{a=$2}}END{{print t,a}}' /proc/meminfo; \
                 df {} | awk 'NR==2{{print $2,$3,$4,$5}}'",
                mountpoint
            );

            let result = Command::new("ssh")
                .args([
                    "-o", "ConnectTimeout=5",
                    "-o", "BatchMode=yes",
                    &host,
                    &remote_cmd,
                ])
                .output();

            match result {
                Ok(output) if output.status.success() => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if let Some(parsed) = Self::parse_output(&stdout) {
                        if let Ok(mut c) = cached.lock() {
                            *c = Some(parsed);
                        }
                    }
                }
                Ok(output) => {
                    tracing::debug!(
                        "status: ssh {} failed: {}",
                        host,
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                Err(e) => {
                    tracing::debug!("status: ssh exec error: {}", e);
                }
            }

            if let Ok(mut p) = pending.lock() {
                *p = false;
            }
        });
    }

    fn parse_output(stdout: &str) -> Option<RawStatus> {
        let lines: Vec<&str> = stdout.lines().collect();
        if lines.len() < 3 {
            return None;
        }

        // Line 0: load averages "1.23 0.89 0.56"
        let load_parts: Vec<&str> = lines[0].split_whitespace().collect();
        if load_parts.len() < 3 {
            return None;
        }
        let load_1m: f64 = load_parts[0].parse().ok()?;
        let load_5m: f64 = load_parts[1].parse().ok()?;
        let load_15m: f64 = load_parts[2].parse().ok()?;

        // Line 1: mem "total_kb available_kb"
        let mem_parts: Vec<&str> = lines[1].split_whitespace().collect();
        if mem_parts.len() < 2 {
            return None;
        }
        let mem_total_kb: u64 = mem_parts[0].parse().ok()?;
        let mem_available_kb: u64 = mem_parts[1].parse().ok()?;

        // Line 2: disk "total used avail percent%"
        let disk_parts: Vec<&str> = lines[2].split_whitespace().collect();
        if disk_parts.len() < 4 {
            return None;
        }
        let disk_percent: u32 = disk_parts[3].trim_end_matches('%').parse().ok()?;
        // avail is in 1K blocks, convert to human-readable
        let avail_kb: u64 = disk_parts[2].parse().ok()?;
        let disk_free = Self::format_bytes(avail_kb);

        Some(RawStatus {
            load_1m,
            load_5m,
            load_15m,
            mem_total_kb,
            mem_available_kb,
            disk_percent,
            disk_free,
        })
    }

    fn apply_cached(&mut self) -> bool {
        let cached = {
            let mut c = self.cached_output.lock().unwrap();
            c.take()
        };
        if let Some(raw) = cached {
            self.load_1m = raw.load_1m;
            self.load_5m = raw.load_5m;
            self.load_15m = raw.load_15m;
            self.mem_total_kb = raw.mem_total_kb;
            let mem_used = raw.mem_total_kb.saturating_sub(raw.mem_available_kb);
            self.mem_used_kb = mem_used;
            self.mem_percent = if raw.mem_total_kb > 0 {
                ((mem_used as f64 / raw.mem_total_kb as f64) * 100.0) as u32
            } else {
                0
            };
            self.disk_percent = raw.disk_percent;
            self.disk_free = raw.disk_free;
            true
        } else {
            false
        }
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

    fn color_for_load(&self) -> Color {
        if self.load_1m >= self.config.critical_threshold_load {
            Color::from_hex(&self.config.critical_foreground).unwrap_or(Color::red())
        } else if self.load_1m >= self.config.warning_threshold_load {
            Color::from_hex(&self.config.warning_foreground).unwrap_or(Color::yellow())
        } else {
            Color::from_hex("#5294e2").unwrap_or(Color::white())
        }
    }

    fn color_for_mem(&self) -> Color {
        if self.mem_percent >= self.config.critical_threshold_mem {
            Color::from_hex(&self.config.critical_foreground).unwrap_or(Color::red())
        } else if self.mem_percent >= self.config.warning_threshold_mem {
            Color::from_hex(&self.config.warning_foreground).unwrap_or(Color::yellow())
        } else {
            Color::from_hex("#5294e2").unwrap_or(Color::white())
        }
    }

    fn color_for_disk(&self) -> Color {
        if self.disk_percent >= self.config.critical_threshold_disk {
            Color::from_hex(&self.config.critical_foreground).unwrap_or(Color::red())
        } else if self.disk_percent >= self.config.warning_threshold_disk {
            Color::from_hex(&self.config.warning_foreground).unwrap_or(Color::yellow())
        } else {
            Color::from_hex("#5294e2").unwrap_or(Color::white())
        }
    }

    fn has_data(&self) -> bool {
        self.mem_total_kb > 0
    }
}

impl Module for StatusModule {
    fn name(&self) -> &'static str {
        "status"
    }

    fn output(&self) -> ModuleOutput {
        if !self.has_data() {
            return ModuleOutput::text(&format!("{}: --", self.config.host), Color::from_hex("#888888").unwrap_or(Color::white()));
        }

        let style = |color: Color| {
            BlockStyle::new()
                .with_foreground(color)
                .with_padding(Padding::horizontal(6.0))
        };

        let (load_text, mem_text, disk_text) = if self.use_alt_format {
            (
                format!(" {:.2} {:.2} {:.2}", self.load_1m, self.load_5m, self.load_15m),
                format!(" {}/{}", Self::format_bytes(self.mem_used_kb), Self::format_bytes(self.mem_total_kb)),
                format!(" {} free", self.disk_free),
            )
        } else {
            (
                format!(" {:.2}", self.load_1m),
                format!(" {:3}%", self.mem_percent),
                format!(" {:3}%", self.disk_percent),
            )
        };

        let load_block = Block::new(&load_text).with_style(style(self.color_for_load()));
        let mem_block = Block::new(&mem_text).with_style(style(self.color_for_mem()));
        let disk_block = Block::new(&disk_text).with_style(style(self.color_for_disk()));

        ModuleOutput::new(vec![load_block, mem_block, disk_block])
    }

    fn interval(&self) -> u64 {
        (self.config.interval as u64) * 1000
    }

    fn update(&mut self) -> bool {
        let changed = self.apply_cached();
        self.spawn_ssh();
        changed
    }

    fn on_click(&mut self, button: u8, _block_index: usize, _x: i16, _y: i16) {
        if button == 1 {
            self.use_alt_format = !self.use_alt_format;
        }
    }
}
