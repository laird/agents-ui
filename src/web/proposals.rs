//! Deciding a proposal from the village board (#483):
//! `POST /api/swarms/{project}/issues/{number}/decision` with
//! `{ "action": "approve" | "reject" | "changes", "comment": "..." }`.
//!
//! - **approve** says so on the issue, then removes the `proposal` label, as
//!   the TUI's approve key does (`App::approve_selected_issue`). Without the
//!   label the issue is claimable, so its crate leaves the board for the docks.
//! - **reject** closes the issue, with the reason when one is given.
//! - **changes** posts the comment (required) and leaves it a proposal, for
//!   `/refine-proposal` to answer.
//!
//! Each goes through autocoder's `autocoder-ctl approve | reject |
//! request-changes` (#541, `autocoder_ctl.rs`), so it works with any issue
//! backend autocoder is set up for; autocoder writes the comment, naming who
//! decided. `approve` there takes no comment, so an approval with one is
//! refused (422; the board never sends one). An autocoder without the verbs
//! gets the `gh` commands of `decision_commands` instead.
//!
//! Only an open issue labelled `proposal` can be decided here: anything else
//! is a 409, so the endpoint can't close or relabel an arbitrary issue. A
//! `--read-only` server refuses it before it gets here (403), as it does
//! question answers. After a decision the village forgets its cached issues
//! for the repo, so the next poll shows the crate move.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::autocoder_ctl::{self, CtlOutcome};
use super::server::WebServerState;
use crate::project_management::{ProjectCommand, ProjectManagementClient};

/// The longest comment accepted, in bytes.
pub const MAX_COMMENT_BYTES: usize = 4000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Approve,
    Reject,
    Changes,
}

#[derive(Debug, Deserialize)]
pub struct DecisionBody {
    pub action: Action,
    #[serde(default)]
    pub comment: Option<String>,
}

/// The tracker commands a decision runs, in order; or why it can't run.
pub fn decision_commands(
    client: &ProjectManagementClient,
    number: u32,
    action: Action,
    comment: Option<&str>,
) -> Result<Vec<ProjectCommand>, &'static str> {
    let comment = comment.map(str::trim).filter(|c| !c.is_empty());
    if comment.is_some_and(|c| c.len() > MAX_COMMENT_BYTES) {
        return Err("the comment is too long");
    }
    let with = |head: &str| match comment {
        Some(c) => format!("{head}\n\n{c}"),
        None => head.to_string(),
    };
    Ok(match action {
        // The comment first: if removing the label then fails, the issue is
        // still a proposal and a retry works (a 409 would say it was decided).
        Action::Approve => vec![
            client.comment_issue(number, &with("✅ Approved from the village board.")),
            client.remove_label(number, "proposal"),
        ],
        Action::Reject => vec![client.close_issue(number, &with("❌ Rejected from the village board."))],
        Action::Changes => {
            let Some(_) = comment else { return Err("say what should change") };
            vec![client.comment_issue(number, &with("✏️ Changes requested from the village board:"))]
        }
    })
}

/// The `autocoder-ctl` verb and arguments for a decision (#541). Run after
/// `decision_commands` has checked the comment.
pub fn ctl_args(number: u32, action: Action, comment: Option<&str>) -> Result<Vec<String>, &'static str> {
    let comment = comment.map(str::trim).filter(|c| !c.is_empty());
    let n = number.to_string();
    Ok(match (action, comment) {
        (Action::Approve, None) => vec!["approve".into(), n],
        (Action::Approve, Some(_)) => return Err("an approval takes no comment"),
        (Action::Reject, None) => vec!["reject".into(), n],
        (Action::Reject, Some(c)) => vec!["reject".into(), n, c.into()],
        (Action::Changes, Some(c)) => vec!["request-changes".into(), n, c.into()],
        (Action::Changes, None) => return Err("say what should change"),
    })
}

/// Whether `gh issue view --json state,labels` shows an open proposal.
pub fn is_open_proposal(view: &Value) -> bool {
    let open = view["state"].as_str().is_some_and(|s| s.eq_ignore_ascii_case("open"));
    let proposal = view["labels"]
        .as_array()
        .is_some_and(|ls| ls.iter().any(|l| l["name"].as_str().or(l.as_str()) == Some("proposal")));
    open && proposal
}

