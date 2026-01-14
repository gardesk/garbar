use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::{debug, info, warn};
use x11rb::connection::{Connection as X11Connection, RequestConnection};
use x11rb::protocol::randr::{self, ConnectionExt as RandrConnectionExt, NotifyMask};
use x11rb::protocol::xproto::{AtomEnum, ChangeWindowAttributesAux, EventMask, Screen, Visualid, ConnectionExt};
use x11rb::rust_connection::RustConnection;

use super::Atoms;

/// Monitor/output information
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub name: String,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub primary: bool,
}

/// X11 connection wrapper
pub struct Connection {
    pub(crate) conn: Arc<RustConnection>,
    pub(crate) screen_num: usize,
    pub(crate) atoms: Atoms,
    /// RandR extension event base (for identifying RandR events)
    randr_event_base: Option<u8>,
}

impl Connection {
    /// Create a new X11 connection
    pub fn new() -> Result<Self> {
        let (conn, screen_num) =
            RustConnection::connect(None).context("Failed to connect to X11 server")?;

        let conn = Arc::new(conn);
        let atoms = Atoms::new(&*conn).context("Failed to intern atoms")?;

        // Initialize RandR extension
        let randr_event_base = Self::init_randr(&conn, screen_num);

        Ok(Self {
            conn,
            screen_num,
            atoms,
            randr_event_base,
        })
    }

    /// Initialize RandR extension and subscribe to events
    fn init_randr(conn: &RustConnection, screen_num: usize) -> Option<u8> {
        // Query RandR extension
        let randr_info = match conn.extension_information(randr::X11_EXTENSION_NAME) {
            Ok(Some(info)) => info,
            Ok(None) => {
                warn!("RandR extension not available");
                return None;
            }
            Err(e) => {
                warn!("Failed to query RandR extension: {}", e);
                return None;
            }
        };

        let event_base = randr_info.first_event;
        debug!("RandR extension found, event_base={}", event_base);

        // Query RandR version (we need at least 1.2 for CRTC info)
        match conn.randr_query_version(1, 5) {
            Ok(cookie) => match cookie.reply() {
                Ok(reply) => {
                    info!(
                        "RandR version {}.{} available",
                        reply.major_version, reply.minor_version
                    );
                }
                Err(e) => {
                    warn!("Failed to query RandR version: {}", e);
                    return None;
                }
            },
            Err(e) => {
                warn!("Failed to send RandR version query: {}", e);
                return None;
            }
        }

        // Get root window for this screen
        let root = conn.setup().roots[screen_num].root;

        // Subscribe to RandR events on the root window
        let notify_mask = NotifyMask::SCREEN_CHANGE
            | NotifyMask::CRTC_CHANGE
            | NotifyMask::OUTPUT_CHANGE;

        if let Err(e) = conn.randr_select_input(root, notify_mask) {
            warn!("Failed to subscribe to RandR events: {}", e);
            return None;
        }

        info!("Subscribed to RandR monitor events");
        Some(event_base)
    }

    /// Get the default screen
    pub fn screen(&self) -> &Screen {
        &self.conn.setup().roots[self.screen_num]
    }

    /// Get screen width
    pub fn screen_width(&self) -> u16 {
        self.screen().width_in_pixels
    }

    /// Get screen height
    pub fn screen_height(&self) -> u16 {
        self.screen().height_in_pixels
    }

    /// Get the root window
    pub fn root(&self) -> u32 {
        self.screen().root
    }

    /// Get the root visual
    pub fn root_visual(&self) -> Visualid {
        self.screen().root_visual
    }

    /// Get the root depth
    pub fn root_depth(&self) -> u8 {
        self.screen().root_depth
    }

    /// Get the atoms
    pub fn atoms(&self) -> &Atoms {
        &self.atoms
    }

    /// Flush pending requests
    pub fn flush(&self) -> Result<()> {
        self.conn.flush()?;
        Ok(())
    }

    /// Poll for the next event (non-blocking)
    pub async fn next_event(&self) -> Result<Option<x11rb::protocol::Event>> {
        // Use poll_for_event for non-blocking check
        // In a real implementation, we'd use the file descriptor with tokio
        match self.conn.poll_for_event()? {
            Some(event) => Ok(Some(event)),
            None => {
                // Small yield to prevent busy-waiting
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                Ok(None)
            }
        }
    }

    /// Generate a new X11 ID
    pub fn generate_id(&self) -> Result<u32> {
        Ok(self.conn.generate_id()?)
    }

    /// Subscribe to PropertyNotify events on the root window
    /// This allows us to get notified when _NET_ACTIVE_WINDOW changes
    pub fn subscribe_to_root_property_changes(&self) -> Result<()> {
        let attrs = ChangeWindowAttributesAux::new()
            .event_mask(EventMask::PROPERTY_CHANGE);
        self.conn.change_window_attributes(self.root(), &attrs)?;
        self.flush()?;
        Ok(())
    }

