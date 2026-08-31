# Running agents-tui well on Omarchy

Omarchy integration for the swarm dashboard: a headless service, a Hyprland
overlay, and a bar widget.

## Why this shape

`tmux` is a fine *control* plane for a swarm — spawn, `send-keys`, `capture-pane`,
kill — and `agents-tui` already uses it as one. What tmux is bad at is the *view*:
seeing many agents at once and noticing which one is stuck.

Those two jobs are separable, because **`capture-pane` works on invisible panes**.
A worker does not have to be on screen to be read or driven. So none of this
replaces tmux; it replaces what you look at.

That matters most on a laptop panel. At 1920x1080 with `scale 1.5` you have
1280x720 logical pixels for the whole desktop. An agent TUI needs ~80 columns
before its status line — the line this workflow's idle detection parses `ctx NN%`
out of — starts wrapping. Two panes fit. Not "many". Tiling is not the answer at
this size; a dashboard you summon is.

## Pieces

| File | Role |
|---|---|
| `agents-ui.service` | systemd user unit: `agents-tui --headless`, no terminal |
| `bin/omarchy-agents-dashboard` | toggles the dashboard on a Hyprland special workspace |
| `plugins/agents-swarm/` | Omarchy bar widget: worker counts + attention notifications |
| `hypr-snippet.lua` | `SUPER + A` binding and window rules |

## Install

```bash
cd /path/to/agents-ui
cargo install --path .
./omarchy/install.sh
```

Then:

```bash
systemctl --user enable --now agents-ui.service
omarchy plugin enable agents.swarm
```

## Use

- `SUPER + A` — dashboard over whatever is in front; same key dismisses it.
- The bar widget shows `⚙ busy/total`, or `⚠ n` when workers are blocked.
  It hides itself entirely when no swarm is running.
- A desktop notification fires on the rising edge of "needs input", so the
  bar does not have to be watched.

Logs: `journalctl --user -u agents-ui -f`

## The `--headless` flag

`--web` on its own still calls `tui::init()` and runs the interactive event
loop, so it dies with the terminal that launched it and has no tty to
initialize under a service manager. `--headless` serves the same web UI and the
same discovery poller with no TUI, and logs to stderr so `journalctl` sees it.
It implies `--web`; pair it with `--web-port` to move off 7878.

The server binds `127.0.0.1` only.
