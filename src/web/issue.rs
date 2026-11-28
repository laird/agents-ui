//! One issue's full detail for its dialog in the village (#564): number,
//! title, body, labels, priority, url, state, why it's blocked, and its
//! recent comments.
//!
//! `GET /api/swarms/{project}/issues/{number}` serves it, read on demand
//! (one `gh issue view`) and kept for a minute, like the travel log
//! (`travel.rs`, #495), so it works on a `--read-only` server.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::server::WebServerState;
use super::source::TtlCache;

/// A label as `gh issue view --json labels` gives it.
#[derive(Debug, Clone, Default, Deserialize)]
struct RawLabel {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RawAuthor {
    #[serde(default)]
    login: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RawComment {
    #[serde(default)]
    author: RawAuthor,
    #[serde(default)]
    body: String,
    #[serde(default, rename = "createdAt")]
    created_at: String,
}

/// An issue as `gh issue view --json` gives it, with the fields this dialog
/// needs: the travel log (`travel.rs`) and the returned-work scan
/// (`returned.rs`) each read their own narrower shape of the same issue.
#[derive(Debug, Clone, Default, Deserialize)]
struct RawIssue {
    #[serde(default)]
    number: u32,
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    labels: Vec<RawLabel>,
    #[serde(default)]
    comments: Vec<RawComment>,
}

/// One comment in the issue dialog.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct IssueComment {
    pub author: String,
    pub created_at: String,
    pub body: String,
}

/// An issue's full detail for its dialog (#564).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct IssueDetail {
    pub number: u32,
    pub title: String,
    #[serde(default)]
    pub body: String,
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub state: String,
    /// Why it's blocked (#564): the newest comment starting with `🚫
    /// **Blocked`, else the newest comment, else absent — the dialog then
    /// says "no reason recorded".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
    pub comments: Vec<IssueComment>,
}

/// The issue's own priority label, `P0`..`P3`, if it has one.
fn priority_of(labels: &[String]) -> Option<String> {
    labels.iter().find(|l| matches!(l.as_str(), "P0" | "P1" | "P2" | "P3")).cloned()
}

/// A comment that announces a block: the worker's reason and question.
fn is_blocked_comment(body: &str) -> bool {
    body.trim_start().starts_with("🚫 **Blocked")
}

/// Why it's blocked, from the newest comment that announces one, else the
/// newest comment, else `None` (the dialog says "no reason recorded").
fn blocked_reason_of(comments: &[RawComment]) -> Option<String> {
    comments
        .iter()
        .rev()
        .find(|c| is_blocked_comment(&c.body))
        .or_else(|| comments.last())
        .map(|c| c.body.clone())
        .filter(|b| !b.is_empty())
}

fn detail_of(issue: RawIssue) -> IssueDetail {
    let labels: Vec<String> = issue.labels.into_iter().map(|l| l.name).filter(|n| !n.is_empty()).collect();
    let priority = priority_of(&labels);
    let blocked_reason = blocked_reason_of(&issue.comments);
    let comments = issue
        .comments
        .into_iter()
        .map(|c| IssueComment { author: c.author.login, created_at: c.created_at, body: c.body })
        .collect();
    IssueDetail {
        number: issue.number,
        title: issue.title,
        body: issue.body,
        labels,
        priority,
        url: (!issue.url.is_empty()).then_some(issue.url),
        state: issue.state,
        blocked_reason,
        comments,
    }
}

/// The issue fields `gh issue view` is asked for.
const ISSUE_FIELDS: &str = "number,title,body,state,url,labels,comments";

/// Details read in the last minute, per repo and issue.
static DETAILS: LazyLock<TtlCache<(PathBuf, u32), Option<IssueDetail>>> = LazyLock::new(|| TtlCache::new(Duration::from_secs(60)));

/// Read issue `n` of `repo`; `None` when `gh` can't show it.
async fn read(transport: &crate::transport::ServerTransport, repo: &Path, n: u32) -> Option<IssueDetail> {
    let args: Vec<String> = ["issue", "view", &n.to_string(), "--json", ISSUE_FIELDS].iter().map(|s| s.to_string()).collect();
    let out = crate::github::gh_repo_read(transport, repo, &args).await.ok()?;
    if !out.status.success() {
        return None;
    }
    let issue: RawIssue = serde_json::from_slice(&out.stdout).ok()?;
    Some(detail_of(issue))
}

