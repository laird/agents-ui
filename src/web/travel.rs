//! An issue's travel log (#495): where its time went, from opened to released.
//!
//! One timeline per issue, from what the server already reads:
//!
//! - **opened**: the issue's creation;
//! - **approved**: the first comment that approves it ("Approved by …"),
//!   for a proposal; an issue nobody had to approve has no such stage;
//! - **claimed**: the last claim comment before it landed (the gate's
//!   `🔒 claim`, `/fix`'s lock or its "Automated Fix Started"), so an
//!   abandoned earlier attempt doesn't count;
//! - **first commit**: the earliest commit on the integration branch whose
//!   subject names the issue (`#N` or `issue-N`), by author date, which a
//!   rebase keeps;
//! - **landed**: closed as completed. On the way, each time the merge gate
//!   handed the work back (its comments and push retries, `gate.rs`, #431);
//! - one stage per environment (`nuc-1`, `release`): the first Deployment to
//!   it created at or after the landing. Deployments are of the integration
//!   branch's tip, so the first one after the landing carries the work. It
//!   counts when the deploy started, whatever its outcome.
//!
//! Each stage records how long it took since the stage before it, and the
//! slowest is named. [`stage_medians`] gives the median of each stage over
//! the issues landed in the GitHub summary's window, for the dialog's footer.
//!
//! `GET /api/swarms/{project}/issues/{number}/travel` serves one issue's log,
//! read on demand (one `gh issue view`, one local `git log`, one Deployments
//! listing) and kept for a minute. Nothing is written, so it works on a
//! `--read-only` server.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::deployments::Deployment;
use super::gate::{Bounce, comment_bounce, push_retry_bounces};
use super::github::{Issue, is_claim};
use super::server::WebServerState;
use super::source::TtlCache;

/// One stage the issue reached.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Stage {
    /// `opened`, `approved`, `claimed`, `first_commit`, `landed`, or
    /// `deployed:<environment>`.
    pub key: String,
    /// When (RFC 3339).
    pub at: String,
    /// Seconds since the stage before it; absent for `opened`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secs: Option<u64>,
}

/// Where an issue's time went.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TravelLog {
    pub number: u32,
    #[serde(default)]
    pub title: String,
    /// `open`, `landed`, or `closed` (closed without landing).
    pub state: String,
    /// The stages reached, in order.
    pub stages: Vec<Stage>,
    /// Each time the gate handed the work back, oldest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bounces: Vec<Bounce>,
    /// The key of the stage that took longest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slowest: Option<String>,
}

fn parse(at: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(at.trim()).ok()
}

/// A comment that approves a proposal: "Approved by @laird." and the like.
pub fn is_approval(body: &str) -> bool {
    let first = body.trim_start();
    first.starts_with("Approved") || first.starts_with("✅ Approved") || first.contains("Approved by @")
}

/// Whether a commit subject names issue `n`: `#n` or `issue-n`, not `#n0`.
pub fn names_issue(subject: &str, n: u32) -> bool {
    let n = n.to_string();
    ["#", "issue-"].iter().any(|p| {
        let pat = format!("{p}{n}");
        subject.match_indices(&pat).any(|(i, _)| !subject[i + pat.len()..].starts_with(|c: char| c.is_ascii_digit()))
    })
}

