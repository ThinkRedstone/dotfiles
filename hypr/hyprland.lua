---------------------
---- MY PROGRAMS ----
---------------------

local terminal    = "kitty"
local fileManager = "thunar"
local menu        = "~/.config/hypr/launcher.sh"


-------------------------------
---- ENVIRONMENT VARIABLES ----
-------------------------------

hl.env("XCURSOR_SIZE", "24")
hl.env("HYPRCURSOR_SIZE", "24")
hl.env("QT_QPA_PLATFORMTHEME", "qt5ct")


-----------------------
---- LOOK AND FEEL ----
-----------------------

-- Border colors come from the active theme (symlinked at ~/.config/theme).
-- switch_theme.sh swaps the symlink and runs `hyprctl reload`, which re-runs this
-- file and re-reads the new theme's colors.
local theme = dofile(os.getenv("HOME") .. "/.config/theme/colors.lua")

hl.config({
    general = {
        gaps_in  = 10,
        gaps_out = 10,

        border_size = 2,

        col = {
            active_border   = theme.active_border,
            inactive_border = theme.inactive_border,
        },

        resize_on_border = false,
        allow_tearing    = false,

        layout = "master",
    },

    decoration = {
        rounding = 10,

        active_opacity   = 1.0,
        inactive_opacity = 1.0,

        shadow = {
            enabled      = true,
            range        = 4,
            render_power = 3,
            color        = 0xee1a1a1a,
        },

        blur = {
            enabled  = true,
            size     = 3,
            passes   = 1,
            vibrancy = 0.1696,
        },
    },

    animations = {
        enabled = true,
    },

    dwindle = {
        preserve_split = true,
    },

    master = {
        new_status = "slave",
    },

    cursor = {
        no_warps = true,
    },

    misc = {
        force_default_wallpaper = -1,
        disable_hyprland_logo   = false,
    },
})

-- Animations
hl.curve("bezier", { type = "bezier", points = { { 0.05, 0.9 }, { 0.1, 1.05 } } })

hl.animation({ leaf = "windows",     enabled = true, speed = 7,  bezier = "bezier" })
hl.animation({ leaf = "windowsOut",  enabled = true, speed = 7,  bezier = "default", style = "popin 80%" })
hl.animation({ leaf = "border",      enabled = true, speed = 10, bezier = "default" })
hl.animation({ leaf = "borderangle", enabled = true, speed = 8,  bezier = "default" })
hl.animation({ leaf = "fade",        enabled = true, speed = 7,  bezier = "default" })
hl.animation({ leaf = "workspaces",  enabled = true, speed = 6,  bezier = "default" })


---------------
---- INPUT ----
---------------

hl.config({
    input = {
        kb_layout  = "us,il",
        kb_variant = "",
        kb_model   = "",
        kb_options = "grp:alt_caps_toggle",
        kb_rules   = "",

        follow_mouse = 2,

        sensitivity = 0,

        touchpad = {
            natural_scroll = false,
        },
    },
})


-------------------
---- AUTOSTART ----
-------------------

hl.on("hyprland.start", function()
    hl.exec_cmd("hyprpaper -c ~/.config/hypr/hyprpaper.conf")
    hl.exec_cmd("dunst")
    --hl.exec_cmd("waybar")
end)


---------------------
---- KEYBINDINGS ----
---------------------

local mainMod = "ALT"

-- Main binds
hl.bind("CTRL + " .. mainMod .. " + T",  hl.dsp.exec_cmd(terminal))
hl.bind(mainMod .. " + Q",               hl.dsp.window.close())
hl.bind("SUPER + " .. mainMod .. " + Q", hl.dsp.exit())
hl.bind("CTRL + " .. mainMod .. " + F",  hl.dsp.exec_cmd(fileManager))
hl.bind(mainMod .. " + T",               hl.dsp.window.float({ action = "toggle" }))
hl.bind(mainMod .. " + R",               hl.dsp.exec_cmd(menu))
hl.bind(mainMod .. " + P",               hl.dsp.window.pseudo())
hl.bind(mainMod .. " + space",           hl.dsp.window.fullscreen({ mode = "maximized" }))
hl.bind(mainMod .. " + tab",             hl.dsp.window.cycle_next())

