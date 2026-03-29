# agents-tui

A terminal UI (TUI) for launching, monitoring, and managing swarms of AI agents working on software repositories. Built with [Ratatui](https://ratatui.rs/) and Rust.

## Prerequisites

### Required

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
- **Git** — for worktree-based parallel agent management:
  ```bash
  # macOS (usually pre-installed, or via Xcode CLI tools)
  xcode-select --install

  # Debian/Ubuntu
  sudo apt install git
  ```
- **Runtime CLI** — install at least one agent runtime:
  - **Claude Code**: see https://docs.anthropic.com/en/docs/claude-code
  - **Codex**: `npm install -g @openai/codex`
  - **Droid**: see https://droid.dev

### Recommended

- **GitHub CLI (`gh`)** — used for issue tracking, work dispatch, and opening issues in the browser:
  ```bash
  # macOS
  brew install gh

  # Debian/Ubuntu
  sudo apt install gh
  # or: see https://cli.github.com/ for other Linux distros
  ```
  After installing, authenticate:
  ```bash
  gh auth login
  ```

### Startup validation

On launch, `agents-tui` checks prerequisites in order:

1. **tmux** — fatal if missing (prints install command and exits)
2. **Agent runtime** (claude/codex/droid) — fatal if the selected runtime is missing (prints install hint and exits)
3. **gh auth status** — non-fatal warning if `gh` is not installed or not authenticated. The TUI still launches, but issue tracking and work dispatch won't function. The status bar shows what to do (e.g., "Run: `gh auth login`")

If `gh` authentication expires while the TUI is running, the issue fetcher detects the error, shows a warning in the status bar, and stops retrying (instead of spamming logs).

### Remote mode

When using `--server <host>`, prerequisites only need to exist on the remote host. The local machine only needs `agents-tui` itself and SSH access to the host.

## Build

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release
```

The binary is produced at `target/debug/agents-tui` (or `target/release/agents-tui` for release builds).

## Install

```bash
# Install to ~/.cargo/bin (must be on your PATH)
cargo install --path .
```

## Run

```bash
# Run directly
cargo run

# Force a runtime
cargo run -- --codex

# Run against a remote host
cargo run -- --codex --server buildbox

# Or after installing
agents-tui

# Shell alias used on this machine
atui
```

Logs are written to `~/Library/Application Support/agents-ui/agents-ui.log` (macOS) or `~/.local/share/agents-ui/agents-ui.log` (Linux).

## Remote Mode

Use `--server <host>` or `--server=<host>` to manage tmux sessions on a remote machine over SSH.

- tmux discovery, pane capture, key injection, repo worktree setup, issue refresh, and runtime launch run on the remote host
- existing swarms reconnect by tmux session name on that same host
- local repo scanning is disabled in remote mode, so launch new swarms by entering the remote repo path explicitly
- if the remote host is missing `tmux` or the selected runtime, `agents-tui` exits with an install message instead of opening the TUI

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
