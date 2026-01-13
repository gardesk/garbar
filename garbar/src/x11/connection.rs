use anyhow::{Context, Result};
use std::sync::Arc;
use x11rb::connection::Connection as X11Connection;
use x11rb::protocol::xproto::{Screen, Visualid};
use x11rb::rust_connection::RustConnection;

use super::Atoms;

/// X11 connection wrapper
pub struct Connection {
    pub(crate) conn: Arc<RustConnection>,
    pub(crate) screen_num: usize,
    pub(crate) atoms: Atoms,
}

impl Connection {
    /// Create a new X11 connection
    pub fn new() -> Result<Self> {
        let (conn, screen_num) =
            RustConnection::connect(None).context("Failed to connect to X11 server")?;

        let conn = Arc::new(conn);
        let atoms = Atoms::new(&*conn).context("Failed to intern atoms")?;

        Ok(Self {
            conn,
            screen_num,
            atoms,
        })
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
}
