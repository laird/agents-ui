//! Worker controls from the village's house dialog (#504, #505, for #488):
//! `POST /api/swarms/{project}/agents/{role}/control` with
//! `{ "action": "nudge" | "release" | "stop" | "restart" }`.
//!
//! - **nudge** starts an idle worker's loop now: it types the runtime's
//!   start-worker-loop command into its pane, as the TUI's `f` key does
//!   (`ClaudeAdapter::start_worker_loop`). A worker that isn't idle (busy,
//!   or waiting on a question) is a 409: typing into it would land in its
//!   work or answer its prompt.
//! - **release** gives up the worker's claimed issue: the explicit-release
//!   comment first, then the `working` label off, as autocoder's own
//!   explicit-release path does, so the crate goes back to the docks. A
//!   worker that holds no issue, or whose issue is no longer open and
//!   `working`, is a 409.
//! - **stop** shuts the worker down, as the TUI's `d` key: the stopped
//!   tombstone (`.codex/stopped` in its worktree) first, so heal and revive
//!   won't respawn it, then its pane is closed: Ctrl+C and `exit` for tmux
//!   (`proxy::kill_pane`'s keys), `herdr pane close` for herdr. A tmux pane
//!   is closed only while it is still the worker's own (see restart). A pane
//!   that is already gone is still a stopped worker, as for the `d` key. Its issue
//!   stays claimed, and the response says so: release it to hand it back.
//! - **restart** quits the worker's runtime and starts it again in the same
//!   pane and worktree, then tells it to work its claimed issue (or start its
//!   loop), as revive does. Ctrl+C until the pane is back at a shell; if it
//!   isn't within 10 s nothing is typed (504), so the launch command never
//!   lands in a live agent's prompt. The pane must still be the worker's own
//!   (its current path is its worktree): tmux renumbers panes when one
//!   closes. Only tmux panes, and only a known runtime: the server has no way
//!   to launch one in a herdr pane yet, so a herdr worker is a 501.
//!
//! Only workers have controls: the manager is a 409. An unknown swarm or role
//! is a 404 and an unknown action a 422. A `--read-only` server refuses it
//! before it gets here (403). The response says what was sent (`{ ok, sent }`)
//! so the page can show it; after a release the village forgets its cached
//! issues for the repo, so the next poll shows the crate move.

use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::pane::Pane;
use super::server::WebServerState;
use super::{AgentSnapshot, SwarmSnapshot};
use crate::adapter::supervisor;
use crate::model::swarm::AgentType;
use crate::project_management::{ProjectCommand, ProjectManagementClient};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Nudge,
    Release,
    Stop,
    Restart,
}

#[derive(Debug, Deserialize)]
pub struct ControlBody {
    pub action: Action,
}

