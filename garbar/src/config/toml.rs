use anyhow::{Context, Result};
use std::path::Path;
use tracing::{debug, info};

use super::types::BarConfig;

/// Load configuration from a TOML file
pub fn load_from_toml<P: AsRef<Path>>(path: P) -> Result<Option<BarConfig>> {
    let path = path.as_ref();

    if !path.exists() {
        debug!("TOML config file not found: {}", path.display());
        return Ok(None);
    }

    info!("Loading config from {}", path.display());

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let config: BarConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;

    debug!("Parsed TOML config: height={}, position={:?}", config.height, config.position);

    Ok(Some(config))
}

/// Save configuration to a TOML file (for generating defaults)
#[allow(dead_code)]
pub fn save_to_toml<P: AsRef<Path>>(path: P, config: &BarConfig) -> Result<()> {
    let content = toml::to_string_pretty(config)
        .context("Failed to serialize config")?;

    std::fs::write(path.as_ref(), content)
        .with_context(|| format!("Failed to write {}", path.as_ref().display()))?;

    Ok(())
}