pub async fn api_decision_handler(
    Path((project, number)): Path<(String, u32)>,
    State(state): State<WebServerState>,
    Json(body): Json<DecisionBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let fail = |code: StatusCode, msg: &str| (code, Json(json!({ "error": msg })));
    let repo = state
        .swarms
        .read()
        .ok()
        .and_then(|s| s.iter().find(|s| s.project_name == project).map(|s| s.repo_path.clone()))
        .filter(|r| !r.is_empty())
        .ok_or_else(|| fail(StatusCode::NOT_FOUND, "no such swarm"))?;
    let repo = std::path::PathBuf::from(repo);
    let client = ProjectManagementClient::from_env();
    let commands = decision_commands(&client, number, body.action, body.comment.as_deref())
        .map_err(|why| fail(StatusCode::UNPROCESSABLE_ENTITY, why))?;

    let view = client.view_issue_json(number, "state,labels");
    let out = state
        .transport
        .output(view.program, &view.args, Some(&repo))
        .await
        .map_err(|_| fail(StatusCode::BAD_GATEWAY, "could not ask the tracker"))?;
    let seen: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    if !out.status.success() || !is_open_proposal(&seen) {
        return Err(fail(StatusCode::CONFLICT, "that issue is no longer an open proposal"));
    }

    let ctl = autocoder_ctl::find();
    let args = ctl_args(number, body.action, body.comment.as_deref());
    if ctl.is_some() {
        let args = args.map_err(|why| fail(StatusCode::UNPROCESSABLE_ENTITY, why))?;
        match autocoder_ctl::run(&state.transport, &repo, ctl.as_deref(), &args).await {
            CtlOutcome::Done { .. } => {
                state.village.forget(&repo);
                return Ok(Json(json!({ "ok": true, "number": number, "action": body.action_word() })));
            }
            CtlOutcome::Failed(why) => {
                state.village.forget(&repo);
                return Err(fail(StatusCode::BAD_GATEWAY, &why));
            }
            CtlOutcome::Unavailable => {} // too old: the gh commands below
        }
    }

    for cmd in &commands {
        let out = state
            .transport
            .output(cmd.program, &cmd.args, Some(&repo))
            .await
            .map_err(|_| fail(StatusCode::BAD_GATEWAY, "could not reach the tracker"))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            tracing::warn!("decision on #{number} failed: {err}");
            state.village.forget(&repo);
            return Err(fail(StatusCode::BAD_GATEWAY, if err.is_empty() { "the tracker refused it" } else { &err }));
        }
    }
    state.village.forget(&repo);
    Ok(Json(json!({ "ok": true, "number": number, "action": body.action_word() })))
}

impl DecisionBody {
    fn action_word(&self) -> &'static str {
        match self.action {
            Action::Approve => "approved",
            Action::Reject => "rejected",
            Action::Changes => "changes requested",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gh() -> ProjectManagementClient {
        ProjectManagementClient::github()
    }

    #[test]
    fn approving_says_so_then_removes_the_proposal_label() {
        let cmds = decision_commands(&gh(), 7, Action::Approve, None).unwrap();
        assert_eq!(cmds[0].args[..3], ["issue", "comment", "7"]);
        assert_eq!(cmds[1].args, vec!["issue", "edit", "7", "--remove-label", "proposal"]);
        assert_eq!(cmds.len(), 2);
    }

    #[test]
    fn rejecting_closes_with_the_reason() {
        let cmds = decision_commands(&gh(), 7, Action::Reject, Some("  out of scope ")).unwrap();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].args[..3], ["issue", "close", "7"]);
        assert!(cmds[0].args[4].ends_with("\n\nout of scope"), "{:?}", cmds[0].args);
    }

    #[test]
    fn asking_for_changes_needs_words_and_keeps_the_label() {
        assert!(decision_commands(&gh(), 7, Action::Changes, None).is_err());
        assert!(decision_commands(&gh(), 7, Action::Changes, Some("   ")).is_err());
        let cmds = decision_commands(&gh(), 7, Action::Changes, Some("smaller, please")).unwrap();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].args[..3], ["issue", "comment", "7"]);
        assert!(cmds[0].args[4].contains("smaller, please"));
    }

    #[test]
    fn a_long_comment_is_refused() {
        let long = "x".repeat(MAX_COMMENT_BYTES + 1);
        assert!(decision_commands(&gh(), 7, Action::Approve, Some(&long)).is_err());
    }

    #[test]
    fn autocoder_ctl_gets_one_verb_per_decision() {
        assert_eq!(ctl_args(7, Action::Approve, None).unwrap(), vec!["approve", "7"]);
        assert_eq!(ctl_args(7, Action::Approve, Some("  ")).unwrap(), vec!["approve", "7"]);
        assert!(ctl_args(7, Action::Approve, Some("ship it")).is_err());
        assert_eq!(ctl_args(7, Action::Reject, None).unwrap(), vec!["reject", "7"]);
        assert_eq!(ctl_args(7, Action::Reject, Some(" out of scope ")).unwrap(), vec!["reject", "7", "out of scope"]);
        assert_eq!(ctl_args(7, Action::Changes, Some("smaller")).unwrap(), vec!["request-changes", "7", "smaller"]);
        assert!(ctl_args(7, Action::Changes, None).is_err());
    }

    #[test]
    fn only_an_open_proposal_can_be_decided() {
        assert!(is_open_proposal(&json!({ "state": "OPEN", "labels": [{ "name": "P3" }, { "name": "proposal" }] })));
        assert!(!is_open_proposal(&json!({ "state": "CLOSED", "labels": [{ "name": "proposal" }] })));
        assert!(!is_open_proposal(&json!({ "state": "OPEN", "labels": [{ "name": "needs-design" }] })));
        assert!(!is_open_proposal(&Value::Null));
    }

    #[test]
    fn the_body_names_its_action() {
        let b: DecisionBody = serde_json::from_value(json!({ "action": "changes", "comment": "hi" })).unwrap();
        assert_eq!(b.action, Action::Changes);
        assert!(serde_json::from_value::<DecisionBody>(json!({ "action": "delete" })).is_err());
    }
}
