use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

use super::signals::SignalHandler;
use crate::render::{
    Block, BlockStyle, Color, Gradient, GradientStop, Layout, Padding,
    RenderSurface, TextRenderer,
};
use crate::x11::{Connection, BarWindow};

/// Get the path to the PID file
fn pid_file_path() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("garbar.pid")
}

/// Check if an existing daemon is running
fn check_existing_daemon() -> Result<()> {
    let pid_path = pid_file_path();

    if pid_path.exists() {
        let pid_str = fs::read_to_string(&pid_path)?;
        let pid: i32 = pid_str.trim().parse()?;

        // Check if process is still running
        let proc_path = format!("/proc/{}", pid);
        if std::path::Path::new(&proc_path).exists() {
            anyhow::bail!(
                "garbar daemon already running (PID {}). \
                 If this is incorrect, remove {}",
                pid,
                pid_path.display()
            );
        } else {
            warn!("Removing stale PID file for PID {}", pid);
            fs::remove_file(&pid_path)?;
        }
    }

    Ok(())
}

/// Write the current process PID to the PID file
fn write_pid_file() -> Result<()> {
    let pid_path = pid_file_path();
    let pid = std::process::id();

    let mut file = fs::File::create(&pid_path)?;
    writeln!(file, "{}", pid)?;

    debug!("Wrote PID {} to {}", pid, pid_path.display());
    Ok(())
}

/// Remove the PID file
fn remove_pid_file() {
    let pid_path = pid_file_path();
    if let Err(e) = fs::remove_file(&pid_path) {
        warn!("Failed to remove PID file: {}", e);
    } else {
        debug!("Removed PID file");
    }
}

/// Main daemon state
pub struct DaemonState {
    conn: Connection,
    bar: BarWindow,
    surface: RenderSurface,
    text_renderer: TextRenderer,
    signal_handler: SignalHandler,
    running: bool,
}

impl DaemonState {
    /// Create a new daemon state
    pub async fn new() -> Result<Self> {
        // Connect to X11
        let conn = Connection::new().context("Failed to connect to X11 display")?;
        info!("Connected to X11 display");

        // Create bar window
        let bar = BarWindow::new(&conn).context("Failed to create bar window")?;
        info!(
            "Created bar window: {}x{} at ({}, {})",
            bar.width, bar.height, bar.x, bar.y
        );

        // Create render surface
        let surface = RenderSurface::new(bar.width, bar.height)
            .context("Failed to create render surface")?;

        // Create text renderer with default fonts
        let text_renderer = TextRenderer::new(&[
            "monospace:size=10".to_string(),
            "Font Awesome 6 Free:size=10".to_string(),
        ]);

        // Set up signal handler
        let signal_handler = SignalHandler::new()?;

        Ok(Self {
            conn,
            bar,
            surface,
            text_renderer,
            signal_handler,
            running: true,
        })
    }

    /// Run the main event loop
    pub async fn run(&mut self) -> Result<()> {
        info!("Entering main event loop");

        // Map the window to make it visible
        self.bar.map(&self.conn)?;
        self.conn.flush()?;

        while self.running {
            tokio::select! {
                // Handle signals
                signal = self.signal_handler.recv() => {
                    match signal {
                        Some(signals::Signal::Terminate) => {
                            info!("Received termination signal, shutting down");
                            self.running = false;
                        }
                        Some(signals::Signal::Reload) => {
                            info!("Received reload signal");
                            self.handle_reload().await?;
                        }
                        None => {
                            warn!("Signal handler closed unexpectedly");
                            self.running = false;
                        }
                    }
                }

                // Handle X11 events
                event = self.conn.next_event() => {
                    match event {
                        Ok(Some(event)) => {
                            self.handle_x11_event(event)?;
                        }
                        Ok(None) => {
                            // No event available, continue
                        }
                        Err(e) => {
                            error!("X11 event error: {}", e);
                            self.running = false;
                        }
                    }
                }
            }
        }

        info!("Event loop terminated");
        Ok(())
    }

