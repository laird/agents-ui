//! Background task that discovers running agent swarms from tmux directly,
//! without requiring the TUI to be running.
//!
//! Polls every few seconds, scans for `claude-*` / `codex-*` / `droid-*` / `gemini-*`
//! tmux sessions, captures pane content, reads status files, and writes the result
//! into `SharedWebState`.

use std::path::PathBuf;
use tokio::time::{Duration, sleep};

use crate::transport::ServerTransport;
use crate::tmux::session as tmux_session;
use crate::tmux::proxy::capture_pane;
use crate::model::status::{AgentState, read_status_file};
use crate::model::swarm::AgentType;

use super::{AgentSnapshot, SharedWebState, SwarmSnapshot};

/// How often to refresh the web state (seconds).
const POLL_INTERVAL_SECS: u64 = 3;

/// Pane scrollback lines to capture.
const SCROLLBACK_LINES: u32 = 200;

/// Entry point: run forever, updating `state` every `POLL_INTERVAL_SECS`.
pub async fn run(state: SharedWebState) {
    let transport = TransportServerTransport::new(None);
    loop {
        match collect_swarms(&transport).await {
            Ok(snapshots) => {
                if let Ok(mut guard) = state.write() {
                    *guard = snapshots;
                }
            }
            Err(e) => {
                tracing::warn!("Web discovery error: {e:#}");
            }
        }
        sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
    }
}

// Type alias to avoid collision with the local `transport` variable name.
type TransportServerTransport = ServerTransport;

/// Discover all active agent swarms from tmux sessions.
async fn collect_swarms(transport: &ServerTransport) -> anyhow::Result<Vec<SwarmSnapshot>> {
    let session_names = tmux_session::discover_agent_sessions(transport).await?;

    let mut swarms = Vec::new();
    for session_name in &session_names {
        match build_swarm_snapshot(transport, session_name).await {
            Ok(Some(snap)) => swarms.push(snap),
            Ok(None) => {}
            Err(e) => {
                tracing::debug!("Skipping session {session_name}: {e:#}");
            }
        }
    }
    Ok(swarms)
}

/// Build a `SwarmSnapshot` for a single tmux session.
/// Returns `None` if the session doesn't look like an agent swarm.
async fn build_swarm_snapshot(
    transport: &ServerTransport,
    session_name: &str,
) -> anyhow::Result<Option<SwarmSnapshot>> {
    // Parse prefix → agent type, project name
    let Some((agent_type, project_name)) = parse_session_name(session_name) else {
        return Ok(None);
    };

    // List panes in this session
    let session_info = tmux_session::list_panes(transport, session_name).await?;
    if session_info.windows.is_empty() {
        return Ok(None);
    }

    // Try to find the repo path from the tmux environment
    let repo_path = find_repo_path(transport, session_name, &project_name).await;
    let repo_path_str = repo_path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();

    // Collect agent snapshots from all panes.
    //
    // Roles are decided by WINDOW NAME, never by index. The launcher builds
    // `-n agents` for workers and `-n review` for the manager
    // (start-parallel-agents.sh), and addresses both by window id precisely
    // because indices are not dependable. Keying on `window.index == 0 &&
    // pane.index == 0` here was worse than fragile: under `base-index 1` /
    // `pane-base-index 1` -- a common tmux setting -- no window or pane has
    // index 0 at all, so no pane was ever the manager, and every swarm was
    // dropped by the `None` arm below. The dashboard reported zero swarms on
    // any such machine.
    //
    // Worker ordinals are positional (1-based) for the same reason: raw
    // `pane_index` yields `worker-2..worker-4` under pane-base-index 1, and
    // looks up `<project>-wt-0` under pane-base-index 0. Position is right
    // under both.
    let mut manager: Option<AgentSnapshot> = None;
    let mut workers: Vec<AgentSnapshot> = Vec::new();
    let mut ordinal: u32 = 0;

    for window in &session_info.windows {
        let is_manager_window = window.name.eq_ignore_ascii_case("review");
        for pane in &window.panes {
            // Only the first pane of the review window is the manager; if it
            // were ever split, the rest are still workers.
            let is_manager = is_manager_window && manager.is_none();
            if !is_manager {
                ordinal += 1;
            }

            let snap = build_agent_snapshot(
                transport,
                &pane.target,
                is_manager,
                ordinal,
                &project_name,
                repo_path.as_deref(),
                &agent_type,
            )
            .await;

            if is_manager {
                manager = Some(snap);
            } else {
                workers.push(snap);
            }
        }
    }

    // A session with workers but no review window is a real, running swarm --
    // it just has no manager attached. Returning None here made it vanish from
    // the dashboard with no explanation anywhere. Report it instead, with a
    // placeholder manager that says what is missing.
    let manager = match manager {
        Some(m) => m,
        None => {
            if workers.is_empty() {
                tracing::warn!(
                    "Session {session_name} has no panes to report; skipping"
                );
                return Ok(None);
            }
            tracing::warn!(
                "Session {session_name} has no 'review' window; reporting {} worker(s) with no manager",
                workers.len()
            );
            missing_manager_snapshot(&project_name)
        }
    };

    let busy_count = workers.iter().filter(|w| is_busy(&w.state)).count();
    let idle_count = workers.iter().filter(|w| !is_busy(&w.state)).count();
    let attention_count = workers.iter().filter(|w| w.waiting_for_input).count();

    Ok(Some(SwarmSnapshot {
        project_name,
        repo_path: repo_path_str,
        agent_type: agent_type.to_string(),
        workflow: None,
        tmux_session: session_name.to_string(),
        stopped: false,
        busy_count,
        idle_count,
        attention_count,
        manager,
        workers,
        issues: Vec::new(), // populated by TUI path; standalone discovery doesn't fetch issues
    }))
}

