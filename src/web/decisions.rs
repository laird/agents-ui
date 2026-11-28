//! Deciding any blocked issue from its village dialog (#565, following #564):
//! `POST /api/swarms/{project}/issues/{number}/decide` with
//! `{ "action": "approve" | "answer" | "reject", "comment": "..." }`.
//!
//! A proposal keeps its own flow (`proposals.rs`, `proposals.js`, their
//! Approve/Ask for changes/Reject). This is for every *other* blocking
//! label, grouped by what the dialog offers:
//!
//! - `needs-approval`, `too-complex`, `future`, `blocked`: **Approve** (an
//!   optional note, then the label is removed) or **Reject** (closed as not
//!   planned, with a reason).
//! - `needs-clarification`, `needs-design`, `needs-feedback`: **Answer**
//!   (the text is posted as a comment, then the label is removed) or
//!   **Reject**.
//!
//! Each goes through autocoder-ctl's `approve` / `reject` verbs where it has
//! them (#541, as `proposals.rs` does); an approval *with* a note, and every
//! `answer` (autocoder-ctl has no verb for it yet — the workflow belongs in
//! laird/agents, not here, so that repo is where a verb for it should be
//! proposed), fall back to `gh` directly. Only an open issue carrying one of the labels above
//! can be decided here: anything else is a 409. A `--read-only` server
//! refuses it before it gets here (403), same as `/decision`.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};

use super::autocoder_ctl::{self, CtlOutcome};
use super::server::WebServerState;
use crate::project_management::{ProjectCommand, ProjectManagementClient};

/// The longest comment accepted, in bytes (same limit as `/decision`).
pub const MAX_COMMENT_BYTES: usize = 4000;

/// Gets Approve or Reject.
const APPROVE_LABELS: &[&str] = &["needs-approval", "too-complex", "future", "blocked"];
/// Gets Answer or Reject.
const ANSWER_LABELS: &[&str] = &["needs-clarification", "needs-design", "needs-feedback"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Approve,
    Answer,
    Reject,
}

#[derive(Debug, Deserialize)]
pub struct DecideBody {
    pub action: Action,
    #[serde(default)]
    pub comment: Option<String>,
}

/// Which group a blocking label's issue is offered: `"approve"` or
/// `"answer"`; neither for a label this endpoint doesn't decide (a
/// proposal's label, or no blocking label at all).
fn label_group(name: &str) -> Option<&'static str> {
    if APPROVE_LABELS.contains(&name) {
        Some("approve")
    } else if ANSWER_LABELS.contains(&name) {
        Some("answer")
    } else {
        None
    }
}

/// The first label `gh issue view --json state,labels` shows that this
/// endpoint decides, on an open issue; `None` otherwise (closed, no such
/// label, or only a proposal's — that stays `/decision`'s).
pub fn decidable_label(view: &Value) -> Option<String> {
    let open = view["state"].as_str().is_some_and(|s| s.eq_ignore_ascii_case("open"));
    if !open {
        return None;
    }
    view["labels"].as_array()?.iter().filter_map(|l| l["name"].as_str().or(l.as_str())).find(|n| label_group(n).is_some()).map(str::to_string)
}

/// The tracker commands a decision runs, in order; or why it can't run.
pub fn decide_commands(
    client: &ProjectManagementClient,
    number: u32,
    label: &str,
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
        // The comment (or label removal below) first: if the other step then
        // fails, the issue still carries the label and a retry works.
        Action::Approve => vec![client.comment_issue(number, &with("✅ Approved from the village board.")), client.remove_label(number, label)],
        Action::Answer => {
            let Some(c) = comment else { return Err("say what the answer is") };
            vec![client.comment_issue(number, c), client.remove_label(number, label)]
        }
        Action::Reject => vec![client.close_issue(number, &with("❌ Rejected from the village board."))],
    })
}

/// The `autocoder-ctl` verb and arguments for a decision (#541); `None` when
/// there isn't one yet (an approval with a note, or any answer — the `gh`
/// commands above run instead).
pub fn ctl_args(number: u32, action: Action, comment: Option<&str>) -> Option<Vec<String>> {
    let comment = comment.map(str::trim).filter(|c| !c.is_empty());
    let n = number.to_string();
    match (action, comment) {
        (Action::Approve, None) => Some(vec!["approve".into(), n]),
        (Action::Reject, None) => Some(vec!["reject".into(), n]),
        (Action::Reject, Some(c)) => Some(vec!["reject".into(), n, c.into()]),
        _ => None,
    }
}

