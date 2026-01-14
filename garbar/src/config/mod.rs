mod lua;
mod toml;
pub mod types;

use anyhow::Result;
use std::path::PathBuf;
use tracing::{debug, info, warn};

pub use types::*;

/// Configuration loader with fallback chain
pub struct ConfigLoader {
    /// Path to gar's init.lua
    lua_path: PathBuf,
    /// Path to standalone TOML config
    toml_path: PathBuf,
}

impl ConfigLoader {
    pub fn new() -> Self {
        let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config"));

        Self {
            lua_path: config_dir.join("gar/init.lua"),
            toml_path: config_dir.join("garbar/config.toml"),
        }
    }

    /// Set custom Lua config path
    pub fn with_lua_path(mut self, path: PathBuf) -> Self {
        self.lua_path = path;
        self
    }

    /// Set custom TOML config path
    pub fn with_toml_path(mut self, path: PathBuf) -> Self {
        self.toml_path = path;
        self
    }

    /// Load configuration with fallback chain:
    /// 1. Try Lua config (gar integration)
    /// 2. Try TOML config (standalone)
    /// 3. Use defaults
    pub fn load(&self) -> Result<BarConfig> {
        // Try Lua first (gar integration)
        if self.lua_path.exists() {
            debug!("Attempting to load Lua config from {}", self.lua_path.display());
            match lua::load_from_lua(&self.lua_path) {
                Ok(Some(config)) => {
                    info!("Loaded config from Lua: {}", self.lua_path.display());
                    return Ok(config);
                }
                Ok(None) => {
                    debug!("No gar.bar table in Lua config, trying TOML fallback");
                }
                Err(e) => {
                    warn!("Failed to load Lua config: {}", e);
                }
            }
        }

        // Try TOML fallback
        if self.toml_path.exists() {
            debug!("Attempting to load TOML config from {}", self.toml_path.display());
            match toml::load_from_toml(&self.toml_path) {
                Ok(Some(config)) => {
                    info!("Loaded config from TOML: {}", self.toml_path.display());
                    return Ok(config);
                }
                Ok(None) => {
                    debug!("TOML config empty or invalid");
                }
                Err(e) => {
                    warn!("Failed to load TOML config: {}", e);
                }
            }
        }

        // Use defaults
        info!("Using default configuration");
        Ok(BarConfig::default())
    }

    /// Reload configuration (called on SIGHUP)
    pub fn reload(&self) -> Result<BarConfig> {
        info!("Reloading configuration...");
        self.load()
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the default config directory for garbar
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("garbar")
}

/// Ensure config directory exists
pub fn ensure_config_dir() -> Result<PathBuf> {
    let dir = config_dir();
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}