/// Build a single agent snapshot from a tmux pane.
/// Build one agent's snapshot.
///
/// `is_manager` and `ordinal` are decided by the caller from window names and
/// pane position. They are deliberately not derived from tmux indices here --
/// see the note in `build_swarm_snapshot`. `ordinal` is 1-based and ignored
/// when `is_manager` is true.
/// Stand-in manager for a session that has workers but no `review` window.
///
/// It carries no tmux target, so the input and pane endpoints cannot be
/// pointed at a pane that does not exist.
fn missing_manager_snapshot(project_name: &str) -> AgentSnapshot {
    AgentSnapshot {
        id: format!("{project_name}/manager"),
        role: "manager".to_string(),
        state: "Missing".to_string(),
        is_manager: true,
        waiting_for_input: false,
        current_issue: None,
        current_issue_title: None,
        pane_content: "No 'review' window in this tmux session -- \
this swarm is running without a manager."
            .to_string(),
        tmux_target: String::new(),
        health: "Unknown".to_string(),
        completed_issue_count: 0,
        resurrection_attempts: 0,
        status_timestamp: None,
    }
}

async fn build_agent_snapshot(
    transport: &ServerTransport,
    target: &str,
    is_manager: bool,
    ordinal: u32,
    project_name: &str,
    repo_path: Option<&std::path::Path>,
    agent_type: &AgentType,
) -> AgentSnapshot {

    // Capture pane content
    let pane_content = capture_pane(transport, target, SCROLLBACK_LINES)
        .await
        .unwrap_or_default();

    // Determine role label
    let role = if is_manager {
        "manager".to_string()
    } else {
        format!("worker-{ordinal}")
    };

    // Derive worktree path for status file lookup
    let worktree_path: Option<PathBuf> = repo_path.and_then(|base| {
        if is_manager {
            Some(base.to_path_buf())
        } else {
            // Workers live in <parent>/<project>-wt-<N>
            let parent = base.parent()?;
            let wt_name = format!("{project_name}-wt-{ordinal}");
            let p = parent.join(&wt_name);
            if p.exists() { Some(p) } else { None }
        }
    });

    // Read status file if available
    let (state_str, current_issue) = if let Some(ref wt) = worktree_path {
        let status_path = wt
            .join(agent_type.status_dir())
            .join("fix-loop.status");
        if status_path.exists() {
            let status = read_status_file(&status_path);
            let issue = match &status.state {
                AgentState::Working { issue } => *issue,
                _ => None,
            };
            (status.state.to_string(), issue)
        } else {
            infer_state_from_pane(&pane_content)
        }
    } else {
        infer_state_from_pane(&pane_content)
    };

    // Detect if waiting for input (simple heuristic: ends with a prompt character)
    let waiting_for_input = pane_content.trim_end().ends_with('?')
        || pane_content.contains("Do you want to")
        || pane_content.contains("Press Enter")
        || pane_content.contains("(y/n)");

    AgentSnapshot {
        id: format!("{project_name}/{role}"),
        role,
        state: state_str,
        is_manager,
        waiting_for_input,
        current_issue,
        current_issue_title: None,
        pane_content,
        tmux_target: target.to_string(),
        health: "Healthy".to_string(),
        completed_issue_count: 0,
        resurrection_attempts: 0,
        status_timestamp: None,
    }
}

