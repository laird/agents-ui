# agents-tui

`agents-tui` is a Rust terminal UI for launching, reconnecting to, and supervising multi-agent coding swarms that run inside `tmux`. It gives you one place to watch manager and worker sessions, inspect blocked work, and jump directly into individual agent panes without leaving the TUI.

## What It Does

- launches new swarms for a repo and reconnects to existing ones by `tmux` session name
- supports multiple runtimes: Claude, Codex, Droid, and Gemini
- monitors manager and worker panes live from inside the TUI
- works locally or against a remote host over SSH with `--server`
- persists the default runtime per repo in `.agents-ui.toml`

## Requirements

### Required

- **Rust** 1.85+
- **tmux**
- **git**
- at least one supported runtime CLI:
  - **Claude Code**: [Anthropic docs](https://docs.anthropic.com/en/docs/claude-code)
  - **Codex**: `npm install -g @openai/codex`
  - **Droid**: [droid.dev](https://droid.dev)
  - **Gemini CLI**: [ai.google.dev](https://ai.google.dev)

### Repo Integration

This app expects the sibling `../agents` repo to be available for launcher scripts, runtime helpers, and marketplace/plugin assets. Some runtime setup is runtime-specific:

- **Claude**: requires the `laird/agents` Claude marketplace/plugin setup
- **Codex**: requires Codex repo or user assets to be installed
- **Droid**: requires the Droid plugin/assets from `laird/agents`
- **Gemini**: binary availability is enough for basic runtime selection

### Recommended

- **GitHub CLI (`gh`)** for issue refresh, dispatch flows, and GitHub integration:

```bash
# macOS
brew install gh

# Debian/Ubuntu
sudo apt install gh

gh auth login
```

If `gh` is missing or unauthenticated, the TUI still starts, but GitHub-backed features are limited.

## Quick Start

```bash
# Build
cargo build

# Run with the repo's saved/default runtime
cargo run

# Pin a runtime from the CLI
cargo run -- --claude
cargo run -- --codex
cargo run -- --droid

# Connect to a remote host over SSH
cargo run -- --codex --server buildbox
```

After install:

```bash
cargo install --path .
agents-tui
```

### Run the village and dashboard as a service

To keep the web UI serving (started at login, restarted on failure), install
it as a user service:

```bash
cargo install --path . && agents-tui service install
```

On Linux this writes the systemd user unit
`~/.config/systemd/user/agents-tui.service` (with the installed binary's
path, and a `PATH` that finds `gh`, `git`, `herdr` and `claude`), enables and
starts it, and waits until `/village` answers. (macOS's launchd is not
supported yet.) Options mirror the server's flags:

- `--web-port N` (default 7890) and `--web-bind ADDR[,ADDR]` (default
  `127.0.0.1`). When tailscale is up it suggests `127.0.0.1,tailscale`, which
  also serves your tailnet devices.
- `--source LAYER=NAME`, repeatable (default `swarms=herdr` when herdr is on
  `PATH`), and `--read-only`.
- `--base-path PREFIX` and `--frame-ancestors SOURCES`, for serving behind a
  reverse proxy (see CLI Options). With a base path, `install`, `status` and
  `restart` check the village at `<prefix>/api/village`.
- `--dry-run` prints the unit and the commands and changes nothing.

**Linger:** systemd stops user services when you log out. Pass `--linger` (it
runs `loginctl enable-linger $USER`) to keep the village serving while you're
logged out. Without it, `install` tells you, but never turns it on by itself.

**No LAN by default:** the UI has no authentication, so `install` refuses
`0.0.0.0` or `::` unless you pass `--allow-lan`. Prefer `127.0.0.1,tailscale`.

Re-run `install` to change options (it shows what changed and restarts).
Each run writes only the options it is given, so name every one you want to
keep.
`agents-tui service status` shows its state, URL and binary;
`agents-tui service restart` restarts it; `agents-tui service uninstall`
stops and removes it. autocoder's `deploy.sh --env deploy` (see CLAUDE.md)
restarts this unit. That environment's name is the first entry in
`.autocoder.json`; rename it to match the host you install on.

The binary is written to `target/debug/agents-tui` or `target/release/agents-tui`.

### Open an agent's pane from the village

An agent's dialog in the village has **Open in herdr** (or tmux): an
`agents-ui://attach?...` link that opens that agent's pane in a terminal on
your computer. Register this binary as the link's handler once:

```bash
agents-tui handler install            # --dry-run shows what it would do
```

- **Linux:** writes `~/.local/share/applications/agents-tui-open.desktop` and
  registers it with `xdg-mime`. The pane opens in `omarchy-launch-terminal`,
  `xdg-terminal-exec`, `$TERMINAL`, or a common emulator.
- **macOS:** builds `~/Applications/AgentsUILinkHandler.app` (an AppleScript
  applet, with its source kept in
  `~/Library/Application Support/agents-tui/link-handler.applescript`),
  declares the scheme, signs it ad hoc and registers it with LaunchServices.
  The pane opens in Ghostty when it is installed, else Terminal.

The handler runs `agents-tui open <url>`, which refuses a host that is not
this machine, a tailnet peer or a saved herdr machine, and attaches with
`herdr agent attach <pane>` (over `ssh -t` for another host). Without a
handler, the page opens the web terminal instead.

**iOS** has no handler: the link opens the web terminal until you choose an
SSH app under it. **Blink** runs `ssh -t <host> herdr agent attach <pane>`
itself (give the page the key from Blink's Settings > x-callback-url);
**Termius** only opens `ssh://<host>`, so the page shows the command to paste.
The page remembers the choice, and **Change SSH app** clears it.

## CLI Options

Supported flags:

- `--claude`
- `--codex`
- `--droid`
- `--gemini`
- `--server <host>`
- `--server=<host>`

Web UI flags: `--web` (alongside the TUI), `--headless` (web UI only), `--web-port <port>`, `--read-only`, `--group <path>` (the group manifest the valley of related repos reads, `~/.agents-ui/group.json` by default; see `docs/village.md`), and `--web-bind <addr>[,<addr>...]`. `--web-bind` sets where the UI listens and defaults to `127.0.0.1`. It takes IPv4 or IPv6 literals, or `tailscale` for this machine's tailnet addresses, and listens on each one. To reach the village from other tailnet devices and from this machine together:

```bash
agents-tui --headless --web-port 7890 --web-bind 127.0.0.1,tailscale --read-only
```

An address that can't be bound fails startup and names the address.

To serve the UI behind a reverse proxy at a path prefix, or inside another application's page:

- `--base-path <prefix>` (or `AGENTS_UI_BASE_PATH`) mounts every route under the prefix: the dashboard at `<prefix>/`, the village at `<prefix>/village`, the API at `<prefix>/api/...`. `<prefix>` redirects to `<prefix>/`, and `/healthz` also answers at the root for probes. Nothing else is served at the root.
- `--frame-ancestors <sources>` (or `AGENTS_UI_FRAME_ANCESTORS`) sends `Content-Security-Policy: frame-ancestors <sources>` on every response, so the named origins may frame the pages. Without it no policy is sent.

```bash
agents-tui --headless --read-only --base-path /ops/village --frame-ancestors "'self' https://example.test"
```

See "Behind a reverse proxy" in `docs/village.md`.

World flags: `--world <dev>=<platform>[@<environment>]` (repeatable) pairs a dev village with the platform it builds, and `--incident-label <label>[,<label>...]` (repeatable; `incident` by default) names the labels that mark an issue the platform raised. A label ending in `:` is a prefix: `--incident-label channel:` matches every label starting with `channel:`, and each crate shows the part after the colon as its channel. See "Worlds" in `docs/village.md`.

Only one runtime flag may be passed at a time. If no runtime flag is provided, `agents-tui` uses the repo's saved default from `.agents-ui.toml` when available, otherwise it starts with `Claude`.

## Startup Checks

On startup, `agents-tui` validates:

1. `tmux`
2. the selected runtime binary and basic runtime readiness
3. `gh auth status` as a non-fatal warning

When running with `--server`, these checks happen on the remote host. Locally you only need the `agents-tui` binary and SSH access.

## Usage

The TUI has three main views:

1. **Repos List**: all discovered swarms
2. **Repo View**: one repo's manager, workers, and work queue
3. **Agent View**: a live view into one agent's `tmux` pane

Key bindings are shown in the status bar. Common navigation:

- `q` quits
- arrow keys or `j` / `k` move selection
- `Enter` drills in
- `Esc` goes back

## Remote Mode

Use `--server <host>` to manage swarms on another machine over SSH.

- session discovery, pane capture, key injection, worktree operations, and runtime launch all happen on the remote host
- existing swarms reconnect by `tmux` session name on that same host
- local repo scanning is disabled in remote mode, so you enter remote repo paths explicitly when launching new swarms

## Logging And Config

- logs are written to `~/Library/Application Support/agents-ui/agents-ui.log` on macOS
- logs are written to `~/.local/share/agents-ui/agents-ui.log` on Linux
- repo-level runtime preference is stored in `.agents-ui.toml`

## Testing

```bash
cargo test
```

Snapshot tests use [`insta`](https://insta.rs/):

```bash
cargo install cargo-insta
cargo test <test_name>
cargo insta review
```

## Project Structure

```text
src/
├── main.rs
├── app.rs
├── event.rs
├── tui.rs
├── model/
├── ui/
├── tmux/
├── adapter/
├── config/
└── scripts/
```

See [SPEC.md](SPEC.md) for the longer product and architecture spec.
