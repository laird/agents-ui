//! What a worker control would send, before it is sent (#506, for #488):
//! `GET /api/swarms/{project}/agents/{role}/control?action=nudge|release`.
//!
//! The village's house dialog asks this when it opens, so its confirmation
//! can show the exact command a nudge types or the comment a release posts,
//! and so it can leave out controls the server can't run for that house.
//! The answer is `{ ok, action, sent }`, or the refusal the POST (#504,
//! `controls.rs`) would give: 404 for an unknown swarm or role, 409 when the
//! action can't run now (a busy worker, a worker holding no issue, the
//! manager). An action the server doesn't support is a 501. Nothing is sent
//! and the tracker is not asked, so a GET is cheap and changes nothing.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::controls::{claimed_issue, nudge_command, release_comment, Action, Refusal};
use super::server::WebServerState;
use super::{AgentSnapshot, SwarmSnapshot};
use crate::model::swarm::AgentType;

#[derive(Debug, Default, Deserialize)]
pub struct PreviewQuery {
    pub action: Option<String>,
}

/// What `action` would send for this agent, or why it can't run now. With
/// `ctl` (this host has `autocoder-ctl`, #540), the call it would make.
pub fn preview(swarm: &SwarmSnapshot, agent: &AgentSnapshot, action: &str, ctl: bool) -> Result<String, Refusal> {
    match action {
        "nudge" => {
            let runtime = AgentType::from_name(&swarm.agent_type).unwrap_or(AgentType::Claude);
            let command = nudge_command(agent, &runtime)?;
            Ok(if ctl { super::ctl::shown(&super::ctl::args(Action::Nudge, &agent.worktree_path, None, &agent.role, Some(&command))) } else { command })
        }
        "release" => {
            if agent.is_manager {
                return Err((StatusCode::CONFLICT, "only workers hold issues"));
            }
            let number = claimed_issue(swarm, agent).ok_or((StatusCode::CONFLICT, "that worker holds no issue"))?;
            if ctl {
                return Ok(super::ctl::shown(&super::ctl::args(Action::Release, &agent.worktree_path, Some(number), &agent.role, None)));
            }
            Ok(format!("{}\n\n(then removes the `working` label from #{number})", release_comment(number, &agent.role)))
        }
        _ => Err((StatusCode::NOT_IMPLEMENTED, "this server can't do that to a worker")),
    }
}

/// The swarm and the agent the path names (as `controls.rs` finds them).
fn find(state: &WebServerState, project: &str, role: &str) -> Result<(SwarmSnapshot, AgentSnapshot), Refusal> {
    let guard = state.swarms.read().map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "state unavailable"))?;
    let swarm = guard.iter().find(|s| s.project_name == project).ok_or((StatusCode::NOT_FOUND, "no such swarm"))?;
    let agent = std::iter::once(&swarm.manager)
        .chain(swarm.workers.iter())
        .find(|a| a.role == role)
        .ok_or((StatusCode::NOT_FOUND, "no such agent"))?;
    Ok((swarm.clone(), agent.clone()))
}

