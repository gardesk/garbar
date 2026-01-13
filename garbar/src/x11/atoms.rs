use anyhow::Result;
use x11rb::protocol::xproto::{Atom, ConnectionExt};

/// X11 atoms used by garbar
#[derive(Debug, Clone)]
pub struct Atoms {
    // Window type hints
    pub net_wm_window_type: Atom,
    pub net_wm_window_type_dock: Atom,

    // Struts for reserving screen space
    pub net_wm_strut: Atom,
    pub net_wm_strut_partial: Atom,

    // Window state
    pub net_wm_state: Atom,
    pub net_wm_state_sticky: Atom,
    pub net_wm_state_above: Atom,

    // Window identification
    pub wm_name: Atom,
    pub net_wm_name: Atom,
    pub wm_class: Atom,

    // String types
    pub utf8_string: Atom,

    // System tray (for future use)
    pub net_system_tray_s0: Atom,
    pub net_system_tray_opcode: Atom,
    pub manager: Atom,
}

impl Atoms {
    /// Intern all required atoms
    pub fn new<C: x11rb::connection::Connection>(conn: &C) -> Result<Self> {
        // Batch intern requests for efficiency
        let net_wm_window_type = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE")?;
        let net_wm_window_type_dock = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE_DOCK")?;
        let net_wm_strut = conn.intern_atom(false, b"_NET_WM_STRUT")?;
        let net_wm_strut_partial = conn.intern_atom(false, b"_NET_WM_STRUT_PARTIAL")?;
        let net_wm_state = conn.intern_atom(false, b"_NET_WM_STATE")?;
        let net_wm_state_sticky = conn.intern_atom(false, b"_NET_WM_STATE_STICKY")?;
        let net_wm_state_above = conn.intern_atom(false, b"_NET_WM_STATE_ABOVE")?;
        let wm_name = conn.intern_atom(false, b"WM_NAME")?;
        let net_wm_name = conn.intern_atom(false, b"_NET_WM_NAME")?;
        let wm_class = conn.intern_atom(false, b"WM_CLASS")?;
        let utf8_string = conn.intern_atom(false, b"UTF8_STRING")?;
        let net_system_tray_s0 = conn.intern_atom(false, b"_NET_SYSTEM_TRAY_S0")?;
        let net_system_tray_opcode = conn.intern_atom(false, b"_NET_SYSTEM_TRAY_OPCODE")?;
        let manager = conn.intern_atom(false, b"MANAGER")?;

        // Wait for all replies
        Ok(Self {
            net_wm_window_type: net_wm_window_type.reply()?.atom,
            net_wm_window_type_dock: net_wm_window_type_dock.reply()?.atom,
            net_wm_strut: net_wm_strut.reply()?.atom,
            net_wm_strut_partial: net_wm_strut_partial.reply()?.atom,
            net_wm_state: net_wm_state.reply()?.atom,
            net_wm_state_sticky: net_wm_state_sticky.reply()?.atom,
            net_wm_state_above: net_wm_state_above.reply()?.atom,
            wm_name: wm_name.reply()?.atom,
            net_wm_name: net_wm_name.reply()?.atom,
            wm_class: wm_class.reply()?.atom,
            utf8_string: utf8_string.reply()?.atom,
            net_system_tray_s0: net_system_tray_s0.reply()?.atom,
            net_system_tray_opcode: net_system_tray_opcode.reply()?.atom,
            manager: manager.reply()?.atom,
        })
    }
}