/// Why a control can't run: the status code and what the page shows.
pub type Refusal = (StatusCode, &'static str);

/// Whether the worker is idle, so a nudge can't land in the middle of its
/// work: only `Idle`, as the TUI's `f` key. Not `Done: ...`: that status
/// can outlive the issue it reports while the loop is already on the next
/// one, and the command would land in a busy session. A worker waiting on a
/// question would take the command as its answer, so it isn't idle either.
pub fn is_idle(agent: &AgentSnapshot) -> bool {
    !agent.waiting_for_input && agent.state == "Idle"
}

/// What a nudge types into the worker's pane, or why it can't.
pub fn nudge_command(agent: &AgentSnapshot, runtime: &AgentType) -> Result<String, Refusal> {
    if agent.is_manager {
        return Err((StatusCode::CONFLICT, "only workers can be nudged"));
    }
    if !is_idle(agent) {
        return Err((StatusCode::CONFLICT, "that worker is busy: wait until it is idle"));
    }
    supervisor::for_runtime(runtime)
        .start_worker_loop_command(runtime)
        .ok_or((StatusCode::CONFLICT, "this runtime has no worker loop to start"))
}

/// The issue the worker holds: the open issue the tracker shows assigned to
/// it as `working`, or else its own status's issue, unless the tracker shows
/// that one assigned to another worker (a stale status must never release a
/// peer's lock).
pub fn claimed_issue(swarm: &SwarmSnapshot, agent: &AgentSnapshot) -> Option<u32> {
    if agent.is_manager {
        return None;
    }
    let mine = |w: &Option<String>| w.as_deref() == Some(agent.role.as_str());
    swarm
        .issues
        .iter()
        .find(|i| i.is_working && mine(&i.assigned_worker))
        .map(|i| i.number)
        .or_else(|| {
            agent.current_issue.filter(|n| {
                !swarm.issues.iter().any(|i| i.number == *n && i.assigned_worker.is_some() && !mine(&i.assigned_worker))
            })
        })
}

/// The explicit-release comment, in autocoder's shape, so peers and
/// `/monitor-workers` read it as a deliberate hand-off, not a crash.
pub fn release_comment(number: u32, role: &str) -> String {
    format!(
        "🔓 **Releasing claim**\n\n\
         **Reason**: released by hand from the village (worker `{role}`)\n\
         **Branch**: `feature/issue-{number}` (left as it is)\n\n\
         Releasing the `working` lock so another agent can pick this up."
    )
}

/// The tracker commands a release runs, in order. The comment comes first:
/// if removing the label then fails, a retry still finds the issue `working`.
pub fn release_commands(client: &ProjectManagementClient, number: u32, role: &str) -> Vec<ProjectCommand> {
    vec![
        client.comment_issue(number, &release_comment(number, role)),
        client.remove_label(number, "working"),
    ]
}

/// Whether `gh issue view --json state,labels` shows an open, `working` issue.
pub fn is_open_and_working(view: &Value) -> bool {
    let open = view["state"].as_str().is_some_and(|s| s.eq_ignore_ascii_case("open"));
    let working = view["labels"]
        .as_array()
        .is_some_and(|ls| ls.iter().any(|l| l["name"].as_str().or(l.as_str()) == Some("working")));
    open && working
}

/// One thing a stop or restart does: run a command, or wait a moment for the
/// pane to take the last one in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Run(&'static str, Vec<String>),
    Wait(u64),
}

fn run(program: &'static str, args: &[&str]) -> Step {
    Step::Run(program, args.iter().map(|s| s.to_string()).collect())
}

/// Where the worker's tombstone goes, or why there is nowhere for it.
fn tombstone(agent: &AgentSnapshot) -> Result<(String, String), Refusal> {
    if agent.worktree_path.is_empty() {
        return Err((StatusCode::CONFLICT, "that worker has no worktree to mark stopped"));
    }
    let dir = format!("{}/.codex", agent.worktree_path.trim_end_matches('/'));
    let file = format!("{dir}/stopped");
    Ok((dir, file))
}

/// What a stop runs, in order: the tombstone (so heal and revive leave it
/// stopped), then the pane closed. Through the transport, so it works on a
/// `--server` host too.
pub fn stop_steps(agent: &AgentSnapshot) -> Result<Vec<Step>, Refusal> {
    if agent.is_manager {
        return Err((StatusCode::CONFLICT, "only workers can be stopped here"));
    }
    let (dir, file) = tombstone(agent)?;
    let target = agent.tmux_target.as_str();
    let mut steps = vec![run("mkdir", &["-p", &dir]), run("touch", &[&file])];
    match agent.mux {
        super::Mux::Tmux => steps.extend([
            run("tmux", &["send-keys", "-t", target, "C-c", ""]),
            Step::Wait(200),
            run("tmux", &["send-keys", "-t", target, "exit", "Enter"]),
        ]),
        super::Mux::Herdr => steps.push(run("herdr", &["pane", "close", target])),
    }
    Ok(steps)
}

/// Why a worker can't be restarted from here, or its runtime.
pub fn restart_check(agent: &AgentSnapshot, agent_type: &str) -> Result<AgentType, Refusal> {
    if agent.is_manager {
        return Err((StatusCode::CONFLICT, "only workers can be restarted here"));
    }
    if agent.mux == super::Mux::Herdr {
        return Err((StatusCode::NOT_IMPLEMENTED, "restart isn't supported for herdr panes yet: stop it and start it from herdr"));
    }
    tombstone(agent)?;
    AgentType::from_name(agent_type).ok_or((StatusCode::CONFLICT, "unknown runtime: restart it from the terminal"))
}