    /// Handle X11 events
    fn handle_x11_event(&mut self, event: x11rb::protocol::Event) -> Result<()> {
        use x11rb::protocol::Event;

        match event {
            Event::Expose(e) if e.window == self.bar.window => {
                debug!("Expose event on bar window");
                self.draw_bar()?;
            }
            Event::ButtonPress(e) if e.event == self.bar.window => {
                debug!(
                    "Button {} pressed at ({}, {})",
                    e.detail, e.event_x, e.event_y
                );
                // TODO: Route to modules
            }
            Event::ConfigureNotify(e) if e.window == self.bar.window => {
                debug!("Configure notify: {}x{}", e.width, e.height);
            }
            Event::DestroyNotify(e) if e.window == self.bar.window => {
                warn!("Bar window destroyed externally");
                self.running = false;
            }
            _ => {
                // Ignore other events for now
            }
        }

        Ok(())
    }

    /// Draw the bar using Cairo
    fn draw_bar(&mut self) -> Result<()> {
        let cr = self.surface.context()?;
        let width = self.bar.width as f64;
        let height = self.bar.height as f64;

        // Create a gradient background
        let background = Gradient::horizontal(vec![
            GradientStop::new(0.0, Color::from_hex("#1a1a2e")?),
            GradientStop::new(0.5, Color::from_hex("#16213e")?),
            GradientStop::new(1.0, Color::from_hex("#1a1a2e")?),
        ]);

        // Fill background
        background.apply(&cr, 0.0, 0.0, width, height);
        cr.rectangle(0.0, 0.0, width, height);
        cr.fill()?;

        // Create a simple layout with demo blocks
        let mut layout = Layout::new();

        // Left: workspaces placeholder
        layout.left.push(
            Block::new("  1  2  3  4  5 ")
                .with_style(
                    BlockStyle::new()
                        .with_foreground(Color::from_hex("#e0e0e0")?)
                        .with_padding(Padding::horizontal(8.0)),
                ),
        );

        // Center: window title placeholder
        layout.center.push(
            Block::new("garbar - Status Bar")
                .with_style(
                    BlockStyle::new()
                        .with_foreground(Color::from_hex("#888888")?)
                        .with_padding(Padding::horizontal(8.0)),
                ),
        );

        // Right: status modules placeholder
        layout.right.push(
            Block::new(" 45%")
                .with_style(
                    BlockStyle::new()
                        .with_foreground(Color::from_hex("#5294e2")?)
                        .with_padding(Padding::horizontal(8.0)),
                ),
        );

        layout.right.push(
            Block::new(" 2.1G")
                .with_style(
                    BlockStyle::new()
                        .with_foreground(Color::from_hex("#98c379")?)
                        .with_padding(Padding::horizontal(8.0)),
                ),
        );

        layout.right.push(
            Block::new(" 85%")
                .with_style(
                    BlockStyle::new()
                        .with_foreground(Color::from_hex("#e5c07b")?)
                        .with_padding(Padding::horizontal(8.0)),
                ),
        );

        layout.right.push(
            Block::new(" Mon Jan 13 14:30")
                .with_style(
                    BlockStyle::new()
                        .with_foreground(Color::from_hex("#e0e0e0")?)
                        .with_padding(Padding::horizontal(8.0)),
                ),
        );

        // Compute and render blocks
        let bar_padding = Padding::new(8.0, 8.0, 0.0, 0.0);
        let positioned = layout.compute(&cr, &self.text_renderer, width, height, &bar_padding);

        for block in &positioned {
            block.render(&cr, &self.text_renderer);
        }

        // Copy surface to window
        self.surface.copy_to_window(&self.conn, self.bar.window, self.bar.gc)?;

        debug!("Drew bar with {} blocks", positioned.len());
        Ok(())
    }

    /// Handle configuration reload
    async fn handle_reload(&mut self) -> Result<()> {
        info!("Reloading configuration...");
        // Redraw bar after reload
        self.draw_bar()?;
        Ok(())
    }
}

impl Drop for DaemonState {
    fn drop(&mut self) {
        debug!("DaemonState dropped, cleaning up");
    }
}

/// Run the daemon
pub async fn run(_config: Option<String>, _foreground: bool) -> Result<()> {
    // Check for existing daemon
    check_existing_daemon()?;

    // Write PID file
    write_pid_file()?;

    // Ensure PID file is removed on exit
    struct PidGuard;
    impl Drop for PidGuard {
        fn drop(&mut self) {
            remove_pid_file();
        }
    }
    let _pid_guard = PidGuard;

    // Create and run daemon
    let mut daemon = DaemonState::new().await?;
    daemon.run().await?;

    info!("Daemon shutdown complete");
    Ok(())
}

use super::signals;