/// The travel log of `issue` (as `gh issue view --json` or `gh issue list
/// --json` gives it), from `log` (`git log --format=%aI%x09%s` of the
/// integration branch) and `deployments` (the repo's, any environment).
pub fn travel(issue: &Issue, log: &str, deployments: &[Deployment]) -> TravelLog {
    let n = issue.number;
    let landed = issue.landed().then(|| issue.closed_at.as_deref().and_then(parse)).flatten();
    let state = if landed.is_some() { "landed" } else if issue.state.eq_ignore_ascii_case("closed") { "closed" } else { "open" };
    // Comments up to the landing: after it, a claim or approval is noise.
    let before = |at: &DateTime<FixedOffset>| landed.is_none_or(|l| *at <= l);
    let comments: Vec<(DateTime<FixedOffset>, &str)> =
        issue.comments.iter().filter_map(|c| parse(&c.created_at).map(|t| (t, c.body.as_str()))).filter(|(t, _)| before(t)).collect();
    let approved = comments.iter().filter(|(_, b)| is_approval(b)).map(|(t, _)| *t).min();
    let claimed = comments.iter().filter(|(_, b)| is_claim(b)).map(|(t, _)| *t).max();
    let commits: Vec<(DateTime<FixedOffset>, &str)> =
        log.lines().filter_map(|l| l.split_once('\t')).filter_map(|(at, s)| parse(at).map(|t| (t, s))).collect();
    let first_commit = commits.iter().filter(|(t, s)| names_issue(s, n) && before(t)).map(|(t, _)| *t).min();

    let mut points: Vec<(String, DateTime<FixedOffset>)> = Vec::new();
    if let Some(t) = parse(&issue.created_at) {
        points.push(("opened".into(), t));
    }
    for (key, t) in [("approved", approved), ("claimed", claimed), ("first_commit", first_commit), ("landed", landed)] {
        if let Some(t) = t {
            points.push((key.into(), t));
        }
    }
    if let Some(l) = landed {
        let mut firsts: BTreeMap<&str, DateTime<FixedOffset>> = BTreeMap::new();
        for d in deployments.iter().filter(|d| !d.environment.is_empty()) {
            if let Some(t) = parse(&d.created_at).filter(|t| *t >= l) {
                let e = firsts.entry(d.environment.as_str()).or_insert(t);
                *e = (*e).min(t);
            }
        }
        let mut envs: Vec<_> = firsts.into_iter().collect();
        envs.sort_by_key(|(_, t)| *t);
        points.extend(envs.into_iter().map(|(e, t)| (format!("deployed:{e}"), t)));
    }

    let mut stages = Vec::with_capacity(points.len());
    let mut prev: Option<DateTime<FixedOffset>> = None;
    for (key, t) in points {
        // A stage recorded before the one it follows (a commit before the
        // claim) took no time of its own.
        let secs = prev.map(|p| (t - p).num_seconds().max(0) as u64);
        prev = Some(prev.map_or(t, |p| p.max(t)));
        stages.push(Stage { key, at: t.to_rfc3339(), secs });
    }
    let slowest = stages.iter().filter_map(|s| s.secs.map(|x| (x, &s.key))).max_by_key(|(x, _)| *x).filter(|(x, _)| *x > 0).map(|(_, k)| k.clone());

    let since = parse(&issue.created_at);
    let mut bounces: Vec<Bounce> = issue
        .comments
        .iter()
        .filter_map(|c| comment_bounce(&c.body, &c.created_at))
        .chain(push_retry_bounces(log).into_iter().filter(|(m, _)| *m == n).map(|(_, b)| b))
        .filter(|b| b.at.as_deref().and_then(parse).is_none_or(|t| since.is_none_or(|s| t >= s) && before(&t)))
        .collect();
    bounces.sort_by(|a, b| a.at.cmp(&b.at));
    TravelLog { number: n, title: issue.title.clone(), state: state.into(), stages, bounces, slowest }
}

/// The median seconds of each stage over the landed `logs`.
pub fn stage_medians<'a>(logs: impl IntoIterator<Item = &'a TravelLog>) -> BTreeMap<String, u64> {
    let mut by: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    for l in logs.into_iter().filter(|l| l.state == "landed") {
        for s in &l.stages {
            if let Some(x) = s.secs {
                by.entry(s.key.clone()).or_default().push(x);
            }
        }
    }
    by.into_iter()
        .map(|(k, mut xs)| {
            xs.sort_unstable();
            let n = xs.len();
            (k, if n % 2 == 1 { xs[n / 2] } else { (xs[n / 2 - 1] + xs[n / 2]) / 2 })
        })
        .collect()
}

/// The stage medians over the issues in `issues` (`gh issue list --json`)
/// that landed, from the same `log` and `deployments` as [`travel`].
pub fn medians_of(issues: &[u8], log: &str, deployments: &[Deployment]) -> BTreeMap<String, u64> {
    let issues: Vec<Issue> = serde_json::from_slice(issues).unwrap_or_default();
    let logs: Vec<TravelLog> = issues.iter().filter(|i| i.landed()).map(|i| travel(i, log, deployments)).collect();
    stage_medians(&logs)
}

/// The issue fields `gh issue view` is asked for.
pub const ISSUE_FIELDS: &str = "number,title,state,createdAt,closedAt,stateReason,comments";

