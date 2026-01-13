use anyhow::{Context as AnyhowContext, Result};
use cairo::{Context, Format, ImageSurface};
use tracing::debug;
use x11rb::connection::Connection as X11Connection;
use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};

use crate::x11::Connection;

/// Render surface backed by Cairo ImageSurface
/// Uses X11 PutImage to copy to window
pub struct RenderSurface {
    surface: ImageSurface,
    width: i32,
    height: i32,
}

impl RenderSurface {
    /// Create a new Cairo ImageSurface
    pub fn new(width: u16, height: u16) -> Result<Self> {
        let surface = ImageSurface::create(Format::ARgb32, width as i32, height as i32)
            .context("Failed to create ImageSurface")?;

        debug!("Created Cairo ImageSurface {}x{}", width, height);

        Ok(Self {
            surface,
            width: width as i32,
            height: height as i32,
        })
    }

    /// Get a Cairo context for drawing
    pub fn context(&self) -> Result<Context> {
        Context::new(&self.surface).context("Failed to create Cairo context")
    }

    /// Copy the surface contents to an X11 window
    pub fn copy_to_window(&mut self, conn: &Connection, window: u32, gc: u32) -> Result<()> {
        self.surface.flush();

        let data = self.surface.data().context("Failed to get surface data")?;

        // Cairo uses ARGB32 (native endian), X11 expects BGRA for ZPixmap on most systems
        // For simplicity, we'll send as-is and rely on the visual matching
        // In practice, we may need byte swapping depending on endianness

        conn.conn.put_image(
            ImageFormat::Z_PIXMAP,
            window,
            gc,
            self.width as u16,
            self.height as u16,
            0,  // dst_x
            0,  // dst_y
            0,  // left_pad
            24, // depth (RGB)
            &data,
        )?;

        conn.conn.flush()?;
        Ok(())
    }

    /// Resize the surface
    pub fn resize(&mut self, width: u16, height: u16) -> Result<()> {
        self.surface = ImageSurface::create(Format::ARgb32, width as i32, height as i32)
            .context("Failed to create ImageSurface")?;
        self.width = width as i32;
        self.height = height as i32;
        Ok(())
    }

    /// Flush the surface
    pub fn flush(&self) {
        self.surface.flush();
    }

    /// Get surface dimensions
    pub fn size(&self) -> (i32, i32) {
        (self.width, self.height)
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }
}

/// Double-buffered render surface for flicker-free drawing
pub struct DoubleBufferedSurface {
    front: RenderSurface,
    back: RenderSurface,
    width: i32,
    height: i32,
}

impl DoubleBufferedSurface {
    pub fn new(width: u16, height: u16) -> Result<Self> {
        let front = RenderSurface::new(width, height)?;
        let back = RenderSurface::new(width, height)?;

        Ok(Self {
            front,
            back,
            width: width as i32,
            height: height as i32,
        })
    }

    /// Get a Cairo context for drawing to the back buffer
    pub fn context(&self) -> Result<Context> {
        self.back.context()
    }

    /// Swap buffers - copy back buffer to front and to window
    pub fn swap(&mut self, conn: &Connection, window: u32, gc: u32) -> Result<()> {
        // Copy back to front
        {
            let cr = self.front.context()?;
            cr.set_source_surface(&self.back.surface, 0.0, 0.0)?;
            cr.paint()?;
        }

        // Copy front to window
        self.front.copy_to_window(conn, window, gc)?;
        Ok(())
    }

    /// Resize the buffers
    pub fn resize(&mut self, width: u16, height: u16) -> Result<()> {
        self.front.resize(width, height)?;
        self.back.resize(width, height)?;
        self.width = width as i32;
        self.height = height as i32;
        Ok(())
    }

    pub fn size(&self) -> (i32, i32) {
        (self.width, self.height)
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }
}
