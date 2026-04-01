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

The binary is written to `target/debug/agents-tui` or `target/release/agents-tui`.

## CLI Options

Supported flags:

- `--claude`
- `--codex`
- `--droid`
- `--gemini`
- `--server <host>`
- `--server=<host>`

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
