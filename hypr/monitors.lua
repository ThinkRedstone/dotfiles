-- Native Hyprland (Lua) replacement for monitor_config.sh + reload_monitors.sh.
--
-- On startup / hotplug / reload we scan the connected monitors by serial and run the
-- matching layout from CONFIGS (keyed by a distinguishing monitor serial); if nothing
-- matches we fall back to CONFIGS.default. Each layout function receives the
-- serial -> monitor table, assigns monitors to the left/middle/right roles (used by
-- the keybinds and workspace bindings below) and positions them.

-- Role -> monitor object (from hl.get_monitors()). Mutated by M.apply(); read by the
-- keybinds and workspace bindings below.
local roles = { left = nil, middle = nil, right = nil }

-- The internal laptop panel reports an EMPTY serial on this machine, so it is keyed
-- under "" in by_serial. (If an external ever also reports no serial, they'd collide.)
local LAPTOP = ""

-- Only (re)configure waybar when the middle monitor actually changes (see apply()).
local last_waybar_monitor = nil

--------------------------------------------------------------------------------
-- Helpers
--------------------------------------------------------------------------------

-- serial -> monitor for every connected output (laptop keyed under LAPTOP = "").
local function scan()
    local by_serial = {}
    for _, mon in ipairs(hl.get_monitors()) do
        by_serial[mon.serial or ""] = mon
    end
    return by_serial
end

local function place(mon, position, scale, transform)
    hl.monitor({ output = mon.name, mode = "preferred", position = position, scale = scale, transform = transform })
end

-- Laptop panel on the left at hidpi (shared by the desk layouts).
local function laptop_left(by_serial)
    local lap = by_serial[LAPTOP]
    if lap then
        place(lap, "-960x1140", 2)
        roles.left = lap
    end
end

--------------------------------------------------------------------------------
-- Layouts: monitor serial -> function(by_serial). "default" handles everything else.
--------------------------------------------------------------------------------

local CONFIGS = {
    -- Dell desk: laptop | U2724DE (middle) | U2724D (right, rotated)
    ["1M64F84"] = function(by_serial)
        laptop_left(by_serial)
        local mid = by_serial["1M64F84"]
        place(mid, "0x0", "auto"); roles.middle = mid
        local right = by_serial["8GHVG84"]
        if right then place(right, "2560x-580", "auto", 1); roles.right = right end
    end,

    -- Laptop | BenQ XL2730Z (middle)
    ["S6F01474SL0"] = function(by_serial)
        laptop_left(by_serial)
        local mid = by_serial["S6F01474SL0"]
        place(mid, "0x0", "auto"); roles.middle = mid
    end,

    -- Dell U2723QE desk: laptop | 2S0SWN3 (middle) | 1V1SWN3 (right, rotated)
    ["2S0SWN3"] = function(by_serial)
        laptop_left(by_serial)
        local mid = by_serial["2S0SWN3"]
        place(mid, "0x0", "auto"); roles.middle = mid
        local right = by_serial["1V1SWN3"]
        if right then place(right, "2560x-580", "auto", 1); roles.right = right end
    end,

    -- Fallback: laptop -> middle, any extra monitors -> left then right (auto placed).
    default = function(by_serial)
        local laptop = by_serial[LAPTOP]

        local extras = {}
        for serial, mon in pairs(by_serial) do
            if serial ~= LAPTOP then extras[#extras + 1] = mon end
        end
        table.sort(extras, function(a, b) return a.name < b.name end)

        local mid = laptop or table.remove(extras, 1)
        if mid then
            place(mid, "0x0", mid == laptop and 1 or "auto")
            roles.middle = mid
        end

        for i, role in ipairs({ "left", "right" }) do
            local mon = extras[i]
            if mon then
                place(mon, "auto", "auto")
                roles[role] = mon
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
        hl.workspace_rule({ workspace = "10", monitor = roles.right.name, persistent = true, default = true, layout_opts = { orientation = "top" } })
    end
end

-- (Re)configure waybar for the middle monitor, but only when it actually changed
-- (compared by .name, since each scan() returns fresh monitor tables). This avoids a
-- SIGUSR2 race: waybar_config.sh starts waybar the first time and USR2-reloads it
-- afterwards. Firing it on every apply would send USR2 to a still-starting waybar
-- (whose default USR2 action is to terminate) and kill it.
local function reconfigure_waybar()
    local mid = roles.middle
    if not mid or mid.name == last_waybar_monitor then return end
    last_waybar_monitor = mid.name
    hl.exec_cmd(string.format('~/.config/hypr/waybar_config.sh "%s"', mid.name))
end

function apply()
    local by_serial = scan()
    if next(by_serial) == nil then return end -- monitors not up yet; an event will retrigger us.

    roles.left, roles.middle, roles.right = nil, nil, nil

    local layout = CONFIGS.default
    for serial in pairs(by_serial) do
        if CONFIGS[serial] then
            layout = CONFIGS[serial]
            break
        end
    end
    layout(by_serial)

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
