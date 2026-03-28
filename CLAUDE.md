# CLAUDE.md — agents-tui

## Project overview

A Rust TUI application for launching, monitoring, and managing swarms of AI agents working on software repositories. See SPEC.md for full product specification.

## Build & run

```bash
source "$HOME/.cargo/env"
cargo build          # Build
cargo run            # Run the TUI
cargo install --path . # Install globally as `agents-tui`
```

## Testing

```bash
cargo test           # Run all tests
cargo test -- --nocapture  # Run with stdout visible
cargo test status    # Run tests matching "status"
cargo test swarm     # Run tests matching "swarm"
```

### Test structure

Tests are inline `#[cfg(test)]` modules in each source file:

- `src/model/status.rs` — Status line parsing, state detection, issue extraction, file I/O
- `src/model/swarm.rs` — Agent counting, lookups, state queries, type methods
- `src/config/persistence.rs` — TOML save/load round-trips, listing, missing files
- `src/app.rs` — Key-to-tmux conversion, pane status inference, tab completion helpers

Tests use `tempfile` for filesystem tests. No mocking of tmux — integration-level tmux testing is done manually via `tmux new-session -d` + `cargo run`.

### Adding tests

Add tests to the `#[cfg(test)] mod tests` block in the relevant source file. For status parsing or data model changes, add unit tests. For tmux interaction changes, test manually.

## Architecture

```
src/
  main.rs           # Entry point, logging, terminal init
  app.rs            # App state, event loop, key handling, navigation
  event.rs          # Async event system (keys, ticks, pane updates)
  tui.rs            # Terminal raw mode setup/teardown
  model/
    status.rs       # AgentState/AgentStatus types, status file parsing
    swarm.rs        # Swarm/AgentInfo types, AgentType, Workflow
  tmux/
    session.rs      # Session discovery, pane listing
    proxy.rs        # Pane capture, send-keys, background watchers
  adapter/
    traits.rs       # AgentRuntime trait
    claude.rs       # Claude Code adapter (worktrees, tmux, launch)
  scripts/
    launcher.rs     # Script/plugin path resolution
  ui/
    repos_list.rs   # Level 1: repos dashboard
    repo_view.rs    # Level 2: manager + workers
    agent_view.rs   # Level 3: live agent session (passthrough)
    new_swarm.rs    # Launch dialog
    theme.rs        # Color/style constants
  config/
    persistence.rs  # Swarm state save/load (~/.agents-ui/)
```

## Key conventions

- Agent sessions run in tmux. The TUI proxies pane content via `tmux capture-pane -e` and sends input via `tmux send-keys`.
- Session naming: `claude-<project>` with window 0 "agents" (workers) and window 1 "review" (manager).
- Worktree naming: `<repo>-wt-<N>` in the repo's parent directory.
- Status is inferred from pane content when no `.codex/loops/fix-loop.status` file exists.
- ANSI colors are preserved via `ansi-to-tui` crate.
- Keystroke passthrough in agent view — all keys forwarded to tmux pane except: double-Esc (back), Alt+0-9/m (navigation), PgUp/PgDn (scroll).
- The autocoder plugin is auto-installed via `claude plugin marketplace add laird/agents` + `claude plugin install autocoder` if not present.
- Claude is launched with `--append-system-prompt` to force tmux (not cmux) usage.

## Automated Testing & Issue Management

This section configures the `/fix` command for autonomous issue resolution.

### Regression Test Suite
```bash
cargo test
```

### Build Verification
```bash
cargo build
```

### Test Framework Details

**Unit Tests**:
- Framework: Rust built-in test framework
- Location: inline `#[cfg(test)]` modules in each source file

**Test Reports**:
- Location: `docs/test/regression-reports/`

