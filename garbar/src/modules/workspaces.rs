use std::env;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::Deserialize;

use crate::config::WorkspacesConfig;
use crate::render::{Block, BlockStyle, Color, Padding};

use super::{Module, ModuleOutput};

/// i3 IPC message types
const I3_IPC_MESSAGE_TYPE_GET_WORKSPACES: u32 = 1;
const I3_IPC_MESSAGE_TYPE_SUBSCRIBE: u32 = 2;

/// i3 IPC event mask (high bit set for events)
const I3_IPC_EVENT_MASK: u32 = 0x80000000;
const I3_IPC_EVENT_WORKSPACE: u32 = I3_IPC_EVENT_MASK | 0;

/// i3 IPC magic string
const I3_IPC_MAGIC: &[u8] = b"i3-ipc";

/// Workspace info from i3 IPC
#[derive(Debug, Clone, Deserialize)]
struct I3Workspace {
    num: i32,
    name: String,
    visible: bool,
    focused: bool,
    urgent: bool,
    #[allow(dead_code)]
    output: String,
}

/// Workspace state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceState {
    Focused,
    Visible,
    Unfocused,
    Urgent,
}

/// Single workspace info
#[derive(Debug, Clone)]
pub struct Workspace {
    pub num: i32,
    pub name: String,
    pub state: WorkspaceState,
}

pub struct WorkspacesModule {
    config: WorkspacesConfig,
    socket_path: Option<PathBuf>,
    workspaces: Arc<Mutex<Vec<Workspace>>>,
    subscribed: Arc<Mutex<bool>>,
}

impl WorkspacesModule {
    pub fn new(config: &WorkspacesConfig) -> Self {
        let socket_path = Self::find_i3_socket();
        match &socket_path {
            Some(path) => tracing::info!("Workspaces: using socket {:?}", path),
            None => tracing::warn!("Workspaces: no i3-compatible socket found!"),
        }

        Self {
            config: config.clone(),
            socket_path,
            workspaces: Arc::new(Mutex::new(Vec::new())),
            subscribed: Arc::new(Mutex::new(false)),
        }
    }

    /// Find the i3/sway/gar IPC socket path
    fn find_i3_socket() -> Option<PathBuf> {
        // Check I3SOCK environment variable first (set by gar or i3)
        if let Ok(path) = env::var("I3SOCK") {
            let path = PathBuf::from(path);
            if path.exists() {
                return Some(path);
            }
        }

        // Check SWAYSOCK for sway
        if let Ok(path) = env::var("SWAYSOCK") {
            let path = PathBuf::from(path);
            if path.exists() {
                return Some(path);
            }
        }

        // Try gar's socket path
        if let Ok(runtime_dir) = env::var("XDG_RUNTIME_DIR") {
            let gar_socket = PathBuf::from(&runtime_dir).join("gar-i3.sock");
            if gar_socket.exists() {
                return Some(gar_socket);
            }
        }

        // Fallback: /tmp/gar-i3.sock
        let tmp_socket = PathBuf::from("/tmp/gar-i3.sock");
        if tmp_socket.exists() {
            return Some(tmp_socket);
        }

        None
    }