/// Logs read in the last minute, per repo and issue.
static LOGS: LazyLock<TtlCache<(PathBuf, u32), Option<TravelLog>>> = LazyLock::new(|| TtlCache::new(Duration::from_secs(60)));

/// Read issue `n` of `repo` and its commits and deployments; `None` when `gh`
/// can't show the issue.
async fn read(transport: &crate::transport::ServerTransport, repo: &Path, n: u32) -> Option<TravelLog> {
    let args: Vec<String> = ["issue", "view", &n.to_string(), "--json", ISSUE_FIELDS].iter().map(|s| s.to_string()).collect();
    let out = crate::github::gh_repo_output(transport, repo, &args).await.ok()?;
    if !out.status.success() {
        return None;
    }
    let issue: Issue = serde_json::from_slice(&out.stdout).ok()?;
    let integration = super::village::integration_branch(transport, repo).await;
    // Commits since the issue was opened: its own, and the gate's re-syncs.
    let since = format!("--since={}", issue.created_at);
    let git: Vec<String> = vec!["log".into(), format!("origin/{integration}"), since, "--format=%aI%x09%s".into()];
    let (log, deployments) = futures::join!(
        transport.output("git", &git, Some(repo)),
        super::deployments::gh_api(transport, repo, "repos/{owner}/{repo}/deployments?per_page=100"),
    );
    let log = log.ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let deployments = deployments.map(|j| super::deployments::parse_deployments(&j)).unwrap_or_default();
    Some(travel(&issue, &log, &deployments))
}

