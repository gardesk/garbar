//! System tray module combining XEmbed (legacy) and SNI (modern) protocols
//!
//! This module provides system tray functionality for garbar:
//! - XEmbed: Legacy X11 protocol for embedding tray icons
//! - SNI: Modern D-Bus StatusNotifierItem protocol
//!
//! The tray module renders SNI icons via Cairo and positions XEmbed icons
//! as X11 child windows within the bar.

mod xembed;
pub mod sni;

pub use xembed::{TrayManager, TrayModule, TrayState};
pub use sni::{StatusNotifierHost, start_watcher, WatcherState, WatcherHandle, WatcherEvent, SniItem, IconData, argb_to_cairo_bgra};
