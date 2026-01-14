use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main bar configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BarConfig {
    /// Bar height in pixels
    pub height: u16,
    /// Bar position: "top" or "bottom"
    pub position: Position,
    /// Bar margins
    pub margin: Margin,
    /// Bar padding
    pub padding: Padding,
    /// Background (color string or gradient config)
    pub background: BackgroundConfig,
    /// Default foreground color
    pub foreground: String,
    /// Window opacity (0.0 to 1.0)
    pub opacity: f64,
    /// Font specifications
    pub fonts: Vec<String>,
    /// Border configuration
    pub border: BorderConfig,
    /// Shadow configuration
    pub shadow: Option<ShadowConfig>,
    /// Module layout
    pub modules_left: Vec<String>,
    pub modules_center: Vec<String>,
    pub modules_right: Vec<String>,
    /// Animation settings
    pub animations: AnimationConfig,
    /// Separator configuration
    pub separator: SeparatorConfig,
    /// Per-module configurations
    pub modules: ModulesConfig,
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            height: 32, // Match polybar 32pt
            position: Position::Top,
            margin: Margin::default(),
            padding: Padding { left: 8.0, right: 16.0, top: 0.0, bottom: 0.0 },
            background: BackgroundConfig::Solid("#1a1a1a".to_string()), // Polybar background
            foreground: "#ffffff".to_string(), // Pure white like polybar
            opacity: 1.0,
            fonts: vec![
                "JetBrainsMono Nerd Font:size=10".to_string(),
                "DejaVu Sans Mono:size=10".to_string(),
                "monospace:size=10".to_string(),
            ],
            border: BorderConfig::default(),
            shadow: None,
            modules_left: vec!["workspaces".to_string(), "window_title".to_string()],
            modules_center: vec![],
            modules_right: vec![
                "cpu".to_string(),
                "memory".to_string(),
                "battery".to_string(),
                "datetime".to_string(),
            ],
            animations: AnimationConfig::default(),
            separator: SeparatorConfig::default(),
            modules: ModulesConfig::default(),
        }
    }
}

/// Bar position
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Position {
    #[default]
    Top,
    Bottom,
}

/// Margin configuration
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Margin {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

/// Padding configuration
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Padding {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

/// Background configuration - either solid color or gradient
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BackgroundConfig {
    Solid(String),
    Gradient(GradientConfig),
}

impl Default for BackgroundConfig {
    fn default() -> Self {
        Self::Solid("#1a1a1a".to_string())
    }
}

/// Gradient configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradientConfig {
    #[serde(rename = "type")]
    pub gradient_type: GradientType,
    #[serde(default)]
    pub direction: GradientDirection,
    pub stops: Vec<GradientStopConfig>,
    /// For radial gradients
    #[serde(default)]
    pub center: Option<(f64, f64)>,
    #[serde(default)]
    pub radius: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GradientType {
    #[default]
    Gradient, // linear
    Radial,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GradientDirection {
    #[default]
    Horizontal,
    Vertical,
    Diagonal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradientStopConfig {
    pub position: f64,
    pub color: String,
}

/// Border configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct BorderConfig {
    pub width: f64,
    pub color: String,
    pub radius: f64,
}

/// Shadow configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_shadow_color")]
    pub color: String,
    #[serde(default = "default_blur")]
    pub blur: f64,
    #[serde(default)]
    pub offset: ShadowOffset,
}

fn default_true() -> bool { true }
fn default_shadow_color() -> String { "#00000080".to_string() }
fn default_blur() -> f64 { 8.0 }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ShadowOffset {
    pub x: f64,
    pub y: f64,
}

/// Animation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AnimationConfig {
    pub enabled: bool,
    pub duration: u32, // milliseconds
    pub easing: String,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            duration: 150,
            easing: "ease-out-cubic".to_string(),
        }
    }
}

/// Separator configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SeparatorConfig {
    pub text: String,
    pub foreground: String,
    pub padding: Padding,
}

impl Default for SeparatorConfig {
    fn default() -> Self {
        Self {
            text: "│".to_string(),
            foreground: "#555555".to_string(),
            padding: Padding { left: 8.0, right: 8.0, top: 0.0, bottom: 0.0 },
        }
    }
}

