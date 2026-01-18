-- garbar configuration (embedded in ~/.config/gar/init.lua)
-- This is the gar.bar table that configures the status bar
-- See the full gar init.lua example for complete configuration

gar.bar = {
    height = 32,
    position = "top",
    background = "#1a1a1a",
    foreground = "#ffffff",
    opacity = 1.0,
    fonts = {
        "JetBrainsMono Nerd Font:size=10",
        "Symbols Nerd Font:size=10",
    },
    padding = { left = 8, right = 16, top = 0, bottom = 0 },

    -- Module layout
    modules_left = { "workspaces", "window_title" },
    modules_center = {},
    modules_right = { "filesystem", "memory", "cpu", "battery", "wlan", "volume", "datetime" },

    -- Module configurations
    modules = {
        workspaces = {
            font_size = 11,
            focused = {
                foreground = "#ffffff",
                background = "transparent",
                underline = { width = 2, color = "#33ccff" },
            },
            unfocused = {
                foreground = "#666666",
                background = "transparent",
            },
            urgent = {
                foreground = "#ffffff",
                background = "#ff5555",
            },
        },
        window_title = {
            max_length = 50,
            empty_text = "Desktop",
        },
        datetime = {
            format = " %a %b %d   %H:%M",
        },
        cpu = {
            format = " {usage}%",
        },
        memory = {
            format = " {percent}%",
        },
        battery = {
            device = "auto",
            format_charging = " {percent}%",
            format_discharging = " {percent}%",
            format_full = " Full",
        },
        -- Script modules (custom commands)
        script = {
            filesystem = {
                exec = [[df -h / | awk 'NR==2 {print " " $4}']],
                interval = 60,
            },
            wlan = {
                exec = [[
IFACE="wlp1s0f0"
if [ -d "/sys/class/net/$IFACE" ]; then
  STATE=$(cat /sys/class/net/$IFACE/operstate 2>/dev/null)
  if [ "$STATE" = "up" ]; then
    ESSID=$(iwgetid -r 2>/dev/null || echo "")
    IP=$(ip -4 addr show $IFACE 2>/dev/null | grep -oP '(?<=inet\s)\d+(\.\d+){3}' | head -1)
    if [ -n "$ESSID" ]; then
      echo " $ESSID $IP"
    else
      echo " connected"
    fi
  else
    echo " offline"
  fi
else
  echo " N/A"
fi
]],
                interval = 5,
            },
            volume = {
                exec = [[
if command -v pamixer >/dev/null 2>&1; then
  if pamixer --get-mute | grep -q true; then
    echo " muted"
  else
    VOL=$(pamixer --get-volume)
    echo " $VOL%"
  fi
elif command -v pactl >/dev/null 2>&1; then
  VOL=$(pactl get-sink-volume @DEFAULT_SINK@ | grep -oP '\d+%' | head -1)
  MUTE=$(pactl get-sink-mute @DEFAULT_SINK@ | grep -oP 'yes|no')
  if [ "$MUTE" = "yes" ]; then
    echo " muted"
  else
    echo " $VOL"
  fi
else
  echo " N/A"
fi
]],
                interval = 1,
            },
        },
    },
}