    /// Start the subscription listener thread
    fn start_subscription(&self) {
        // Check if already subscribed
        {
            let mut subscribed = self.subscribed.lock().unwrap();
            if *subscribed {
                return;
            }
            *subscribed = true;
        }

        let socket_path = match &self.socket_path {
            Some(p) => p.clone(),
            None => return,
        };

        let workspaces = Arc::clone(&self.workspaces);
        let subscribed = Arc::clone(&self.subscribed);

        thread::spawn(move || {
            loop {
                // Connect to socket
                let mut stream = match UnixStream::connect(&socket_path) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::debug!("Workspaces subscription connect failed: {}", e);
                        thread::sleep(Duration::from_secs(1));
                        continue;
                    }
                };

                // Subscribe to workspace events
                let subscribe_payload = b"[\"workspace\"]";
                let mut message = Vec::new();
                message.extend_from_slice(I3_IPC_MAGIC);
                message.extend_from_slice(&(subscribe_payload.len() as u32).to_le_bytes());
                message.extend_from_slice(&I3_IPC_MESSAGE_TYPE_SUBSCRIBE.to_le_bytes());
                message.extend_from_slice(subscribe_payload);

                if stream.write_all(&message).is_err() {
                    thread::sleep(Duration::from_secs(1));
                    continue;
                }

                // Read subscribe response
                let mut header = [0u8; 14];
                if stream.read_exact(&mut header).is_err() {
                    thread::sleep(Duration::from_secs(1));
                    continue;
                }

                // Read and discard subscribe response payload
                let length = u32::from_le_bytes([header[6], header[7], header[8], header[9]]) as usize;
                let mut payload = vec![0u8; length];
                let _ = stream.read_exact(&mut payload);

                tracing::info!("Workspaces: subscribed to workspace events");

                // Do initial workspace fetch
                Self::fetch_workspaces_static(&socket_path, &workspaces);

                // Listen for events
                loop {
                    let mut header = [0u8; 14];
                    if stream.read_exact(&mut header).is_err() {
                        tracing::debug!("Workspaces: subscription connection lost, reconnecting...");
                        break; // Reconnect
                    }

                    // Verify magic
                    if &header[0..6] != I3_IPC_MAGIC {
                        break;
                    }

                    let length = u32::from_le_bytes([header[6], header[7], header[8], header[9]]) as usize;
                    let msg_type = u32::from_le_bytes([header[10], header[11], header[12], header[13]]);

                    // Read payload
                    let mut payload = vec![0u8; length];
                    if stream.read_exact(&mut payload).is_err() {
                        break;
                    }

                    // Check if this is a workspace event
                    if msg_type == I3_IPC_EVENT_WORKSPACE {
                        tracing::info!("Workspaces: received workspace event, fetching fresh data");
                        // Fetch fresh workspace list
                        Self::fetch_workspaces_static(&socket_path, &workspaces);
                        let ws = workspaces.lock().unwrap();
                        tracing::info!("Workspaces: updated list has {} workspaces, focused: {:?}",
                            ws.len(),
                            ws.iter().find(|w| w.state == WorkspaceState::Focused).map(|w| &w.name));
                    } else {
                        tracing::debug!("Workspaces: received message type 0x{:08x}", msg_type);
                    }
                }
            }

            // Mark as not subscribed if thread exits
            if let Ok(mut s) = subscribed.lock() {
                *s = false;
            }
        });
    }

    /// Fetch workspaces (static version for use in thread)
    fn fetch_workspaces_static(socket_path: &PathBuf, workspaces: &Arc<Mutex<Vec<Workspace>>>) {
        let mut stream = match UnixStream::connect(socket_path) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to connect for GET_WORKSPACES: {}", e);
                return;
            }
        };

        // Set timeout for fetching
        let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
        let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));

        // Send GET_WORKSPACES
        let mut message = Vec::new();
        message.extend_from_slice(I3_IPC_MAGIC);
        message.extend_from_slice(&0u32.to_le_bytes()); // No payload
        message.extend_from_slice(&I3_IPC_MESSAGE_TYPE_GET_WORKSPACES.to_le_bytes());

        if let Err(e) = stream.write_all(&message) {
            tracing::warn!("Failed to write GET_WORKSPACES: {}", e);
            return;
        }

        // Read response
        let mut header = [0u8; 14];
        if let Err(e) = stream.read_exact(&mut header) {
            tracing::warn!("Failed to read GET_WORKSPACES header: {}", e);
            return;
        }

        if &header[0..6] != I3_IPC_MAGIC {
            return;
        }

        let length = u32::from_le_bytes([header[6], header[7], header[8], header[9]]) as usize;
        let mut payload = vec![0u8; length];
        if stream.read_exact(&mut payload).is_err() {
            return;
        }

        // Parse workspaces
        if let Ok(json_str) = String::from_utf8(payload) {
            tracing::info!("GET_WORKSPACES raw response ({} bytes): {}", json_str.len(), &json_str[..json_str.len().min(200)]);
            match serde_json::from_str::<Vec<I3Workspace>>(&json_str) {
                Ok(i3_workspaces) => {
                    let focused_in_response: Vec<_> = i3_workspaces.iter().filter(|w| w.focused).map(|w| &w.name).collect();
                    tracing::info!("Parsed workspaces from JSON, focused: {:?}", focused_in_response);
                let mut ws_list: Vec<Workspace> = i3_workspaces
                    .into_iter()
                    .map(|ws| {
                        let state = if ws.urgent {
                            WorkspaceState::Urgent
                        } else if ws.focused {
                            WorkspaceState::Focused
                        } else if ws.visible {
                            WorkspaceState::Visible
                        } else {
                            WorkspaceState::Unfocused
                        };
                        Workspace {
                            num: ws.num,
                            name: ws.name,
                            state,
                        }
                    })
                    .collect();

                ws_list.sort_by_key(|w| w.num);

                if let Ok(mut ws) = workspaces.lock() {
                    *ws = ws_list;
                }
                }
                Err(e) => {
                    tracing::warn!("Failed to parse workspaces JSON: {}", e);
                }
            }
        }
    }
}

