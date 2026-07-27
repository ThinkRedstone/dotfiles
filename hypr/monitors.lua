-- Native Hyprland (Lua) replacement for monitor_config.sh + reload_monitors.sh.
--
-- On startup / hotplug / reload we scan the connected monitors and run the matching layout
-- from CONFIGS (keyed by a distinguishing monitor serial); if nothing matches we fall back to
-- CONFIGS.default. Each layout function receives the monitor list, assigns monitors to the
-- left/middle/right roles (used by the keybinds and workspace bindings below) and positions them.

-- Role -> monitor object (from hl.get_monitors()). Mutated by M.apply(); read by the
-- keybinds and workspace bindings below.
local roles = { left = nil, middle = nil, right = nil }

-- Only (re)configure waybar when the middle monitor actually changes (see apply()).
local last_waybar_monitor = nil

--------------------------------------------------------------------------------
-- Helpers
--------------------------------------------------------------------------------

local function place(monitor, position, scale, transform)
    hl.monitor({ output = monitor.name, mode = "preferred", position = position, scale = scale, transform = transform })
end

-- The internal laptop panel is the output whose name starts with "eDP".
local function is_laptop(monitor) return monitor.name:sub(1, 3) == "eDP" end

local function find_laptop(monitors)
    for _, monitor in ipairs(monitors) do
        if is_laptop(monitor) then return monitor end
    end
end

local function find_by_serial(monitors, serial)
    for _, monitor in ipairs(monitors) do
        if monitor.description:find(serial, 1, true) then return monitor end
    end
end

-- Laptop panel on the left at hidpi (shared by the desk layouts).
local function laptop_left(monitors)
    local laptop = find_laptop(monitors)
    if laptop then
        place(laptop, "-960x1140", 2)
        roles.left = laptop
    end
end

--------------------------------------------------------------------------------
-- Layouts: monitor serial -> function(monitors). "default" handles everything else.
--------------------------------------------------------------------------------

local CONFIGS = {
    -- Dell desk: laptop | U2724DE (middle) | U2724D (right, rotated)
    ["1M64F84"] = function(monitors)
        laptop_left(monitors)
        local middle = find_by_serial(monitors, "1M64F84")
        place(middle, "0x0", "auto"); roles.middle = middle
        local right = find_by_serial(monitors, "8GHVG84")
        if right then place(right, "2560x-580", "auto", 1); roles.right = right end
    end,

    -- Laptop | BenQ XL2730Z (middle)
    ["S6F01474SL0"] = function(monitors)
        laptop_left(monitors)
        local middle = find_by_serial(monitors, "S6F01474SL0")
        place(middle, "0x0", "auto"); roles.middle = middle
    end,

    -- Dell U2723QE desk: laptop | 2S0SWN3 (middle) | 1V1SWN3 (right, rotated)
    ["2S0SWN3"] = function(monitors)
        laptop_left(monitors)
        local middle = find_by_serial(monitors, "2S0SWN3")
        place(middle, "0x0", "auto"); roles.middle = middle
        local right = find_by_serial(monitors, "1V1SWN3")
        if right then place(right, "2560x-580", "auto", 1); roles.right = right end
    end,

    -- Fallback: laptop -> middle, any extra monitors -> left then right (auto placed).
    default = function(monitors)
        local laptop = find_laptop(monitors)

        local extras = {}
        for _, monitor in ipairs(monitors) do
            if not is_laptop(monitor) then extras[#extras + 1] = monitor end
        end
        table.sort(extras, function(a, b) return a.name < b.name end)

        local middle = laptop or table.remove(extras, 1)
        if middle then
            place(middle, "0x0", middle == laptop and 1 or "auto")
            roles.middle = middle
        end

        for i, role in ipairs({ "left", "right" }) do
            local monitor = extras[i]
            if monitor then
                place(monitor, "auto", "auto")
                roles[role] = monitor
            end
        end
    end,
}

--------------------------------------------------------------------------------
-- Apply
--------------------------------------------------------------------------------

local function bind_workspaces()
    if roles.middle then
        for i = 1, 8 do
            hl.workspace_rule({ workspace = tostring(i), monitor = roles.middle.name, persistent = true, default = (i == 1) or nil })
        end
    end
    if roles.left then
        hl.workspace_rule({ workspace = "9", monitor = roles.left.name, persistent = true, default = true })
    end
    if roles.right then
        hl.workspace_rule({ workspace = "10", monitor = roles.right.name, persistent = true, default = true, layout = "lua:vstack" })
    end
end

-- (Re)configure waybar for the middle monitor, but only when it actually changed
-- (compared by .name, since each apply() reads fresh monitor tables). This avoids a
-- SIGUSR2 race: waybar_config.sh starts waybar the first time and USR2-reloads it
-- afterwards. Firing it on every apply would send USR2 to a still-starting waybar
-- (whose default USR2 action is to terminate) and kill it.
local function reconfigure_waybar()
    local middle = roles.middle
    if not middle or middle.name == last_waybar_monitor then return end
    last_waybar_monitor = middle.name
    hl.exec_cmd(string.format('~/.config/hypr/waybar_config.sh "%s"', middle.name))
end

function apply()
    local monitors = hl.get_monitors()
    if #monitors == 0 then return end -- monitors not up yet; an event will retrigger us.

    roles.left, roles.middle, roles.right = nil, nil, nil

    local layout = CONFIGS.default
    for serial, fn in pairs(CONFIGS) do
        if serial ~= "default" and find_by_serial(monitors, serial) then layout = fn; break end
    end
    layout(monitors)

    bind_workspaces()
    reconfigure_waybar()
end

--------------------------------------------------------------------------------
-- Monitor keybindings (role-based; resolved at press time since names are dynamic)
--------------------------------------------------------------------------------

local mainMod = "ALT"

local function focus_monitor(role)
    return function()
        local mon = roles[role]
        if mon then hl.dispatch(hl.dsp.focus({ monitor = mon.name })) end
    end
end

local function move_to_monitor(role)
    return function()
        local mon = roles[role]
        if mon then hl.dispatch(hl.dsp.window.move({ monitor = mon.name })) end
    end
end

hl.bind(mainMod .. " + U", focus_monitor("left"))
hl.bind(mainMod .. " + I", focus_monitor("middle"))
hl.bind(mainMod .. " + O", focus_monitor("right"))

hl.bind(mainMod .. " + SHIFT + U", move_to_monitor("left"))
hl.bind(mainMod .. " + SHIFT + I", move_to_monitor("middle"))
hl.bind(mainMod .. " + SHIFT + O", move_to_monitor("right"))

-- Re-detect and re-apply the monitor layout (was SUPER+ALT+M -> monitor_config.sh).
hl.bind("SUPER + " .. mainMod .. " + M", apply)

-- Apply on reload (parse-time; early-returns on fresh boot) and on startup / hotplug.
apply()
hl.on("hyprland.start",  apply)
hl.on("monitor.added",   apply)
hl.on("monitor.removed", apply)