/// What tmux says the pane is: `#{pane_current_path}\t#{pane_current_command}`.
pub const PANE_FORMAT: &str = "#{pane_current_path}\t#{pane_current_command}";

/// Whether the pane is in the worker's worktree (from [`PANE_FORMAT`]), so a
/// renumbered pane isn't someone else's.
pub fn is_its_pane(shown: &str, worktree: &str) -> bool {
    let path = shown.trim().split('\t').next().unwrap_or("").trim_end_matches('/');
    !path.is_empty() && path == worktree.trim().trim_end_matches('/')
}

/// Whether the pane is back at a shell, its runtime gone (from
/// [`PANE_FORMAT`]): only then is the launch command typed, never into a live
/// agent. The program tmux sees, not the screen: the runtime's last output
/// is still on it, and a shell prompt can look like an agent's.
pub fn back_at_shell(shown: &str) -> bool {
    let command = shown.trim().split('\t').nth(1).unwrap_or("").trim().trim_start_matches('-');
    matches!(command, "bash" | "zsh" | "fish" | "sh" | "dash" | "ksh" | "nu" | "tcsh")
}

/// Run `steps` in order through the transport, stopping at the first that fails.
async fn run_steps(transport: &crate::transport::ServerTransport, steps: &[Step]) -> Result<(), String> {
    for step in steps {
        match step {
            Step::Wait(ms) => tokio::time::sleep(std::time::Duration::from_millis(*ms)).await,
            Step::Run(program, args) => {
                let out = transport.output(program, args, None).await.map_err(|e| e.to_string())?;
                if !out.status.success() {
                    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    return Err(if err.is_empty() { format!("{program} failed") } else { err });
                }
            }
        }
    }
    Ok(())
}