/// `GET /api/swarms/{project}/issues/{number}/travel`: the issue's travel log.
pub async fn api_travel_handler(
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
    LOGS.get_or((repo.clone(), n), || read(transport, &repo, n))
        .await
        .map(|l| Json(json!(l)))
        .ok_or_else(|| fail(StatusCode::BAD_GATEWAY, "could not read the issue from GitHub"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(json: &str) -> Issue {
        serde_json::from_str(json).unwrap()
    }

    fn dep(env: &str, at: &str) -> Deployment {
        Deployment { id: 1, sha: "abc".into(), created_at: at.into(), environment: env.into(), description: None, creator: None }
    }

    /// A proposal that waited for approval, was claimed, bounced at the gate
    /// once, landed, and went to nuc-1 and then out as a release.
    const LANDED: &str = r#"{"number": 495, "title": "Travel log", "state": "CLOSED", "stateReason": "COMPLETED",
      "createdAt": "2026-11-10T10:00:00Z", "closedAt": "2026-11-12T12:00:00Z",
      "comments": [
        {"body": "Approved by @laird.", "createdAt": "2026-11-11T10:00:00Z"},
        {"body": "🔒 claim nuc-1 1", "createdAt": "2026-11-11T11:00:00Z"},
        {"body": "🔒 claim nuc-2 2", "createdAt": "2026-11-12T08:00:00Z"},
        {"body": "⚠️ **Auto-merge to master hit a conflict**\n\nsrc/web/server.rs", "createdAt": "2026-11-12T11:00:00Z"},
        {"body": "Approved by someone, much later", "createdAt": "2026-11-13T10:00:00Z"}
      ]}"#;
    const LOG: &str = "2026-11-12T13:00:00+00:00\tUnrelated #4950\n\
        2026-11-12T11:30:00+00:00\tRe-sync origin/master into feature/issue-495 (push retry 1)\n\
        2026-11-12T09:00:00+00:00\tImplement #495: travel log\n\
        2026-11-12T08:30:00+00:00\tSomething else #49\n";

    #[test]
    fn a_landed_issue_travels_from_opened_to_released() {
        let deps = [dep("release", "2026-11-12T12:20:00Z"), dep("nuc-1", "2026-11-12T12:10:00Z"), dep("nuc-1", "2026-11-12T12:40:00Z"), dep("nuc-1", "2026-11-12T11:00:00Z")];
        let log = travel(&issue(LANDED), LOG, &deps);
        assert_eq!(log.state, "landed");
        let keys: Vec<_> = log.stages.iter().map(|s| (s.key.as_str(), s.secs)).collect();
        assert_eq!(
            keys,
            vec![
                ("opened", None),
                ("approved", Some(24 * 3600)),
                // The last claim before the landing, not the abandoned first.
                ("claimed", Some(22 * 3600)),
                ("first_commit", Some(3600)),
                ("landed", Some(3 * 3600)),
                // The first deployment after the landing, not the one before it.
                ("deployed:nuc-1", Some(600)),
                ("deployed:release", Some(600)),
            ]
        );
        assert_eq!(log.slowest.as_deref(), Some("approved"));
        let kinds: Vec<_> = log.bounces.iter().map(|b| b.kind).collect();
        assert_eq!(kinds, vec![super::super::gate::BounceKind::Conflict, super::super::gate::BounceKind::PushRetry]);
    }

    #[test]
    fn an_open_issue_has_only_the_stages_it_reached() {
        let open = issue(r#"{"number": 7, "state": "OPEN", "createdAt": "2026-11-10T10:00:00Z",
            "comments": [{"body": "🤖 **Automated Fix Started**", "createdAt": "2026-11-10T10:05:00Z"}]}"#);
        let log = travel(&open, "", &[dep("nuc-1", "2026-11-11T00:00:00Z")]);
        assert_eq!(log.state, "open");
        let keys: Vec<_> = log.stages.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(keys, vec!["opened", "claimed"]);
        assert_eq!(log.slowest.as_deref(), Some("claimed"));
    }

    #[test]
    fn closed_without_landing_is_not_landed() {
        let gone = issue(r#"{"number": 8, "state": "CLOSED", "stateReason": "NOT_PLANNED", "createdAt": "2026-11-10T10:00:00Z", "closedAt": "2026-11-10T11:00:00Z"}"#);
        let log = travel(&gone, "", &[]);
        assert_eq!(log.state, "closed");
        assert_eq!(log.stages.len(), 1);
        assert_eq!(log.slowest, None);
    }

    #[test]
    fn a_commit_before_the_claim_takes_no_time() {
        let i = issue(r#"{"number": 9, "state": "OPEN", "createdAt": "2026-11-10T10:00:00Z",
            "comments": [{"body": "🔒 claim w 1", "createdAt": "2026-11-10T12:00:00Z"}]}"#);
        let log = travel(&i, "2026-11-10T11:00:00+00:00\tFix #9", &[]);
        let secs: Vec<_> = log.stages.iter().map(|s| (s.key.as_str(), s.secs)).collect();
        assert_eq!(secs, vec![("opened", None), ("claimed", Some(7200)), ("first_commit", Some(0))]);
    }

    #[test]
    fn subjects_name_an_issue_exactly() {
        assert!(names_issue("Implement #495: x", 495));
        assert!(names_issue("Re-sync origin/master into feature/issue-495 (push retry 1)", 495));
        assert!(names_issue("Fix #495", 495));
        assert!(!names_issue("Fix #4950", 495));
        assert!(!names_issue("Fix #49", 495));
        assert!(!names_issue("issue-4951", 495));
    }

    #[test]
    fn approvals_are_recognised() {
        assert!(is_approval("Approved by @laird."));
        assert!(is_approval("Approved by @laird: all four phases."));
        assert!(!is_approval("Not approved yet"));
        assert!(!is_approval("🔒 claim nuc-1 1"));
    }

    #[test]
    fn medians_cover_landed_issues_only() {
        let a = travel(&issue(LANDED), LOG, &[]);
        let mut b = a.clone();
        b.stages.iter_mut().filter(|s| s.key == "approved").for_each(|s| s.secs = Some(2 * 3600));
        let open = TravelLog { state: "open".into(), stages: vec![Stage { key: "approved".into(), at: String::new(), secs: Some(1) }], ..Default::default() };
        let m = stage_medians([&a, &b, &open]);
        assert_eq!(m.get("approved"), Some(&(13 * 3600)));
        assert_eq!(m.get("landed"), Some(&(3 * 3600)));
        assert!(!m.contains_key("opened"));
    }

    #[test]
    fn medians_from_an_issue_listing() {
        let listing = format!("[{LANDED}, {{\"number\": 1, \"state\": \"OPEN\", \"createdAt\": \"2026-11-10T10:00:00Z\"}}]");
        let m = medians_of(listing.as_bytes(), LOG, &[]);
        assert_eq!(m.get("approved"), Some(&(24 * 3600)));
        assert!(medians_of(b"not json", LOG, &[]).is_empty());
    }
}