pub async fn api_decide_handler(
    Path((project, number)): Path<(String, u32)>,
    State(state): State<WebServerState>,
    Json(body): Json<DecideBody>,
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

    let view = client.view_issue_json(number, "state,labels");
    let out = state.transport.output(view.program, &view.args, Some(&repo)).await.map_err(|_| fail(StatusCode::BAD_GATEWAY, "could not ask the tracker"))?;
    let seen: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    let label = out.status.success().then(|| decidable_label(&seen)).flatten();
    let Some(label) = label else {
        return Err(fail(StatusCode::CONFLICT, "that issue has no blocking label this dialog can decide"));
    };
    let group = label_group(&label).expect("decidable_label only returns a label with a group");
    let wrong_action = match body.action {
        Action::Approve => group != "approve",
        Action::Answer => group != "answer",
        Action::Reject => false,
    };
    if wrong_action {
        return Err(fail(StatusCode::UNPROCESSABLE_ENTITY, "that action isn't offered for this issue's label"));
    }

    let commands =
        decide_commands(&client, number, &label, body.action, body.comment.as_deref()).map_err(|why| fail(StatusCode::UNPROCESSABLE_ENTITY, why))?;

    // An approval with a note, or any answer, has no autocoder-ctl verb: gh below.
    let ctl = autocoder_ctl::find();
    if let (Some(_), Some(args)) = (&ctl, ctl_args(number, body.action, body.comment.as_deref())) {
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
        let out = state.transport.output(cmd.program, &cmd.args, Some(&repo)).await.map_err(|_| fail(StatusCode::BAD_GATEWAY, "could not reach the tracker"))?;
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

impl DecideBody {
    fn action_word(&self) -> &'static str {
        match self.action {
            Action::Approve => "approved",
            Action::Answer => "answered",
            Action::Reject => "rejected",
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
    fn approving_says_so_then_removes_the_labels_own_name() {
        let cmds = decide_commands(&gh(), 7, "needs-approval", Action::Approve, None).unwrap();
        assert_eq!(cmds[0].args[..3], ["issue", "comment", "7"]);
        assert_eq!(cmds[1].args, vec!["issue", "edit", "7", "--remove-label", "needs-approval"]);
    }

    #[test]
    fn answering_posts_the_text_then_removes_the_label() {
        assert!(decide_commands(&gh(), 7, "needs-design", Action::Answer, None).is_err());
        assert!(decide_commands(&gh(), 7, "needs-design", Action::Answer, Some("  ")).is_err());
        let cmds = decide_commands(&gh(), 7, "needs-design", Action::Answer, Some("use the account id")).unwrap();
        assert_eq!(cmds[0].args[..3], ["issue", "comment", "7"]);
        assert_eq!(cmds[0].args[4], "use the account id");
        assert_eq!(cmds[1].args, vec!["issue", "edit", "7", "--remove-label", "needs-design"]);
    }

    #[test]
    fn rejecting_closes_with_the_reason_whatever_the_label() {
        let cmds = decide_commands(&gh(), 7, "too-complex", Action::Reject, Some("needs a human")).unwrap();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].args[..3], ["issue", "close", "7"]);
        assert!(cmds[0].args[4].ends_with("\n\nneeds a human"));
    }

    #[test]
    fn a_long_comment_is_refused() {
        let long = "x".repeat(MAX_COMMENT_BYTES + 1);
        assert!(decide_commands(&gh(), 7, "needs-approval", Action::Approve, Some(&long)).is_err());
    }

    #[test]
    fn autocoder_ctl_has_no_verb_for_an_answer_or_a_noted_approval() {
        assert_eq!(ctl_args(7, Action::Approve, None), Some(vec!["approve".into(), "7".into()]));
        assert_eq!(ctl_args(7, Action::Approve, Some("ship it")), None);
        assert_eq!(ctl_args(7, Action::Reject, None), Some(vec!["reject".into(), "7".into()]));
        assert_eq!(ctl_args(7, Action::Reject, Some("out of scope")), Some(vec!["reject".into(), "7".into(), "out of scope".into()]));
        assert_eq!(ctl_args(7, Action::Answer, Some("use the account id")), None);
    }

    #[test]
    fn each_label_is_decidable_only_through_its_own_group() {
        let approve_issue = json!({ "state": "OPEN", "labels": [{ "name": "P1" }, { "name": "needs-approval" }] });
        assert_eq!(decidable_label(&approve_issue).as_deref(), Some("needs-approval"));
        assert_eq!(label_group("needs-approval"), Some("approve"));
        assert_eq!(label_group("needs-design"), Some("answer"));
        // A proposal's label is `/decision`'s, not this endpoint's.
        assert_eq!(decidable_label(&json!({ "state": "OPEN", "labels": [{ "name": "proposal" }] })), None);
        assert_eq!(decidable_label(&json!({ "state": "CLOSED", "labels": [{ "name": "needs-approval" }] })), None);
        assert_eq!(decidable_label(&json!({ "state": "OPEN", "labels": [] })), None);
        assert_eq!(decidable_label(&Value::Null), None);
    }

    #[test]
    fn the_body_names_its_action() {
        let b: DecideBody = serde_json::from_value(json!({ "action": "answer", "comment": "hi" })).unwrap();
        assert_eq!(b.action, Action::Answer);
        assert!(serde_json::from_value::<DecideBody>(json!({ "action": "delete" })).is_err());
    }
}