-- Move focus with mainMod + vim keys
hl.bind(mainMod .. " + H", hl.dsp.focus({ direction = "left" }))
hl.bind(mainMod .. " + L", hl.dsp.focus({ direction = "right" }))
hl.bind(mainMod .. " + K", hl.dsp.focus({ direction = "up" }))
hl.bind(mainMod .. " + J", hl.dsp.focus({ direction = "down" }))

-- Move windows with mainMod + SHIFT + vim keys
hl.bind(mainMod .. " + SHIFT + H", hl.dsp.window.move({ direction = "left" }))
hl.bind(mainMod .. " + SHIFT + L", hl.dsp.window.move({ direction = "right" }))
hl.bind(mainMod .. " + SHIFT + K", hl.dsp.window.move({ direction = "up" }))
hl.bind(mainMod .. " + SHIFT + J", hl.dsp.window.move({ direction = "down" }))

-- Switch / move-to workspace with mainMod (+ SHIFT) + letter
local workspaceKeys = { "a", "s", "d", "f", "z", "x", "c", "v" }
for i, key in ipairs(workspaceKeys) do
    hl.bind(mainMod .. " + " .. key,           hl.dsp.focus({ workspace = tostring(i) }))
    hl.bind(mainMod .. " + SHIFT + " .. key,   hl.dsp.window.move({ workspace = tostring(i) }))
end

-- Scroll through existing workspaces with mainMod + scroll
hl.bind(mainMod .. " + mouse_down", hl.dsp.focus({ workspace = "e+1" }))
hl.bind(mainMod .. " + mouse_up",   hl.dsp.focus({ workspace = "e-1" }))

-- Move/resize windows with mainMod + LMB/RMB and dragging
hl.bind(mainMod .. " + mouse:272", hl.dsp.window.drag(),   { mouse = true })
hl.bind(mainMod .. " + mouse:273", hl.dsp.window.resize(), { mouse = true })

-- Function keys (locked = active even on lockscreen, repeating = key repeat)
hl.bind("XF86AudioRaiseVolume",  hl.dsp.exec_cmd("wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%+"), { locked = true, repeating = true })
hl.bind("XF86AudioLowerVolume",  hl.dsp.exec_cmd("wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-"), { locked = true, repeating = true })
hl.bind("XF86AudioMute",         hl.dsp.exec_cmd("wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle"), { locked = true })
hl.bind("XF86AudioPlay",         hl.dsp.exec_cmd("playerctl play-pause"), { locked = true })
hl.bind("XF86AudioPrev",         hl.dsp.exec_cmd("playerctl previous"),   { locked = true })
hl.bind("XF86AudioNext",         hl.dsp.exec_cmd("playerctl next"),       { locked = true })
hl.bind("XF86MonBrightnessDown", hl.dsp.exec_cmd("brightnessctl s 5%-"),  { locked = true, repeating = true })
hl.bind("XF86MonBrightnessUp",   hl.dsp.exec_cmd("brightnessctl s 5%+"),  { locked = true, repeating = true })


--------------------------------
---- WINDOWS AND WORKSPACES ----
--------------------------------

-- "No gaps when only" for single-tiled / single-fullscreen workspaces.
hl.workspace_rule({ workspace = "w[tv1]", gaps_out = 0, gaps_in = 0 })
hl.workspace_rule({ workspace = "f[1]",   gaps_out = 0, gaps_in = 0 })
hl.window_rule({ name = "no-gaps-wtv1", match = { float = false, workspace = "w[tv1]" }, border_size = 0, rounding = 0 })
hl.window_rule({ name = "no-gaps-f1",   match = { float = false, workspace = "f[1]" },   border_size = 0, rounding = 0 })

-- Ignore maximize requests from all apps.
hl.window_rule({ name = "no-maximize", match = { class = ".*" }, suppress_event = "maximize" })

-- Strip rounding on fullscreen windows.
hl.window_rule({ name = "fullscreen-clean", match = { fullscreen = true }, rounding = 0 })

-- Push tiny empty-title dialogs off to the corner, unfocused and pinned.
hl.window_rule({
    name  = "float-dialog",
    match = { title = "^$" },
    float    = true,
    no_focus = true,
    move     = "100% 100%",
    pin      = true,
})


------------------
---- MONITORS ----
------------------

-- Automatic monitor detection + layout, workspace<->monitor binding, and the
-- ALT+U/I/O + SUPER+ALT+M monitor keybinds. Also keeps waybar on the middle monitor.
require("monitors")
