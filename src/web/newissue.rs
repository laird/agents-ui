//! Filing a new issue from the village (#490):
//! `POST /api/swarms/{project}/issues` with `{ "title", "body", "labels" }`.
//!
//! It runs autocoder's `autocoder-ctl file` in the swarm's repo (#541,
//! `autocoder_ctl.rs`), so the issue lands in whichever backend autocoder is
//! set up for; an autocoder without the verb gets
//! `ProjectManagementClient::create_issue`, as the TUI's new-issue dialog
//! does. Either way each value is its own argument (no shell). The server checks what it is given: a title of 1–256 characters,
//! a body of at most 64 KB, and labels only from `ALLOWED_LABELS`, at most
//! one type and one priority. No `proposal` label, so the issue goes straight
//! into the queue. A `--read-only` server refuses it before it gets here
//! (403). Afterwards the village forgets its cached issues for the repo, so
//! the new crate shows in the backlog on the next poll.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::autocoder_ctl::{self, CtlOutcome};
use super::server::WebServerState;
use crate::project_management::{ProjectCommand, ProjectManagementClient};

pub const MAX_TITLE_CHARS: usize = 256;
pub const MAX_BODY_BYTES: usize = 64 * 1024;
pub const TYPES: &[&str] = &["enhancement", "bug"];
pub const PRIORITIES: &[&str] = &["P0", "P1", "P2", "P3"];

#[derive(Debug, Deserialize)]
pub struct NewIssueBody {
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
}

/// What is filed: the title, the body and the labels, checked.
pub struct Checked<'a> {
    pub title: &'a str,
    pub body: Option<&'a str>,
    pub labels: Vec<String>,
}

/// The `create` command for `new`, or why it can't be filed.
pub fn create_command(client: &ProjectManagementClient, new: &NewIssueBody) -> Result<ProjectCommand, &'static str> {
    let c = check(new)?;
    Ok(client.create_issue(c.title, Some(c.body.unwrap_or("")), &c.labels))
}

/// The `autocoder-ctl file` arguments for `new`, or why it can't be filed.
pub fn ctl_args(new: &NewIssueBody) -> Result<Vec<String>, &'static str> {
    let c = check(new)?;
    let mut args = vec!["file".to_string(), "--title".into(), c.title.into()];
    if let Some(b) = c.body {
        args.extend(["--body".into(), b.into()]);
    }
    for l in c.labels {
        args.extend(["--label".into(), l]);
    }
    Ok(args)
}

/// Checks a new issue against the rules in the header.
pub fn check(new: &NewIssueBody) -> Result<Checked<'_>, &'static str> {
    let title = new.title.trim();
    if title.is_empty() {
        return Err("give it a title");
    }
    if title.chars().count() > MAX_TITLE_CHARS || title.contains(['\n', '\r']) {
        return Err("the title must be one line of at most 256 characters");
    }
    let body = new.body.as_deref().map(str::trim).filter(|b| !b.is_empty());
    if body.is_some_and(|b| b.len() > MAX_BODY_BYTES) {
        return Err("the body is longer than 64 KB");
    }
    let mut labels: Vec<String> = Vec::new();
    for l in &new.labels {
        if !TYPES.contains(&l.as_str()) && !PRIORITIES.contains(&l.as_str()) {
            return Err("labels must be enhancement or bug, and P0 to P3");
        }
        if !labels.contains(l) {
            labels.push(l.clone());
        }
    }
    let count = |set: &[&str]| labels.iter().filter(|l| set.contains(&l.as_str())).count();
    if count(TYPES) > 1 || count(PRIORITIES) > 1 {
        return Err("pick one type and one priority");
    }
    Ok(Checked { title, body, labels })
}

/// The new issue's URL and number from what `gh issue create` prints.
pub fn created(stdout: &str) -> Option<(String, u32)> {
    let url = stdout.split_whitespace().rev().find(|w| w.starts_with("http"))?;
    let number = url.rsplit('/').next()?.parse().ok()?;
    Some((url.to_string(), number))
}

