use anyhow::Result;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::Html,
    response::sse::{Event, KeepAlive, Sse},
    routing::{delete, get, patch, post},
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use super::SharedWebState;
use crate::transport::ServerTransport;

/// The embedded single-page web UI.
const INDEX_HTML: &str = include_str!("ui.html");

/// Default port for the web server.
pub const DEFAULT_PORT: u16 = 7878;

/// Combined state for the web server: swarm snapshots + agents dir for launching.
#[derive(Clone)]
pub struct WebServerState {
    pub swarms: SharedWebState,
    pub agents_dir: PathBuf,
    /// Transport used for live tmux pane captures in the pane endpoint.
    pub transport: ServerTransport,
}

async fn index_handler() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn api_swarms_handler(State(state): State<WebServerState>) -> Json<Value> {
    let swarms = state
        .swarms
        .read()
        .map_err(|e| tracing::warn!("Web state lock poisoned: {e}"))
        .ok()
        .map(|s| s.clone())
        .unwrap_or_default();
    Json(json!({ "swarms": swarms }))
}

/// Returns the pane content for a specific agent, captured live from tmux.
/// `GET /api/swarms/:project/agents/:role/pane`
async fn api_pane_handler(
    Path((project, role)): Path<(String, String)>,
    State(state): State<WebServerState>,
) -> Result<Json<Value>, StatusCode> {
    // Look up the agent in the shared state to get its tmux target and cached data.
    let (tmux_target, cached_state, cached_content) = {
        let guard = state
            .swarms
            .read()
            .map_err(|e| {
                tracing::warn!("Web state lock poisoned: {e}");
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let swarm = guard
            .iter()
            .find(|s| s.project_name == project)
            .ok_or(StatusCode::NOT_FOUND)?;

        let agent = find_agent(swarm, &role).ok_or(StatusCode::NOT_FOUND)?;
        (agent.tmux_target.clone(), agent.state.clone(), agent.pane_content.clone())
    };

    // Do a live capture from tmux with ANSI escape codes (-e flag) to ensure
    // colors and styling are preserved in the web UI, plus the cursor position
    // so the page can draw a caret.
    let capture =
        match crate::tmux::proxy::capture_pane_with_cursor(&state.transport, &tmux_target, 500)
            .await
        {
            Ok(capture) => capture,
            Err(e) => {
                tracing::warn!(
                    "Live pane capture failed for {tmux_target}: {e}, using cached content"
                );
                crate::tmux::proxy::PaneCapture {
                    content: cached_content,
                    ..Default::default()
                }
            }
        };

    Ok(Json(pane_frame(&role, &tmux_target, &cached_state, &capture)))
}

/// The JSON shape shared by the pane endpoint and the pane stream, so a frame
/// pushed over SSE is interchangeable with one fetched over HTTP and the page
/// has a single code path for applying either.
fn pane_frame(
    role: &str,
    tmux_target: &str,
    agent_state: &str,
    capture: &crate::tmux::proxy::PaneCapture,
) -> Value {
    json!({
        "role": role,
        "tmux_target": tmux_target,
        "state": agent_state,
        "pane_content": capture.content,
        "cursor_x": capture.cursor_x,
        "cursor_y": capture.cursor_y,
        "pane_height": capture.pane_height,
    })
}

/// Body for the send-input endpoint.
#[derive(Debug, Deserialize, Serialize)]
pub struct SendInputBody {
    /// Text to send to the agent.
    pub text: String,
    /// Whether to append Enter. Defaults to true, so existing callers that
    /// send a whole line are unchanged.
    ///
    /// The session view sets this false to stream characters as they are
    /// typed. That is what makes the agent's own completion menu work: typing
    /// `/autocoder:` only offers completions if the agent receives the
    /// keystrokes one at a time, the way a terminal delivers them. Sending the
    /// finished line with Enter appended gives it no chance to offer anything.
    #[serde(default = "default_append_enter")]
    pub enter: bool,
}

fn default_append_enter() -> bool {
    true
}

/// Body for the send-key endpoint.
#[derive(Debug, Deserialize, Serialize)]
pub struct SendKeyBody {
    /// A tmux key name, e.g. "Up", "Enter", "Escape", "C-c".
    pub key: String,
}

/// Translate a requested key into the tmux key name to send, rejecting
/// anything not on the list.
///
/// The transport shell-quotes its arguments, so this is not guarding against
/// injection. It guards against a public endpoint being able to drive a tmux
/// pane with arbitrary key sequences: only keys a person could press while
/// looking at the pane are accepted.
pub fn tmux_key_name(requested: &str) -> Option<String> {
    // Navigation and editing keys, spelled the way tmux spells them.
    const NAMED: &[(&str, &str)] = &[
        ("up", "Up"),
        ("down", "Down"),
        ("left", "Left"),
        ("right", "Right"),
        ("enter", "Enter"),
        ("return", "Enter"),
        ("escape", "Escape"),
        ("esc", "Escape"),
        ("tab", "Tab"),
        ("backtab", "BTab"),
        ("shift-tab", "BTab"),
        ("backspace", "BSpace"),
        ("space", "Space"),
        ("home", "Home"),
        ("end", "End"),
        ("pageup", "PageUp"),
        ("pagedown", "PageDown"),
        ("delete", "DC"),
    ];

    let lower = requested.trim().to_ascii_lowercase();

    if let Some((_, tmux)) = NAMED.iter().find(|(name, _)| *name == lower) {
        return Some((*tmux).to_string());
    }

    // Ctrl chords: C-a through C-z only. Ctrl-c is the reason this endpoint
    // exists at all -- interrupting a wedged agent from the dashboard.
    if let Some(rest) = lower.strip_prefix("c-") {
        let mut chars = rest.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            if c.is_ascii_lowercase() {
                return Some(format!("C-{c}"));
            }
        }
        return None;
    }

    // Function keys F1-F12.
    if let Some(rest) = lower.strip_prefix('f') {
        if let Ok(n) = rest.parse::<u8>() {
            if (1..=12).contains(&n) {
                return Some(format!("F{n}"));
            }
        }
    }

    None
}

/// Send a single raw key to an agent's tmux pane, without appending Enter.
/// `POST /api/swarms/:project/agents/:role/key`
///
/// The text endpoint always appends Enter, which cannot drive an interactive
/// selection menu -- an agent sitting on an arrow-key picker (`/model`,
/// `/plugin`, a numbered prompt) could not be answered from the dashboard at
/// all.
async fn api_key_handler(
    Path((project, role)): Path<(String, String)>,
    State(state): State<WebServerState>,
    Json(body): Json<SendKeyBody>,
) -> Result<Json<Value>, StatusCode> {
    let Some(tmux_key) = tmux_key_name(&body.key) else {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    };

    let tmux_target = {
        let guard = state
            .swarms
            .read()
            .map_err(|e| {
                tracing::warn!("Web state lock poisoned: {e}");
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let swarm = guard
            .iter()
            .find(|s| s.project_name == project)
            .ok_or(StatusCode::NOT_FOUND)?;

        find_agent(swarm, &role)
            .ok_or(StatusCode::NOT_FOUND)?
            .tmux_target
            .clone()
    };

    crate::tmux::proxy::send_keys_no_enter(
        &crate::transport::ServerTransport::new(None),
        &tmux_target,
        &tmux_key,
    )
    .await
    .map_err(|e| {
        tracing::warn!("tmux send-keys failed for {tmux_target}: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(serde_json::json!({ "ok": true, "key": tmux_key })))
}

/// One item in an ordered batch of input.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum InputItem {
    /// A named key, validated through [`tmux_key_name`] exactly as the
    /// single-key endpoint validates its input.
    Key { key: String },
    /// Literal text, sent with `-l` and no Enter.
    Text { text: String },
}

/// Body for the batched send-keys endpoint.
#[derive(Debug, Deserialize, Serialize)]
pub struct SendKeysBatchBody {
    pub items: Vec<InputItem>,
}

/// Upper bound on a single batch. The page coalesces one animation frame of
/// input, which is a handful of keys; anything near this is a client bug or an
/// attempt to use the endpoint as an arbitrary-length input pump.
pub const MAX_BATCH_ITEMS: usize = 64;

/// Upper bound on the literal text in one batch, in bytes.
pub const MAX_BATCH_TEXT_BYTES: usize = 4096;

/// Resolve a batch into the ordered tmux sends it represents, rejecting the
/// whole batch if any part of it is invalid.
///
/// Validation is all-or-nothing on purpose. Sending the valid prefix and then
/// failing would leave the pane holding half a keystroke sequence with no way
/// for the caller to know where it stopped -- worse than sending nothing.
pub fn resolve_batch(items: &[InputItem]) -> Option<Vec<InputItem>> {
    if items.is_empty() || items.len() > MAX_BATCH_ITEMS {
        return None;
    }

    let text_bytes: usize = items
        .iter()
        .map(|item| match item {
            InputItem::Text { text } => text.len(),
            InputItem::Key { .. } => 0,
        })
        .sum();
    if text_bytes > MAX_BATCH_TEXT_BYTES {
        return None;
    }

    let mut resolved = Vec::with_capacity(items.len());
    for item in items {
        match item {
            // Every key goes through the same allowlist the single-key
            // endpoint uses. Batching must not become a way around it.
            InputItem::Key { key } => resolved.push(InputItem::Key {
                key: tmux_key_name(key)?,
            }),
            InputItem::Text { text } => {
                if text.is_empty() {
                    return None;
                }
                resolved.push(InputItem::Text { text: text.clone() });
            }
        }
    }
    Some(resolved)
}

/// Send an ordered batch of keys and literal text to an agent's pane.
/// `POST /api/swarms/:project/agents/:role/keys`
///
/// The single-key endpoint is fine for one press, but the page fires one
/// request per keystroke and nothing sequences them: two keys pressed in quick
/// succession are two in-flight requests that can land in either order, so
/// fast typing or a held arrow key arrives at the pane scrambled. This takes
/// the keys already in order and sends them in that order, awaiting each, so
/// the pane sees exactly what was typed.
async fn api_keys_batch_handler(
    Path((project, role)): Path<(String, String)>,
    State(state): State<WebServerState>,
    Json(body): Json<SendKeysBatchBody>,
) -> Result<Json<Value>, StatusCode> {
    let Some(resolved) = resolve_batch(&body.items) else {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    };

    let tmux_target = lookup_tmux_target(&state, &project, &role)?;

    let sent = resolved.len();
    for item in &resolved {
        let result = match item {
            InputItem::Key { key } => {
                crate::tmux::proxy::send_keys_no_enter(&state.transport, &tmux_target, key).await
            }
            InputItem::Text { text } => {
                crate::tmux::proxy::send_literal_ordered(&state.transport, &tmux_target, text).await
            }
        };
        result.map_err(|e| {
            tracing::warn!("tmux send-keys failed for {tmux_target}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

    Ok(Json(json!({ "ok": true, "sent": sent })))
}

/// How often the pane stream re-reads tmux. Fast enough that a completion
/// menu or a moving menu highlight is drawn while it is still on screen --
/// the 2s poll this replaces could miss such a frame entirely.
const STREAM_POLL: Duration = Duration::from_millis(120);

/// Stream pane updates as server-sent events.
/// `GET /api/swarms/:project/agents/:role/pane/stream`
///
/// Only changed frames are sent, so an idle pane costs one tmux read per tick
/// and no network traffic.
async fn api_pane_stream_handler(
    Path((project, role)): Path<(String, String)>,
    State(state): State<WebServerState>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    // Resolve once, up front, so an unknown agent is a 404 rather than a
    // stream that opens and then silently never yields anything.
    let tmux_target = lookup_tmux_target(&state, &project, &role)?;

    struct StreamState {
        state: WebServerState,
        project: String,
        role: String,
        tmux_target: String,
        last: Option<(crate::tmux::proxy::PaneCapture, String)>,
    }

    let stream = futures::stream::unfold(
        StreamState {
            state,
            project,
            role,
            tmux_target,
            last: None,
        },
        |mut st| async move {
            loop {
                tokio::time::sleep(STREAM_POLL).await;

                let capture = match crate::tmux::proxy::capture_pane_with_cursor(
                    &st.state.transport,
                    &st.tmux_target,
                    500,
                )
                .await
                {
                    Ok(capture) => capture,
                    // A failed read is usually a pane that is gone or briefly
                    // busy. Keep the stream open and try again -- tearing it
                    // down would drop the page back to polling for good.
                    Err(e) => {
                        tracing::debug!("Pane stream capture failed for {}: {e}", st.tmux_target);
                        continue;
                    }
                };

                let agent_state = current_agent_state(&st.state, &st.project, &st.role);

                if st.last.as_ref() == Some(&(capture.clone(), agent_state.clone())) {
                    continue;
                }

                let frame = pane_frame(&st.role, &st.tmux_target, &agent_state, &capture);
                st.last = Some((capture, agent_state));
                return Some((Ok(Event::default().data(frame.to_string())), st));
            }
        },
    );

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// Look up an agent's tmux target, mapping the failure modes to status codes.
fn lookup_tmux_target(
    state: &WebServerState,
    project: &str,
    role: &str,
) -> Result<String, StatusCode> {
    let guard = state
        .swarms
        .read()
        .map_err(|e| {
            tracing::warn!("Web state lock poisoned: {e}");
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let swarm = guard
        .iter()
        .find(|s| s.project_name == project)
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(find_agent(swarm, role)
        .ok_or(StatusCode::NOT_FOUND)?
        .tmux_target
        .clone())
}

/// The agent's last known state string, or empty if it has since disappeared.
fn current_agent_state(state: &WebServerState, project: &str, role: &str) -> String {
    state
        .swarms
        .read()
        .ok()
        .and_then(|guard| {
            guard
                .iter()
                .find(|s| s.project_name == project)
                .and_then(|swarm| find_agent(swarm, role))
                .map(|agent| agent.state.clone())
        })
        .unwrap_or_default()
}

/// Send a line of input to an agent's tmux pane.
/// `POST /api/swarms/:project/agents/:role/input`
async fn api_input_handler(
    Path((project, role)): Path<(String, String)>,
    State(state): State<WebServerState>,
    Json(body): Json<SendInputBody>,
) -> Result<Json<Value>, StatusCode> {
    // Look up the tmux target while holding only a read lock.
    let tmux_target = {
        let guard = state
            .swarms
            .read()
            .map_err(|e| {
                tracing::warn!("Web state lock poisoned: {e}");
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let swarm = guard
            .iter()
            .find(|s| s.project_name == project)
            .ok_or(StatusCode::NOT_FOUND)?;

        find_agent(swarm, &role)
            .ok_or(StatusCode::NOT_FOUND)?
            .tmux_target
            .clone()
    };

    // Reject empty input.
    if body.text.is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    if body.enter {
        crate::tmux::proxy::send_keys(
            &crate::transport::ServerTransport::new(None),
            &tmux_target,
            &body.text,
        )
        .await
    } else {
        // -l, so the text is taken literally. Without it tmux resolves an
        // argument that happens to match a key name -- a lone "Up", "Space"
        // or "Enter" typed by the user -- as that key instead of those
        // characters.
        crate::tmux::proxy::send_literal(&tmux_target, &body.text).await
    }
    .map_err(|e| {
        tracing::warn!("tmux send-keys failed for {tmux_target}: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({ "ok": true })))
}

/// Returns a list of available git repositories discovered from common directories.
/// `GET /api/repos`
async fn api_repos_handler() -> Json<Value> {
    let mut repos = Vec::new();

    // Directories to scan for git repos
    let mut scan_dirs: Vec<PathBuf> = Vec::new();

    if let Some(home) = dirs::home_dir() {
        scan_dirs.push(home.join("src"));
        scan_dirs.push(home.join("projects"));
        scan_dirs.push(home.join("code"));
    }

    // Also scan the parent of the current working directory
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(parent) = cwd.parent() {
            scan_dirs.push(parent.to_path_buf());
        }
    }

    for dir in scan_dirs {
        if !dir.exists() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join(".git").exists() {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if !name.is_empty() {
                    repos.push(json!({
                        "name": name,
                        "path": path.to_string_lossy(),
                    }));
                }
            }
        }
    }

    // Deduplicate by path
    repos.dedup_by(|a, b| a["path"] == b["path"]);

    Json(json!({ "repos": repos }))
}

/// Returns the list of supported agent types.
/// `GET /api/agent-types`
async fn api_agent_types_handler() -> Json<Value> {
    Json(json!({
        "agent_types": ["Claude", "Codex", "Droid", "Gemini"]
    }))
}

/// Body for the launch-swarm endpoint.
#[derive(Debug, Deserialize)]
pub struct LaunchSwarmBody {
    pub repo_path: String,
    pub agent_type: String,
    pub num_workers: u32,
}

/// Launch a new swarm.
/// `POST /api/swarms`
/// Returns 202 Accepted immediately; the swarm launches in a background task.
async fn api_launch_swarm_handler(
    State(state): State<WebServerState>,
    Json(body): Json<LaunchSwarmBody>,
) -> Result<Json<Value>, StatusCode> {
    use crate::adapter::claude::ClaudeAdapter;
    use crate::adapter::traits::SwarmConfig;
    use crate::model::swarm::AgentType;
    use crate::transport::ServerTransport;

    let repo_path = PathBuf::from(&body.repo_path);
    if !repo_path.exists() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let agent_type: AgentType = match body.agent_type.as_str() {
        "Claude" => AgentType::Claude,
        "Codex"  => AgentType::Codex,
        "Droid"  => AgentType::Droid,
        "Gemini" => AgentType::Gemini,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let project_name = repo_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".to_string());

    let config = SwarmConfig {
        repo_path,
        agent_type: agent_type.clone(),
        num_workers: body.num_workers,
        agents_dir: state.agents_dir.clone(),
        resume_seeds: None,
    };

    tokio::spawn(async move {
        let adapter = ClaudeAdapter::new(agent_type, ServerTransport::new(None));
        match adapter.launch_with_progress(&config, |msg| tracing::info!("{msg}")).await {
            Ok(_swarm) => tracing::info!("Swarm launched for {}", config.repo_path.display()),
            Err(e) => tracing::error!("Failed to launch swarm for {}: {e:#}", config.repo_path.display()),
        }
    });

    Ok(Json(json!({ "ok": true, "project_name": project_name })))
}

/// Stop all workers in a swarm.
/// `DELETE /api/swarms/:project`
/// Sends Ctrl+C to every worker pane. Returns 404 if the project is not found.
async fn api_stop_swarm_handler(
    Path(project): Path<String>,
    State(state): State<WebServerState>,
) -> Result<Json<Value>, StatusCode> {
    use crate::adapter::claude::ClaudeAdapter;
    use crate::adapter::traits::AgentRuntime;
    use crate::model::issue::IssueCache;
    use crate::model::status::{AgentState, AgentStatus};
    use crate::model::swarm::{AgentInfo, AgentType, Swarm, WorkerHealth};
    use crate::transport::ServerTransport;

    let snapshot = {
        let guard = state
            .swarms
            .read()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        guard
            .iter()
            .find(|s| s.project_name == project)
            .cloned()
            .ok_or(StatusCode::NOT_FOUND)?
    };

    let agent_type: AgentType = match snapshot.agent_type.as_str() {
        "Codex" => AgentType::Codex,
        "Droid" => AgentType::Droid,
        "Gemini" => AgentType::Gemini,
        _ => AgentType::Claude,
    };

    let repo_path = std::path::PathBuf::from(&snapshot.repo_path);

    // Rebuild enough of the Swarm to write handoffs and tear it down. Unlike
    // the add-worker path this keeps each agent's real worktree and pane
    // content: the handoff is largely made of them.
    let to_info = |a: &crate::web::AgentSnapshot| AgentInfo {
        id: format!("{}/{}", snapshot.project_name, a.role),
        role: a.role.clone(),
        branch: a.branch.clone(),
        worktree_path: if a.worktree_path.is_empty() {
            repo_path.clone()
        } else {
            std::path::PathBuf::from(&a.worktree_path)
        },
        tmux_target: a.tmux_target.clone(),
        status: AgentStatus { timestamp: None, state: AgentState::Unknown(a.state.clone()) },
        is_manager: a.is_manager,
        pane_content: a.pane_content.clone(),
        dispatched_issue: None,
        current_issue: a.current_issue,
        current_issue_title: a.current_issue_title.clone(),
        waiting_for_input: a.waiting_for_input,
        resurrection_attempts: a.resurrection_attempts,
        completed_issue_count: a.completed_issue_count,
        health: WorkerHealth::default(),
    };

    let swarm = Swarm {
        repo_path: repo_path.clone(),
        project_name: snapshot.project_name.clone(),
        agent_type: agent_type.clone(),
        workflow: None,
        tmux_session: snapshot.tmux_session.clone(),
        manager: to_info(&snapshot.manager),
        workers: snapshot.workers.iter().map(to_info).collect(),
        issue_cache: IssueCache::default(),
        stopped: false,
    };

    let agent_count = swarm.workers.len() + 1;

    // Persist enough to resume later even with nothing in memory — e.g. this
    // process restarting before anyone resumes it.
    let _ = crate::config::persistence::save_swarm_state(
        &swarm.project_name,
        &crate::config::persistence::SwarmState {
            repo_path: swarm.repo_path.to_string_lossy().into_owned(),
            agent_type: swarm.agent_type.to_string(),
            tmux_session: swarm.tmux_session.clone(),
            workflow: swarm.workflow.as_ref().map(|w| w.to_string()),
            num_workers: swarm.workers.len() as u32,
        },
    );

    // Handoffs then teardown, off the request: writing a handoff per agent
    // runs git and gh in each worktree, which is far too slow to hold an HTTP
    // response open for.
    //
    // This previously sent C-c to the worker panes and returned. Ctrl-C
    // interrupts an agent's current turn rather than exiting it, the manager
    // was never signalled at all, and nothing marked the swarm stopped -- so
    // heal_workers would respawn whatever did die. The swarm stayed up.
    tokio::spawn(async move {
        let transport = ServerTransport::new(None);
        let branch = crate::handoff::integration_branch(&transport, &swarm.repo_path).await;

        for line in crate::handoff::write_all(&transport, &swarm, &branch).await {
            tracing::info!("handoff {}", line);
        }

        let adapter = ClaudeAdapter::new(agent_type, ServerTransport::new(None));
        match adapter.teardown(&swarm).await {
            Ok(()) => tracing::info!("Stopped swarm {}", swarm.project_name),
            Err(e) => tracing::error!("teardown failed for {}: {e:#}", swarm.project_name),
        }
    });

    Ok(Json(json!({ "ok": true, "stopping": agent_count })))
}

/// Add one worker to a running swarm.
/// `POST /api/swarms/:project/workers`
/// Returns 404 if the project is not found, 202 Accepted on success.
/// Optional body for the add-worker endpoint to override the agent type.
#[derive(Debug, Deserialize, Default)]
pub struct AddWorkerBody {
    /// Agent type override (e.g. "Claude", "Codex", "Droid", "Gemini").
    /// If omitted, the swarm's current agent type is used.
    pub agent_type: Option<String>,
}

async fn api_add_worker_handler(
    Path(project): Path<String>,
    State(state): State<WebServerState>,
    body: Option<Json<AddWorkerBody>>,
) -> Result<Json<Value>, StatusCode> {
    use crate::adapter::claude::ClaudeAdapter;
    use crate::adapter::traits::AgentRuntime;
    use crate::model::issue::IssueCache;
    use crate::model::status::{AgentState, AgentStatus};
    use crate::model::swarm::{AgentInfo, AgentType, Swarm, WorkerHealth};
    use crate::transport::ServerTransport;

    let snapshot = {
        let guard = state
            .swarms
            .read()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        guard
            .iter()
            .find(|s| s.project_name == project)
            .cloned()
            .ok_or(StatusCode::NOT_FOUND)?
    };

    // Use the body's agent_type override if provided, otherwise fall back to the swarm's type.
    let type_str = body
        .and_then(|b| b.0.agent_type)
        .unwrap_or_else(|| snapshot.agent_type.clone());
    let agent_type: AgentType = match type_str.as_str() {
        "Claude" => AgentType::Claude,
        "Codex"  => AgentType::Codex,
        "Droid"  => AgentType::Droid,
        "Gemini" => AgentType::Gemini,
        _ => AgentType::Claude,
    };

    // Build a minimal Swarm from the snapshot so we can call add_worker.
    let repo_path = std::path::PathBuf::from(&snapshot.repo_path);
    let idle_status = AgentStatus { timestamp: None, state: AgentState::Idle };
    let manager_info = AgentInfo {
        id: format!("{}/manager", snapshot.project_name),
        role: "manager".to_string(),
        branch: snapshot.manager.branch.clone(),
        worktree_path: repo_path.clone(),
        tmux_target: snapshot.manager.tmux_target.clone(),
        status: idle_status.clone(),
        is_manager: true,
        pane_content: String::new(),
        dispatched_issue: None,
        current_issue: None,
        current_issue_title: None,
        waiting_for_input: false,
        resurrection_attempts: 0,
        completed_issue_count: 0,
        health: WorkerHealth::default(),
    };
    let workers: Vec<AgentInfo> = snapshot
        .workers
        .iter()
        .map(|w| AgentInfo {
            id: format!("{}/{}", snapshot.project_name, w.role),
            role: w.role.clone(),
            branch: w.branch.clone(),
            worktree_path: repo_path.clone(),
            tmux_target: w.tmux_target.clone(),
            status: idle_status.clone(),
            is_manager: false,
            pane_content: String::new(),
            dispatched_issue: None,
            current_issue: None,
            current_issue_title: None,
            waiting_for_input: false,
            resurrection_attempts: 0,
            completed_issue_count: 0,
            health: WorkerHealth::default(),
        })
        .collect();

    let swarm = Swarm {
        repo_path,
        project_name: snapshot.project_name.clone(),
        agent_type: agent_type.clone(),
        workflow: None,
        tmux_session: snapshot.tmux_session.clone(),
        manager: manager_info,
        workers,
        issue_cache: IssueCache::default(),
        stopped: false,
    };

    tokio::spawn(async move {
        let adapter = ClaudeAdapter::new(agent_type, ServerTransport::new(None));
        match adapter.add_worker(&swarm).await {
            Ok(info) => tracing::info!("Added worker {} to {}", info.role, swarm.project_name),
            Err(e) => tracing::error!("add_worker failed for {}: {e:#}", swarm.project_name),
        }
    });

    Ok(Json(json!({ "ok": true })))
}

/// Switch the agent runtime for a running swarm.
/// `PATCH /api/swarms/:project`  body: `{"agent_type": "Gemini"}`
async fn api_switch_agent_handler(
    Path(project): Path<String>,
    State(state): State<WebServerState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<Value>, StatusCode> {
    use crate::adapter::claude::ClaudeAdapter;
    use crate::adapter::traits::AgentRuntime;
    use crate::model::issue::IssueCache;
    use crate::model::status::{AgentState, AgentStatus};
    use crate::model::swarm::{AgentInfo, AgentType, Swarm, WorkerHealth};
    use crate::transport::ServerTransport;

    let new_type_str = body
        .get("agent_type")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?
        .to_string();

    let new_runtime: AgentType = match new_type_str.as_str() {
        "Claude" => AgentType::Claude,
        "Codex"  => AgentType::Codex,
        "Droid"  => AgentType::Droid,
        "Gemini" => AgentType::Gemini,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let snapshot = {
        let guard = state
            .swarms
            .read()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        guard
            .iter()
            .find(|s| s.project_name == project)
            .cloned()
            .ok_or(StatusCode::NOT_FOUND)?
    };

    let current_runtime: AgentType = match snapshot.agent_type.as_str() {
        "Claude" => AgentType::Claude,
        "Codex"  => AgentType::Codex,
        "Droid"  => AgentType::Droid,
        "Gemini" => AgentType::Gemini,
        _ => AgentType::Claude,
    };

    let repo_path = std::path::PathBuf::from(&snapshot.repo_path);
    let idle_status = AgentStatus { timestamp: None, state: AgentState::Idle };
    let manager_info = AgentInfo {
        id: format!("{}/manager", snapshot.project_name),
        role: "manager".to_string(),
        branch: snapshot.manager.branch.clone(),
        worktree_path: repo_path.clone(),
        tmux_target: snapshot.manager.tmux_target.clone(),
        status: idle_status.clone(),
        is_manager: true,
        pane_content: String::new(),
        dispatched_issue: None,
        current_issue: None,
        current_issue_title: None,
        waiting_for_input: false,
        resurrection_attempts: 0,
        completed_issue_count: 0,
        health: WorkerHealth::default(),
    };
    let workers: Vec<AgentInfo> = snapshot
        .workers
        .iter()
        .map(|w| AgentInfo {
            id: format!("{}/{}", snapshot.project_name, w.role),
            role: w.role.clone(),
            branch: w.branch.clone(),
            worktree_path: repo_path.clone(),
            tmux_target: w.tmux_target.clone(),
            status: idle_status.clone(),
            is_manager: false,
            pane_content: String::new(),
            dispatched_issue: None,
            current_issue: None,
            current_issue_title: None,
            waiting_for_input: false,
            resurrection_attempts: 0,
            completed_issue_count: 0,
            health: WorkerHealth::default(),
        })
        .collect();

    let mut swarm = Swarm {
        repo_path,
        project_name: snapshot.project_name.clone(),
        agent_type: current_runtime.clone(),
        workflow: None,
        tmux_session: snapshot.tmux_session.clone(),
        manager: manager_info,
        workers,
        issue_cache: IssueCache::default(),
        stopped: false,
    };

    tokio::spawn(async move {
        let adapter = ClaudeAdapter::new(current_runtime, ServerTransport::new(None));
        match adapter.switch_agent(&mut swarm, new_runtime).await {
            Ok(()) => tracing::info!("Switched {} to {new_type_str}", swarm.project_name),
            Err(e) => tracing::error!("switch_agent failed for {}: {e:#}", swarm.project_name),
        }
    });

    Ok(Json(json!({ "ok": true })))
}

/// Resume a stopped swarm.
/// `POST /api/swarms/:project/resume`
/// Returns immediately; the swarm resumes in a background task (worktree/tmux
/// setup and handoff seeding are too slow to hold a response open for).
/// Returns 404 if the project is not known to this process at all — neither
/// live nor previously stopped-and-persisted.
async fn api_resume_swarm_handler(
    Path(project): Path<String>,
    State(state): State<WebServerState>,
) -> Result<Json<Value>, StatusCode> {
    use crate::adapter::claude::ClaudeAdapter;
    use crate::adapter::traits::AgentRuntime;
    use crate::model::swarm::AgentType;
    use crate::transport::ServerTransport;

    // Prefer the live snapshot (this process saw the swarm before it was
    // stopped); otherwise fall back to what was persisted at stop time, so
    // this also works with nothing in memory, e.g. after this process itself
    // restarted.
    let live = state
        .swarms
        .read()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .iter()
        .find(|s| s.project_name == project)
        .map(|s| (s.repo_path.clone(), s.agent_type.clone()));

    let (repo_path, agent_type_name) = match live {
        Some(found) => found,
        None => {
            let saved = crate::config::persistence::load_swarm_state(&project)
                .ok()
                .flatten()
                .ok_or(StatusCode::NOT_FOUND)?;
            (saved.repo_path, saved.agent_type)
        }
    };

    let agent_type: AgentType = match agent_type_name.as_str() {
        "Codex" => AgentType::Codex,
        "Droid" => AgentType::Droid,
        "Gemini" => AgentType::Gemini,
        _ => AgentType::Claude,
    };
    let repo_path = std::path::PathBuf::from(repo_path);
    if !repo_path.exists() {
        return Err(StatusCode::BAD_REQUEST);
    }

    tokio::spawn(async move {
        let adapter = ClaudeAdapter::new(agent_type.clone(), ServerTransport::new(None));
        match adapter.resume(&repo_path, &agent_type).await {
            Ok((_swarm, notes)) => {
                tracing::info!("Resumed swarm {project}: {}", notes.join("; "));
            }
            Err(e) => tracing::error!("Failed to resume swarm {project}: {e:#}"),
        }
    });

    Ok(Json(json!({ "ok": true })))
}

/// Find an agent (manager or worker) by role within a swarm snapshot.
fn find_agent<'a>(
    swarm: &'a super::SwarmSnapshot,
    role: &str,
) -> Option<&'a super::AgentSnapshot> {
    if swarm.manager.role == role {
        return Some(&swarm.manager);
    }
    swarm.workers.iter().find(|w| w.role == role)
}

/// Start the web server on the given port, sharing `state` with the TUI app.
/// This function runs until the server shuts down or an error occurs.
pub async fn run(port: u16, state: WebServerState) -> Result<()> {
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/swarms", get(api_swarms_handler).post(api_launch_swarm_handler))
        .route("/api/repos", get(api_repos_handler))
        .route("/api/agent-types", get(api_agent_types_handler))
        .route(
            "/api/swarms/{project}",
            delete(api_stop_swarm_handler).patch(api_switch_agent_handler),
        )
        .route(
            "/api/swarms/{project}/resume",
            post(api_resume_swarm_handler),
        )
        .route(
            "/api/swarms/{project}/workers",
            post(api_add_worker_handler),
        )
        .route(
            "/api/swarms/{project}/agents/{role}/pane",
            get(api_pane_handler),
        )
        .route(
            "/api/swarms/{project}/agents/{role}/input",
            post(api_input_handler),
        )
        .route(
            "/api/swarms/{project}/agents/{role}/key",
            post(api_key_handler),
        )
        .route(
            "/api/swarms/{project}/agents/{role}/keys",
            post(api_keys_batch_handler),
        )
        .route(
            "/api/swarms/{project}/agents/{role}/pane/stream",
            get(api_pane_stream_handler),
        )
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("Web UI listening on http://{addr}");
    eprintln!("agents-tui: web UI available at http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::{AgentSnapshot, SwarmSnapshot, new_shared_state};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt; // for .oneshot()

    fn make_web_state(shared: SharedWebState) -> WebServerState {
        WebServerState {
            swarms: shared,
            agents_dir: PathBuf::from("/tmp/test-agents"),
            transport: ServerTransport::default(),
        }
    }

    fn make_app(state: WebServerState) -> Router {
        Router::new()
            .route("/", get(index_handler))
            .route("/api/swarms", get(api_swarms_handler).post(api_launch_swarm_handler))
            .route("/api/repos", get(api_repos_handler))
            .route("/api/agent-types", get(api_agent_types_handler))
            .route("/api/swarms/{project}", delete(api_stop_swarm_handler))
            .route(
                "/api/swarms/{project}/resume",
                post(api_resume_swarm_handler),
            )
            .route("/api/swarms/{project}/workers", post(api_add_worker_handler))
            .route(
                "/api/swarms/{project}/agents/{role}/pane",
                get(api_pane_handler),
            )
            .route(
                "/api/swarms/{project}/agents/{role}/input",
                post(api_input_handler),
            )
            .route(
                "/api/swarms/{project}/agents/{role}/key",
                post(api_key_handler),
            )
            .route(
                "/api/swarms/{project}/agents/{role}/keys",
                post(api_keys_batch_handler),
            )
            .with_state(state)
    }

    fn sample_agent(role: &str, is_manager: bool) -> AgentSnapshot {
        AgentSnapshot {
            id: format!("test/{role}"),
            role: role.to_string(),
            state: if is_manager { "Working #1".to_string() } else { "Idle".to_string() },
            is_manager,
            waiting_for_input: false,
            current_issue: None,
            current_issue_title: None,
            pane_content: format!("Pane output for {role}"),
            tmux_target: format!("claude-test:0.{}", if is_manager { 0 } else { 1 }),
            health: "Healthy".to_string(),
            completed_issue_count: 0,
            resurrection_attempts: 0,
            status_timestamp: None,
            worktree_path: if is_manager {
                "/repos/test".to_string()
            } else {
                format!("/repos/test-wt-1")
            },
            branch: Some(if is_manager { "master".to_string() } else { role.to_string() }),
        }
    }

    fn sample_swarm(name: &str) -> SwarmSnapshot {
        SwarmSnapshot {
            project_name: name.to_string(),
            repo_path: format!("/repos/{name}"),
            agent_type: "Claude".to_string(),
            workflow: None,
            tmux_session: format!("claude-{name}"),
            stopped: false,
            busy_count: 2,
            idle_count: 1,
            attention_count: 0,
            manager: sample_agent("manager", true),
            workers: vec![sample_agent("worker-1", false)],
            issues: Vec::new(),
        }
    }

    #[tokio::test]
    async fn index_returns_html() {
        let state = make_web_state(new_shared_state());
        let app = make_app(state);
        let req = Request::builder()
            .uri("/")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let headers = resp.headers();
        let content_type = headers.get("content-type").unwrap().to_str().unwrap();
        assert!(content_type.contains("text/html"));
    }

    #[tokio::test]
    async fn api_swarms_empty_when_no_swarms() {
        let state = make_web_state(new_shared_state());
        let app = make_app(state);
        let req = Request::builder()
            .uri("/api/swarms")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["swarms"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn api_swarms_returns_populated_state() {
        let shared = new_shared_state();
        {
            let mut guard = shared.write().unwrap();
            guard.push(sample_swarm("my-project"));
        }

        let state = make_web_state(shared);
        let app = make_app(state);
        let req = Request::builder()
            .uri("/api/swarms")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let swarms = json["swarms"].as_array().unwrap();
        assert_eq!(swarms.len(), 1);
        assert_eq!(swarms[0]["project_name"], "my-project");
        assert_eq!(swarms[0]["agent_type"], "Claude");
        assert_eq!(swarms[0]["busy_count"], 2);
    }

    #[tokio::test]
    async fn api_pane_returns_agent_content() {
        let shared = new_shared_state();
        {
            let mut guard = shared.write().unwrap();
            guard.push(sample_swarm("test-proj"));
        }

        let state = make_web_state(shared);
        let app = make_app(state);
        let req = Request::builder()
            .uri("/api/swarms/test-proj/agents/manager/pane")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["role"], "manager");
        assert!(json["pane_content"].as_str().unwrap().contains("manager"));
    }

    #[tokio::test]
    async fn api_pane_returns_worker_content() {
        let shared = new_shared_state();
        {
            let mut guard = shared.write().unwrap();
            guard.push(sample_swarm("test-proj"));
        }

        let state = make_web_state(shared);
        let app = make_app(state);
        let req = Request::builder()
            .uri("/api/swarms/test-proj/agents/worker-1/pane")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["role"], "worker-1");
    }

    #[tokio::test]
    async fn api_pane_returns_404_for_unknown_swarm() {
        let state = make_web_state(new_shared_state());
        let app = make_app(state);
        let req = Request::builder()
            .uri("/api/swarms/ghost-project/agents/manager/pane")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn api_pane_returns_404_for_unknown_agent() {
        let shared = new_shared_state();
        {
            let mut guard = shared.write().unwrap();
            guard.push(sample_swarm("test-proj"));
        }
        let state = make_web_state(shared);
        let app = make_app(state);
        let req = Request::builder()
            .uri("/api/swarms/test-proj/agents/tester/pane")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn api_input_rejects_empty_text() {
        let shared = new_shared_state();
        {
            let mut guard = shared.write().unwrap();
            guard.push(sample_swarm("test-proj"));
        }
        let state = make_web_state(shared);
        let app = make_app(state);
        let req = Request::builder()
            .method("POST")
            .uri("/api/swarms/test-proj/agents/manager/input")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"text":""}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn index_html_contains_expected_content() {
        let body = INDEX_HTML;
        assert!(body.contains("Agents UI"));
        assert!(body.contains("/api/swarms"));
        assert!(body.contains("viewport"));
    }

    #[tokio::test]
    async fn index_html_contains_session_view() {
        let body = INDEX_HTML;
        assert!(body.contains("/pane"));
        assert!(body.contains("/input"));
    }

    // The page must drive the pane through the sequenced queue and the live
    // stream, not the per-keystroke fetch and 2s poll they replaced.
    #[tokio::test]
    async fn index_html_wires_the_sequenced_input_queue() {
        let body = INDEX_HTML;
        assert!(body.contains("/keys"), "batch endpoint not called");
        assert!(body.contains("queueInput"));
        assert!(body.contains("flushInputQueue"));
        assert!(
            body.contains("requestAnimationFrame"),
            "input should be coalesced per animation frame"
        );
    }

    #[tokio::test]
    async fn index_html_wires_the_live_pane_stream() {
        let body = INDEX_HTML;
        assert!(body.contains("EventSource"));
        assert!(body.contains("/pane/stream"));
        assert!(body.contains("startSessionStream"));
        assert!(body.contains("stopSessionStream"));
    }

    #[tokio::test]
    async fn index_html_renders_a_cursor() {
        let body = INDEX_HTML;
        assert!(body.contains("pane-cursor"), "no caret element");
        assert!(body.contains("paneCursorTarget"));
        assert!(body.contains("cellWidth"), "caret must count cells, not chars");
    }

    #[tokio::test]
    async fn index_html_contains_hash_routing() {
        let body = INDEX_HTML;
        assert!(body.contains("hashchange"));
        assert!(body.contains("applyRoute"));
        assert!(body.contains("location.hash"));
    }

    #[tokio::test]
    async fn api_agent_types_returns_list() {
        let state = make_web_state(new_shared_state());
        let app = make_app(state);
        let req = Request::builder()
            .uri("/api/agent-types")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let types = json["agent_types"].as_array().unwrap();
        assert!(types.iter().any(|t| t == "Claude"));
        assert!(types.iter().any(|t| t == "Codex"));
    }

    #[tokio::test]
    async fn api_repos_returns_json() {
        let state = make_web_state(new_shared_state());
        let app = make_app(state);
        let req = Request::builder()
            .uri("/api/repos")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["repos"].is_array());
    }

    #[tokio::test]
    async fn api_launch_swarm_rejects_missing_path() {
        let state = make_web_state(new_shared_state());
        let app = make_app(state);
        let req = Request::builder()
            .method("POST")
            .uri("/api/swarms")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"repo_path":"/nonexistent/path/xyz","agent_type":"Claude","num_workers":3}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn api_launch_swarm_rejects_invalid_agent_type() {
        let state = make_web_state(new_shared_state());
        let app = make_app(state);
        let req = Request::builder()
            .method("POST")
            .uri("/api/swarms")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"repo_path":"/tmp","agent_type":"InvalidAgent","num_workers":3}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn api_stop_swarm_returns_ok_for_known_project() {
        let shared = new_shared_state();
        {
            let mut guard = shared.write().unwrap();
            guard.push(sample_swarm("my-proj"));
        }
        let app = make_app(make_web_state(shared));
        let req = Request::builder()
            .method("DELETE")
            .uri("/api/swarms/my-proj")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1024).await.unwrap())
                .unwrap();
        assert_eq!(body["ok"], true);
    }

    #[tokio::test]
    async fn api_stop_swarm_returns_404_for_unknown_project() {
        let app = make_app(make_web_state(new_shared_state()));
        let req = Request::builder()
            .method("DELETE")
            .uri("/api/swarms/ghost")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn api_resume_swarm_returns_404_for_unknown_project() {
        let app = make_app(make_web_state(new_shared_state()));
        let req = Request::builder()
            .method("POST")
            .uri("/api/swarms/ghost/resume")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn api_resume_swarm_returns_ok_for_a_project_known_from_persisted_state() {
        // Nothing live in memory -- only a `swarm.toml` from a previous stop,
        // as would be the case after this process itself restarted. A unique
        // project name keeps this independent of other tests touching the
        // shared config dir.
        let project = format!("resume-endpoint-test-{}", std::process::id());
        let repo_path = std::env::temp_dir().join(format!("agents-ui-{project}-repo"));
        std::fs::create_dir_all(&repo_path).unwrap();

        crate::config::persistence::save_swarm_state(
            &project,
            &crate::config::persistence::SwarmState {
                repo_path: repo_path.to_string_lossy().into_owned(),
                agent_type: "Claude".to_string(),
                tmux_session: format!("claude-{project}"),
                workflow: None,
                num_workers: 1,
            },
        )
        .unwrap();

        let app = make_app(make_web_state(new_shared_state()));
        let req = Request::builder()
            .method("POST")
            .uri(format!("/api/swarms/{project}/resume"))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let dir = crate::config::persistence::config_dir()
            .join("swarms")
            .join(&project);
        std::fs::remove_dir_all(dir).ok();
        std::fs::remove_dir_all(repo_path).ok();
    }

    #[tokio::test]
    async fn api_add_worker_returns_404_for_unknown_project() {
        let app = make_app(make_web_state(new_shared_state()));
        let req = Request::builder()
            .method("POST")
            .uri("/api/swarms/ghost/workers")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn tmux_key_name_accepts_navigation_keys() {
        for (input, expected) in [
            ("Up", "Up"),
            ("down", "Down"),
            ("Enter", "Enter"),
            ("return", "Enter"),
            ("Escape", "Escape"),
            ("esc", "Escape"),
            ("shift-tab", "BTab"),
            ("backspace", "BSpace"),
            ("delete", "DC"),
            ("pagedown", "PageDown"),
        ] {
            assert_eq!(
                super::tmux_key_name(input).as_deref(),
                Some(expected),
                "key {input}"
            );
        }
    }

    #[test]
    fn tmux_key_name_accepts_ctrl_chords_and_function_keys() {
        assert_eq!(super::tmux_key_name("C-c").as_deref(), Some("C-c"));
        assert_eq!(super::tmux_key_name("c-a").as_deref(), Some("C-a"));
        assert_eq!(super::tmux_key_name("F5").as_deref(), Some("F5"));
        assert_eq!(super::tmux_key_name("f12").as_deref(), Some("F12"));
    }

    #[test]
    fn tmux_key_name_rejects_anything_off_the_list() {
        for bad in [
            "",
            "   ",
            "C-",           // no chord letter
            "C-cc",         // more than one letter
            "C-1",          // not a lowercase letter
            "F0",           // out of range
            "F13",          // out of range
            "kill-session", // a tmux command, not a key
            "Up Enter",     // no multi-key sequences
            "send-keys",
        ] {
            assert!(
                super::tmux_key_name(bad).is_none(),
                "should have rejected {bad:?}"
            );
        }
    }

    #[tokio::test]
    async fn key_endpoint_rejects_a_key_that_is_not_allowed() {
        let shared = new_shared_state();
        {
            let mut guard = shared.write().unwrap();
            guard.push(sample_swarm("test-proj"));
        }

        let app = make_app(make_web_state(shared));
        let req = Request::builder()
            .method("POST")
            .uri("/api/swarms/test-proj/agents/manager/key")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"key":"kill-session"}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        // Rejected before any agent lookup or tmux call.
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn key_endpoint_404s_for_an_unknown_agent() {
        let shared = new_shared_state();
        {
            let mut guard = shared.write().unwrap();
            guard.push(sample_swarm("test-proj"));
        }

        let app = make_app(make_web_state(shared));
        let req = Request::builder()
            .method("POST")
            .uri("/api/swarms/test-proj/agents/worker-99/key")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"key":"Up"}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // ── Batched input ─────────────────────────────────────────────────────

    #[test]
    fn resolve_batch_normalises_every_key_through_the_allowlist() {
        let items = vec![
            InputItem::Key { key: "up".into() },
            InputItem::Text { text: "ab".into() },
            InputItem::Key { key: "esc".into() },
        ];
        let resolved = super::resolve_batch(&items).expect("batch should resolve");
        assert_eq!(
            resolved,
            vec![
                InputItem::Key { key: "Up".into() },
                InputItem::Text { text: "ab".into() },
                InputItem::Key {
                    key: "Escape".into()
                },
            ]
        );
    }

    #[test]
    fn resolve_batch_preserves_order() {
        let items = vec![
            InputItem::Text { text: "a".into() },
            InputItem::Key {
                key: "Down".into(),
            },
            InputItem::Text { text: "b".into() },
        ];
        let resolved = super::resolve_batch(&items).unwrap();
        // Ordering is the entire point of this endpoint.
        assert_eq!(
            resolved,
            vec![
                InputItem::Text { text: "a".into() },
                InputItem::Key {
                    key: "Down".into()
                },
                InputItem::Text { text: "b".into() },
            ]
        );
    }

    // Batching must not become a way around the allowlist that the single-key
    // endpoint enforces.
    #[test]
    fn resolve_batch_rejects_the_whole_batch_if_any_key_is_disallowed() {
        let items = vec![
            InputItem::Key { key: "Up".into() },
            InputItem::Key {
                key: "kill-session".into(),
            },
        ];
        assert!(super::resolve_batch(&items).is_none());
    }

    #[test]
    fn resolve_batch_rejects_empty_and_oversized_batches() {
        assert!(super::resolve_batch(&[]).is_none());

        let too_many: Vec<InputItem> = (0..super::MAX_BATCH_ITEMS + 1)
            .map(|_| InputItem::Key { key: "Up".into() })
            .collect();
        assert!(super::resolve_batch(&too_many).is_none());

        let at_limit: Vec<InputItem> = (0..super::MAX_BATCH_ITEMS)
            .map(|_| InputItem::Key { key: "Up".into() })
            .collect();
        assert!(super::resolve_batch(&at_limit).is_some());
    }

    #[test]
    fn resolve_batch_rejects_oversized_text() {
        let big = "x".repeat(super::MAX_BATCH_TEXT_BYTES + 1);
        assert!(super::resolve_batch(&[InputItem::Text { text: big }]).is_none());
    }

    #[test]
    fn resolve_batch_rejects_empty_text() {
        assert!(super::resolve_batch(&[InputItem::Text { text: String::new() }]).is_none());
    }

    #[tokio::test]
    async fn keys_endpoint_rejects_a_disallowed_key_before_touching_tmux() {
        let shared = new_shared_state();
        {
            let mut guard = shared.write().unwrap();
            guard.push(sample_swarm("test-proj"));
        }

        let app = make_app(make_web_state(shared));
        let req = Request::builder()
            .method("POST")
            .uri("/api/swarms/test-proj/agents/manager/keys")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"items":[{"type":"key","key":"Up"},{"type":"key","key":"kill-session"}]}"#,
            ))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn keys_endpoint_404s_for_an_unknown_agent() {
        let shared = new_shared_state();
        {
            let mut guard = shared.write().unwrap();
            guard.push(sample_swarm("test-proj"));
        }

        let app = make_app(make_web_state(shared));
        let req = Request::builder()
            .method("POST")
            .uri("/api/swarms/test-proj/agents/worker-99/keys")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"items":[{"type":"key","key":"Up"}]}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn keys_endpoint_rejects_an_empty_batch() {
        let shared = new_shared_state();
        {
            let mut guard = shared.write().unwrap();
            guard.push(sample_swarm("test-proj"));
        }

        let app = make_app(make_web_state(shared));
        let req = Request::builder()
            .method("POST")
            .uri("/api/swarms/test-proj/agents/manager/keys")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"items":[]}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn input_item_deserialises_both_variants() {
        let parsed: Vec<InputItem> =
            serde_json::from_str(r#"[{"type":"key","key":"Up"},{"type":"text","text":"hi"}]"#)
                .unwrap();
        assert_eq!(
            parsed,
            vec![
                InputItem::Key { key: "Up".into() },
                InputItem::Text { text: "hi".into() },
            ]
        );
    }

    // The pane endpoint must carry the cursor fields the page needs to draw a
    // caret, even when the live capture fails and it falls back to cached
    // content (pane_height 0 = "no cursor known").
    #[tokio::test]
    async fn api_pane_includes_cursor_fields() {
        let shared = new_shared_state();
        {
            let mut guard = shared.write().unwrap();
            guard.push(sample_swarm("test-proj"));
        }

        let app = make_app(make_web_state(shared));
        let req = Request::builder()
            .uri("/api/swarms/test-proj/agents/manager/pane")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        for field in ["cursor_x", "cursor_y", "pane_height"] {
            assert!(json.get(field).is_some(), "missing {field}");
            assert!(json[field].is_u64(), "{field} should be a number");
        }
    }
}
