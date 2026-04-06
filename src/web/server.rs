use anyhow::Result;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::Html,
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::path::PathBuf;

use super::SharedWebState;

/// The embedded single-page web UI.
const INDEX_HTML: &str = include_str!("ui.html");

/// Default port for the web server.
pub const DEFAULT_PORT: u16 = 7878;

/// Combined state for the web server: swarm snapshots + agents dir for launching.
#[derive(Clone)]
pub struct WebServerState {
    pub swarms: SharedWebState,
    pub agents_dir: PathBuf,
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

/// Returns the pane content for a specific agent.
/// `GET /api/swarms/:project/agents/:role/pane`
async fn api_pane_handler(
    Path((project, role)): Path<(String, String)>,
    State(state): State<WebServerState>,
) -> Result<Json<Value>, StatusCode> {
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

    Ok(Json(json!({
        "role": agent.role,
        "tmux_target": agent.tmux_target,
        "state": agent.state,
        "pane_content": agent.pane_content,
    })))
}

/// Body for the send-input endpoint.
#[derive(Debug, Deserialize, Serialize)]
pub struct SendInputBody {
    /// Text to send to the agent (Enter appended).
    pub text: String,
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

    // Send the text via tmux send-keys (text + Enter).
    crate::tmux::proxy::send_keys(
        &crate::transport::ServerTransport::new(None),
        &tmux_target,
        &body.text,
    )
    .await
    .map_err(|e| {
        tracing::warn!("tmux send-keys failed for {tmux_target}: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({ "ok": true })))
}

/// Stop a running swarm by killing its tmux session.
/// `DELETE /api/swarms/:project`
async fn api_stop_swarm_handler(
    Path(project): Path<String>,
    State(state): State<WebServerState>,
) -> Result<Json<Value>, StatusCode> {
    // Look up the tmux session name while holding only a read lock.
    let tmux_session = {
        let guard = state
            .swarms
            .read()
            .map_err(|e| {
                tracing::warn!("Web state lock poisoned: {e}");
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        guard
            .iter()
            .find(|s| s.project_name == project)
            .ok_or(StatusCode::NOT_FOUND)?
            .tmux_session
            .clone()
    };

    // Kill the tmux session.
    let transport = crate::transport::ServerTransport::new(None);
    transport
        .output(
            "tmux",
            &[
                "kill-session".to_string(),
                "-t".to_string(),
                tmux_session.clone(),
            ],
            None,
        )
        .await
        .map_err(|e| {
            tracing::warn!("tmux kill-session failed for {tmux_session}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::info!("Stopped swarm via web UI: project={project} session={tmux_session}");
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
        .route("/api/swarms/{project}", delete(api_stop_swarm_handler))
        .route(
            "/api/swarms/{project}/agents/{role}/pane",
            get(api_pane_handler),
        )
        .route(
            "/api/swarms/{project}/agents/{role}/input",
            post(api_input_handler),
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
                "/api/swarms/{project}/agents/{role}/pane",
                get(api_pane_handler),
            )
            .route(
                "/api/swarms/{project}/agents/{role}/input",
                post(api_input_handler),
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

    #[tokio::test]
    async fn index_html_contains_ansi_renderer() {
        // Verify the ANSI-to-HTML converter is present and wired up
        let body = INDEX_HTML;
        assert!(body.contains("ansiToHtml"), "ansiToHtml function must be defined");
        assert!(body.contains("ansiPalette") || body.contains("const pal"), "ANSI palette must be present");
        assert!(!body.contains("t.textContent = state.session.paneContent"), "pane must not use textContent");
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

    // ── DELETE /api/swarms/:project ────────────────────────────────────────────

    #[tokio::test]
    async fn api_stop_swarm_returns_404_for_unknown_project() {
        let state = make_web_state(new_shared_state());
        let app = make_app(state);
        let req = Request::builder()
            .method("DELETE")
            .uri("/api/swarms/nonexistent-project")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn api_stop_swarm_found_swarm_attempts_kill() {
        // This test verifies the happy path: when the swarm exists,
        // the handler looks up the tmux_session and attempts to kill it.
        // Since "tmux kill-session" will fail in a test environment (no tmux),
        // the handler returns INTERNAL_SERVER_ERROR rather than 200 OK —
        // but crucially it does NOT return 404, proving the swarm was found.
        let shared = new_shared_state();
        {
            let mut guard = shared.write().unwrap();
            guard.push(sample_swarm("my-project"));
        }
        let state = make_web_state(shared);
        let app = make_app(state);
        let req = Request::builder()
            .method("DELETE")
            .uri("/api/swarms/my-project")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        // The swarm was found (not 404); tmux may or may not be available.
        assert_ne!(resp.status(), StatusCode::NOT_FOUND);
    }
}
