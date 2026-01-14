//! System tray module using the freedesktop.org System Tray Protocol (XEMBED)
//!
//! This module implements the system tray manager specification:
//! https://specifications.freedesktop.org/systemtray-spec/systemtray-spec-latest.html
//!
//! Key concepts:
//! - Selection owner: garbar claims _NET_SYSTEM_TRAY_S{screen} to become the tray manager
//! - XEMBED: Tray icons are embedded windows reparented to a container
//! - Client messages: Apps request docking via SYSTEM_TRAY_REQUEST_DOCK

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use x11rb::connection::Connection as X11Connection;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt, CreateWindowAux,
    EventMask, PropMode, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;

use crate::config::TrayConfig;
use crate::render::{Block, BlockStyle, Color, Padding};

use super::{Module, ModuleOutput};

/// System tray opcodes from the spec
const SYSTEM_TRAY_REQUEST_DOCK: u32 = 0;
const SYSTEM_TRAY_BEGIN_MESSAGE: u32 = 1;
const SYSTEM_TRAY_CANCEL_MESSAGE: u32 = 2;

/// XEMBED message types
const XEMBED_EMBEDDED_NOTIFY: u32 = 0;
const XEMBED_PROTOCOL_VERSION: u32 = 0;

/// A single tray icon
#[derive(Debug, Clone)]
struct TrayIcon {
    window: Window,
    width: u16,
    height: u16,
    mapped: bool,
}

/// Atoms needed for system tray protocol
#[derive(Debug, Clone)]
struct TrayAtoms {
    net_system_tray_s0: Atom,
    net_system_tray_opcode: Atom,
    net_system_tray_orientation: Atom,
    net_system_tray_visual: Atom,
    manager: Atom,
    xembed: Atom,
    xembed_info: Atom,
}

impl TrayAtoms {
    fn new(conn: &RustConnection, screen: usize) -> Option<Self> {
        // The selection atom is screen-specific
        let selection_name = format!("_NET_SYSTEM_TRAY_S{}", screen);

        let net_system_tray_s0 = conn.intern_atom(false, selection_name.as_bytes()).ok()?;
        let net_system_tray_opcode = conn.intern_atom(false, b"_NET_SYSTEM_TRAY_OPCODE").ok()?;
        let net_system_tray_orientation = conn.intern_atom(false, b"_NET_SYSTEM_TRAY_ORIENTATION").ok()?;
        let net_system_tray_visual = conn.intern_atom(false, b"_NET_SYSTEM_TRAY_VISUAL").ok()?;
        let manager = conn.intern_atom(false, b"MANAGER").ok()?;
        let xembed = conn.intern_atom(false, b"_XEMBED").ok()?;
        let xembed_info = conn.intern_atom(false, b"_XEMBED_INFO").ok()?;

        Some(Self {
            net_system_tray_s0: net_system_tray_s0.reply().ok()?.atom,
            net_system_tray_opcode: net_system_tray_opcode.reply().ok()?.atom,
            net_system_tray_orientation: net_system_tray_orientation.reply().ok()?.atom,
            net_system_tray_visual: net_system_tray_visual.reply().ok()?.atom,
            manager: manager.reply().ok()?.atom,
            xembed: xembed.reply().ok()?.atom,
            xembed_info: xembed_info.reply().ok()?.atom,
        })
    }
}

/// Shared state between TrayManager and TrayModule
#[derive(Debug)]
pub struct TrayState {
    icons: HashMap<Window, TrayIcon>,
    /// Total width of all tray icons (for module output)
    total_width: f64,
    /// X position where tray icons should be placed (set by layout)
    tray_x: i16,
    /// Y position for tray icons
    tray_y: i16,
}

impl TrayState {
    fn new() -> Self {
        Self {
            icons: HashMap::new(),
            total_width: 0.0,
            tray_x: 0,
            tray_y: 0,
        }
    }
}