impl Module for WorkspacesModule {
    fn name(&self) -> &'static str {
        "workspaces"
    }

    fn output(&self) -> ModuleOutput {
        let workspaces = self.workspaces.lock().unwrap();

        if workspaces.is_empty() {
            return ModuleOutput::empty();
        }

        // Debug: log workspace states during render
        let focused_names: Vec<_> = workspaces.iter()
            .filter(|w| w.state == WorkspaceState::Focused)
            .map(|w| w.name.as_str())
            .collect();
        tracing::trace!("Render: workspaces with Focused state: {:?}", focused_names);

        let blocks: Vec<Block> = workspaces
            .iter()
            .map(|ws| {
                let style_cfg = match ws.state {
                    WorkspaceState::Focused => &self.config.focused,
                    WorkspaceState::Visible => &self.config.unfocused,
                    WorkspaceState::Unfocused => &self.config.unfocused,
                    WorkspaceState::Urgent => &self.config.urgent,
                };

                let foreground =
                    Color::from_hex(&style_cfg.foreground).unwrap_or(Color::white());
                let background = if style_cfg.background == "transparent" {
                    None
                } else {
                    Color::from_hex(&style_cfg.background).ok()
                };

                let mut style = BlockStyle::new()
                    .with_foreground(foreground)
                    .with_padding(Padding::new(8.0, 8.0, 4.0, 4.0));

                if let Some(bg) = background {
                    style = style.with_background(bg);
                }

                if let Some(ref underline) = style_cfg.underline {
                    if let Ok(color) = Color::from_hex(&underline.color) {
                        style = style.with_underline(underline.width, color);
                    }
                }

                if let Some(size) = self.config.font_size {
                    style = style.with_font_size(size);
                }

                Block::new(&ws.name).with_style(style)
            })
            .collect();

        ModuleOutput::new(blocks)
    }

    fn interval(&self) -> u64 {
        1000 // Subscriptions handle instant updates, polling is just backup
    }

    fn update(&mut self) -> bool {
        // Retry socket discovery if not found at startup (race with gar)
        if self.socket_path.is_none() {
            self.socket_path = Self::find_i3_socket();
            if let Some(ref path) = self.socket_path {
                tracing::info!("Workspaces: found socket on retry {:?}", path);
            }
        }
        // Start subscription thread on first update
        self.start_subscription();
        // Workspace changes arrive via subscription, not polling
        false
    }

    fn on_click(&mut self, button: u8, block_index: usize, _x: i16, _y: i16) {
        if button == 1 {
            // Get workspace name for this block
            let workspace_name = {
                let workspaces = self.workspaces.lock().unwrap();
                workspaces.get(block_index).map(|ws| ws.name.clone())
            };

            if let Some(name) = workspace_name {
                if let Some(ref socket_path) = self.socket_path {
                    self.switch_workspace(socket_path, &name);
                }
            }
        }
    }
}

impl WorkspacesModule {
    /// Switch to a workspace via i3 IPC
    fn switch_workspace(&self, socket_path: &PathBuf, workspace_name: &str) {
        let mut stream = match UnixStream::connect(socket_path) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to connect for workspace switch: {}", e);
                return;
            }
        };

        // Set short timeout
        let _ = stream.set_write_timeout(Some(Duration::from_millis(100)));
        let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));

        // i3 IPC RUN_COMMAND = 0
        const I3_IPC_MESSAGE_TYPE_RUN_COMMAND: u32 = 0;

        // Build command: workspace <name>
        let command = format!("workspace {}", workspace_name);
        let payload = command.as_bytes();

        let mut message = Vec::new();
        message.extend_from_slice(I3_IPC_MAGIC);
        message.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        message.extend_from_slice(&I3_IPC_MESSAGE_TYPE_RUN_COMMAND.to_le_bytes());
        message.extend_from_slice(payload);

        if let Err(e) = stream.write_all(&message) {
            tracing::warn!("Failed to send workspace switch command: {}", e);
            return;
        }

        tracing::debug!("Sent workspace switch command: {}", command);

        // Read response (optional, just to complete the transaction)
        let mut header = [0u8; 14];
        let _ = stream.read_exact(&mut header);
    }
}
