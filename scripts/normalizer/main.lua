-- Full-track normalization: fixed gain, with one short transition when ready.
local mp = require 'mp'
local utils = require 'mp.utils'
local helper = utils.join_path(mp.get_script_directory(), '../../bin/normalize-track')
local controller = utils.join_path(mp.get_script_directory(), '../../bin/focus-music')
local serial = 0
local ramp = nil
local gain = 0

-- Older mpv ytdl hooks open the URL before yt-dlp's available_at time.
-- Wait after the built-in hook has resolved the stream and before mpv opens it.
mp.add_hook('on_load', 11, function(hook)
    local result = mp.get_property_native('user-data/mpv/ytdl/json-subprocess-result')
    local data = result and result.stdout and utils.parse_json(result.stdout)
    if type(data) ~= 'table' then return end
    local available = tonumber(data.available_at) or 0
    local formats = data.requested_formats or data.requested_downloads
    if type(formats) == 'table' then
        for _, format in ipairs(formats) do
            if type(format) == 'table' then
                available = math.max(available, tonumber(format.available_at) or 0)
            end
        end
    end
    local delay = math.min(60, math.max(0, available - os.time()))
    if delay > 0 then
        hook:defer()
        mp.add_timeout(delay, function() hook:cont() end)
    end
end)

local function set_state(state, target)
    mp.set_property_native('user-data/focus-music-normalization', {state = state, gain_db = target or gain})
end

local function apply_gain(target, token)
    if ramp then ramp:kill(); ramp = nil end
    local initial = gain
    local started = mp.get_time()
    ramp = mp.add_periodic_timer(0.1, function()
        if token ~= serial then ramp:kill(); ramp = nil; return end
        local progress = math.min(1, (mp.get_time() - started) / 3)
        gain = initial + (target - initial) * progress
        mp.commandv('af-command', 'focus-normalizer', 'volume', tostring(10 ^ (gain / 20)), 'volume')
        if progress == 1 then ramp:kill(); ramp = nil end
    end)
end

local function analyze(token, index, generation)
    if token ~= serial then return end
    set_state('measuring')
    mp.command_native_async({name = 'subprocess', playback_only = false,
        capture_stdout = true, capture_stderr = true,
        args = {helper, 'current', tostring(index), tostring(generation)}},
        function(success, result)
            if token ~= serial then return end
            local parsed = success and result.status == 0 and utils.parse_json(result.stdout) or nil
            if parsed and parsed.state == 'pending' then
                mp.add_timeout(3, function() analyze(token, index, generation) end)
                return
            end
            if not parsed or parsed.state ~= 'ready' then
                set_state(parsed and parsed.state or 'unavailable', 0)
                return
            end
            set_state('ready', parsed.gain_db)
            apply_gain(parsed.gain_db, token)
            mp.command_native_async({name = 'subprocess', playback_only = false,
                capture_stdout = true, capture_stderr = true,
                args = {helper, 'prefetch', tostring(index), tostring(generation)}}, function() end)
        end)
end

local function on_file_loaded()
    serial = serial + 1
    local token = serial
    if ramp then ramp:kill(); ramp = nil end
    gain = 0
    mp.commandv('af', 'add', '@focus-normalizer:lavfi=[volume=1]')
    set_state('measuring', 0)
    local index = mp.get_property_number('playlist-pos', 0)
    mp.command_native_async({name = 'subprocess', playback_only = false,
        capture_stdout = true, capture_stderr = true,
        args = {controller, 'status'}}, function(success, result)
            if token ~= serial then return end
            local status = success and result.status == 0 and utils.parse_json(result.stdout) or nil
            if status and status.generation then analyze(token, index, status.generation)
            else set_state('unavailable', 0) end
        end)
end

mp.register_event('file-loaded', on_file_loaded)
if mp.get_property('path') then on_file_loaded() end

mp.register_event('end-file', function()
    serial = serial + 1
    if ramp then ramp:kill(); ramp = nil end
end)