pub async fn api_control_preview_handler(
    Path((project, role)): Path<(String, String)>,
    State(state): State<WebServerState>,
    Query(q): Query<PreviewQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let fail = |(code, msg): Refusal| (code, Json(json!({ "error": msg })));
    let (swarm, agent) = find(&state, &project, &role).map_err(fail)?;
    let action = q.action.unwrap_or_default();
    let kind = match action.as_str() {
        "nudge" => Some(Action::Nudge),
        "release" => Some(Action::Release),
        _ => None,
    };
    let ctl = match kind {
        Some(a) => super::ctl::applies(&state.transport, &swarm.repo_path, &agent.worktree_path, a).await.is_some(),
        None => false,
    };
    let sent = preview(&swarm, &agent, &action, ctl).map_err(fail)?;
    let via = if ctl { "autocoder-ctl" } else { "village" };
    Ok(Json(json!({ "ok": true, "action": action, "sent": sent, "via": via })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::IssueSnapshot;

    fn agent(role: &str, state: &str, is_manager: bool) -> AgentSnapshot {
        AgentSnapshot {
            id: format!("p/{role}"),
            role: role.into(),
            state: state.into(),
            is_manager,
            tmux_target: "claude-p:0.1".into(),
            health: "Healthy".into(),
            ..Default::default()
        }
    }

    fn swarm(issues: Vec<IssueSnapshot>) -> SwarmSnapshot {
        SwarmSnapshot {
            project_name: "p".into(),
            repo_path: "/repos/p".into(),
            agent_type: "Claude".into(),
            tmux_session: "claude-p".into(),
            idle_count: 1,
            manager: agent("manager", "Idle", true),
            workers: vec![agent("worker-1", "Idle", false)],
            issues,
            ..Default::default()
        }
    }

    fn working(number: u32, worker: &str) -> IssueSnapshot {
        IssueSnapshot {
            number,
            state: "open".into(),
            labels: vec!["working".into()],
            is_working: true,
            assigned_worker: Some(worker.into()),
            ..Default::default()
        }
    }

    #[test]
    fn a_nudge_previews_the_command_the_post_would_type() {
        let s = swarm(vec![]);
        let idle = agent("worker-1", "Idle", false);
        assert_eq!(preview(&s, &idle, "nudge", false), nudge_command(&idle, &AgentType::Claude));
        assert!(preview(&s, &idle, "nudge", false).unwrap().contains("fix-loop"), "{:?}", preview(&s, &idle, "nudge", false));
        assert_eq!(preview(&s, &agent("worker-1", "Working: #7", false), "nudge", false).unwrap_err().0, StatusCode::CONFLICT);
    }

    #[test]
    fn a_release_previews_the_comment_and_the_label() {
        let s = swarm(vec![working(42, "worker-2")]);
        let sent = preview(&s, &agent("worker-2", "Working: #42", false), "release", false).unwrap();
        assert!(sent.starts_with(&release_comment(42, "worker-2")));
        assert!(sent.ends_with("(then removes the `working` label from #42)"));
        assert_eq!(preview(&s, &agent("worker-3", "Idle", false), "release", false).unwrap_err().0, StatusCode::CONFLICT);
        assert_eq!(preview(&s, &agent("manager", "Idle", true), "release", false).unwrap_err().0, StatusCode::CONFLICT);
    }

    #[test]
    fn an_action_the_server_does_not_have_is_501() {
        let s = swarm(vec![]);
        for action in ["restart", "stop", ""] {
            assert_eq!(preview(&s, &agent("worker-1", "Idle", false), action, false).unwrap_err().0, StatusCode::NOT_IMPLEMENTED, "{action}");
        }
    }

    /// #540: with autocoder-ctl, the confirmation shows the call; the
    /// village's own checks still refuse first.
    #[test]
    fn with_autocoder_ctl_the_preview_is_the_call() {
        let s = swarm(vec![working(42, "worker-2")]);
        let idle = AgentSnapshot { worktree_path: "/repos/p-wt-1".into(), ..agent("worker-1", "Idle", false) };
        assert_eq!(preview(&s, &idle, "nudge", true).unwrap(), "autocoder-ctl nudge /repos/p-wt-1 -- /autocoder:fix-loop");
        let w2 = agent("worker-2", "Working: #42", false);
        assert_eq!(preview(&s, &w2, "release", true).unwrap(), "autocoder-ctl release 42 --reason 'released by hand from the village (worker `worker-2`)'");
        assert_eq!(preview(&s, &agent("worker-1", "Working: #7", false), "nudge", true).unwrap_err().0, StatusCode::CONFLICT, "busy: refused");
        assert_eq!(preview(&s, &agent("worker-3", "Idle", false), "release", true).unwrap_err().0, StatusCode::CONFLICT);
    }
}