    /// Get the currently active window from _NET_ACTIVE_WINDOW
    pub fn get_active_window(&self) -> Option<u32> {
        let reply = self.conn
            .get_property(
                false,
                self.root(),
                self.atoms.net_active_window,
                AtomEnum::WINDOW,
                0,
                1,
            )
            .ok()?
            .reply()
            .ok()?;

        if reply.value_len == 1 && reply.value.len() >= 4 {
            let window = u32::from_ne_bytes(reply.value[0..4].try_into().ok()?);
            if window != 0 {
                return Some(window);
            }
        }
        None
    }

    /// Get the title of a window from _NET_WM_NAME or WM_NAME
    pub fn get_window_title(&self, window: u32) -> Option<String> {
        // Try _NET_WM_NAME first (UTF-8)
        if let Ok(reply) = self.conn.get_property(
            false,
            window,
            self.atoms.net_wm_name,
            self.atoms.utf8_string,
            0,
            1024,
        ) {
            if let Ok(reply) = reply.reply() {
                if reply.value_len > 0 {
                    if let Ok(title) = String::from_utf8(reply.value) {
                        return Some(title);
                    }
                }
            }
        }

        // Fall back to WM_NAME
        if let Ok(reply) = self.conn.get_property(
            false,
            window,
            self.atoms.wm_name,
            AtomEnum::STRING,
            0,
            1024,
        ) {
            if let Ok(reply) = reply.reply() {
                if reply.value_len > 0 {
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

    /// Get the RandR event base (for identifying RandR events)
    pub fn randr_event_base(&self) -> Option<u8> {
        self.randr_event_base
    }

    /// Check if RandR is available
    pub fn has_randr(&self) -> bool {
        self.randr_event_base.is_some()
    }

    /// Query all connected monitors/outputs
    pub fn query_monitors(&self) -> Result<Vec<MonitorInfo>> {
        let root = self.root();
        let mut monitors = Vec::new();

        // Get screen resources
        let resources = self
            .conn
            .randr_get_screen_resources_current(root)?
            .reply()
            .context("Failed to get screen resources")?;

        // Get primary output
        let primary_output = self
            .conn
            .randr_get_output_primary(root)?
            .reply()
            .map(|r| r.output)
            .unwrap_or(0);

        // Iterate through CRTCs to find active outputs
        for crtc in &resources.crtcs {
            let crtc_info = match self.conn.randr_get_crtc_info(*crtc, resources.config_timestamp) {
                Ok(cookie) => match cookie.reply() {
                    Ok(info) => info,
                    Err(_) => continue,
                },
                Err(_) => continue,
            };

            // Skip disabled CRTCs
            if crtc_info.width == 0 || crtc_info.height == 0 {
                continue;
            }

            // Get the first output connected to this CRTC for the name
            let output_name = if let Some(&output) = crtc_info.outputs.first() {
                match self
                    .conn
                    .randr_get_output_info(output, resources.config_timestamp)
                {
                    Ok(cookie) => match cookie.reply() {
                        Ok(info) => String::from_utf8_lossy(&info.name).to_string(),
                        Err(_) => format!("CRTC-{}", crtc),
                    },
                    Err(_) => format!("CRTC-{}", crtc),
                }
            } else {
                format!("CRTC-{}", crtc)
            };

            let is_primary = crtc_info.outputs.contains(&primary_output);

            monitors.push(MonitorInfo {
                name: output_name,
                x: crtc_info.x,
                y: crtc_info.y,
                width: crtc_info.width,
                height: crtc_info.height,
                primary: is_primary,
            });
        }

        // Sort by x position (left to right)
        monitors.sort_by_key(|m| m.x);

        debug!("Found {} monitors: {:?}", monitors.len(), monitors);
        Ok(monitors)
    }

    /// Get the primary monitor (or first monitor if no primary is set)
    pub fn primary_monitor(&self) -> Result<Option<MonitorInfo>> {
        let monitors = self.query_monitors()?;

        // Try to find primary
        if let Some(primary) = monitors.iter().find(|m| m.primary) {
            return Ok(Some(primary.clone()));
        }

        // Fall back to first monitor
        Ok(monitors.into_iter().next())
    }

    /// Check if an event is a RandR ScreenChangeNotify event
    pub fn is_randr_screen_change(&self, event: &x11rb::protocol::Event) -> bool {
        if let Some(base) = self.randr_event_base {
            if matches!(event, x11rb::protocol::Event::RandrScreenChangeNotify(_)) {
                return true;
            }
            // Also check raw event type in case of generic event
            let event_type = event.raw_response_type();
            return event_type == base; // ScreenChangeNotify is base + 0
        }
        false
    }

    /// Check if an event is a RandR Notify event (CRTC/Output change)
    pub fn is_randr_notify(&self, event: &x11rb::protocol::Event) -> bool {
        if let Some(base) = self.randr_event_base {
            if matches!(event, x11rb::protocol::Event::RandrNotify(_)) {
                return true;
            }
            let event_type = event.raw_response_type();
            return event_type == base + 1; // Notify is base + 1
        }
        false
    }
}
