//! IPC protocol types for garbar communication
//!
//! Uses JSON for simplicity and compatibility.

use serde::{Deserialize, Serialize};

/// Commands that can be sent to garbar via IPC
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    /// Show the bar (make it visible)
    Show,
    /// Hide the bar
    Hide,
    /// Toggle bar visibility
    Toggle,
    /// Reload configuration from disk
    Reload,
    /// Gracefully quit the daemon
    Quit,
    /// Get current status
    Status,
    /// Update a specific module
    UpdateModule { module: String },
}

/// Response from garbar
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl Response {
    /// Create a success response
    pub fn ok() -> Self {
        Self {
            success: true,
            error: None,
            data: None,
        }
    }

    /// Create a success response with data
    pub fn ok_with_data(data: serde_json::Value) -> Self {
        Self {
            success: true,
            error: None,
            data: Some(data),
        }
    }

    /// Create an error response
    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            error: Some(message.into()),
            data: None,
        }
    }
}

/// Status information returned by the Status command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusInfo {
    pub visible: bool,
    pub width: u16,
    pub height: u16,
    pub modules_left: Vec<String>,
    pub modules_center: Vec<String>,
    pub modules_right: Vec<String>,
}
