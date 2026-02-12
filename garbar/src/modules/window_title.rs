use x11rb::connection::Connection as X11Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, Window};
use x11rb::rust_connection::RustConnection;

use crate::config::WindowTitleConfig;
use crate::render::{Block, BlockStyle, Color, Margin, Padding};

use super::{Module, ModuleOutput};

pub struct WindowTitleModule {
    config: WindowTitleConfig,
    conn: Option<RustConnection>,
    root: Window,
    atoms: Option<WindowTitleAtoms>,
    current_title: String,
}

struct WindowTitleAtoms {
    net_active_window: u32,
    net_wm_name: u32,
    wm_name: u32,
    utf8_string: u32,
}

impl WindowTitleModule {
    pub fn new(config: &WindowTitleConfig) -> Self {
        // Create our own X11 connection for querying window titles
        let (conn, root, atoms) = match RustConnection::connect(None) {
            Ok((conn, screen_num)) => {
                let root = conn.setup().roots[screen_num].root;
                let atoms = Self::intern_atoms(&conn);
                tracing::info!("WindowTitle: connected to X11");
                (Some(conn), root, atoms)
            }
            Err(e) => {
                tracing::warn!("WindowTitle: failed to connect to X11: {}", e);
                (None, 0, None)
            }
        };

        // Don't call update() here - let the async event loop handle it
        Self {
            config: config.clone(),
            conn,
            root,
            atoms,
            current_title: String::new(),
        }
    }

    fn intern_atoms(conn: &RustConnection) -> Option<WindowTitleAtoms> {
        let net_active_window = conn.intern_atom(false, b"_NET_ACTIVE_WINDOW").ok()?;
        let net_wm_name = conn.intern_atom(false, b"_NET_WM_NAME").ok()?;
        let wm_name = conn.intern_atom(false, b"WM_NAME").ok()?;
        let utf8_string = conn.intern_atom(false, b"UTF8_STRING").ok()?;

        Some(WindowTitleAtoms {
            net_active_window: net_active_window.reply().ok()?.atom,
            net_wm_name: net_wm_name.reply().ok()?.atom,
            wm_name: wm_name.reply().ok()?.atom,
            utf8_string: utf8_string.reply().ok()?.atom,
        })
    }

    /// Get the currently focused window ID from _NET_ACTIVE_WINDOW
    fn get_active_window(&self) -> Option<Window> {
        let conn = self.conn.as_ref()?;
        let atoms = self.atoms.as_ref()?;

        let reply = conn
            .get_property(
                false,
                self.root,
                atoms.net_active_window,
                AtomEnum::WINDOW,
                0,
                1,
            )
            .ok()?
            .reply()
            .ok()?;

        if reply.value_len == 1 {
            let window = u32::from_ne_bytes(reply.value[0..4].try_into().ok()?);
            if window != 0 {
                return Some(window);
            }
        }
        None
    }

    /// Get the window title from _NET_WM_NAME or WM_NAME
    fn get_window_title(&self, window: Window) -> Option<String> {
        let conn = self.conn.as_ref()?;
        let atoms = self.atoms.as_ref()?;

        // Try _NET_WM_NAME first (UTF-8)
        if let Ok(reply) = conn
            .get_property(false, window, atoms.net_wm_name, atoms.utf8_string, 0, 1024)
        {
            if let Ok(reply) = reply.reply() {
                if reply.value_len > 0 {
                    if let Ok(title) = String::from_utf8(reply.value) {
                        return Some(title);
                    }
                }
            }
        }

        // Fall back to WM_NAME (may be Latin-1)
        if let Ok(reply) = conn.get_property(
            false,
            window,
            atoms.wm_name,
            AtomEnum::STRING,
            0,
            1024,
        ) {
            if let Ok(reply) = reply.reply() {
                if reply.value_len > 0 {
                    // Try UTF-8 first, then Latin-1
                    if let Ok(title) = String::from_utf8(reply.value.clone()) {
                        return Some(title);
                    }
                    // Convert Latin-1 to UTF-8
                    return Some(reply.value.iter().map(|&c| c as char).collect());
                }
            }
        }

        None
    }

    /// Truncate title to max length with ellipsis
    fn truncate_title(&self, title: &str) -> String {
        if title.chars().count() <= self.config.max_length {
            title.to_string()
        } else {
            let truncated: String = title.chars().take(self.config.max_length - 1).collect();
            format!("{}{}", truncated, self.config.ellipsis)
        }
    }

    fn read_window_title(&mut self) {
        self.current_title = if let Some(window) = self.get_active_window() {
            self.get_window_title(window)
                .map(|t| self.truncate_title(&t))
                .unwrap_or_else(|| self.config.empty_text.clone())
        } else {
            self.config.empty_text.clone()
        };
    }
}

impl Module for WindowTitleModule {
    fn name(&self) -> &'static str {
        "window_title"
    }

    fn output(&self) -> ModuleOutput {
        let foreground = Color::from_hex("#888888").unwrap_or(Color::white());

        ModuleOutput::single(
            Block::new(&self.current_title).with_style(
                BlockStyle::new()
                    .with_foreground(foreground)
                    .with_padding(Padding::horizontal(8.0))
                    .with_margin(Margin::new(16.0, 0.0, 0.0, 0.0)), // Left margin for spacing from workspaces
            ),
        )
    }

    fn interval(&self) -> u64 {
        50 // Check every 50ms for snappy focus tracking
    }

    fn update(&mut self) -> bool {
        let old_title = self.current_title.clone();
        self.read_window_title();
        self.current_title != old_title
    }
}
