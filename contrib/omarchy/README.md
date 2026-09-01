# Omarchy bar widget

A status-bar readout for agents-tui swarms, for [Omarchy](https://omarchy.org/)'s
Quickshell bar. Shows live busy/idle counts, goes loud when a worker is blocked
on input, and opens the dashboard on click.

```
󰛡 6/8      6 of 8 workers busy
󰀪 2        2 workers waiting for input (highlighted)
󰛡          dimmed: no swarm running, still opens the dashboard
```

The widget reads `/api/swarms` from the headless dashboard rather than shelling
out to tmux, so there is exactly one poller in the system: if the counts here
are wrong, the dashboard is wrong, and there is only one place to look.

## Requirements

- Omarchy 4.x (Quickshell shell, Hyprland 0.56+)
- `agents-tui --headless --web-port 7878` running
- `jq`, and a browser (`chromium`, `google-chrome-stable`, `brave`, or `firefox`)

## Install

```bash
cp -r plugins/agents.swarm ~/.config/omarchy/plugins/
install -m755 bin/omarchy-agents-dashboard ~/.local/bin/
```

Add the widget to the bar:

```bash
omarchy bar put agents.swarm --section center
```

Then add the window rules below to `~/.config/hypr/hyprland.lua` and run
`hyprctl reload`. They are not shipped as a file because that path is personal
config, not something a package should overwrite.

```lua
-- Agents swarm dashboard: keep it on its own special workspace so
-- omarchy-agents-dashboard can toggle it over whatever is in front.
-- Chromium derives this app_id from the --app URL (the port is dropped);
-- the "agents-dashboard" class is what Firefox would set from --class.
o.window({ class = "^chrome-127\\.0\\.0\\.1__-Default$" }, { workspace = "special:agents silent" })
o.window({ class = "^agents-dashboard$" }, { workspace = "special:agents silent" })
```

## Settings

Configured inline in `~/.config/omarchy/shell.json`, or through the widget's
settings panel:

| Key | Default | Meaning |
|---|---|---|
| `port` | `7878` | Port `agents-tui --headless` listens on |
| `refreshIntervalSec` | `5` | The dashboard polls tmux every 3s; polling faster gains nothing |
| `notifyOnAttention` | `true` | Desktop notification when a worker starts waiting |
| `openCommand` | `""` | Command run on click; empty uses `omarchy-agents-dashboard` |

## Notes for Hyprland 0.56 / Omarchy 4

Two things changed under this stack that the launcher has to work around, both
of which fail silently rather than loudly:

**`hyprctl dispatch` takes Lua, not bare words.** Arguments are wrapped as
`hl.dispatch(...)` and must be an `hl.dsp.*` dispatcher object. The old forms
are now syntax errors:

```
$ hyprctl dispatch exec "[workspace special:agents silent] chromium"
error: ']' expected near 'special'
```

Use `hl.dsp.exec_cmd("...")` and `hl.dsp.workspace.toggle_special("...")`.

**An exec window rule cannot place a window from an already-running browser.**
`chromium --app=URL` only signals the existing chromium process, so the window
is mapped by a PID Hyprland never associated with the exec and the
`[workspace ...]` prefix silently misses. Placement has to come from a
class-matched window rule instead, which is why the rules above are required
rather than optional.

**Chromium ignores `--class` when `--app` is used**, deriving its own app_id
from the URL (`chrome-127.0.0.1__-Default`, port dropped). The launcher matches
on the page title as well so the same code works under Firefox, which does
honour `--class`.