/// System tray manager - handles X11 selection and icon embedding
pub struct TrayManager {
    conn: Arc<RustConnection>,
    screen_num: usize,
    root: Window,
    /// Hidden selection owner window
    selection_window: Window,
    /// Bar window where icons are embedded
    bar_window: Window,
    atoms: TrayAtoms,
    state: Arc<Mutex<TrayState>>,
    config: TrayConfig,
    /// Whether we successfully became the tray manager
    is_owner: bool,
}

impl TrayManager {
    /// Create a new tray manager
    pub fn new(
        conn: Arc<RustConnection>,
        screen_num: usize,
        bar_window: Window,
        config: &TrayConfig,
    ) -> Option<Self> {
        let root = conn.setup().roots[screen_num].root;
        let atoms = TrayAtoms::new(&conn, screen_num)?;

        // Create a hidden window to own the selection
        let selection_window = conn.generate_id().ok()?;
        let values = CreateWindowAux::new()
            .event_mask(EventMask::PROPERTY_CHANGE);

        conn.create_window(
            0, // depth - CopyFromParent
            selection_window,
            root,
            -1, -1, // off-screen
            1, 1,   // 1x1 pixel
            0,
            WindowClass::INPUT_ONLY,
            0, // visual - CopyFromParent
            &values,
        ).ok()?;

        tracing::debug!("Created tray selection window: {}", selection_window);

        Some(Self {
            conn,
            screen_num,
            root,
            selection_window,
            bar_window,
            atoms,
            state: Arc::new(Mutex::new(TrayState::new())),
            config: config.clone(),
            is_owner: false,
        })
    }

    /// Acquire the system tray selection and broadcast MANAGER message
    pub fn acquire_selection(&mut self) -> bool {
        // Try to acquire the selection
        let result = self.conn.set_selection_owner(
            self.selection_window,
            self.atoms.net_system_tray_s0,
            x11rb::CURRENT_TIME,
        );

        if result.is_err() {
            tracing::warn!("Failed to set selection owner");
            return false;
        }

        // Verify we got the selection
        let owner = self.conn
            .get_selection_owner(self.atoms.net_system_tray_s0)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .map(|reply| reply.owner);

        if owner != Some(self.selection_window) {
            tracing::warn!("Failed to acquire tray selection (another tray is running?)");
            return false;
        }

        // Set tray orientation (horizontal)
        let _ = self.conn.change_property32(
            PropMode::REPLACE,
            self.selection_window,
            self.atoms.net_system_tray_orientation,
            AtomEnum::CARDINAL,
            &[0], // 0 = horizontal, 1 = vertical
        );

        // Broadcast MANAGER client message to root window
        // This tells applications that a system tray is now available
        let event = ClientMessageEvent::new(
            32,
            self.root,
            self.atoms.manager,
            [
                x11rb::CURRENT_TIME,
                self.atoms.net_system_tray_s0,
                self.selection_window,
                0,
                0,
            ],
        );

        if self.conn.send_event(false, self.root, EventMask::STRUCTURE_NOTIFY, event).is_err() {
            tracing::warn!("Failed to broadcast MANAGER message");
            return false;
        }

        let _ = self.conn.flush();
        self.is_owner = true;
        tracing::info!("Acquired system tray selection, broadcasting MANAGER");
        true
    }

    /// Handle a client message (potentially a dock request)
    pub fn handle_client_message(&mut self, event: &ClientMessageEvent) -> bool {
        if event.type_ != self.atoms.net_system_tray_opcode {
            return false;
        }

        let opcode = event.data.as_data32()[1];
        let icon_window = event.data.as_data32()[2] as Window;

        match opcode {
            SYSTEM_TRAY_REQUEST_DOCK => {
                tracing::info!("Dock request from window {}", icon_window);
                self.dock_icon(icon_window);
                true
            }
            SYSTEM_TRAY_BEGIN_MESSAGE => {
                tracing::debug!("Begin message from {}", icon_window);
                true
            }
            SYSTEM_TRAY_CANCEL_MESSAGE => {
                tracing::debug!("Cancel message from {}", icon_window);
                true
            }
            _ => {
                tracing::debug!("Unknown tray opcode: {}", opcode);
                false
            }
        }
    }