/// Infer agent state from pane content when no status file is available.
fn infer_state_from_pane(content: &str) -> (String, Option<u32>) {
    let trimmed = content.trim_end();
    // Look for "Working #NNN" pattern
    if let Some(idx) = trimmed.rfind("Working #") {
        let rest = &trimmed[idx + "Working #".len()..];
        let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = num.parse::<u32>() {
            return (format!("Working #{n}"), Some(n));
        }
    }
    if trimmed.contains("Idle") || trimmed.ends_with("$ ") || trimmed.ends_with("% ") {
        return ("Idle".to_string(), None);
    }
    ("Unknown".to_string(), None)
}

/// Returns true if the state string looks like active work.
fn is_busy(state: &str) -> bool {
    state.starts_with("Working")
}

/// Parse `claude-myrepo` → `(AgentType::Claude, "myrepo")`.
fn parse_session_name(name: &str) -> Option<(AgentType, String)> {
    if let Some(rest) = name.strip_prefix("claude-") {
        Some((AgentType::Claude, rest.to_string()))
    } else if let Some(rest) = name.strip_prefix("codex-") {
        Some((AgentType::Codex, rest.to_string()))
    } else if let Some(rest) = name.strip_prefix("droid-") {
        Some((AgentType::Droid, rest.to_string()))
    } else if let Some(rest) = name.strip_prefix("gemini-") {
        Some((AgentType::Gemini, rest.to_string()))
    } else {
        None
    }
}

/// Find the repo path for a session via tmux environment or filesystem heuristics.
async fn find_repo_path(
    transport: &ServerTransport,
    session_name: &str,
    project_name: &str,
) -> Option<PathBuf> {
    // Try tmux PWD environment variable first
    if let Ok(output) = transport
        .output(
            "tmux",
            &[
                "show-environment".to_string(),
                "-t".to_string(),
                session_name.to_string(),
                "PWD".to_string(),
            ],
            None,
        )
        .await
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(path_str) = stdout.trim().strip_prefix("PWD=") {
                let path = PathBuf::from(path_str);
                // Walk up to find the actual repo root (not a worktree)
                let base = strip_worktree_suffix(&path, project_name);
                if base.exists() {
                    return Some(base);
                }
            }
        }
    }

    // Fallback: look for <cwd>/<project> or siblings
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join(project_name);
        if candidate.exists() {
            return Some(candidate);
        }
        // cwd itself might be the project
        if cwd.file_name().map(|n| n.to_string_lossy().as_ref() == project_name).unwrap_or(false) {
            return Some(cwd);
        }
    }

    None
}

/// Strip `-wt-N` suffix so we get the base repo path from a worktree path.
fn strip_worktree_suffix(path: &std::path::Path, project_name: &str) -> PathBuf {
    if let Some(parent) = path.parent() {
        if let Some(name) = path.file_name() {
            let name_str = name.to_string_lossy();
            // e.g. "agents-ui-wt-1" → strip "-wt-1" → check parent/project_name
            if name_str.contains("-wt-") {
                let base = parent.join(project_name);
                if base.exists() {
                    return base;
                }
            }
        }
    }
    path.to_path_buf()
}
