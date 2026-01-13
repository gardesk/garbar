//! garbar - A configurable status bar for the gar desktop suite
//!
//! This library provides the core functionality for garbar, including:
//! - X11 window management and event handling
//! - Module system for extensible status indicators
//! - Configuration via Lua (integrated with gar) or standalone TOML
//! - IPC for control via garbarctl

pub mod daemon;
pub mod x11;

// Future modules (uncomment as implemented)
// pub mod config;
// pub mod modules;
// pub mod render;
// pub mod ipc;
// pub mod animation;
