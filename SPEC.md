# Agents UI — Product Specification

## Overview

A cross-platform application for launching, monitoring, and managing swarms of AI agents working on software repositories. The user interacts primarily with a manager agent per repo, which orchestrates worker agents. The UI provides visibility into the swarm's state, surfaces items needing human attention, and allows drilling into individual agent sessions for direct interaction.

## Architecture

### Layered Design

```
┌─────────────────────────────────────┐
│  UI Layer (TUI / Desktop / Mobile)  │
├─────────────────────────────────────┤
│  Agent Daemon (local server)        │
├─────────────────────────────────────┤
│  Runtime Adapters                   │
│  (Claude Code, Codex, Droid, etc.)  │
├─────────────────────────────────────┤
│  Agent Definitions (../agents/)     │
└─────────────────────────────────────┘
```

- **UI Layer:** Consumes the daemon API. TUI first (Ratatui), then desktop/mobile GUI (Tauri v2).
- **Agent Daemon:** Local server managing agent lifecycles, streaming output, and persisting state. Communicates with UIs over a local socket (WebSocket or gRPC). Designed so remote access (via Tailscale or similar) is a natural extension.
- **Runtime Adapters:** Abstraction layer over agent CLIs (Claude Code, Codex, Droid, Gemini CLI). Each adapter knows how to spawn a session, send input, capture output, and manage worktrees for its runtime.
- **Agent Definitions:** Leverages the existing plugins, commands, skills, and protocols in `../agents/`.

### Language & Reuse Strategy

- **Rust** for the daemon and core logic (agent management, state, communication).
- **Ratatui** for the TUI.
- **Tauri v2** for future desktop and mobile GUI (Rust backend + web frontend, supports macOS/Linux/Windows/iOS/Android).
- Shared Rust core across all platforms; only the UI layer changes.

## Navigation Hierarchy

```
Repos List ─────► Repo View ─────► Agent View
(all repos)       (one repo's       (one agent's
                   swarm team)       live session)
```

### Level 1: Repos List

The top-level dashboard showing all repos with active or paused swarms.

| Column        | Description                               |
|---------------|-------------------------------------------|
| Repo          | Repository name/path                      |
| Workflow      | Modernize / Autocoder / etc.              |
| Status        | Active / Paused / Idle                    |
| Agents        | Count of active workers (e.g., "3/5 busy")|
| Attention     | Count of blocked/needs-input items        |

Actions:
- Select a repo to drill into its Repo View
- Launch a new swarm (opens a manager session for an unconfigured repo)
- Resume a previously paused swarm

### Level 2: Repo View

Shows the team for one repo: manager + workers, their status, and the work queue.

**Manager Panel:**
- Manager agent status and recent activity
- Chat interface to talk with the manager
- GitHub issues summary (prioritized, with labels)
- Blocked items / questions needing human attention (surfaced as actionable items)

**Workers Panel:**

| Column        | Description                               |
|---------------|-------------------------------------------|
| Worker        | Agent ID / name                           |
| Status        | Working / Idle / Blocked / Waiting        |
| Current Task  | Issue # and title, or "idle"              |
| Runtime       | Claude Code / Codex / etc.                |

Actions:
- Talk with the manager (primary interaction point)
- Review and act on blocked items (approve proposals, refine issues, cancel tasks)
- Add or remove workers
- View GitHub issues list, reprioritize
- Drill into any agent's live session
- Launch manager commands (e.g., `/fix`, `/assess`, `/review-blocked`)

### Level 3: Agent View

Full live session view for one agent (manager or worker).