/// `GET /api/swarms/{project}/issues/{number}`: the issue's full detail.
pub async fn api_issue_handler(
    UrlPath((project, n)): UrlPath<(String, u32)>,
    State(state): State<WebServerState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let fail = |code: StatusCode, msg: &str| (code, Json(json!({ "error": msg })));
    let repo = state
        .swarms
        .read()
        .ok()
        .and_then(|s| s.iter().find(|s| s.project_name == project).map(|s| s.repo_path.clone()))
        .filter(|r| !r.is_empty())
        .ok_or_else(|| fail(StatusCode::NOT_FOUND, "no such swarm"))?;
    let repo = PathBuf::from(repo);
    let transport = &state.transport;
    DETAILS
        .get_or((repo.clone(), n), || read(transport, &repo, n))
        .await
        .map(|d| Json(json!(d)))
        .ok_or_else(|| fail(StatusCode::BAD_GATEWAY, "could not read the issue from GitHub"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(json: &str) -> RawIssue {
        serde_json::from_str(json).unwrap()
    }

    const BLOCKED: &str = r#"{"number": 7, "title": "Saved carts", "body": "Do the thing.", "state": "OPEN",
      "url": "https://github.com/laird/agents-ui/issues/7",
      "labels": [{"name": "needs-design"}, {"name": "P1"}],
      "comments": [
        {"author": {"login": "worker-1"}, "body": "🔒 claim worker-1 1", "createdAt": "2026-11-10T10:00:00Z"},
        {"author": {"login": "worker-1"}, "body": "🚫 **Blocked**: which column should it key on?", "createdAt": "2026-11-10T11:00:00Z"},
        {"author": {"login": "laird"}, "body": "Use the account id.", "createdAt": "2026-11-10T12:00:00Z"}
      ]}"#;

    #[test]
    fn a_blocked_issue_carries_its_labels_priority_and_reason() {
        let d = detail_of(issue(BLOCKED));
        assert_eq!(d.number, 7);
        assert_eq!(d.labels, vec!["needs-design", "P1"]);
        assert_eq!(d.priority.as_deref(), Some("P1"));
        assert_eq!(d.url.as_deref(), Some("https://github.com/laird/agents-ui/issues/7"));
        // The newest 🚫 **Blocked comment, not the later unrelated reply.
        assert_eq!(d.blocked_reason.as_deref(), Some("🚫 **Blocked**: which column should it key on?"));
        assert_eq!(d.comments.len(), 3);
        assert_eq!(d.comments[1].author, "worker-1");
    }

    /// With no `🚫 **Blocked` comment, the newest comment stands in.
    #[test]
    fn without_a_blocked_comment_the_newest_comment_stands_in() {
        let plain = r#"{"number": 8, "title": "x", "state": "OPEN", "comments": [
          {"author": {"login": "a"}, "body": "first", "createdAt": "2026-11-10T10:00:00Z"},
          {"author": {"login": "b"}, "body": "latest note", "createdAt": "2026-11-10T11:00:00Z"}
        ]}"#;
        let d = detail_of(issue(plain));
        assert_eq!(d.blocked_reason.as_deref(), Some("latest note"));
    }

    /// With no comments at all, there is no reason to carry; the dialog
    /// says "no reason recorded" itself.
    #[test]
    fn with_no_comments_there_is_no_reason() {
        let none = r#"{"number": 9, "title": "x", "state": "OPEN", "comments": []}"#;
        let d = detail_of(issue(none));
        assert_eq!(d.blocked_reason, None);
    }

    #[test]
    fn an_issue_with_no_priority_label_has_none() {
        let d = detail_of(issue(r#"{"number": 10, "title": "x", "state": "OPEN", "labels": [{"name": "bug"}]}"#));
        assert_eq!(d.priority, None);
    }

    #[test]
    fn blocked_comments_are_recognised() {
        assert!(is_blocked_comment("🚫 **Blocked**: need a decision"));
        assert!(!is_blocked_comment("Not blocked"));
        assert!(!is_blocked_comment("🔒 claim worker-1 1"));
    }
}