/// The steps' commands, one a line, for the page to show.
fn described(steps: &[Step]) -> String {
    steps
        .iter()
        .filter_map(|s| match s {
            Step::Run(p, a) => Some(format!("{p} {}", a.join(" ")).trim_end().to_string()),
            Step::Wait(_) => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// What tmux says the pane is ([`PANE_FORMAT`]), or `None` when there is no such pane.
async fn tmux_pane_shown(t: &crate::transport::ServerTransport, target: &str) -> Option<String> {
    let args: Vec<String> = ["display-message", "-p", "-t", target, PANE_FORMAT].iter().map(|s| s.to_string()).collect();
    let out = t.output("tmux", &args, None).await.ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Restart a tmux worker that `restart_check` passed: quit its runtime, and
/// only once its pane is a shell again launch it and tell it what to work.
/// What was typed, or why it stopped and what status that is.
async fn restart(state: &WebServerState, swarm: &SwarmSnapshot, agent: &AgentSnapshot, runtime: &AgentType) -> Result<Vec<String>, (StatusCode, String)> {
    let t = &state.transport;
    let target = agent.tmux_target.clone();
    let tmux = |args: &[&str]| -> Vec<String> { args.iter().map(|s| s.to_string()).collect() };
    let gateway = |e: String| (StatusCode::BAD_GATEWAY, e);
    let shown = || tmux_pane_shown(t, &target);
    // Still its pane? tmux renumbers panes when one closes.
    if !shown().await.is_some_and(|s| is_its_pane(&s, &agent.worktree_path)) {
        return Err((StatusCode::CONFLICT, "its pane is gone or is another worker's now: nothing was sent".into()));
    }
    let pane = Pane { target: target.clone(), mux: agent.mux };
    let mut sent = Vec::new();
    // Ctrl+C until the runtime has quit: a busy agent takes one to stop its
    // turn and another to exit. Nothing is typed until the pane is a shell.
    let started = std::time::Instant::now();
    let mut presses = 0;
    loop {
        match shown().await {
            Some(s) if back_at_shell(&s) => break,
            // The runtime was the pane's own program: quitting it closed the pane.
            None => return Err((StatusCode::CONFLICT, "its pane closed when the runtime quit: start it again from the terminal".into())),
            Some(_) => {}
        }
        if started.elapsed() > std::time::Duration::from_secs(10) {
            return Err((StatusCode::GATEWAY_TIMEOUT, "its runtime didn't exit within 10 s: nothing was typed".into()));
        }
        if presses < 4 {
            super::pane::send_key(t, &pane, "C-c").await.map_err(|e| gateway(e.to_string()))?;
            presses += 1;
            sent.push("Ctrl+C".to_string());
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    let adapter = crate::adapter::claude::ClaudeAdapter::new(runtime.clone(), t.clone());
    let launch = adapter.pane_launch_command(runtime, &swarm.tmux_session, false);
    super::pane::send_line(t, &pane, &launch).await.map_err(|e| gateway(e.to_string()))?;
    sent.push(launch);
    // Started again: it is no longer stopped.
    let (_, file) = tombstone(agent).map_err(|(c, m)| (c, m.to_string()))?;
    let _ = t.output("rm", &tmux(&["-f", &file]), None).await;
    if let Some(cmd) = crate::adapter::claude::ClaudeAdapter::pane_bootstrap_command(runtime, claimed_issue(swarm, agent)) {
        // As revive does: give the runtime a moment to come up first.
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        super::pane::send_line(t, &pane, &cmd).await.map_err(|e| gateway(e.to_string()))?;
        sent.push(cmd);
    }
    Ok(sent)
}

/// The swarm and the agent the path names, cloned out of the shared state.
fn find(state: &WebServerState, project: &str, role: &str) -> Result<(SwarmSnapshot, AgentSnapshot), Refusal> {
    let guard = state.swarms.read().map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "state unavailable"))?;
    let swarm = guard.iter().find(|s| s.project_name == project).ok_or((StatusCode::NOT_FOUND, "no such swarm"))?;
    let agent = std::iter::once(&swarm.manager)
        .chain(swarm.workers.iter())
        .find(|a| a.role == role)
        .ok_or((StatusCode::NOT_FOUND, "no such agent"))?;
    Ok((swarm.clone(), agent.clone()))
}

pub async fn api_control_handler(
    Path((project, role)): Path<(String, String)>,
    State(state): State<WebServerState>,
    body: Result<Json<ControlBody>, JsonRejection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let fail = |(code, msg): (StatusCode, &str)| (code, Json(json!({ "error": msg })));
    let (swarm, agent) = find(&state, &project, &role).map_err(fail)?;
    let Json(body) = body.map_err(|_| fail((StatusCode::UNPROCESSABLE_ENTITY, "the action is nudge, release, stop or restart")))?;

    match body.action {
        Action::Nudge => {
            let runtime = AgentType::from_name(&swarm.agent_type).unwrap_or(AgentType::Claude);
            let command = nudge_command(&agent, &runtime).map_err(fail)?;
            let pane = Pane { target: agent.tmux_target.clone(), mux: agent.mux };
            super::pane::send_line(&state.transport, &pane, &command).await.map_err(|e| {
                tracing::warn!("nudging {project}/{role} failed: {e}");
                fail((StatusCode::BAD_GATEWAY, "could not type into its pane"))
            })?;
            Ok(Json(json!({ "ok": true, "action": "nudge", "sent": command })))
        }
        Action::Stop => {
            let steps = stop_steps(&agent).map_err(fail)?;
            // The tombstone must be written; closing a pane that is already gone is not a failure.
            let (mark, close) = steps.split_at(2);
            run_steps(&state.transport, mark).await.map_err(|err| {
                tracing::warn!("stopping {project}/{role}: no tombstone: {err}");
                (StatusCode::BAD_GATEWAY, Json(json!({ "error": format!("could not mark it stopped: {err}") })))
            })?;
            // A tmux pane is closed only if it is still the worker's own: an
            // earlier stop renumbers the panes, and a stale page would close a peer's.
            let its_own = agent.mux != super::Mux::Tmux
                || tmux_pane_shown(&state.transport, &agent.tmux_target).await.is_some_and(|s| is_its_pane(&s, &agent.worktree_path));
            let closed = if its_own { run_steps(&state.transport, close).await } else { Err("its pane is gone or is another worker's now: left alone".into()) };
            if let Err(err) = &closed {
                tracing::info!("stopping {project}/{role}: its pane didn't close ({err}); it is marked stopped");
            }
            let claimed = claimed_issue(&swarm, &agent);
            Ok(Json(json!({
                "ok": true,
                "action": "stop",
                "sent": described(&steps),
                "closed": closed.is_ok(),
                "note": claimed.map(|n| format!("#{n} stays claimed: release it to hand it back")),
            })))
        }
        Action::Restart => {
            let runtime = restart_check(&agent, &swarm.agent_type).map_err(fail)?;
            let sent = restart(&state, &swarm, &agent, &runtime).await.map_err(|(code, err)| {
                tracing::warn!("restarting {project}/{role}: {err}");
                (code, Json(json!({ "error": err })))
            })?;
            Ok(Json(json!({ "ok": true, "action": "restart", "sent": sent.join("\n") })))
        }
        Action::Release => {
            if agent.is_manager {
                return Err(fail((StatusCode::CONFLICT, "only workers hold issues")));
            }
            let number = claimed_issue(&swarm, &agent)
                .ok_or_else(|| fail((StatusCode::CONFLICT, "that worker holds no issue")))?;
            if swarm.repo_path.is_empty() {
                return Err(fail((StatusCode::NOT_FOUND, "no repo for that swarm")));
            }
            let repo = std::path::PathBuf::from(&swarm.repo_path);
            let client = ProjectManagementClient::from_env();

            let view = client.view_issue_json(number, "state,labels");
            let out = state
                .transport
                .output(view.program, &view.args, Some(&repo))
                .await
                .map_err(|_| fail((StatusCode::BAD_GATEWAY, "could not ask the tracker")))?;
            let seen: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
            if !out.status.success() || !is_open_and_working(&seen) {
                return Err(fail((StatusCode::CONFLICT, "that issue is no longer claimed")));
            }

            let comment = release_comment(number, &role);
            for cmd in release_commands(&client, number, &role) {
                let out = state
                    .transport
                    .output(cmd.program, &cmd.args, Some(&repo))
                    .await
                    .map_err(|_| fail((StatusCode::BAD_GATEWAY, "could not reach the tracker")))?;
                if !out.status.success() {
                    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    tracing::warn!("releasing #{number} from {project}/{role} failed: {err}");
                    state.village.forget(&repo);
                    let msg = if err.is_empty() { "the tracker refused it".to_string() } else { err };
                    return Err((StatusCode::BAD_GATEWAY, Json(json!({ "error": msg }))));
                }
            }
            state.village.forget(&repo);
            Ok(Json(json!({
                "ok": true,
                "action": "release",
                "number": number,
                "sent": format!("{comment}\n\n(then removed the `working` label from #{number})"),
            })))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::IssueSnapshot;

    fn worker(state: &str) -> AgentSnapshot {
        AgentSnapshot {
            id: "p/worker-1".into(),
            role: "worker-1".into(),
            state: state.into(),
            is_manager: false,
            waiting_for_input: false,
            current_issue: None,
            current_issue_title: None,
            pane_content: String::new(),
            tmux_target: "claude-p:0.1".into(),
            health: "Healthy".into(),
            completed_issue_count: 0,
            resurrection_attempts: 0,
            status_timestamp: None,
            worktree_path: String::new(),
            branch: None,
            mux: Default::default(),
            limit: None,
            context_pct: None,
            context_window: None,
        }
    }

    fn swarm(issues: Vec<IssueSnapshot>) -> SwarmSnapshot {
        SwarmSnapshot {
            project_name: "p".into(),
            repo_path: "/repos/p".into(),
            repo_key: String::new(),
            manager_by: None,
            agent_type: "Claude".into(),
            workflow: None,
            tmux_session: "claude-p".into(),
            stopped: false,
            busy_count: 0,
            idle_count: 1,
            attention_count: 0,
            manager: AgentSnapshot { role: "manager".into(), is_manager: true, ..worker("Idle") },
            workers: vec![worker("Idle")],
            issues,
        }
    }

    #[test]
    fn a_nudge_starts_an_idle_workers_loop_as_the_f_key_does() {
        assert_eq!(nudge_command(&worker("Idle"), &AgentType::Claude).unwrap(), "/autocoder:fix-loop");
        // "Done" can outlive its issue while the loop is on the next one: busy.
        assert_eq!(nudge_command(&worker("Done: #7"), &AgentType::Claude).unwrap_err().0, StatusCode::CONFLICT);
        // A wrapper-supervised runtime gets its wrapper, whatever the TUI would send.
        let droid = supervisor::for_runtime(&AgentType::Droid).start_worker_loop_command(&AgentType::Droid);
        assert_eq!(nudge_command(&worker("Idle"), &AgentType::Droid).ok(), droid);
    }

    #[test]
    fn a_busy_or_asking_worker_or_the_manager_is_not_nudged() {
        for state in ["Working #3", "Working", "Starting", "Stopped"] {
            assert_eq!(nudge_command(&worker(state), &AgentType::Claude).unwrap_err().0, StatusCode::CONFLICT, "{state}");
        }
        let asking = AgentSnapshot { waiting_for_input: true, ..worker("Idle") };
        assert_eq!(nudge_command(&asking, &AgentType::Claude).unwrap_err().0, StatusCode::CONFLICT);
        let manager = AgentSnapshot { is_manager: true, ..worker("Idle") };
        assert_eq!(nudge_command(&manager, &AgentType::Claude).unwrap_err().0, StatusCode::CONFLICT);
    }

    #[test]
    fn the_claimed_issue_is_the_workers_own_or_the_one_assigned_to_it() {
        let w = AgentSnapshot { current_issue: Some(12), ..worker("Working #12") };
        assert_eq!(claimed_issue(&swarm(vec![]), &w), Some(12));
        let assigned = IssueSnapshot {
            number: 9,
            is_working: true,
            assigned_worker: Some("worker-1".into()),
            ..Default::default()
        };
        let other = IssueSnapshot { number: 4, is_working: true, assigned_worker: Some("worker-2".into()), ..Default::default() };
        assert_eq!(claimed_issue(&swarm(vec![other.clone(), assigned]), &worker("Idle")), Some(9));
        assert_eq!(claimed_issue(&swarm(vec![other.clone()]), &worker("Idle")), None);
        // The tracker's assignment wins over a stale status, and a status
        // naming a peer's issue releases nothing.
        let stale = AgentSnapshot { current_issue: Some(4), ..worker("Idle") };
        assert_eq!(claimed_issue(&swarm(vec![other.clone()]), &stale), None);
        let both = IssueSnapshot { number: 9, is_working: true, assigned_worker: Some("worker-1".into()), ..Default::default() };
        assert_eq!(claimed_issue(&swarm(vec![other, both]), &stale), Some(9));
        let s = swarm(vec![]);
        assert_eq!(claimed_issue(&s, &AgentSnapshot { current_issue: Some(3), ..s.manager.clone() }), None);
    }

    #[test]
    fn a_release_comments_first_then_removes_working() {
        let cmds = release_commands(&ProjectManagementClient::github(), 7, "worker-2");
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].args[..3], ["issue", "comment", "7"]);
        assert_eq!(cmds[0].args[4], release_comment(7, "worker-2"));
        assert_eq!(cmds[1].args, vec!["issue", "edit", "7", "--remove-label", "working"]);
    }

    #[test]
    fn the_release_comment_is_autocoders_explicit_release() {
        let c = release_comment(7, "worker-2");
        assert!(c.starts_with("🔓 **Releasing claim**"), "{c}");
        assert!(c.contains("worker `worker-2`") && c.contains("feature/issue-7"), "{c}");
        assert!(c.ends_with("so another agent can pick this up."), "{c}");
    }

    #[test]
    fn only_an_open_working_issue_is_released() {
        assert!(is_open_and_working(&json!({ "state": "OPEN", "labels": [{ "name": "P2" }, { "name": "working" }] })));
        assert!(!is_open_and_working(&json!({ "state": "CLOSED", "labels": [{ "name": "working" }] })));
        assert!(!is_open_and_working(&json!({ "state": "OPEN", "labels": [] })));
        assert!(!is_open_and_working(&Value::Null));
    }

    #[test]
    fn the_body_names_its_action() {
        let b: ControlBody = serde_json::from_value(json!({ "action": "release" })).unwrap();
        assert_eq!(b.action, Action::Release);
        assert!(serde_json::from_value::<ControlBody>(json!({ "action": "explode" })).is_err());
    }

    fn with_worktree(mut a: AgentSnapshot, mux: crate::web::Mux) -> AgentSnapshot {
        a.worktree_path = "/repos/p-wt-1/".into();
        a.mux = mux;
        if mux == crate::web::Mux::Herdr {
            a.tmux_target = "w5:p1".into();
        }
        a
    }

    fn runs(steps: &[Step]) -> Vec<String> {
        steps
            .iter()
            .map(|s| match s {
                Step::Run(p, a) => format!("{p} {}", a.join("|")),
                Step::Wait(ms) => format!("wait {ms}"),
            })
            .collect()
    }

    #[test]
    fn stop_marks_the_worker_stopped_before_closing_its_pane() {
        let tmux = stop_steps(&with_worktree(worker("Working #5"), crate::web::Mux::Tmux)).unwrap();
        assert_eq!(
            runs(&tmux),
            [
                "mkdir -p|/repos/p-wt-1/.codex",
                "touch /repos/p-wt-1/.codex/stopped",
                "tmux send-keys|-t|claude-p:0.1|C-c|",
                "wait 200",
                "tmux send-keys|-t|claude-p:0.1|exit|Enter",
            ],
            "as the TUI's d key (proxy::kill_pane)"
        );
        let herdr = stop_steps(&with_worktree(worker("Idle"), crate::web::Mux::Herdr)).unwrap();
        assert_eq!(runs(&herdr), ["mkdir -p|/repos/p-wt-1/.codex", "touch /repos/p-wt-1/.codex/stopped", "herdr pane|close|w5:p1"]);
    }

    #[test]
    fn restart_is_for_a_tmux_worker_on_a_known_runtime() {
        let tmux = with_worktree(worker("Working #5"), crate::web::Mux::Tmux);
        assert_eq!(restart_check(&tmux, "Claude").unwrap(), AgentType::Claude);
        assert_eq!(restart_check(&tmux, "a-new-runtime").unwrap_err().0, StatusCode::CONFLICT, "never launches Claude in its place");
        let herdr = with_worktree(worker("Idle"), crate::web::Mux::Herdr);
        assert_eq!(restart_check(&herdr, "Claude").unwrap_err().0, StatusCode::NOT_IMPLEMENTED);
    }

    #[test]
    fn restart_only_types_into_its_own_pane_once_it_is_a_shell() {
        assert!(is_its_pane("/repos/p-wt-1\tclaude\n", "/repos/p-wt-1/"));
        assert!(!is_its_pane("/repos/p-wt-2\tclaude\n", "/repos/p-wt-1"), "renumbered: another worker's pane");
        assert!(!is_its_pane("", "/repos/p-wt-1"));
        assert!(back_at_shell("/repos/p-wt-1\tbash\n"), "the runtime quit: a shell");
        assert!(back_at_shell("/repos/p-wt-1\t-zsh\n"), "a login shell");
        assert!(!back_at_shell("/repos/p-wt-1\tclaude\n"), "a live agent: nothing is typed into it");
        assert!(!back_at_shell("/repos/p-wt-1\tnode\n"));
        assert!(!back_at_shell(""));
    }

    #[test]
    fn a_restarted_worker_is_told_to_work_its_issue() {
        use crate::adapter::claude::ClaudeAdapter;
        let with_issue = ClaudeAdapter::pane_bootstrap_command(&AgentType::Claude, Some(12)).unwrap();
        assert!(with_issue.contains("12"), "{with_issue}");
        assert!(ClaudeAdapter::pane_bootstrap_command(&AgentType::Claude, None).is_some(), "no issue: its loop");
    }

    #[test]
    fn stop_and_restart_are_for_workers_with_a_worktree() {
        let mut manager = with_worktree(worker("Idle"), crate::web::Mux::Tmux);
        manager.is_manager = true;
        assert_eq!(stop_steps(&manager).unwrap_err().0, StatusCode::CONFLICT);
        assert_eq!(restart_check(&manager, "Claude").unwrap_err().0, StatusCode::CONFLICT);
        assert_eq!(stop_steps(&worker("Idle")).unwrap_err().0, StatusCode::CONFLICT, "no worktree: nowhere for the tombstone");
        assert_eq!(serde_json::from_str::<ControlBody>(r#"{"action":"stop"}"#).unwrap().action, Action::Stop);
        assert_eq!(serde_json::from_str::<ControlBody>(r#"{"action":"restart"}"#).unwrap().action, Action::Restart);
    }
}
