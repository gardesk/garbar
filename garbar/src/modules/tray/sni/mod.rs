//! StatusNotifierItem (SNI) protocol implementation for garbar
//!
//! Implements the freedesktop.org StatusNotifierItem specification
//! for modern D-Bus based system tray support.

pub mod watcher;
pub mod host;
pub mod item;
pub mod icons;

pub use watcher::{StatusNotifierWatcher, start_watcher, WatcherState};
pub use host::StatusNotifierHost;
pub use item::SniItem;
pub use icons::{IconData, argb_to_cairo_bgra};
