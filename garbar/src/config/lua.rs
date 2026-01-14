use anyhow::{anyhow, Context, Result};
use mlua::{Lua, Table, Value};
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info, warn};

use super::types::*;

/// Convert mlua Error to anyhow Error
fn lua_err(e: mlua::Error) -> anyhow::Error {
    anyhow!("Lua error: {}", e)
}

/// Setup stub functions for gar's Lua API
/// These no-ops allow garbar to execute gar's init.lua without errors
fn setup_gar_stubs(lua: &Lua, gar: &Table) -> Result<()> {
    // gar.set(key, value) - configuration setter
    let set_fn = lua.create_function(|_, (_key, _value): (String, Value)| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("set", set_fn).map_err(lua_err)?;

    // gar.bind(keyspec, callback) - keybinding
    let bind_fn = lua.create_function(|_, (_keyspec, _callback): (String, Value)| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("bind", bind_fn).map_err(lua_err)?;

    // gar.exec(cmd) - execute command
    let exec_fn = lua.create_function(|_, _cmd: String| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("exec", exec_fn).map_err(lua_err)?;

    // gar.exec_once(cmd) - execute command once at startup
    let exec_once_fn = lua.create_function(|_, _cmd: String| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("exec_once", exec_once_fn).map_err(lua_err)?;

    // gar.rule(match, actions) - window rules
    let rule_fn = lua.create_function(|_, (_match_table, _actions_table): (Table, Table)| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("rule", rule_fn).map_err(lua_err)?;

    // gar.picom_rule(config) - picom window rules
    let picom_rule_fn = lua.create_function(|_, _config: Table| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("picom_rule", picom_rule_fn).map_err(lua_err)?;

    // gar.focus(direction) - focus window
    let focus_fn = lua.create_function(|_, _direction: String| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("focus", focus_fn).map_err(lua_err)?;

    // gar.swap(direction) - swap window
    let swap_fn = lua.create_function(|_, _direction: String| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("swap", swap_fn).map_err(lua_err)?;

    // gar.resize(direction, amount) - resize window
    let resize_fn = lua.create_function(|_, (_direction, _amount): (String, f32)| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("resize", resize_fn).map_err(lua_err)?;

    // gar.workspace(n) - switch to workspace
    let workspace_fn = lua.create_function(|_, _n: i64| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("workspace", workspace_fn).map_err(lua_err)?;

    // gar.workspace_next() - next workspace
    let workspace_next_fn = lua.create_function(|_, ()| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("workspace_next", workspace_next_fn).map_err(lua_err)?;

    // gar.workspace_prev() - previous workspace
    let workspace_prev_fn = lua.create_function(|_, ()| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("workspace_prev", workspace_prev_fn).map_err(lua_err)?;

    // gar.move(n) - move window to workspace
    let move_fn = lua.create_function(|_, _n: i64| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("move", move_fn).map_err(lua_err)?;

    // gar.move_to_workspace(n) - move window to workspace (alias)
    let move_to_workspace_fn = lua.create_function(|_, _n: i64| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("move_to_workspace", move_to_workspace_fn).map_err(lua_err)?;

    // gar.focus_monitor(target) - focus monitor
    let focus_monitor_fn = lua.create_function(|_, _target: String| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("focus_monitor", focus_monitor_fn).map_err(lua_err)?;

    // gar.move_to_monitor(target) - move window to monitor
    let move_to_monitor_fn = lua.create_function(|_, _target: String| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("move_to_monitor", move_to_monitor_fn).map_err(lua_err)?;

    // gar.close() - close window
    let close_fn = lua.create_function(|_, ()| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("close", close_fn).map_err(lua_err)?;

    // gar.toggle_floating() - toggle floating
    let toggle_floating_fn = lua.create_function(|_, ()| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("toggle_floating", toggle_floating_fn).map_err(lua_err)?;

    // gar.equalize() - equalize splits
    let equalize_fn = lua.create_function(|_, ()| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("equalize", equalize_fn).map_err(lua_err)?;

    // gar.reload() - reload config
    let reload_fn = lua.create_function(|_, ()| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("reload", reload_fn).map_err(lua_err)?;

    // gar.exit() - exit gar
    let exit_fn = lua.create_function(|_, ()| {
        Ok(())
    }).map_err(lua_err)?;
    gar.set("exit", exit_fn).map_err(lua_err)?;

    Ok(())
}

/// Load configuration from gar's init.lua file
pub fn load_from_lua<P: AsRef<Path>>(path: P) -> Result<Option<BarConfig>> {
    let path = path.as_ref();

    if !path.exists() {
        debug!("Lua config file not found: {}", path.display());
        return Ok(None);
    }

    info!("Loading config from {}", path.display());

    let lua = Lua::new();
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    // Create the gar global table with stub functions for gar's API
    // This allows garbar to parse gar's init.lua without errors
    let gar = lua.create_table().map_err(lua_err)?;
    setup_gar_stubs(&lua, &gar)?;
    lua.globals().set("gar", gar).map_err(lua_err)?;

    // Execute the config file
    lua.load(&content)
        .set_name(path.to_string_lossy())
        .exec()
        .map_err(|e| anyhow!("Failed to execute {}: {}", path.display(), e))?;

    // Try to get gar.bar
    let gar: Table = lua.globals().get("gar").map_err(lua_err)?;
    let bar_value: Value = gar.get("bar").map_err(lua_err)?;

    match bar_value {
        Value::Table(bar_table) => {
            let config = parse_bar_config(&bar_table)?;
            Ok(Some(config))
        }
        Value::Nil => {
            debug!("No gar.bar table found in config");
            Ok(None)
        }
        _ => {
            warn!("gar.bar is not a table");
            Ok(None)
        }
    }
}

/// Parse the gar.bar table into BarConfig
fn parse_bar_config(table: &Table) -> Result<BarConfig> {
    let mut config = BarConfig::default();

    // Basic properties
    if let Ok(height) = table.get::<u16>("height") {
        config.height = height;
    }

    if let Ok(pos) = table.get::<String>("position") {
        config.position = match pos.to_lowercase().as_str() {
            "bottom" => Position::Bottom,
            _ => Position::Top,
        };
    }

    // Margin
    if let Ok(margin) = table.get::<Table>("margin") {
        config.margin = parse_margin(&margin);
    }

    // Padding
    if let Ok(padding) = table.get::<Table>("padding") {
        config.padding = parse_padding(&padding);
    }

    // Background
    if let Ok(bg) = table.get::<Value>("background") {
        config.background = parse_background(bg);
    }

    // Foreground
    if let Ok(fg) = table.get::<String>("foreground") {
        config.foreground = fg;
    }

    // Fonts
    if let Ok(fonts) = table.get::<Table>("fonts") {
        config.fonts = parse_string_array(&fonts);
    }

    // Border
    if let Ok(border) = table.get::<Table>("border") {
        config.border = parse_border(&border);
    }

    // Shadow
    if let Ok(shadow) = table.get::<Table>("shadow") {
        config.shadow = Some(parse_shadow(&shadow));
    }

    // Module layout
    if let Ok(left) = table.get::<Table>("modules_left") {
        config.modules_left = parse_string_array(&left);
    }
    if let Ok(center) = table.get::<Table>("modules_center") {
        config.modules_center = parse_string_array(&center);
    }
    if let Ok(right) = table.get::<Table>("modules_right") {
        config.modules_right = parse_string_array(&right);
    }

    // Animations
    if let Ok(anim) = table.get::<Table>("animations") {
        config.animations = parse_animations(&anim);
    }

    // Separator
    if let Ok(sep) = table.get::<Table>("separator") {
        config.separator = parse_separator(&sep);
    }

    // Modules config
    if let Ok(modules) = table.get::<Table>("modules") {
        config.modules = parse_modules_config(&modules);
    }

    debug!("Parsed bar config: height={}, position={:?}", config.height, config.position);
    Ok(config)
}

fn parse_margin(table: &Table) -> Margin {
    Margin {
        top: table.get("top").unwrap_or(0.0),
        right: table.get("right").unwrap_or(0.0),
        bottom: table.get("bottom").unwrap_or(0.0),
        left: table.get("left").unwrap_or(0.0),
    }
}

fn parse_padding(table: &Table) -> Padding {
    Padding {
        top: table.get("top").unwrap_or(0.0),
        right: table.get("right").unwrap_or(0.0),
        bottom: table.get("bottom").unwrap_or(0.0),
        left: table.get("left").unwrap_or(0.0),
    }
}

fn parse_background(value: Value) -> BackgroundConfig {
    match value {
        Value::String(s) => {
            if let Ok(s) = s.to_str() {
                BackgroundConfig::Solid(s.to_string())
            } else {
                BackgroundConfig::default()
            }
        }
        Value::Table(table) => {
            let gradient_type = table.get::<String>("type")
                .map(|t| match t.to_lowercase().as_str() {
                    "radial" => GradientType::Radial,
                    _ => GradientType::Gradient,
                })
                .unwrap_or(GradientType::Gradient);

            let direction = table.get::<String>("direction")
                .map(|d| match d.to_lowercase().as_str() {
                    "vertical" => GradientDirection::Vertical,
                    "diagonal" => GradientDirection::Diagonal,
                    _ => GradientDirection::Horizontal,
                })
                .unwrap_or(GradientDirection::Horizontal);

            let stops = if let Ok(stops_table) = table.get::<Table>("stops") {
                parse_gradient_stops(&stops_table)
            } else {
                vec![]
            };

            let center = table.get::<Table>("center").ok().and_then(|t| {
                let x: f64 = t.get("x").unwrap_or(0.5);
                let y: f64 = t.get("y").unwrap_or(0.5);
                Some((x, y))
            });

            let radius = table.get::<f64>("radius").ok();

            BackgroundConfig::Gradient(GradientConfig {
                gradient_type,
                direction,
                stops,
                center,
                radius,
            })
        }
        _ => BackgroundConfig::default(),
    }
}

fn parse_gradient_stops(table: &Table) -> Vec<GradientStopConfig> {
    let mut stops = Vec::new();

    if let Ok(pairs) = table.pairs::<i64, Table>().collect::<Result<Vec<_>, _>>() {
        for (_, stop_table) in pairs {
            let position: f64 = stop_table.get("position").unwrap_or(0.0);
            let color: String = stop_table.get("color").unwrap_or_default();
            stops.push(GradientStopConfig { position, color });
        }
    }

    // Sort by position
    stops.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap_or(std::cmp::Ordering::Equal));

    stops
}

fn parse_border(table: &Table) -> BorderConfig {
    BorderConfig {
        width: table.get("width").unwrap_or(0.0),
        color: table.get("color").unwrap_or_default(),
        radius: table.get("radius").unwrap_or(0.0),
    }
}

fn parse_shadow(table: &Table) -> ShadowConfig {
    let offset = if let Ok(off) = table.get::<Table>("offset") {
        ShadowOffset {
            x: off.get("x").unwrap_or(0.0),
            y: off.get("y").unwrap_or(2.0),
        }
    } else {
        ShadowOffset::default()
    };

    ShadowConfig {
        enabled: table.get("enabled").unwrap_or(true),
        color: table.get("color").unwrap_or_else(|_| "#00000080".to_string()),
        blur: table.get("blur").unwrap_or(8.0),
        offset,
    }
}

fn parse_animations(table: &Table) -> AnimationConfig {
    AnimationConfig {
        enabled: table.get("enabled").unwrap_or(true),
        duration: table.get("duration").unwrap_or(150),
        easing: table.get("easing").unwrap_or_else(|_| "ease-out-cubic".to_string()),
    }
}

fn parse_separator(table: &Table) -> SeparatorConfig {
    let padding = if let Ok(p) = table.get::<Table>("padding") {
        parse_padding(&p)
    } else {
        Padding { left: 8.0, right: 8.0, top: 0.0, bottom: 0.0 }
    };

    SeparatorConfig {
        text: table.get("text").unwrap_or_else(|_| "│".to_string()),
        foreground: table.get("foreground").unwrap_or_else(|_| "#555555".to_string()),
        padding,
    }
}

fn parse_string_array(table: &Table) -> Vec<String> {
    let mut result = Vec::new();
    if let Ok(pairs) = table.pairs::<i64, String>().collect::<Result<Vec<_>, _>>() {
        for (_, value) in pairs {
            result.push(value);
        }
    }
    result
}

fn parse_modules_config(table: &Table) -> ModulesConfig {
    let mut config = ModulesConfig::default();

    if let Ok(ws) = table.get::<Table>("workspaces") {
        config.workspaces = parse_workspaces_config(&ws);
    }

    if let Ok(wt) = table.get::<Table>("window_title") {
        config.window_title = parse_window_title_config(&wt);
    }

    if let Ok(cpu) = table.get::<Table>("cpu") {
        config.cpu = parse_cpu_config(&cpu);
    }

    if let Ok(mem) = table.get::<Table>("memory") {
        config.memory = parse_memory_config(&mem);
    }

    if let Ok(bat) = table.get::<Table>("battery") {
        config.battery = parse_battery_config(&bat);
    }

    if let Ok(net) = table.get::<Table>("network") {
        config.network = parse_network_config(&net);
    }

    if let Ok(pa) = table.get::<Table>("pulseaudio") {
        config.pulseaudio = parse_pulseaudio_config(&pa);
    }

    if let Ok(dt) = table.get::<Table>("datetime") {
        config.datetime = parse_datetime_config(&dt);
    }

    if let Ok(fs) = table.get::<Table>("filesystem") {
        config.filesystem = parse_filesystem_config(&fs);
    }

    if let Ok(tray) = table.get::<Table>("tray") {
        config.tray = parse_tray_config(&tray);
    }

    if let Ok(script) = table.get::<Table>("script") {
        config.script = parse_script_configs(&script);
    }

    config
}

fn parse_workspaces_config(table: &Table) -> WorkspacesConfig {
    let mut config = WorkspacesConfig::default();

    if let Ok(v) = table.get::<bool>("show_empty") { config.show_empty = v; }
    if let Ok(v) = table.get::<bool>("show_urgent") { config.show_urgent = v; }
    if let Ok(v) = table.get::<bool>("pin_workspaces") { config.pin_workspaces = v; }

    if let Ok(focused) = table.get::<Table>("focused") {
        config.focused = parse_workspace_style(&focused);
    }
    if let Ok(unfocused) = table.get::<Table>("unfocused") {
        config.unfocused = parse_workspace_style(&unfocused);
    }
    if let Ok(urgent) = table.get::<Table>("urgent") {
        config.urgent = parse_workspace_style(&urgent);
    }

    config
}

fn parse_workspace_style(table: &Table) -> WorkspaceStateStyle {
    let underline = if let Ok(ul) = table.get::<Table>("underline") {
        Some(UnderlineConfig {
            width: ul.get("width").unwrap_or(2.0),
            color: ul.get("color").unwrap_or_default(),
        })
    } else {
        None
    };

    WorkspaceStateStyle {
        background: table.get("background").unwrap_or_else(|_| "transparent".to_string()),
        foreground: table.get("foreground").unwrap_or_else(|_| "#ffffff".to_string()),
        underline,
    }
}

fn parse_window_title_config(table: &Table) -> WindowTitleConfig {
    WindowTitleConfig {
        max_length: table.get("max_length").unwrap_or(50),
        ellipsis: table.get("ellipsis").unwrap_or_else(|_| "…".to_string()),
        empty_text: table.get("empty_text").unwrap_or_else(|_| "Desktop".to_string()),
        show_icon: table.get("show_icon").unwrap_or(true),
        icon_spacing: table.get("icon_spacing").unwrap_or(8.0),
    }
}

fn parse_cpu_config(table: &Table) -> CpuConfig {
    let mut config = CpuConfig::default();

    if let Ok(v) = table.get::<String>("format") { config.format = v; }
    if let Ok(v) = table.get::<u32>("interval") { config.interval = v; }
    if let Ok(v) = table.get::<u32>("warning_threshold") { config.warning_threshold = v; }
    if let Ok(v) = table.get::<u32>("critical_threshold") { config.critical_threshold = v; }
    if let Ok(v) = table.get::<String>("warning_foreground") { config.warning_foreground = v; }
    if let Ok(v) = table.get::<String>("critical_foreground") { config.critical_foreground = v; }

    config
}

fn parse_memory_config(table: &Table) -> MemoryConfig {
    let mut config = MemoryConfig::default();

    if let Ok(v) = table.get::<String>("format") { config.format = v; }
    if let Ok(v) = table.get::<String>("format_alt") { config.format_alt = v; }
    if let Ok(v) = table.get::<u32>("interval") { config.interval = v; }
    if let Ok(v) = table.get::<u32>("warning_threshold") { config.warning_threshold = v; }
    if let Ok(v) = table.get::<u32>("critical_threshold") { config.critical_threshold = v; }

    config
}

fn parse_battery_config(table: &Table) -> BatteryConfig {
    let mut config = BatteryConfig::default();

    if let Ok(v) = table.get::<String>("device") { config.device = v; }
    if let Ok(v) = table.get::<String>("format_charging") { config.format_charging = v; }
    if let Ok(v) = table.get::<String>("format_discharging") { config.format_discharging = v; }
    if let Ok(v) = table.get::<String>("format_full") { config.format_full = v; }
    if let Ok(v) = table.get::<u32>("low_threshold") { config.low_threshold = v; }
    if let Ok(v) = table.get::<String>("low_animation") { config.low_animation = v; }
    if let Ok(v) = table.get::<String>("low_foreground") { config.low_foreground = v; }

    if let Ok(icons) = table.get::<Table>("icons") {
        config.icons = parse_battery_icons(&icons);
    }

    config
}

fn parse_battery_icons(table: &Table) -> Vec<BatteryIcon> {
    let mut icons = Vec::new();
    if let Ok(pairs) = table.pairs::<i64, Table>().collect::<Result<Vec<_>, _>>() {
        for (_, icon_table) in pairs {
            icons.push(BatteryIcon {
                threshold: icon_table.get("threshold").unwrap_or(100),
                icon: icon_table.get("icon").unwrap_or_default(),
            });
        }
    }
    icons.sort_by_key(|i| i.threshold);
    icons
}

fn parse_network_config(table: &Table) -> NetworkConfig {
    let mut config = NetworkConfig::default();

    if let Ok(v) = table.get::<String>("interface") { config.interface = v; }
    if let Ok(v) = table.get::<String>("format_connected") { config.format_connected = v; }
    if let Ok(v) = table.get::<String>("format_disconnected") { config.format_disconnected = v; }
    if let Ok(v) = table.get::<String>("format_ethernet") { config.format_ethernet = v; }
    if let Ok(v) = table.get::<bool>("show_speed") { config.show_speed = v; }
    if let Ok(v) = table.get::<String>("speed_format") { config.speed_format = v; }

    config
}

fn parse_pulseaudio_config(table: &Table) -> PulseaudioConfig {
    let mut config = PulseaudioConfig::default();

    if let Ok(v) = table.get::<String>("format") { config.format = v; }
    if let Ok(v) = table.get::<String>("format_muted") { config.format_muted = v; }
    if let Ok(v) = table.get::<u32>("scroll_step") { config.scroll_step = v; }
    if let Ok(v) = table.get::<String>("on_click") { config.on_click = v; }
    if let Ok(v) = table.get::<String>("on_click_middle") { config.on_click_middle = v; }
    if let Ok(v) = table.get::<String>("on_click_right") { config.on_click_right = v; }

    if let Ok(icons) = table.get::<Table>("icons") {
        config.icons = parse_volume_icons(&icons);
    }

    config
}

fn parse_volume_icons(table: &Table) -> Vec<VolumeIcon> {
    let mut icons = Vec::new();
    if let Ok(pairs) = table.pairs::<i64, Table>().collect::<Result<Vec<_>, _>>() {
        for (_, icon_table) in pairs {
            icons.push(VolumeIcon {
                threshold: icon_table.get("threshold").unwrap_or(100),
                icon: icon_table.get("icon").unwrap_or_default(),
            });
        }
    }
    icons.sort_by_key(|i| i.threshold);
    icons
}

fn parse_datetime_config(table: &Table) -> DatetimeConfig {
    DatetimeConfig {
        format: table.get("format").unwrap_or_else(|_| " %a %b %d   %H:%M".to_string()),
        format_alt: table.get("format_alt").unwrap_or_else(|_| " %Y-%m-%d   %H:%M:%S".to_string()),
        interval: table.get("interval").unwrap_or(1),
        tooltip: table.get("tooltip").unwrap_or(true),
    }
}

fn parse_filesystem_config(table: &Table) -> FilesystemConfig {
    FilesystemConfig {
        mountpoint: table.get("mountpoint").unwrap_or_else(|_| "/".to_string()),
        format: table.get("format").unwrap_or_else(|_| " {percent_used}%".to_string()),
        warning_threshold: table.get("warning_threshold").unwrap_or(80),
        critical_threshold: table.get("critical_threshold").unwrap_or(95),
        interval: table.get("interval").unwrap_or(30),
    }
}

fn parse_tray_config(table: &Table) -> TrayConfig {
    let padding = if let Ok(p) = table.get::<Table>("padding") {
        parse_padding(&p)
    } else {
        Padding { left: 4.0, right: 4.0, top: 0.0, bottom: 0.0 }
    };

    TrayConfig {
        icon_size: table.get("icon_size").unwrap_or(18),
        spacing: table.get("spacing").unwrap_or(8.0),
        padding,
    }
}

fn parse_script_configs(table: &Table) -> HashMap<String, ScriptConfig> {
    let mut scripts = HashMap::new();

    if let Ok(pairs) = table.pairs::<String, Table>().collect::<Result<Vec<_>, _>>() {
        for (name, script_table) in pairs {
            scripts.insert(name, ScriptConfig {
                exec: script_table.get("exec").unwrap_or_default(),
                interval: script_table.get("interval").unwrap_or(30),
                tail: script_table.get("tail").unwrap_or(false),
                format: script_table.get("format").unwrap_or_default(),
                click_left: script_table.get("click_left").unwrap_or_default(),
                click_middle: script_table.get("click_middle").unwrap_or_default(),
                click_right: script_table.get("click_right").unwrap_or_default(),
                scroll_up: script_table.get("scroll_up").unwrap_or_default(),
                scroll_down: script_table.get("scroll_down").unwrap_or_default(),
                font_size: script_table.get("font_size").ok(),
            });
        }
    }

    scripts
}
