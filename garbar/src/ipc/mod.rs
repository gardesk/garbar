//! IPC module for garbar daemon communication
//!
//! Provides a Unix socket-based IPC server for controlling the bar
//! via garbarctl or other tools.

pub mod protocol;
pub mod server;

pub use protocol::{Command, Response, StatusInfo};
pub use server::IpcServer;