/// Container for all module configurations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ModulesConfig {
    pub workspaces: WorkspacesConfig,
    pub window_title: WindowTitleConfig,
    pub cpu: CpuConfig,
    pub memory: MemoryConfig,
    pub battery: BatteryConfig,
    pub network: NetworkConfig,
    pub pulseaudio: PulseaudioConfig,
    pub datetime: DatetimeConfig,
    pub filesystem: FilesystemConfig,
    pub tray: TrayConfig,
    #[serde(default)]
    pub script: HashMap<String, ScriptConfig>,
}

/// Workspaces module config
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspacesConfig {
    pub show_empty: bool,
    pub show_urgent: bool,
    pub pin_workspaces: bool,
    pub focused: WorkspaceStateStyle,
    pub unfocused: WorkspaceStateStyle,
    pub urgent: WorkspaceStateStyle,
    pub transition: Option<TransitionConfig>,
    /// Font size override (in points)
    pub font_size: Option<f64>,
}

impl Default for WorkspacesConfig {
    fn default() -> Self {
        Self {
            show_empty: false,
            show_urgent: true,
            pin_workspaces: false,
            focused: WorkspaceStateStyle {
                background: "transparent".to_string(), // No background, just underline
                foreground: "#ffffff".to_string(),
                underline: Some(UnderlineConfig {
                    width: 2.0,
                    color: "#33ccff".to_string(), // info color (cyan)
                }),
            },
            unfocused: WorkspaceStateStyle {
                background: "transparent".to_string(),
                foreground: "#666666".to_string(), // disabled color
                underline: None,
            },
            urgent: WorkspaceStateStyle {
                background: "#ff5555".to_string(), // critical
                foreground: "#ffffff".to_string(),
                underline: None,
            },
            transition: None,
            font_size: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct WorkspaceStateStyle {
    pub background: String,
    pub foreground: String,
    pub underline: Option<UnderlineConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnderlineConfig {
    pub width: f64,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionConfig {
    pub property: String,
    pub duration: u32,
    pub easing: String,
}

/// Window title module config
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowTitleConfig {
    pub max_length: usize,
    pub ellipsis: String,
    pub empty_text: String,
    pub show_icon: bool,
    pub icon_spacing: f64,
}

impl Default for WindowTitleConfig {
    fn default() -> Self {
        Self {
            max_length: 50,
            ellipsis: "…".to_string(),
            empty_text: "Desktop".to_string(),
            show_icon: true,
            icon_spacing: 8.0,
        }
    }
}

/// CPU module config
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CpuConfig {
    pub format: String,
    pub interval: u32,
    pub warning_threshold: u32,
    pub critical_threshold: u32,
    pub warning_foreground: String,
    pub critical_foreground: String,
    pub graph: Option<GraphConfig>,
}

impl Default for CpuConfig {
    fn default() -> Self {
        Self {
            format: " {usage}%".to_string(),
            interval: 2,
            warning_threshold: 70,
            critical_threshold: 90,
            warning_foreground: "#ffaa00".to_string(),
            critical_foreground: "#ff5555".to_string(),
            graph: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConfig {
    pub enabled: bool,
    pub width: u32,
    pub style: String, // "bars", "line", "area"
    pub color: String,
}

/// Memory module config
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    pub format: String,
    pub format_alt: String,
    pub interval: u32,
    pub warning_threshold: u32,
    pub critical_threshold: u32,
    pub warning_foreground: String,
    pub critical_foreground: String,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            format: " {percent}%".to_string(),
            format_alt: " {used}/{total}".to_string(),
            interval: 5,
            warning_threshold: 80,
            critical_threshold: 95,
            warning_foreground: "#ffaa00".to_string(),
            critical_foreground: "#ff5555".to_string(),
        }
    }
}

/// Battery module config
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BatteryConfig {
    pub device: String,
    pub format_charging: String,
    pub format_discharging: String,
    pub format_full: String,
    pub icons: Vec<BatteryIcon>,
    pub low_threshold: u32,
    pub low_animation: String,
    pub low_foreground: String,
}

impl Default for BatteryConfig {
    fn default() -> Self {
        Self {
            device: "auto".to_string(),
            format_charging: " {percent}%".to_string(),
            format_discharging: "{icon} {percent}%".to_string(),
            format_full: " Full".to_string(),
            icons: vec![
                BatteryIcon { threshold: 10, icon: "".to_string() },
                BatteryIcon { threshold: 25, icon: "".to_string() },
                BatteryIcon { threshold: 50, icon: "".to_string() },
                BatteryIcon { threshold: 75, icon: "".to_string() },
                BatteryIcon { threshold: 100, icon: "".to_string() },
            ],
            low_threshold: 15,
            low_animation: "blink".to_string(),
            low_foreground: "#ff5555".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryIcon {
    pub threshold: u32,
    pub icon: String,
}

/// Network module config
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkConfig {
    pub interface: String,
    pub format_connected: String,
    pub format_disconnected: String,
    pub format_ethernet: String,
    pub show_speed: bool,
    pub speed_format: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            interface: "auto".to_string(),
            format_connected: " {essid}".to_string(),
            format_disconnected: " Offline".to_string(),
            format_ethernet: " {ip}".to_string(),
            show_speed: false,
            speed_format: "↓{down} ↑{up}".to_string(),
        }
    }
}

/// Pulseaudio module config
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PulseaudioConfig {
    pub format: String,
    pub format_muted: String,
    pub icons: Vec<VolumeIcon>,
    pub scroll_step: u32,
    pub on_click: String,
    pub on_click_middle: String,
    pub on_click_right: String,
}

impl Default for PulseaudioConfig {
    fn default() -> Self {
        Self {
            format: "{icon} {volume}%".to_string(),
            format_muted: " Muted".to_string(),
            icons: vec![
                VolumeIcon { threshold: 0, icon: "".to_string() },
                VolumeIcon { threshold: 33, icon: "".to_string() },
                VolumeIcon { threshold: 66, icon: "".to_string() },
            ],
            scroll_step: 5,
            on_click: "pavucontrol".to_string(),
            on_click_middle: "toggle_mute".to_string(),
            on_click_right: "next_sink".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeIcon {
    pub threshold: u32,
    pub icon: String,
}

/// Datetime module config
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DatetimeConfig {
    pub format: String,
    pub format_alt: String,
    pub interval: u32,
    pub tooltip: bool,
}

impl Default for DatetimeConfig {
    fn default() -> Self {
        Self {
            format: " %a %b %d   %H:%M".to_string(),
            format_alt: " %Y-%m-%d   %H:%M:%S".to_string(),
            interval: 1,
            tooltip: true,
        }
    }
}

/// Filesystem module config
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FilesystemConfig {
    pub mountpoint: String,
    pub format: String,
    pub warning_threshold: u32,
    pub critical_threshold: u32,
    pub interval: u32,
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            mountpoint: "/".to_string(),
            format: " {percent_used}%".to_string(),
            warning_threshold: 80,
            critical_threshold: 95,
            interval: 30,
        }
    }
}

/// Tray module config
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TrayConfig {
    pub icon_size: u32,
    pub spacing: f64,
    pub padding: Padding,
}

impl Default for TrayConfig {
    fn default() -> Self {
        Self {
            icon_size: 18,
            spacing: 8.0,
            padding: Padding { left: 4.0, right: 4.0, top: 0.0, bottom: 0.0 },
        }
    }
}

/// Script module config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptConfig {
    pub exec: String,
    #[serde(default = "default_interval")]
    pub interval: u32,
    #[serde(default)]
    pub tail: bool,
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub click_left: String,
    #[serde(default)]
    pub click_middle: String,
    #[serde(default)]
    pub click_right: String,
    /// Command to run on scroll up
    #[serde(default)]
    pub scroll_up: String,
    /// Command to run on scroll down
    #[serde(default)]
    pub scroll_down: String,
    /// Font size override (in points)
    #[serde(default)]
    pub font_size: Option<f64>,
}

fn default_interval() -> u32 { 30 }