pub async fn api_new_issue_handler(
    Path(project): Path<String>,
    State(state): State<WebServerState>,
    Json(new): Json<NewIssueBody>,
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
    let cmd = create_command(&ProjectManagementClient::from_env(), &new).map_err(|why| fail(StatusCode::UNPROCESSABLE_ENTITY, why))?;
    let args = ctl_args(&new).map_err(|why| fail(StatusCode::UNPROCESSABLE_ENTITY, why))?;
    match autocoder_ctl::run(&state.transport, &repo, autocoder_ctl::find().as_deref(), &args).await {
        CtlOutcome::Done { target, detail } => {
            state.village.forget(&repo);
            let number = detail["number"].as_u64().or_else(|| target.and_then(|t| t.parse().ok()));
            return Ok(Json(match number {
                Some(n) => json!({ "ok": true, "number": n }),
                None => json!({ "ok": true }),
            }));
        }
        CtlOutcome::Failed(why) => {
            tracing::warn!("filing an issue in {project} failed: {why}");
            return Err(fail(StatusCode::BAD_GATEWAY, &why));
        }
        CtlOutcome::Unavailable => {} // too old: create_command's gh below
    }
    let out = state
        .transport
        .output(cmd.program, &cmd.args, Some(&repo))
        .await
        .map_err(|_| fail(StatusCode::BAD_GATEWAY, "could not reach the tracker"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        tracing::warn!("filing an issue in {project} failed: {err}");
        return Err(fail(StatusCode::BAD_GATEWAY, if err.is_empty() { "the tracker refused it" } else { &err }));
    }
    state.village.forget(&repo);
    let stdout = String::from_utf8_lossy(&out.stdout);
    Ok(Json(match created(&stdout) {
        Some((url, number)) => json!({ "ok": true, "number": number, "url": url }),
        None => json!({ "ok": true }),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new(title: &str, body: Option<&str>, labels: &[&str]) -> NewIssueBody {
        NewIssueBody { title: title.into(), body: body.map(Into::into), labels: labels.iter().map(|l| l.to_string()).collect() }
    }
    fn gh() -> ProjectManagementClient {
        ProjectManagementClient::github()
    }

    #[test]
    fn files_with_its_type_and_priority_as_separate_arguments() {
        let cmd = create_command(&gh(), &new(" Fix the ship ", Some("it sinks; rm -rf $HOME"), &["bug", "P2"])).unwrap();
        assert_eq!(cmd.program, "gh");
        assert_eq!(cmd.args, vec!["issue", "create", "--title", "Fix the ship", "--body", "it sinks; rm -rf $HOME", "--label", "bug,P2"]);
    }

    #[test]
    fn a_missing_body_is_filed_empty() {
        // gh asks interactively for a body it isn't given.
        let cmd = create_command(&gh(), &new("T", None, &[])).unwrap();
        assert_eq!(cmd.args, vec!["issue", "create", "--title", "T", "--body", ""]);
    }

    #[test]
    fn the_title_is_one_line_of_1_to_256_characters() {
        assert!(create_command(&gh(), &new("   ", None, &[])).is_err());
        assert!(create_command(&gh(), &new("a\nb", None, &[])).is_err());
        assert!(create_command(&gh(), &new(&"é".repeat(256), None, &[])).is_ok());
        assert!(create_command(&gh(), &new(&"é".repeat(257), None, &[])).is_err());
    }

    #[test]
    fn the_body_is_at_most_64_kb() {
        assert!(create_command(&gh(), &new("T", Some(&"x".repeat(MAX_BODY_BYTES)), &[])).is_ok());
        assert!(create_command(&gh(), &new("T", Some(&"x".repeat(MAX_BODY_BYTES + 1)), &[])).is_err());
    }

    #[test]
    fn labels_come_only_from_the_allowlist_one_of_each() {
        assert!(create_command(&gh(), &new("T", None, &["proposal"])).is_err());
        assert!(create_command(&gh(), &new("T", None, &["--repo=evil/x"])).is_err());
        assert!(create_command(&gh(), &new("T", None, &["bug", "enhancement"])).is_err());
        assert!(create_command(&gh(), &new("T", None, &["P0", "P3"])).is_err());
        let cmd = create_command(&gh(), &new("T", None, &["P3", "P3", "enhancement"])).unwrap();
        assert_eq!(cmd.args.last().unwrap(), "P3,enhancement");
    }

    #[test]
    fn autocoder_ctl_files_with_the_same_checks() {
        assert_eq!(
            ctl_args(&new(" Fix the ship ", Some("it sinks"), &["bug", "P2"])).unwrap(),
            vec!["file", "--title", "Fix the ship", "--body", "it sinks", "--label", "bug", "--label", "P2"]
        );
        assert_eq!(ctl_args(&new("T", Some("  "), &[])).unwrap(), vec!["file", "--title", "T"]);
        assert!(ctl_args(&new("T", None, &["proposal"])).is_err());
        assert!(ctl_args(&new("a\nb", None, &[])).is_err());
    }

    #[test]
    fn reads_the_new_issue_from_what_gh_prints() {
        let out = "Creating issue in laird/agents-ui\n\nhttps://github.com/laird/agents-ui/issues/512\n";
        assert_eq!(created(out), Some(("https://github.com/laird/agents-ui/issues/512".into(), 512)));
        assert_eq!(created("nothing"), None);
    }
}
