mod battery;
mod cpu;
mod datetime;
mod memory;
mod quick_settings;
mod script;
pub mod tray;
mod window_title;
mod workspaces;

pub use battery::BatteryModule;
pub use cpu::CpuModule;
pub use datetime::DatetimeModule;
pub use memory::MemoryModule;
pub use quick_settings::QuickSettingsModule;
pub use script::ScriptModule;
pub use tray::{TrayManager, TrayModule, TrayState};
pub use window_title::WindowTitleModule;
pub use workspaces::WorkspacesModule;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::BarConfig;
use crate::render::{Block, BlockStyle, Color, Padding};

/// Output from a module - one or more blocks to render
#[derive(Debug, Clone)]
pub struct ModuleOutput {
    pub blocks: Vec<Block>,
}

impl ModuleOutput {
    pub fn new(blocks: Vec<Block>) -> Self {
        Self { blocks }
    }

    pub fn single(block: Block) -> Self {
        Self { blocks: vec![block] }
    }

    pub fn empty() -> Self {
        Self { blocks: vec![] }
    }

    /// Create a simple text block with default styling
    pub fn text(text: &str, foreground: Color) -> Self {
        Self::single(
            Block::new(text).with_style(
                BlockStyle::new()
                    .with_foreground(foreground)
                    .with_padding(Padding::horizontal(8.0)),
            ),
        )
    }
}

/// Trait for status bar modules
pub trait Module: Send + Sync {
    /// Module name (used for config lookup)
    fn name(&self) -> &'static str;

    /// Get the current output to render
    fn output(&self) -> ModuleOutput;

    /// Update interval in milliseconds (0 = event-driven only)
    fn interval(&self) -> u64;

    /// Called periodically or on events to update state
    fn update(&mut self);

    /// Handle click events (button: 1=left, 2=middle, 3=right)
    /// block_index indicates which block within the module was clicked
    fn on_click(&mut self, _button: u8, _block_index: usize, _x: i16, _y: i16) {
        // Default: do nothing
    }

    /// Handle scroll events
    /// direction: true = up (Button4), false = down (Button5)
    /// block_index indicates which block within the module was scrolled
    fn on_scroll(&mut self, _up: bool, _block_index: usize, _x: i16, _y: i16) {
        // Default: do nothing
    }
}

/// Registry of active modules
pub struct ModuleRegistry {
    modules: HashMap<String, Arc<RwLock<Box<dyn Module>>>>,
    order_left: Vec<String>,
    order_center: Vec<String>,
    order_right: Vec<String>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            order_left: Vec::new(),
            order_center: Vec::new(),
            order_right: Vec::new(),
        }
    }

    /// Create registry from config, instantiating requested modules
    pub fn from_config(config: &BarConfig) -> Self {
        let mut registry = Self::new();

        // Collect all unique module names from config
        let all_modules: Vec<&str> = config
            .modules_left
            .iter()
            .chain(config.modules_center.iter())
            .chain(config.modules_right.iter())
            .map(|s| s.as_str())
            .collect();

        // Instantiate each unique module
        for name in &all_modules {
            if registry.modules.contains_key(*name) {
                continue;
            }

            // Check if this is a script module (either "script:name" or defined in scripts)
            let script_name = name.strip_prefix("script:").unwrap_or(*name);

            let module: Option<Box<dyn Module>> = match *name {
                "datetime" => Some(Box::new(DatetimeModule::new(&config.modules.datetime))),
                "workspaces" => Some(Box::new(WorkspacesModule::new(&config.modules.workspaces))),
                "cpu" => Some(Box::new(CpuModule::new(&config.modules.cpu))),
                "memory" => Some(Box::new(MemoryModule::new(&config.modules.memory))),
                "battery" => Some(Box::new(BatteryModule::new(&config.modules.battery))),
                "window_title" => Some(Box::new(WindowTitleModule::new(&config.modules.window_title))),
                "quick_settings" => Some(Box::new(QuickSettingsModule::new(&config.modules.quick_settings))),
                "network" => None,      // TODO: implement
                "pulseaudio" => None,   // TODO: implement
                // Tray is special - it's initialized by DaemonState with TrayManager
                // Create a placeholder that will be replaced
                "tray" => Some(Box::new(TrayModule::placeholder(&config.modules.tray))),
                _ => {
                    // Try to find a script module with this name
                    if let Some(script_config) = config.modules.script.get(script_name) {
                        tracing::info!("Loading script module: {}", script_name);
                        Some(Box::new(ScriptModule::new(script_name, script_config)))
                    } else {
                        tracing::warn!("Unknown module: {}", name);
                        None
                    }
                }
            };

            if let Some(m) = module {
                registry
                    .modules
                    .insert(name.to_string(), Arc::new(RwLock::new(m)));
            }
        }

        // Store order
        registry.order_left = config.modules_left.clone();
        registry.order_center = config.modules_center.clone();
        registry.order_right = config.modules_right.clone();

        registry
    }

    /// Get outputs for left-aligned modules
    pub async fn left_outputs(&self) -> Vec<ModuleOutput> {
        self.outputs_for(&self.order_left).await
    }

    /// Get outputs for center-aligned modules
    pub async fn center_outputs(&self) -> Vec<ModuleOutput> {
        self.outputs_for(&self.order_center).await
    }

    /// Get outputs for right-aligned modules
    pub async fn right_outputs(&self) -> Vec<ModuleOutput> {
        self.outputs_for(&self.order_right).await
    }

    async fn outputs_for(&self, names: &[String]) -> Vec<ModuleOutput> {
        let mut outputs = Vec::new();
        for name in names {
            if let Some(module) = self.modules.get(name) {
                let guard = module.read().await;
                outputs.push(guard.output());
            }
        }
        outputs
    }

    /// Update all modules
    pub async fn update_all(&self) {
        for module in self.modules.values() {
            let mut guard = module.write().await;
            guard.update();
        }
    }

    /// Update a specific module by name
    pub async fn update(&self, name: &str) {
        if let Some(module) = self.modules.get(name) {
            let mut guard = module.write().await;
            guard.update();
        }
    }

    /// Get minimum update interval across all modules (for tick rate)
    pub fn min_interval(&self) -> u64 {
        // Default to 1 second if no modules
        1000
    }

    /// Dispatch a click event to a module
    pub async fn dispatch_click(&self, module_name: &str, button: u8, block_index: usize, x: i16, y: i16) {
        if let Some(module) = self.modules.get(module_name) {
            let mut guard = module.write().await;
            guard.on_click(button, block_index, x, y);
        }
    }

    /// Dispatch a scroll event to a module
    pub async fn dispatch_scroll(&self, module_name: &str, up: bool, block_index: usize, x: i16, y: i16) {
        if let Some(module) = self.modules.get(module_name) {
            let mut guard = module.write().await;
            guard.on_scroll(up, block_index, x, y);
        }
    }

    /// Get list of modules in order (left, center, right)
    pub fn module_order(&self) -> (&[String], &[String], &[String]) {
        (&self.order_left, &self.order_center, &self.order_right)
    }

    /// Replace a module (used for tray initialization)
    pub fn replace_module(&mut self, name: &str, module: Box<dyn Module>) {
        self.modules.insert(name.to_string(), Arc::new(RwLock::new(module)));
    }

    /// Check if a module exists
    pub fn has_module(&self, name: &str) -> bool {
        self.modules.contains_key(name)
    }
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}
