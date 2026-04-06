use anyhow::Result;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::Html,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::net::SocketAddr;

use super::SharedWebState;

/// The embedded single-page web UI.
const INDEX_HTML: &str = include_str!("ui.html");

/// Default port for the web server.
pub const DEFAULT_PORT: u16 = 7878;

async fn index_handler() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn api_swarms_handler(State(state): State<SharedWebState>) -> Json<Value> {
    let swarms = state
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
    State(state): State<SharedWebState>,
) -> Result<Json<Value>, StatusCode> {
    let guard = state
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
    State(state): State<SharedWebState>,
    Json(body): Json<SendInputBody>,
) -> Result<Json<Value>, StatusCode> {
    // Look up the tmux target while holding only a read lock.
    let tmux_target = {
        let guard = state
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
pub async fn run(port: u16, state: SharedWebState) -> Result<()> {
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/swarms", get(api_swarms_handler))
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

    fn make_app(state: SharedWebState) -> Router {
        Router::new()
            .route("/", get(index_handler))
            .route("/api/swarms", get(api_swarms_handler))
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
        let state = new_shared_state();
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
        let state = new_shared_state();
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
        let state = new_shared_state();
        {
            let mut guard = state.write().unwrap();
            guard.push(sample_swarm("my-project"));
        }

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
        let state = new_shared_state();
        {
            let mut guard = state.write().unwrap();
            guard.push(sample_swarm("test-proj"));
        }

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
        let state = new_shared_state();
        {
            let mut guard = state.write().unwrap();
            guard.push(sample_swarm("test-proj"));
        }

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
        let state = new_shared_state();
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
        let state = new_shared_state();
        {
            let mut guard = state.write().unwrap();
            guard.push(sample_swarm("test-proj"));
        }
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
        let state = new_shared_state();
        {
            let mut guard = state.write().unwrap();
            guard.push(sample_swarm("test-proj"));
        }
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
}
