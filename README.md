# agents-ui

A terminal UI (TUI) for launching, monitoring, and managing swarms of AI agents working on software repositories. Built with [Ratatui](https://ratatui.rs/) and Rust.

## Prerequisites

- **Rust toolchain** (1.85+) — install via [rustup](https://rustup.rs/):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **tmux** — required for agent session management:
  ```bash
  # macOS
  brew install tmux

  # Debian/Ubuntu
  sudo apt install tmux
  ```
- **Git** — for worktree-based parallel agent management

## Build

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release
```

The binary is produced at `target/debug/agents-ui` (or `target/release/agents-ui` for release builds).

## Install

```bash
# Install to ~/.cargo/bin (must be on your PATH)
cargo install --path .
```

## Run

```bash
# Run directly
cargo run

# Or after installing
agents-ui
```

Logs are written to `~/Library/Application Support/agents-ui/agents-ui.log` (macOS) or `~/.local/share/agents-ui/agents-ui.log` (Linux).

## Usage

The TUI provides a three-level navigation hierarchy:

1. **Repos List** — dashboard of all repos with active/paused agent swarms
2. **Repo View** — manager + workers for a single repo, work queue, blocked items
3. **Agent View** — live session view for an individual agent with input/output

Key bindings are shown in the status bar. Press `q` to quit, arrow keys or `j`/`k` to navigate, `Enter` to drill in, `Esc` to go back.

## Project Structure

```
src/
├── main.rs        # Entry point, logging setup
├── app.rs         # Application state and main event loop
├── event.rs       # Terminal event handling
├── tui.rs         # Terminal initialization/restoration
├── model/         # Data models (swarm, agent, repo)
├── ui/            # UI rendering (repos list, repo view, agent view)
├── tmux/          # tmux session proxying
├── adapter/       # Runtime adapters (Claude Code, etc.)
├── config/        # Configuration loading
└── scripts/       # Shell script integration
```

See [SPEC.md](SPEC.md) for the full product specification.