- Streaming output from the agent's session (proxied from tmux pane)
- Input field to send messages/commands to the agent (sent via `tmux send-keys`)
- Current task context (issue #, phase, worktree path)
- Work products viewer (see below)
- Ability to cancel current task, reassign, or pause

The manager agent runs as a Claude Code session in the **base repo** (main worktree). Workers each run in their own **git worktrees** created by `start-parallel-agents.sh`.

## Session Proxying via tmux

Since `start-parallel-agents.sh` launches agents inside tmux (or cmux) sessions, the daemon proxies these sessions into the TUI rather than requiring the user to leave the TUI to attach to tmux.

### How It Works

1. **Capture output:** The daemon polls each agent's tmux pane via `tmux capture-pane -p -t <pane>` (or uses tmux control mode `tmux -C` for event-driven updates). Captured content is streamed over WebSocket to the TUI.
2. **Send input:** When the user types in the Agent View, the daemon sends keystrokes to the tmux pane via `tmux send-keys -t <pane> "<input>" Enter`.
3. **Pane tracking:** The daemon maintains a mapping of agent ID to tmux session/window/pane target (e.g., `claude-myrepo:workers.0`). This mapping is derived from `start-parallel-agents.sh`'s session naming conventions.
4. **Scrollback:** The TUI can request historical pane content via `tmux capture-pane -p -S -<lines>` for scrollback.

### Benefits

- User never leaves the TUI — all agent interaction happens inline.
- tmux sessions remain the underlying runtime, so `join-parallel-agents.sh` still works if the user wants to attach directly.
- The daemon is a thin proxy layer, not a terminal emulator — tmux handles the heavy lifting.

## Swarm Lifecycle

The daemon leverages the existing agent orchestration scripts from `../agents/` rather than reimplementing agent coordination. These scripts handle worktree creation, multiplexer management, fix-loops, dispatch, status tracking, monitoring, and deployment coordination.

### Existing Scripts Leveraged

| Script | Purpose | Used By Daemon |
|--------|---------|----------------|
| `start-parallel-agents.sh` | Create worktrees, launch N agents in tmux/cmux sessions | Swarm startup |
| `stop-parallel-agents.sh` | Kill all agent sessions for a project | Swarm stop |
| `join-parallel-agents.sh` | Reconnect to running sessions | Session attach |
| `end-parallel-agents.sh` | Terminate sessions + optional worktree cleanup | Swarm teardown |
| `codex-fix-loop.sh` / `droid-fix-loop.sh` | Per-worker continuous fix loop with iteration control | Worker lifecycle |
| `codex-monitor-workers.sh` / `droid-monitor-workers.sh` | Poll worker status, dispatch issues to idle workers | Manager monitoring |
| `codex-autocoder.sh` / `droid-autocoder.sh` | Single-pass workflow execution (fix, review-blocked) | Worker task execution |
| `codex-start-parallel.sh` | Convenience wrapper for Codex parallel startup | Codex swarm startup |
| `stop-hook.sh` | Multi-agent idle detection, deployment coordination | Automated deployment |

### Coordination Mechanisms

The existing scripts use file-based coordination that the daemon reads directly:

- **Status files** (`.codex/loops/fix-loop.status`, `.factory/loops/fix-loop.status`) — Per-worker status (working, idle, `IDLE_NO_WORK_AVAILABLE`). The daemon polls these to populate the Workers Panel.
- **Dispatch files** (`.codex/loops/fix-loop.dispatch`, `.factory/loops/fix-loop.dispatch`) — Write an issue number to assign work to an idle worker. The daemon writes these when the user (or manager) assigns tasks.
- **Stop files** — Signal graceful shutdown to individual workers.
- **`CLAUDE_CODE_TASK_LIST_ID`** — Shared task list ID for multi-agent awareness across Claude Code sessions.
- **`CLAUDE_CODE_INTEGRATION_BRANCH`** — Target branch for deployment when all agents are idle.
- **GitHub labels** — `working` label as concurrency lock, priority labels (P0-P3), blocking labels (`needs-approval`, `needs-design`, etc.).

### Starting a Swarm

1. User selects "new swarm" and points at a repo path.
2. App spawns a **manager agent** session in the base repo (main worktree).
3. Manager asks the user (via chat): what workflow (modernize / autocoder), how many workers, runtime preference, etc.
4. Daemon calls `start-parallel-agents.sh` with the appropriate flags (`--agent claude|codex|droid|gemini`, worker count) to create git worktrees and launch worker sessions.
5. For autocoder workflows, each worker runs its runtime's fix-loop script (e.g., `codex-fix-loop.sh`), which continuously picks up and executes tasks.
6. The manager runs the monitor-workers logic (equivalent to `codex-monitor-workers.sh`) to dispatch issues to idle workers and surface blocked items.

### Ongoing Management

- The daemon continuously reads worker **status files** to update the Workers Panel in real time.
- Manager coordinates workers using existing plugin commands (`/fix`, `/review-blocked`, `/monitor-workers`, `/list-proposals`, `/approve-proposal`, etc.).
- User interacts with manager to: review blocked issues, approve proposals, reprioritize, add/remove workers.
- Adding a worker: daemon creates a new worktree (`git worktree add`) and launches a new agent session with the appropriate fix-loop.
- Removing a worker: daemon writes a stop file for the worker, waits for graceful shutdown, then cleans up the worktree.
- The daemon can dispatch specific issues to specific idle workers by writing to their dispatch files.
- User can drill into any worker to observe or intervene directly.

### Deployment Coordination

When all workers are idle (detected via status files and the `stop-hook.sh` idle detection logic):
- The daemon (or manager agent) can trigger deployment: merge worker branches into the integration branch, run validation, and execute the deploy command.
- This mirrors the existing `stop-hook.sh` behavior where the main worktree agent detects all-idle state and triggers deployment.

### Reconnecting / Resuming

The TUI is ephemeral — it can be closed and reopened at any time. Agent swarms persist in tmux sessions independently of the TUI. On startup, the daemon must discover and reconnect to any running swarms:

1. **Discover tmux sessions:** Scan for sessions matching the naming convention (`{agent}-{project}`, e.g., `claude-myrepo`) via `tmux list-sessions`. This mirrors `join-parallel-agents.sh`.
2. **Discover worktrees:** For each session, enumerate git worktrees via `git worktree list` in the base repo to find worker paths (`{repo}-wt-{N}`).
3. **Read status files:** For each worktree, read the status file (`.codex/loops/fix-loop.status`) to determine current agent state.
4. **Rebuild swarm model:** Reconstruct the `Swarm` with manager + workers, their tmux pane targets, and current status.
5. **Resume proxying:** Begin capturing pane output and watching status files as if the TUI had launched the swarm.

This means:
- The TUI can be quit and restarted without affecting running agents.
- If a swarm was launched outside the TUI (e.g., via `start-parallel-agents.sh` directly), the TUI discovers and adopts it on next startup.
- The daemon also persists its own state (`~/.agents-ui/swarms/`) as a hint, but tmux session discovery is the source of truth for what's actually running.
- Manager agent remembers what it was last doing (leveraging agent memory/context).

### Stopping

- **Pause:** Write stop files for all workers, wait for graceful shutdown, preserve worktrees and state for resume.
- **Stop:** Call `stop-parallel-agents.sh` to kill sessions, then `end-parallel-agents.sh` for worktree cleanup.
- **Teardown:** Full cleanup including worktree and branch removal (with user confirmation).

## Runtime Adapter Interface

Each adapter implements a common trait:

```rust
trait AgentRuntime {
    /// Spawn a new agent session in the given worktree
    fn spawn(config: AgentConfig) -> Result<SessionHandle>;

    /// Send a message/command to a running session
    fn send(session: &SessionHandle, input: &str) -> Result<()>;

    /// Subscribe to the agent's output stream
    fn output_stream(session: &SessionHandle) -> Result<Stream<OutputEvent>>;

    /// Get current session status
    fn status(session: &SessionHandle) -> Result<AgentStatus>;

    /// Terminate the session
    fn kill(session: &SessionHandle) -> Result<()>;
}
```

### V1 Adapter: Claude Code

- Spawns `claude` CLI processes via `start-parallel-agents.sh --agent claude`
- Workers run the autocoder stop-hook fix-loop for continuous operation
- Captures stdout/stderr for output streaming; sends input via stdin
- Reads `.codex/loops/fix-loop.status` for worker status (shared status file convention)
- Writes `.codex/loops/fix-loop.dispatch` to assign issues to workers
- Worktree lifecycle managed by `start-parallel-agents.sh` / `end-parallel-agents.sh`
- Leverages `CLAUDE_CODE_TASK_LIST_ID` for multi-agent task awareness

### Future Adapters

- **Codex:** Uses `codex-start-parallel.sh`, `codex-fix-loop.sh`, `codex-monitor-workers.sh`, `codex-autocoder.sh`. Status via `.codex/loops/fix-loop.status`, dispatch via `.codex/loops/fix-loop.dispatch`.
- **Droid (Factory):** Uses `droid-fix-loop.sh`, `droid-monitor-workers.sh`, `droid-autocoder.sh`. Status via `.factory/loops/fix-loop.status`, dispatch via `.factory/loops/fix-loop.dispatch`.
- **Gemini (Antigravity):** Spawns Gemini agent CLI with `.agent/` configuration.

Each adapter delegates to the runtime-specific scripts from `../agents/` for lifecycle management, but exposes the same `AgentRuntime` trait to the daemon. The scripts handle the runtime-specific details (CLI flags, status file paths, dispatch mechanisms), keeping the adapters thin.

## Work Product Viewer

Agents produce work products (markdown docs, ADRs, diagrams, images). The UI must render these.

### TUI Rendering

- **Markdown:** Rendered inline in the TUI with syntax highlighting and formatting.
- **Mermaid diagrams:** Rendered to SVG/PNG via `mmdc` (Mermaid CLI), then opened externally.
- **PNG/SVG images:** Opened in the system viewer (`open` on macOS, `xdg-open` on Linux).
- **Local preview server:** A lightweight HTTP server serves rendered work products on `localhost`, providing a richer view in a browser. This server becomes the seed for the future GUI.

### Future GUI Rendering

- All work products rendered inline (markdown, Mermaid, images) in the Tauri webview.
- The preview server from the TUI phase evolves into the GUI frontend.

## Blocked Items & Notifications

The UI surfaces items needing human attention so the user doesn't have to poll:

- **Proposals needing approval** (autocoder `needs-approval` label)
- **Issues needing design input** (`needs-design` label)
- **Issues needing clarification** (`needs-clarification` label)
- **Quality gate failures** (security score too low, tests failing)
- **Agent questions** (agent waiting for user input)
- **Idle agents** (no more work to assign)

These appear as a notification count on the Repos List, and as an actionable list in the Repo View. The user can act on them inline (approve, respond, reprioritize) primarily through the manager chat.

## State Persistence

The daemon stores state locally (e.g., `~/.agents-ui/`):

```
~/.agents-ui/
├── daemon.toml          # Daemon configuration
├── swarms/
│   ├── <repo-hash>/
│   │   ├── swarm.toml   # Workflow, runtime, worker count, etc.
│   │   ├── state.json   # Agent sessions, current tasks, resume info
│   │   └── history/     # Session logs for review
```

## Future: Remote Agents

Designed for but not implemented in V1:

- The daemon already exposes a socket API; remote access means exposing it over the network.
- **Tailscale** for zero-config, secure connectivity through firewalls.
- Remote daemon runs on the machine with the agents; UI connects over Tailscale.
- Same API, same UI — local and remote are identical from the UI's perspective.

## V1 Scope Summary

| Feature                              | V1  | Future |
|--------------------------------------|-----|--------|
| TUI interface (Ratatui)              | Yes |        |
| Desktop/Mobile GUI (Tauri)           |     | Yes    |
| Local agent management               | Yes |        |
| Remote agent management              |     | Yes    |
| Claude Code runtime adapter          | Yes |        |
| Codex / Droid / Gemini adapters      |     | Yes    |
| Repos list dashboard                 | Yes |        |
| Repo view with manager + workers     | Yes |        |
| Agent live session view + interaction | Yes |        |
| Chat with manager and workers        | Yes |        |
| Markdown rendering in TUI            | Yes |        |
| Mermaid/image external viewer        | Yes |        |
| Local preview server for rich render | Yes |        |
| Blocked items / notifications        | Yes |        |
| Add/remove workers dynamically       | Yes |        |
| Swarm state persistence & resume     | Yes |        |
| Inline work product rendering (GUI)  |     | Yes    |
| 1-2 repos, 1-5 workers + manager    | Yes |        |
| Multi-repo at scale                  |     | Yes    |

## Tech Stack Summary

| Component       | Technology          |
|-----------------|---------------------|
| Language         | Rust                |
| TUI Framework    | Ratatui             |
| Future GUI       | Tauri v2            |
| Agent Daemon     | Rust (tokio async)  |
| IPC              | WebSocket (local)   |
| Agent Definitions| ../agents/ plugins  |
| State Storage    | TOML + JSON files   |
| Diagram Render   | Mermaid CLI (mmdc)  |
| Future Networking| Tailscale           |
