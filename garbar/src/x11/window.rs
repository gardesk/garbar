use anyhow::Result;
use tracing::debug;
use x11rb::connection::Connection as X11Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ColormapAlloc, ConfigureWindowAux, ConnectionExt, CreateGCAux, CreateWindowAux,
    EventMask, PropMode, WindowClass,
};
use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;

use super::Connection;

/// Default bar height in pixels
const DEFAULT_BAR_HEIGHT: u16 = 32;

/// Bar window
pub struct BarWindow {
    pub window: u32,
    pub gc: u32,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

impl BarWindow {
    /// Create a new bar window with default height
    pub fn new(conn: &Connection) -> Result<Self> {
        Self::with_height(conn, DEFAULT_BAR_HEIGHT)
    }

    /// Create a new bar window with specified height
    pub fn with_height(conn: &Connection, height: u16) -> Result<Self> {
        let width = conn.screen_width();
        let x: i16 = 0;
        let y: i16 = 0; // Top of screen

        // Generate window ID
        let window = conn.generate_id()?;

        // Create colormap for proper color handling
        let colormap = conn.generate_id()?;
        conn.conn.create_colormap(
            ColormapAlloc::NONE,
            colormap,
            conn.root(),
            conn.root_visual(),
        )?;

        // Window attributes
        let values = CreateWindowAux::new()
            .background_pixel(0x1a1a1a) // Dark gray background
            .border_pixel(0x000000)
            .event_mask(
                EventMask::EXPOSURE
                    | EventMask::BUTTON_PRESS
                    | EventMask::BUTTON_RELEASE
                    | EventMask::ENTER_WINDOW
                    | EventMask::LEAVE_WINDOW
                    | EventMask::POINTER_MOTION
                    | EventMask::STRUCTURE_NOTIFY,
            )
            .override_redirect(1) // Bypass window manager
            .colormap(colormap);

        // Create the window
        conn.conn.create_window(
            conn.root_depth(),
            window,
            conn.root(),
            x,
            y,
            width,
            height,
            0, // border width
            WindowClass::INPUT_OUTPUT,
            conn.root_visual(),
            &values,
        )?;

        debug!("Created window {} with dimensions {}x{}", window, width, height);

        // Create graphics context for drawing
        let gc = conn.generate_id()?;
        let gc_values = CreateGCAux::new()
            .foreground(0xffffff) // White foreground
            .background(0x1a1a1a); // Dark gray background

        conn.conn.create_gc(gc, window, &gc_values)?;

        // Set window properties
        Self::set_window_properties(conn, window, width, height)?;

        Ok(Self {
            window,
            gc,
            x,
            y,
            width,
            height,
        })
    }

    /// Set EWMH properties for the bar window
    fn set_window_properties(conn: &Connection, window: u32, width: u16, height: u16) -> Result<()> {
        let atoms = conn.atoms();

        // Set window type to dock
        conn.conn.change_property32(
            PropMode::REPLACE,
            window,
            atoms.net_wm_window_type,
            AtomEnum::ATOM,
            &[atoms.net_wm_window_type_dock],
        )?;

        // Set window state (sticky + above)
        conn.conn.change_property32(
            PropMode::REPLACE,
            window,
            atoms.net_wm_state,
            AtomEnum::ATOM,
            &[atoms.net_wm_state_sticky, atoms.net_wm_state_above],
        )?;

        // Set struts to reserve screen space
        // Format: left, right, top, bottom
        conn.conn.change_property32(
            PropMode::REPLACE,
            window,
            atoms.net_wm_strut,
            AtomEnum::CARDINAL,
            &[0, 0, height as u32, 0],
        )?;

        // Set partial struts (more detailed)
        // Format: left, right, top, bottom,
        //         left_start_y, left_end_y, right_start_y, right_end_y,
        //         top_start_x, top_end_x, bottom_start_x, bottom_end_x
        conn.conn.change_property32(
            PropMode::REPLACE,
            window,
            atoms.net_wm_strut_partial,
            AtomEnum::CARDINAL,
            &[
                0,
                0,
                height as u32,
                0,
                0,
                0,
                0,
                0,
                0,
                (width - 1) as u32,
                0,
                0,
            ],
        )?;

        // Set window name
        conn.conn.change_property8(
            PropMode::REPLACE,
            window,
            atoms.net_wm_name,
            atoms.utf8_string,
            b"garbar",
        )?;

        conn.conn.change_property8(
            PropMode::REPLACE,
            window,
            atoms.wm_name,
            AtomEnum::STRING,
            b"garbar",
        )?;

        // Set WM_CLASS (instance, class)
        conn.conn.change_property8(
            PropMode::REPLACE,
            window,
            atoms.wm_class,
            AtomEnum::STRING,
            b"garbar\0garbar\0",
        )?;

        debug!("Set window properties for window {}", window);
        Ok(())
    }

    /// Set window opacity (0.0 to 1.0)
    pub fn set_opacity(&self, conn: &Connection, opacity: f64) -> Result<()> {
        let atoms = conn.atoms();
        // Opacity is a 32-bit cardinal where 0xFFFFFFFF = 100% opaque
        let opacity_value = (opacity.clamp(0.0, 1.0) * u32::MAX as f64) as u32;
        conn.conn.change_property32(
            PropMode::REPLACE,
            self.window,
            atoms.net_wm_window_opacity,
            AtomEnum::CARDINAL,
            &[opacity_value],
        )?;
        debug!("Set window opacity to {:.2}", opacity);
        Ok(())
    }

    /// Map the window to make it visible
    pub fn map(&self, conn: &Connection) -> Result<()> {
        conn.conn.map_window(self.window)?;
        debug!("Mapped window {}", self.window);
        Ok(())
    }

    /// Unmap the window to hide it
    pub fn unmap(&self, conn: &Connection) -> Result<()> {
        conn.conn.unmap_window(self.window)?;
        debug!("Unmapped window {}", self.window);
        Ok(())
    }

    /// Draw the bar (placeholder - just fills with background color)
    pub fn draw(&self, conn: &Connection) -> Result<()> {
        // For now, just clear to background color
        // This will be replaced with proper Cairo rendering in Sprint 1
        conn.conn.clear_area(
            false,
            self.window,
            0,
            0,
            self.width,
            self.height,
        )?;

        conn.conn.flush()?;
        debug!("Drew bar window");
        Ok(())
    }

    /// Resize the bar window
    pub fn resize(&mut self, conn: &Connection, width: u16, height: u16) -> Result<()> {
        let values = ConfigureWindowAux::new().width(width as u32).height(height as u32);
        conn.conn.configure_window(self.window, &values)?;
        self.width = width;
        self.height = height;

        // Update struts
        Self::set_window_properties(conn, self.window, width, height)?;

        debug!("Resized bar to {}x{}", width, height);
        Ok(())
    }

    /// Move the bar window
    pub fn move_to(&mut self, conn: &Connection, x: i16, y: i16) -> Result<()> {
        let values = ConfigureWindowAux::new().x(x as i32).y(y as i32);
        conn.conn.configure_window(self.window, &values)?;
        self.x = x;
        self.y = y;
        debug!("Moved bar to ({}, {})", x, y);
        Ok(())
    }
}

impl Drop for BarWindow {
    fn drop(&mut self) {
        debug!("BarWindow dropped (window {})", self.window);
        // Note: Window is automatically destroyed when connection closes
    }
}
