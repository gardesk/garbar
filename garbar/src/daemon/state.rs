use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use super::signals::SignalHandler;
use crate::config::{
    BackgroundConfig, BarConfig, ConfigLoader,
    GradientDirection as ConfigGradientDir, GradientType,
};
use crate::ipc::{Command as IpcCommand, IpcServer};
use crate::modules::{ModuleRegistry, TrayManager, TrayModule};
use crate::render::{
    Background, Color, Gradient, GradientDirection,
    GradientStop, Layout, Padding, RenderSurface, TextRenderer,
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

/// Convert config background to render background
fn config_to_background(config: &BackgroundConfig) -> Result<Background> {
    match config {
        BackgroundConfig::Solid(color_str) => {
            let color = Color::from_hex(color_str)
                .with_context(|| format!("Invalid background color: {}", color_str))?;
            Ok(Background::Solid(color))
        }
        BackgroundConfig::Gradient(grad_cfg) => {
            let stops: Result<Vec<_>> = grad_cfg
                .stops
                .iter()
                .map(|s| {
                    let color = Color::from_hex(&s.color)
                        .with_context(|| format!("Invalid gradient color: {}", s.color))?;
                    Ok(GradientStop::new(s.position, color))
                })
                .collect();
            let stops = stops?;

            let gradient = match grad_cfg.gradient_type {
                GradientType::Gradient => {
                    // Linear gradient
                    match grad_cfg.direction {
                        ConfigGradientDir::Horizontal => Gradient::horizontal(stops),
                        ConfigGradientDir::Vertical => Gradient::vertical(stops),
                        ConfigGradientDir::Diagonal => Gradient::Linear {
                            direction: GradientDirection::Diagonal,
                            stops,
                        },
                    }
                }
                GradientType::Radial => {
                    let center = grad_cfg.center.unwrap_or((0.5, 0.5));
                    let radius = grad_cfg.radius.unwrap_or(1.0);
                    Gradient::radial(center, radius, stops)
                }
            };
            Ok(Background::Gradient(gradient))
        }
    }
}

/// Tracks which module and block index a positioned block belongs to
#[derive(Debug, Clone)]
struct BlockOwner {
    module_name: String,
    block_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// Main daemon state
pub struct DaemonState {
    conn: Connection,
    bar: BarWindow,
    surface: RenderSurface,
    text_renderer: TextRenderer,
    signal_handler: SignalHandler,
    config: BarConfig,
    config_loader: ConfigLoader,
    modules: ModuleRegistry,
    running: bool,
    /// Cached block positions for hit testing
    block_owners: Vec<BlockOwner>,
    /// IPC server for garbarctl communication
    ipc_server: IpcServer,
    /// Channel to receive IPC commands
    ipc_rx: Receiver<IpcCommand>,
    /// Whether the bar is currently visible
    visible: bool,
    /// System tray manager
    tray_manager: Option<TrayManager>,
}

impl DaemonState {
    /// Create a new daemon state
    pub async fn new() -> Result<Self> {
        // Load configuration
        let config_loader = ConfigLoader::new();
        let config = config_loader.load()?;
        info!(
            "Loaded config: height={}, position={:?}",
            config.height, config.position
        );

        // Connect to X11
        let conn = Connection::new().context("Failed to connect to X11 display")?;
        info!("Connected to X11 display");

        // Create bar window with config height
        let bar = BarWindow::with_height(&conn, config.height)
            .context("Failed to create bar window")?;
        info!(
            "Created bar window: {}x{} at ({}, {})",
            bar.width, bar.height, bar.x, bar.y
        );

        // Apply window opacity
        bar.set_opacity(&conn, config.opacity)?;

        // Create render surface
        let surface = RenderSurface::new(bar.width, bar.height)
            .context("Failed to create render surface")?;

        // Create text renderer with configured fonts
        let text_renderer = TextRenderer::new(&config.fonts);

        // Initialize modules from config
        let mut modules = ModuleRegistry::from_config(&config);
        info!(
            "Initialized modules: left={:?}, center={:?}, right={:?}",
            config.modules_left, config.modules_center, config.modules_right
        );

        // Initialize system tray if the tray module is configured
        let tray_manager = if modules.has_module("tray") {
            match TrayManager::new(
                Arc::clone(&conn.conn),
                conn.screen_num,
                bar.window,
                &config.modules.tray,
            ) {
                Some(mut manager) => {
                    if manager.acquire_selection() {
                        // Replace placeholder tray module with properly initialized one
                        let tray_module = TrayModule::new(&config.modules.tray, manager.shared_state());
                        modules.replace_module("tray", Box::new(tray_module));
                        info!("System tray initialized");
                        Some(manager)
                    } else {
                        warn!("Failed to acquire system tray selection (another tray running?)");
                        None
                    }
                }
                None => {
                    warn!("Failed to create TrayManager");
                    None
                }
            }
        } else {
            None
        };

        // Set up signal handler
        let signal_handler = SignalHandler::new()?;

        // Initialize IPC server
        let (mut ipc_server, ipc_rx) = IpcServer::new();
        if let Err(e) = ipc_server.start() {
            warn!("Failed to start IPC server: {}", e);
        }

        Ok(Self {
            conn,
            bar,
            surface,
            text_renderer,
            config,
            config_loader,
            modules,
            signal_handler,
            running: true,
            block_owners: Vec::new(),
            ipc_server,
            ipc_rx,
            visible: true,
            tray_manager,
        })
    }

    /// Run the main event loop
    pub async fn run(&mut self) -> Result<()> {
        info!("Entering main event loop");

        // Subscribe to PropertyNotify on root window for instant focus tracking
        if let Err(e) = self.conn.subscribe_to_root_property_changes() {
            warn!("Failed to subscribe to root property changes: {}", e);
        }

        // Map the window to make it visible
        self.bar.map(&self.conn)?;
        self.conn.flush()?;

        // Initial update and draw
        self.modules.update_all().await;
        self.draw_bar().await?;

        // Create update timer (50ms for snappy workspace updates)
        let mut update_interval = tokio::time::interval(Duration::from_millis(50));

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
                        Ok(Some(evt)) => {
                            self.handle_x11_event(evt).await?;
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

                // Periodic module updates
                _ = update_interval.tick() => {
                    // Check for IPC commands (non-blocking)
                    self.poll_ipc_commands().await?;

                    self.modules.update_all().await;
                    if self.visible {
                        self.draw_bar().await?;
                    }
                }
            }
        }

        // Clean up IPC server
        self.ipc_server.stop();

        info!("Event loop terminated");
        Ok(())
    }

    /// Poll for IPC commands (non-blocking)
    async fn poll_ipc_commands(&mut self) -> Result<()> {
        use std::sync::mpsc::TryRecvError;

        loop {
            match self.ipc_rx.try_recv() {
                Ok(cmd) => {
                    self.handle_ipc_command(cmd).await?;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    warn!("IPC channel disconnected");
                    break;
                }
            }
        }
        Ok(())
    }

    /// Handle an IPC command
    async fn handle_ipc_command(&mut self, cmd: IpcCommand) -> Result<()> {
        match cmd {
            IpcCommand::Show => {
                info!("IPC: show command received");
                if !self.visible {
                    self.bar.map(&self.conn)?;
                    self.conn.flush()?;
                    self.visible = true;
                    self.draw_bar().await?;
                }
            }
            IpcCommand::Hide => {
                info!("IPC: hide command received");
                if self.visible {
                    self.bar.unmap(&self.conn)?;
                    self.conn.flush()?;
                    self.visible = false;
                }
            }
            IpcCommand::Toggle => {
                info!("IPC: toggle command received");
                if self.visible {
                    self.bar.unmap(&self.conn)?;
                    self.conn.flush()?;
                    self.visible = false;
                } else {
                    self.bar.map(&self.conn)?;
                    self.conn.flush()?;
                    self.visible = true;
                    self.draw_bar().await?;
                }
            }
            IpcCommand::Reload => {
                info!("IPC: reload command received");
                self.handle_reload().await?;
            }
            IpcCommand::Quit => {
                info!("IPC: quit command received");
                self.running = false;
            }
            IpcCommand::Status => {
                // Status is handled by the IPC server directly, ignore here
            }
            IpcCommand::UpdateModule { module } => {
                info!("IPC: update module '{}' command received", module);
                self.modules.update(&module).await;
                if self.visible {
                    self.draw_bar().await?;
                }
            }
        }
        Ok(())
    }

    /// Handle X11 events
    async fn handle_x11_event(&mut self, event: x11rb::protocol::Event) -> Result<()> {
        use x11rb::protocol::Event;

        // Check for RandR events first (they use extension event types)
        if self.conn.is_randr_screen_change(&event) || self.conn.is_randr_notify(&event) {
            info!("RandR event detected, checking for monitor changes");
            self.handle_monitor_change().await?;
            return Ok(());
        }

        match event {
            Event::Expose(e) if e.window == self.bar.window => {
                debug!("Expose event on bar window");
                self.draw_bar().await?;
            }
            Event::ButtonPress(e) if e.event == self.bar.window => {
                debug!(
                    "Button {} pressed at ({}, {})",
                    e.detail, e.event_x, e.event_y
                );
                // Hit test to find which block was clicked/scrolled
                let x = e.event_x as f64;
                let y = e.event_y as f64;
                if let Some(owner) = self.hit_test(x, y) {
                    match e.detail {
                        // Button4 = scroll up, Button5 = scroll down
                        4 | 5 => {
                            let scroll_up = e.detail == 4;
                            debug!(
                                "Scroll {} on module '{}' block {}",
                                if scroll_up { "up" } else { "down" },
                                owner.module_name,
                                owner.block_index
                            );
                            self.modules
                                .dispatch_scroll(
                                    &owner.module_name,
                                    scroll_up,
                                    owner.block_index,
                                    e.event_x,
                                    e.event_y,
                                )
                                .await;
                        }
                        // Button 1-3 = normal click (left, middle, right)
                        button => {
                            debug!(
                                "Click hit module '{}' block {}",
                                owner.module_name, owner.block_index
                            );
                            self.modules
                                .dispatch_click(
                                    &owner.module_name,
                                    button,
                                    owner.block_index,
                                    e.event_x,
                                    e.event_y,
                                )
                                .await;
                        }
                    }
                    // Redraw after click/scroll (module state may have changed)
                    self.draw_bar().await?;
                }
            }
            Event::ConfigureNotify(e) if e.window == self.bar.window => {
                debug!("Configure notify: {}x{}", e.width, e.height);
            }
            Event::DestroyNotify(e) if e.window == self.bar.window => {
                warn!("Bar window destroyed externally");
                self.running = false;
            }
            // Handle PropertyNotify on root window for instant focus tracking
            Event::PropertyNotify(e) if e.window == self.conn.root() => {
                let atoms = self.conn.atoms();
                if e.atom == atoms.net_active_window {
                    debug!("_NET_ACTIVE_WINDOW changed, updating window title");
                    self.modules.update("window_title").await;
                    self.draw_bar().await?;
                }
            }
            // RandR events are also delivered as specific event types
            Event::RandrScreenChangeNotify(_) => {
                info!("RandR ScreenChangeNotify event");
                self.handle_monitor_change().await?;
            }
            Event::RandrNotify(_) => {
                info!("RandR Notify event (CRTC/Output change)");
                self.handle_monitor_change().await?;
            }
            // Handle system tray client messages
            Event::ClientMessage(e) => {
                if let Some(ref mut tray) = self.tray_manager {
                    if tray.handle_client_message(&e) {
                        // Tray icon may have been added, redraw
                        self.draw_bar().await?;
                    }
                }
            }
            // Handle tray icon destruction
            Event::DestroyNotify(e) => {
                if let Some(ref mut tray) = self.tray_manager {
                    if tray.handle_destroy(e.window) {
                        debug!("Tray icon {} destroyed", e.window);
                        self.draw_bar().await?;
                    }
                }
            }
            // Handle tray icon unmapping
            Event::UnmapNotify(e) => {
                if let Some(ref mut tray) = self.tray_manager {
                    if tray.handle_unmap(e.window) {
                        debug!("Tray icon {} unmapped", e.window);
                        self.draw_bar().await?;
                    }
                }
            }
            _ => {
                // Ignore other events for now
            }
        }

        Ok(())
    }

    /// Handle monitor configuration changes
    async fn handle_monitor_change(&mut self) -> Result<()> {
        info!("Handling monitor change...");

        // Query current monitors
        let monitors = match self.conn.query_monitors() {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to query monitors: {}", e);
                return Ok(());
            }
        };

        if monitors.is_empty() {
            warn!("No monitors found after change");
            return Ok(());
        }

        // Get the primary monitor (or first one)
        let monitor = self
            .conn
            .primary_monitor()?
            .unwrap_or_else(|| monitors[0].clone());

        info!(
            "Primary monitor: {} ({}x{} at {},{})",
            monitor.name, monitor.width, monitor.height, monitor.x, monitor.y
        );

        // Check if bar geometry needs to change
        let new_width = monitor.width;
        let new_x = monitor.x;
        let new_y = monitor.y;

        let needs_resize = self.bar.width != new_width;
        let needs_move = self.bar.x != new_x || self.bar.y != new_y;

        if needs_resize || needs_move {
            info!(
                "Updating bar geometry: {}x{} at ({}, {}) -> {}x{} at ({}, {})",
                self.bar.width,
                self.config.height,
                self.bar.x,
                self.bar.y,
                new_width,
                self.config.height,
                new_x,
                new_y
            );

            // Move and resize bar
            if needs_move {
                self.bar.move_to(&self.conn, new_x, new_y)?;
            }

            if needs_resize {
                self.bar.resize(&self.conn, new_width, self.config.height)?;

                // Recreate render surface with new dimensions
                self.surface = RenderSurface::new(new_width, self.config.height)
                    .context("Failed to create new render surface")?;
            }

            self.conn.flush()?;

            // Redraw with new geometry
            self.draw_bar().await?;

            info!("Bar geometry updated successfully");
        } else {
            debug!("Monitor change detected but bar geometry unchanged");
        }

        // Re-query workspace list (monitors may have different workspaces)
        self.modules.update("workspaces").await;
        self.draw_bar().await?;

        Ok(())
    }

    /// Hit test to find which block contains the given point
    fn hit_test(&self, x: f64, y: f64) -> Option<BlockOwner> {
        for owner in &self.block_owners {
            if x >= owner.x && x < owner.x + owner.width && y >= owner.y && y < owner.y + owner.height
            {
                return Some(owner.clone());
            }
        }
        None
    }

    /// Draw the bar using Cairo
    async fn draw_bar(&mut self) -> Result<()> {
        let width = self.bar.width as f64;
        let height = self.bar.height as f64;
        let block_count;

        // Get background from config
        let background = config_to_background(&self.config.background)?;

        // Get module order and outputs
        let (order_left, order_center, order_right) = self.modules.module_order();
        let order_left: Vec<String> = order_left.to_vec();
        let order_center: Vec<String> = order_center.to_vec();
        let order_right: Vec<String> = order_right.to_vec();

        let left_outputs = self.modules.left_outputs().await;
        let center_outputs = self.modules.center_outputs().await;
        let right_outputs = self.modules.right_outputs().await;

        // Track block ownership: (module_name, block_index_within_module)
        let mut block_ownership: Vec<(String, usize)> = Vec::new();

        // Scope the Cairo context so it's dropped before we access surface data
        {
            let cr = self.surface.context()?;

            // Explicitly clear surface to prevent any residual content
            cr.set_operator(cairo::Operator::Clear);
            cr.paint()?;
            cr.set_operator(cairo::Operator::Over);

            // Fill background from config
            background.apply(&cr, 0.0, 0.0, width, height);
            cr.rectangle(0.0, 0.0, width, height);
            cr.fill()?;

            // Build layout from module outputs, tracking ownership
            let mut layout = Layout::new();

            for (module_name, output) in order_left.iter().zip(left_outputs.iter()) {
                for (block_idx, block) in output.blocks.iter().enumerate() {
                    layout.left.push(block.clone());
                    block_ownership.push((module_name.clone(), block_idx));
                }
            }

            for (module_name, output) in order_center.iter().zip(center_outputs.iter()) {
                for (block_idx, block) in output.blocks.iter().enumerate() {
                    layout.center.push(block.clone());
                    block_ownership.push((module_name.clone(), block_idx));
                }
            }

            for (module_name, output) in order_right.iter().zip(right_outputs.iter()) {
                for (block_idx, block) in output.blocks.iter().enumerate() {
                    layout.right.push(block.clone());
                    block_ownership.push((module_name.clone(), block_idx));
                }
            }

            // Compute and render blocks using config padding
            let bar_padding = Padding::new(
                self.config.padding.left,
                self.config.padding.right,
                self.config.padding.top,
                self.config.padding.bottom,
            );
            let positioned = layout.compute(&cr, &self.text_renderer, width, height, &bar_padding);

            // Update block_owners for hit testing
            self.block_owners.clear();
            for (pos_block, (module_name, block_idx)) in positioned.iter().zip(block_ownership.iter()) {
                self.block_owners.push(BlockOwner {
                    module_name: module_name.clone(),
                    block_index: *block_idx,
                    x: pos_block.x,
                    y: pos_block.y,
                    width: pos_block.width,
                    height: pos_block.height,
                });

                // Position tray icons at the tray block location
                if module_name == "tray" {
                    if let Some(ref mut tray) = self.tray_manager {
                        // Center icons vertically in the bar
                        let icon_size = self.config.modules.tray.icon_size as i16;
                        let y_offset = ((height as i16) - icon_size) / 2;
                        tray.set_position(pos_block.x as i16, y_offset);
                    }
                }
            }

            for block in &positioned {
                block.render(&cr, &self.text_renderer);
            }

            block_count = positioned.len();
        } // cr is dropped here, releasing the surface borrow

        // Copy surface to window
        self.surface.copy_to_window(&self.conn, self.bar.window, self.bar.gc)?;

        debug!("Drew bar with {} blocks", block_count);
        Ok(())
    }

    /// Handle configuration reload
    async fn handle_reload(&mut self) -> Result<()> {
        info!("Reloading configuration...");

        // Reload config from disk
        match self.config_loader.load() {
            Ok(new_config) => {
                info!(
                    "Reload: loaded config with modules_left={:?}, modules_right={:?}",
                    new_config.modules_left, new_config.modules_right
                );

                // Check if height changed - requires window resize
                if new_config.height != self.config.height {
                    info!("Bar height changed: {} -> {}", self.config.height, new_config.height);
                    self.bar.resize(&self.conn, self.bar.width, new_config.height)?;
                    self.surface = RenderSurface::new(self.bar.width, new_config.height)
                        .context("Failed to create new render surface")?;
                }

                // Check if fonts changed - requires new text renderer
                if new_config.fonts != self.config.fonts {
                    info!("Fonts changed, recreating text renderer");
                    self.text_renderer = TextRenderer::new(&new_config.fonts);
                }

                // Recreate modules with new config
                info!("Reload: recreating module registry...");
                self.modules = ModuleRegistry::from_config(&new_config);
                info!("Reload: module registry recreated");

                self.config = new_config;
                info!("Configuration reloaded successfully");
            }
            Err(e) => {
                error!("Failed to reload configuration: {}", e);
                warn!("Keeping previous configuration");
            }
        }

        // Update modules and redraw
        self.modules.update_all().await;
        self.draw_bar().await?;
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