    /// Dock a tray icon (reparent it to the bar)
    fn dock_icon(&mut self, icon_window: Window) {
        let icon_size = self.config.icon_size;

        // Subscribe to events on the icon window
        let values = x11rb::protocol::xproto::ChangeWindowAttributesAux::new()
            .event_mask(
                EventMask::STRUCTURE_NOTIFY
                    | EventMask::PROPERTY_CHANGE
                    | EventMask::EXPOSURE
            );

        if self.conn.change_window_attributes(icon_window, &values).is_err() {
            tracing::warn!("Failed to change attributes on icon {}", icon_window);
            return;
        }

        // Calculate position for this icon
        let x = {
            let state = self.state.lock().unwrap();
            let icon_count = state.icons.len() as i16;
            state.tray_x + icon_count * (icon_size as i16 + self.config.spacing as i16)
        };

        // Reparent the icon window to the bar
        if self.conn.reparent_window(icon_window, self.bar_window, x, 0).is_err() {
            tracing::warn!("Failed to reparent icon {}", icon_window);
            return;
        }

        // Resize to our icon size
        let values = ConfigureWindowAux::new()
            .width(icon_size)
            .height(icon_size);

        let _ = self.conn.configure_window(icon_window, &values);

        // Map the icon
        let _ = self.conn.map_window(icon_window);

        // Send XEMBED_EMBEDDED_NOTIFY
        self.send_xembed_notify(icon_window);

        // Track the icon
        {
            let mut state = self.state.lock().unwrap();
            state.icons.insert(icon_window, TrayIcon {
                window: icon_window,
                width: icon_size as u16,
                height: icon_size as u16,
                mapped: true,
            });
            self.update_total_width(&mut state);
        }

        let _ = self.conn.flush();
        tracing::info!("Docked icon {} at x={}", icon_window, x);
    }

    /// Send XEMBED_EMBEDDED_NOTIFY to an icon
    fn send_xembed_notify(&self, icon_window: Window) {
        let event = ClientMessageEvent::new(
            32,
            icon_window,
            self.atoms.xembed,
            [
                x11rb::CURRENT_TIME,
                XEMBED_EMBEDDED_NOTIFY,
                0, // detail
                self.bar_window,
                XEMBED_PROTOCOL_VERSION,
            ],
        );

        let _ = self.conn.send_event(false, icon_window, EventMask::NO_EVENT, event);
    }

    /// Handle an icon being destroyed
    pub fn handle_destroy(&mut self, window: Window) -> bool {
        let mut state = self.state.lock().unwrap();
        if state.icons.remove(&window).is_some() {
            tracing::info!("Tray icon {} destroyed", window);
            self.update_total_width(&mut state);
            drop(state);
            self.reposition_icons();
            true
        } else {
            false
        }
    }

    /// Handle an icon being unmapped
    pub fn handle_unmap(&mut self, window: Window) -> bool {
        let mut state = self.state.lock().unwrap();
        if let Some(icon) = state.icons.get_mut(&window) {
            icon.mapped = false;
            tracing::debug!("Tray icon {} unmapped", window);
            self.update_total_width(&mut state);
            drop(state);
            self.reposition_icons();
            true
        } else {
            false
        }
    }

    /// Reposition all icons after one is added/removed
    fn reposition_icons(&mut self) {
        let state = self.state.lock().unwrap();
        let icon_size = self.config.icon_size as i16;
        let spacing = self.config.spacing as i16;
        let base_x = state.tray_x;
        let base_y = state.tray_y;

        let mut x = base_x;
        for icon in state.icons.values() {
            if icon.mapped {
                let values = ConfigureWindowAux::new()
                    .x(x as i32)
                    .y(base_y as i32);

                let _ = self.conn.configure_window(icon.window, &values);
                x += icon_size + spacing;
            }
        }

        let _ = self.conn.flush();
    }

    /// Update the total width calculation
    fn update_total_width(&self, state: &mut TrayState) {
        let mapped_count = state.icons.values().filter(|i| i.mapped).count();
        if mapped_count == 0 {
            state.total_width = 0.0;
        } else {
            let icon_size = self.config.icon_size as f64;
            let spacing = self.config.spacing;
            state.total_width = (mapped_count as f64 * icon_size)
                + ((mapped_count - 1) as f64 * spacing)
                + self.config.padding.left
                + self.config.padding.right;
        }
    }

    /// Set the position where tray icons should be placed
    pub fn set_position(&mut self, x: i16, y: i16) {
        {
            let mut state = self.state.lock().unwrap();
            state.tray_x = x + self.config.padding.left as i16;
            state.tray_y = y;
        }
        self.reposition_icons();
    }

    /// Get the total width needed for the tray
    pub fn total_width(&self) -> f64 {
        self.state.lock().unwrap().total_width
    }

    /// Get the number of docked icons
    pub fn icon_count(&self) -> usize {
        self.state.lock().unwrap().icons.len()
    }

    /// Get the shared state for the module
    pub fn shared_state(&self) -> Arc<Mutex<TrayState>> {
        Arc::clone(&self.state)
    }

    /// Check if we're the selection owner
    pub fn is_owner(&self) -> bool {
        self.is_owner
    }

    /// Get the opcode atom for filtering events
    pub fn opcode_atom(&self) -> Atom {
        self.atoms.net_system_tray_opcode
    }

    /// Release the selection on shutdown
    pub fn release(&mut self) {
        if self.is_owner {
            // Unmap and reparent all icons back to root
            let state = self.state.lock().unwrap();
            for icon in state.icons.values() {
                let _ = self.conn.unmap_window(icon.window);
                let _ = self.conn.reparent_window(icon.window, self.root, 0, 0);
            }
            drop(state);

            // Release the selection
            let _ = self.conn.set_selection_owner(
                x11rb::NONE,
                self.atoms.net_system_tray_s0,
                x11rb::CURRENT_TIME,
            );

            // Destroy our selection window
            let _ = self.conn.destroy_window(self.selection_window);
            let _ = self.conn.flush();

            self.is_owner = false;
            tracing::info!("Released system tray selection");
        }
    }
}

impl Drop for TrayManager {
    fn drop(&mut self) {
        self.release();
    }
}

/// Tray module - thin wrapper for ModuleRegistry integration
pub struct TrayModule {
    config: TrayConfig,
    state: Arc<Mutex<TrayState>>,
}

impl TrayModule {
    /// Create a new tray module with shared state from TrayManager
    pub fn new(config: &TrayConfig, state: Arc<Mutex<TrayState>>) -> Self {
        Self {
            config: config.clone(),
            state,
        }
    }

    /// Create a placeholder module (when tray manager isn't available)
    pub fn placeholder(config: &TrayConfig) -> Self {
        Self {
            config: config.clone(),
            state: Arc::new(Mutex::new(TrayState::new())),
        }
    }
}

impl Module for TrayModule {
    fn name(&self) -> &'static str {
        "tray"
    }

    fn output(&self) -> ModuleOutput {
        let state = self.state.lock().unwrap();
        let width = state.total_width;

        if width <= 0.0 {
            return ModuleOutput::empty();
        }

        // Return a spacer block with the width of the tray
        // The actual icons are X11 windows positioned by TrayManager
        let style = BlockStyle::new()
            .with_padding(Padding::new(
                self.config.padding.left,
                self.config.padding.right,
                0.0,
                0.0,
            ));

        // Create an invisible block that reserves space
        // We use a fixed-width block for the tray area
        let icon_count = state.icons.values().filter(|i| i.mapped).count();
        let label = format!("{}", " ".repeat(icon_count * 3)); // Rough spacing

        ModuleOutput::single(
            Block::new(&label)
                .with_style(style.with_foreground(Color::transparent()))
                .with_min_width(width),
        )
    }

    fn interval(&self) -> u64 {
        0 // Event-driven only
    }

    fn update(&mut self) {
        // State is updated by TrayManager, nothing to do here
    }
}
