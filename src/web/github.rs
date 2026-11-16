//! One GitHub summary per repo for the village (#498), served on
//! `/api/village` as `github`.
//!
//! The almanac (#485) and the GitHub panel (#492) both read it, so a wall
//! display makes one set of GitHub calls per refresh, however many features
//! or browsers want it. A refresh is seven reads, all at once:
//!
//! - one `gh issue list --state all` of the issues updated in the window,
//!   with their comments: what was opened, what landed and when it was
//!   claimed, and where the merge gate handed work back (its comments, as
//!   `gate::comment_bounce` reads them, #431);
//! - one `gh api .../deployments`: what was deployed each day;
//! - the integration branch's `git log`, locally: what was reverted, and
//!   what landed in the last 24 h (#492); and its root commits' author
//!   dates, for how young the repo is (#508); and its commits by author
//!   date, for the travel log's median time per stage (#495, `travel.rs`);
//! - for the GitHub panel (#492): one `gh pr list` of the open pull requests,
//!   one `gh pr list --search review-requested:@me` of those waiting on you,
//!   and one `gh issue list --state open` with labels. The issue read above
//!   can't serve the labels: it only has issues updated in the window, and an
//!   old proposal still waiting is open all the same.
//!
//! Each is cached per repo for [`refresh_ttl`] (90 s), and nothing is ever
//! written, so it works on a `--read-only` server. When `gh` is missing or not
//! signed in there is no summary: the village leaves `github` out and draws as
//! before.
//!
//! Each of the panel's reads that fails leaves its fields out, so the panel
//! says it couldn't read them rather than showing zeros.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, FixedOffset, NaiveDate};
use serde::{Deserialize, Serialize};

use super::source::TtlCache;
use crate::transport::ServerTransport;

/// Days the summary covers, today included.
pub const DAYS: usize = 14;

/// How far back the travel log's medians look for an issue's first commit (#495).
const AUTHORED_DAYS: i64 = 60;

/// How long a summary is reused: 90 s, or `AGENTS_UI_GITHUB_REFRESH_SECS`
/// held between 60 and 120 s, so a wall display never eats the rate limit.
pub fn refresh_ttl() -> Duration {
    ttl_from(std::env::var("AGENTS_UI_GITHUB_REFRESH_SECS").ok().as_deref())
}

/// [`refresh_ttl`] from the variable's value, if any.
pub fn ttl_from(secs: Option<&str>) -> Duration {
    Duration::from_secs(secs.and_then(|s| s.trim().parse::<u64>().ok()).unwrap_or(90).clamp(60, 120))
}

/// What GitHub says about a repo's recent work.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Summary {
    /// When it was fetched (RFC 3339).
    pub as_of: String,
    /// The last [`DAYS`] days, oldest first, today last.
    pub days: Vec<Day>,
    /// The median time from an issue's claim to its landing, in seconds, over
    /// the issues landed in the window whose claim is on record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub median_claim_to_land_secs: Option<u64>,
    /// Issues closed today, for any reason (#492).
    #[serde(default)]
    pub closed_today: u32,
    /// Open pull requests (#492); absent when `gh pr list` couldn't be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_prs: Option<u32>,
    /// When the oldest open pull request was opened (RFC 3339).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oldest_pr_at: Option<String>,
    /// Open pull requests asking for a review from the account this server's
    /// `gh` is signed in as, or its teams (#492): `review-requested:@me`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_requests: Option<u32>,
    /// Open issues (#492); absent when they couldn't be listed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_issues: Option<u32>,
    /// Open issues per label: `proposal`, `needs-*`, `P0`–`P3` and the rest.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub open_by_label: BTreeMap<String, u32>,
    /// Commits that reached the integration branch in the last 24 h (#492);
    /// absent when its log can't be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commits_24h: Option<u32>,
    /// The local date of the integration branch's first commit, `YYYY-MM-DD`
    /// (#508): how young the repo is, so an average over [`DAYS`] can leave
    /// out the days before it existed. Absent when the log can't be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_commit: Option<String>,
    /// The median seconds each stage of an issue's travel log took (#495,
    /// `travel.rs`), over the issues landed in the window: `approved`,
    /// `claimed`, `first_commit`, `landed`, `deployed:<environment>`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stage_medians: BTreeMap<String, u64>,
}

/// The oldest of the root commits' author dates (`git log --max-parents=0
/// --format=%aI`: a rebase or import keeps them), as a local date `YYYY-MM-DD`.
pub fn first_commit_date(log: &str, offset: &FixedOffset) -> Option<String> {
    log.lines().filter_map(|l| local_date(l, offset)).min().map(|d| d.format("%Y-%m-%d").to_string())
}

/// One day's throughput.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Day {
    /// The server's local date, `YYYY-MM-DD`.
    pub date: String,
    /// Issues closed as completed that day: work that landed.
    pub landed: u32,
    /// Issues opened that day.
    pub opened: u32,
    /// Times the merge gate handed work back that day: a conflict or a failed
    /// gate (its comments), or a push rejected and re-synced (its commits).
    pub bounced: u32,
    /// Reverts on the integration branch that day; absent when its log can't be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reverted: Option<u32>,
    /// Commits deployed that day: GitHub Deployments created, one per commit
    /// however many environments it went to (a deploy and its release are
    /// one), whatever their outcome.
    pub deploys: u32,
}

/// An issue as `gh issue list --json number,state,createdAt,closedAt,stateReason,comments` gives it.
/// The travel log (#495, `travel.rs`) reads the same shape, one issue at a time.
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct Issue {
    #[serde(default)]
    pub(crate) number: u32,
    #[serde(default)]
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) state: String,
    #[serde(default, rename = "createdAt")]
    pub(crate) created_at: String,
    #[serde(default, rename = "closedAt")]
    pub(crate) closed_at: Option<String>,
    #[serde(default, rename = "stateReason")]
    pub(crate) state_reason: Option<String>,
    #[serde(default)]
    pub(crate) comments: Vec<Comment>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct Comment {
    #[serde(default)]
    pub(crate) body: String,
    #[serde(default, rename = "createdAt")]
    pub(crate) created_at: String,
}

impl Issue {
    /// Closed as completed: work that landed, not one closed as not planned
    /// or a duplicate.
    pub(crate) fn landed(&self) -> bool {
        self.state.eq_ignore_ascii_case("closed")
            && !matches!(self.state_reason.as_deref().map(str::to_ascii_uppercase).as_deref(), Some("NOT_PLANNED") | Some("DUPLICATE"))
    }
}

/// A comment that claims the issue: the gate's `🔒 claim`, `/fix`'s lock or
/// its "Automated Fix Started".
pub(crate) fn is_claim(body: &str) -> bool {
    let first = body.trim_start();
    first.starts_with("🔒") || first.contains("Automated Fix Started") || first.contains("Implementation Started")
}

/// The local date of an RFC 3339 time.
fn local_date(at: &str, offset: &FixedOffset) -> Option<NaiveDate> {
    DateTime::parse_from_rfc3339(at.trim()).ok().map(|t| t.with_timezone(offset).date_naive())
}

/// The summary from what was read: `issues` (`gh issue list` JSON),
/// `deployments` (the deployments listing, if it could be read), and `log`
/// (`git log --format=%cI%x09%s` of the integration branch, if it could be
/// read: its reverts and push retries). `today` and `offset` are the
/// server's local date and offset from UTC.
pub fn summarize(issues: &[u8], deployments: Option<&str>, log: Option<&str>, today: NaiveDate, offset: FixedOffset, as_of: String) -> Option<Summary> {
    let issues: Vec<Issue> = serde_json::from_slice(issues).ok()?;
    let first = today - chrono::Duration::days(DAYS as i64 - 1);
    let mut days: Vec<Day> = (0..DAYS)
        .map(|k| Day { date: (first + chrono::Duration::days(k as i64)).format("%Y-%m-%d").to_string(), reverted: log.map(|_| 0), ..Day::default() })
        .collect();
    let slot = |at: &str| local_date(at, &offset).filter(|d| *d >= first && *d <= today).map(|d| (d - first).num_days() as usize);
    let mut waits = Vec::new();
    let mut closed_today = 0;
    for issue in &issues {
        if let Some(k) = slot(&issue.created_at) {
            days[k].opened += 1;
        }
        if issue.state.eq_ignore_ascii_case("closed") && issue.closed_at.as_deref().and_then(|at| local_date(at, &offset)) == Some(today) {
            closed_today += 1;
        }
        let landed = issue.landed();
        if let (true, Some(closed)) = (landed, issue.closed_at.as_deref()) {
            if let Some(k) = slot(closed) {
                days[k].landed += 1;
                // The claim that led to this landing: the last before it, not
                // an earlier attempt that was abandoned.
                if let Ok(to) = DateTime::parse_from_rfc3339(closed) {
                    let claimed = issue.comments.iter().filter(|c| is_claim(&c.body)).filter_map(|c| DateTime::parse_from_rfc3339(&c.created_at).ok()).filter(|at| *at <= to).max();
                    if let Some(from) = claimed {
                        waits.push((to - from).num_seconds() as u64);
                    }
                }
            }
        }
        for c in issue.comments.iter().filter(|c| super::gate::comment_bounce(&c.body, &c.created_at).is_some()) {
            if let Some(k) = slot(&c.created_at) {
                days[k].bounced += 1;
            }
        }
    }
    if let Some(json) = deployments {
        let mut seen = std::collections::HashSet::new();
        for d in super::deployments::parse_deployments(json) {
            if let Some(k) = slot(&d.created_at) {
                days[k].deploys += seen.insert((k, d.sha.clone())) as u32;
            }
        }
    }
    if let Some(log) = log {
        for line in log.lines() {
            let Some((at, subject)) = line.split_once('\t') else { continue };
            if let (true, Some(k)) = (subject.starts_with("Revert "), slot(at)) {
                *days[k].reverted.get_or_insert(0) += 1;
            }
        }
        // A push rejected and re-synced is a bounce too, as the village's marks count it (#431).
        for (_, b) in super::gate::push_retry_bounces(log) {
            if let Some(k) = b.at.as_deref().and_then(slot) {
                days[k].bounced += 1;
            }
        }
    }
    Some(Summary { as_of, days, median_claim_to_land_secs: median(&mut waits), closed_today, ..Summary::default() })
}

/// An open pull request as `gh pr list --json number,createdAt` gives it.
#[derive(Debug, Deserialize)]
struct Pr {
    #[serde(default, rename = "createdAt")]
    created_at: String,
}

/// `gh pr list --json number,createdAt` → how many, and when the oldest was
/// opened; `None` when it isn't a listing.
pub fn parse_prs(json: &[u8]) -> Option<(u32, Option<String>)> {
    let prs: Vec<Pr> = serde_json::from_slice(json).ok()?;
    let oldest = prs.iter().filter_map(|p| DateTime::parse_from_rfc3339(&p.created_at).ok().map(|t| (t, &p.created_at))).min_by_key(|(t, _)| *t);
    Some((prs.len() as u32, oldest.map(|(_, at)| at.clone())))
}

#[derive(Debug, Deserialize)]
struct Labelled {
    #[serde(default)]
    labels: Vec<Label>,
}

#[derive(Debug, Deserialize)]
struct Label {
    #[serde(default)]
    name: String,
}

/// `gh issue list --state open --json number,labels` → how many are open,
/// and how many carry each label; `None` when it isn't a listing.
pub fn parse_open_issues(json: &[u8]) -> Option<(u32, BTreeMap<String, u32>)> {
    let issues: Vec<Labelled> = serde_json::from_slice(json).ok()?;
    let mut by = BTreeMap::new();
    for l in issues.iter().flat_map(|i| &i.labels).filter(|l| !l.name.is_empty()) {
        *by.entry(l.name.clone()).or_insert(0) += 1;
    }
    Some((issues.len() as u32, by))
}

/// Commits in `log` (`git log --format=%cI%x09%s`) committed at or after
/// `since`: what reached the integration branch since then.
pub fn commits_since(log: &str, since: DateTime<FixedOffset>) -> u32 {
    log.lines()
        .filter_map(|l| l.split_once('\t'))
        .filter(|(at, _)| DateTime::parse_from_rfc3339(at.trim()).is_ok_and(|t| t >= since))
        .count() as u32
}

/// What the GitHub panel (#492) reads besides the issues: `prs` (`gh pr
/// list`), `reviews` (the same, searched for review requests), `open`
/// (`gh issue list --state open` with labels), each if it could be read, and
/// the integration branch's `log` up to `now`. What wasn't read stays absent.
pub fn add_panel(s: &mut Summary, prs: Option<&[u8]>, reviews: Option<&[u8]>, open: Option<&[u8]>, log: Option<&str>, now: DateTime<FixedOffset>) {
    if let Some((n, oldest)) = prs.and_then(parse_prs) {
        (s.open_prs, s.oldest_pr_at) = (Some(n), oldest);
    }
    s.review_requests = reviews.and_then(parse_prs).map(|(n, _)| n);
    if let Some((n, by)) = open.and_then(parse_open_issues) {
        (s.open_issues, s.open_by_label) = (Some(n), by);
    }
    s.commits_24h = log.map(|l| commits_since(l, now - chrono::Duration::hours(24)));
}

fn median(xs: &mut [u64]) -> Option<u64> {
    if xs.is_empty() {
        return None;
    }
    xs.sort_unstable();
    let n = xs.len();
    Some(if n % 2 == 1 { xs[n / 2] } else { (xs[n / 2 - 1] + xs[n / 2]) / 2 })
}

/// Read GitHub and git for `repo`, whose work lands on `integration`. `None`
/// when `gh` can't list its issues (missing, signed out, rate limited). The
/// seven reads run together. The issue list stops at 500, about three times
/// what this repo updates in 14 days; the panel's at 500 pull requests and
/// 1000 open issues.
pub async fn fetch(transport: &ServerTransport, repo: &Path, integration: &str) -> Option<Summary> {
    let now = chrono::Local::now();
    let (today, offset) = (now.date_naive(), *now.offset());
    let since = (today - chrono::Duration::days(DAYS as i64)).format("%Y-%m-%d").to_string();
    let args: Vec<String> = [
        "issue", "list", "--state", "all", "--limit", "500",
        "--search", &format!("updated:>={since}"),
        "--json", "number,title,state,createdAt,closedAt,stateReason,comments",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let gh = |args: Vec<String>| async move {
        let out = crate::github::gh_repo_output(transport, repo, &args).await.ok()?;
        out.status.success().then_some(out.stdout)
    };
    let issues = gh(args);
    let prs = gh(PR_LIST.iter().map(|s| s.to_string()).collect());
    let reviews = gh(PR_LIST.iter().map(|s| s.to_string()).chain(["--search".into(), "review-requested:@me".into()]).collect());
    let open = gh(OPEN_ISSUES.iter().map(|s| s.to_string()).collect());
    let deployments = super::deployments::gh_api(transport, repo, "repos/{owner}/{repo}/deployments?per_page=100");
    let log = async {
        if integration.is_empty() {
            return None;
        }
        let args: Vec<String> = vec!["log".into(), format!("origin/{integration}"), format!("--since={since}"), "--format=%cI%x09%s".into()];
        let out = transport.output("git", &args, Some(repo)).await.ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    // The repo's age (#508): its root commits' dates, a quick local read.
    let roots = async {
        if integration.is_empty() {
            return None;
        }
        let args: Vec<String> = vec!["log".into(), format!("origin/{integration}"), "--max-parents=0".into(), "--format=%aI".into()];
        let out = transport.output("git", &args, Some(repo)).await.ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    // The travel log's medians (#495) date first commits by author, which a
    // rebase keeps. An issue landed in the window may have been started well
    // before it, so this log reaches further back (`--since` is by commit date).
    let authored = async {
        if integration.is_empty() {
            return None;
        }
        let back = (today - chrono::Duration::days(AUTHORED_DAYS)).format("%Y-%m-%d").to_string();
        let args: Vec<String> = vec!["log".into(), format!("origin/{integration}"), format!("--since={back}"), "--format=%aI%x09%s".into()];
        let out = transport.output("git", &args, Some(repo)).await.ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let (issues, deployments, log, prs, reviews, open, roots, authored) = futures::join!(issues, deployments, log, prs, reviews, open, roots, authored);
    let issues = issues?;
    let mut s = summarize(&issues, deployments.as_deref(), log.as_deref(), today, offset, now.to_rfc3339_opts(chrono::SecondsFormat::Secs, false))?;
    let deployed = deployments.as_deref().map(super::deployments::parse_deployments).unwrap_or_default();
    s.stage_medians = super::travel::medians_of(&issues, authored.as_deref().unwrap_or(""), &deployed);
    add_panel(&mut s, prs.as_deref(), reviews.as_deref(), open.as_deref(), log.as_deref(), now.fixed_offset());
    s.first_commit = roots.as_deref().and_then(|r| first_commit_date(r, &offset));
    Some(s)
}

/// The panel's pull request read (#492): the open ones, when each was opened.
const PR_LIST: [&str; 8] = ["pr", "list", "--state", "open", "--limit", "500", "--json", "number,createdAt"];
/// The panel's open issues, with their labels.
const OPEN_ISSUES: [&str; 8] = ["issue", "list", "--state", "open", "--limit", "1000", "--json", "number,labels"];

/// Summaries per repo and integration branch, each reused for
/// [`refresh_ttl`]: polls inside it make no `gh` calls. A failed read is
/// cached too, so a signed-out `gh` isn't asked again on every poll. Polls of
/// one repo arriving together while its summary is stale wait for one read
/// instead of each starting their own; other repos don't wait on it.
pub struct Summaries {
    cache: TtlCache<Key, Option<Summary>>,
    /// One lock per repo and branch, held while it is read.
    reading: std::sync::Mutex<std::collections::HashMap<Key, std::sync::Arc<tokio::sync::Mutex<()>>>>,
}

type Key = (PathBuf, String);

impl Default for Summaries {
    fn default() -> Self {
        Self::new(refresh_ttl())
    }
}

impl Summaries {
    pub fn new(ttl: Duration) -> Self {
        Self { cache: TtlCache::new(ttl), reading: Default::default() }
    }

    pub async fn get(&self, transport: &ServerTransport, repo: &Path, integration: &str) -> Option<Summary> {
        self.get_with(repo, integration, || fetch(transport, repo, integration)).await
    }

    /// `get`, with the read given: what the cache does, testable without `gh`.
    pub async fn get_with<F: std::future::Future<Output = Option<Summary>>>(&self, repo: &Path, integration: &str, read: impl FnOnce() -> F) -> Option<Summary> {
        let key = (repo.to_path_buf(), integration.to_string());
        let lock = self.reading.lock().unwrap_or_else(|e| e.into_inner()).entry(key.clone()).or_default().clone();
        let _turn = lock.lock().await;
        self.cache.get_or(key, read).await
    }

    /// Drop what was read for `repo`, on every integration branch.
    pub fn forget(&self, repo: &Path) {
        self.cache.forget_where(|(r, _)| r == repo);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// #508: the repo's age is its oldest root commit's local date.
    #[test]
    fn the_first_commit_is_the_oldest_root_as_a_local_date() {
        let utc = FixedOffset::east_opt(0).unwrap();
        assert_eq!(first_commit_date("2026-11-10T09:00:00+00:00\n2026-11-03T23:30:00+00:00\n", &utc).as_deref(), Some("2026-11-03"));
        let tokyo = FixedOffset::east_opt(9 * 3600).unwrap();
        assert_eq!(first_commit_date("2026-11-03T23:30:00+00:00", &tokyo).as_deref(), Some("2026-11-04"));
        assert_eq!(first_commit_date("", &utc), None);
        assert_eq!(first_commit_date("not a date", &utc), None);
    }

    /// `gh issue list --json number,state,createdAt,closedAt,stateReason,comments`
    /// as GitHub answers it, cut to what matters.
    const ISSUES: &str = r#"[
      {"number": 484, "state": "CLOSED", "createdAt": "2026-11-14T22:52:33Z", "closedAt": "2026-11-16T05:30:00Z", "stateReason": "COMPLETED",
       "comments": [
         {"body": "🔒 claim nuc-2013134 1794804283840", "createdAt": "2026-11-16T04:44:44Z"},
         {"body": "🤖 **Automated Fix Started**\n\nBranch", "createdAt": "2026-11-16T04:46:00Z"},
         {"body": "⚠️ **Auto-merge to master hit a conflict**\n\nsrc/web/server.rs", "createdAt": "2026-11-16T05:10:00Z"}
       ]},
      {"number": 483, "state": "CLOSED", "createdAt": "2026-11-15T10:00:00Z", "closedAt": "2026-11-16T03:00:00Z", "stateReason": "COMPLETED",
       "comments": [{"body": "🔒 claim nuc-1 1", "createdAt": "2026-11-16T02:00:00Z"},
                    {"body": "❌ **Auto-merge to master failed** at the gate", "createdAt": "2026-11-16T02:30:00Z"}]},
      {"number": 470, "state": "CLOSED", "createdAt": "2026-10-01T10:00:00Z", "closedAt": "2026-11-16T01:00:00Z", "stateReason": "NOT_PLANNED", "comments": []},
      {"number": 498, "state": "OPEN", "createdAt": "2026-11-16T04:00:00Z", "closedAt": null, "stateReason": null, "comments": []},
      {"number": 300, "state": "CLOSED", "createdAt": "2026-09-01T10:00:00Z", "closedAt": "2026-10-01T10:00:00Z", "stateReason": "COMPLETED", "comments": []}
    ]"#;

    const DEPLOYMENTS: &str = r#"[
      {"id": 3, "sha": "c1155de", "created_at": "2026-11-16T05:41:00Z", "environment": "release", "description": "Release v2026.11.16.1"},
      {"id": 2, "sha": "c1155de", "created_at": "2026-11-16T05:40:00Z", "environment": "nuc-1"},
      {"id": 1, "sha": "a8064d0", "created_at": "2026-11-16T05:39:00Z", "environment": "nuc-1"},
      {"id": 0, "sha": "1dce1fa", "created_at": "2026-11-12T08:00:00Z"}
    ]"#;

    const LOG: &str = "2026-11-16T05:00:00+00:00\tImplement #484: a usage or rate limit\n\
                       2026-11-16T04:00:00+00:00\tRe-sync origin/master into feature/issue-491 (push retry 1)\n\
                       2026-11-15T09:00:00+00:00\tRevert \"Implement #476: shadows at 2×\"\n";

    fn utc() -> FixedOffset {
        FixedOffset::east_opt(0).unwrap()
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 16).unwrap()
    }

    #[test]
    fn a_summary_counts_each_days_work_over_fourteen_days() {
        let s = summarize(ISSUES.as_bytes(), Some(DEPLOYMENTS), Some(LOG), today(), utc(), "2026-11-16T06:00:00Z".into()).unwrap();
        assert_eq!(s.days.len(), DAYS);
        assert_eq!((s.days[0].date.as_str(), s.days[DAYS - 1].date.as_str()), ("2026-11-03", "2026-11-16"), "oldest first, today last");
        let day = |d: &str| s.days.iter().find(|x| x.date == d).unwrap().clone();
        let t = day("2026-11-16");
        assert_eq!(t.landed, 2, "not planned and an old close are not landings");
        assert_eq!(t.opened, 1);
        assert_eq!(t.bounced, 3, "a conflict, a failed gate, and a push re-synced");
        assert_eq!(t.deploys, 2, "c1155de to nuc-1 and its release is one deploy; a8064d0 another");
        assert_eq!(t.reverted, Some(0));
        assert_eq!(day("2026-11-15").reverted, Some(1));
        assert_eq!((day("2026-11-15").opened, day("2026-11-14").opened), (1, 1));
        assert_eq!(day("2026-11-12").deploys, 1);
        // Claim to land, from the last claim before it landed: 44m and 1h; the median of two is their mean.
        assert_eq!(s.median_claim_to_land_secs, Some((2640 + 3600) / 2));
        assert_eq!(s.as_of, "2026-11-16T06:00:00Z");
    }

    #[test]
    fn days_are_the_servers_local_days() {
        // 22:52 UTC on the 14th is the 15th in Tokyo.
        let tokyo = FixedOffset::east_opt(9 * 3600).unwrap();
        let s = summarize(ISSUES.as_bytes(), None, None, today(), tokyo, String::new()).unwrap();
        let day = |d: &str| s.days.iter().find(|x| x.date == d).unwrap().clone();
        assert_eq!((day("2026-11-14").opened, day("2026-11-15").opened), (0, 2), "#484 joins #483 on the 15th");
    }

    #[test]
    fn what_could_not_be_read_is_left_out_not_zero_or_an_error() {
        let s = summarize(ISSUES.as_bytes(), None, None, today(), utc(), String::new()).unwrap();
        assert!(s.days.iter().all(|d| d.reverted.is_none() && d.deploys == 0), "no log: reverted unknown");
        let json = serde_json::to_value(&s.days[0]).unwrap();
        assert!(json.get("reverted").is_none());
        assert_eq!(summarize(b"not json", None, None, today(), utc(), String::new()), None, "gh's issues unreadable: no summary");
        let none = summarize(b"[]", None, None, today(), utc(), String::new()).unwrap();
        assert_eq!(none.median_claim_to_land_secs, None);
        assert!(serde_json::to_value(&none).unwrap().get("median_claim_to_land_secs").is_none());
    }

    #[tokio::test]
    async fn polls_inside_the_refresh_interval_make_no_new_reads() {
        let reads = AtomicUsize::new(0);
        let read = || async {
            reads.fetch_add(1, Ordering::SeqCst);
            Some(Summary { as_of: "now".into(), ..Summary::default() })
        };
        let cache = Summaries::new(Duration::from_secs(90));
        let repo = Path::new("/src/shop");
        assert!(cache.get_with(repo, "master", read).await.is_some());
        assert!(cache.get_with(repo, "master", read).await.is_some());
        assert_eq!(reads.load(Ordering::SeqCst), 1, "the second poll is served from the cache");
        cache.forget(repo);
        cache.get_with(repo, "master", read).await;
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        // A failed read is cached too: a signed-out gh isn't asked on every poll.
        let fails = AtomicUsize::new(0);
        let fail = || async {
            fails.fetch_add(1, Ordering::SeqCst);
            None
        };
        let other = Path::new("/src/other");
        assert_eq!(cache.get_with(other, "master", fail).await, None);
        assert_eq!(cache.get_with(other, "master", fail).await, None);
        assert_eq!(fails.load(Ordering::SeqCst), 1);
    }

    /// `gh pr list --state open --json number,createdAt`, as GitHub answers it.
    const PRS: &str = r#"[
      {"number": 12, "createdAt": "2026-11-15T09:00:00Z"},
      {"number": 9, "createdAt": "2026-11-02T18:30:00Z"},
      {"number": 14, "createdAt": "2026-11-16T05:00:00Z"}
    ]"#;
    /// The same, searched `review-requested:@me`.
    const REVIEWS: &str = r#"[{"number": 12, "createdAt": "2026-11-15T09:00:00Z"}]"#;
    /// `gh issue list --state open --json number,labels`.
    const OPEN: &str = r#"[
      {"number": 501, "labels": [{"id": "a", "name": "proposal", "color": "ededed"}, {"name": "P2"}]},
      {"number": 502, "labels": [{"name": "proposal"}, {"name": "P3"}]},
      {"number": 503, "labels": [{"name": "needs-clarification"}, {"name": "P1"}]},
      {"number": 504, "labels": [{"name": "P3"}, {"name": "working"}]},
      {"number": 505, "labels": []}
    ]"#;

    fn now() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2026-11-16T06:00:00Z").unwrap()
    }

    #[test]
    fn the_panel_reads_open_prs_reviews_labels_and_the_last_day() {
        let mut s = summarize(ISSUES.as_bytes(), None, Some(LOG), today(), utc(), "2026-11-16T06:00:00Z".into()).unwrap();
        add_panel(&mut s, Some(PRS.as_bytes()), Some(REVIEWS.as_bytes()), Some(OPEN.as_bytes()), Some(LOG), now());
        assert_eq!(s.open_prs, Some(3));
        assert_eq!(s.oldest_pr_at.as_deref(), Some("2026-11-02T18:30:00Z"), "the oldest, whatever the listing's order");
        assert_eq!(s.review_requests, Some(1));
        assert_eq!(s.open_issues, Some(5));
        let by = |l: &str| s.open_by_label.get(l).copied();
        assert_eq!((by("proposal"), by("needs-clarification"), by("P3"), by("P1"), by("P0")), (Some(2), Some(1), Some(2), Some(1), None));
        assert_eq!(s.commits_24h, Some(3), "the revert at 09:00 yesterday is 21 h old");
        assert_eq!(commits_since(LOG, now() - chrono::Duration::hours(12)), 2, "it is older than 12 h");
        assert_eq!(s.closed_today, 3, "every issue closed today, not planned included; the old close is not today");
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["open_by_label"]["proposal"], 2);
        assert_eq!(json["oldest_pr_at"], "2026-11-02T18:30:00Z");
    }

    #[test]
    fn what_the_panel_could_not_read_is_left_out_not_zero() {
        let mut s = summarize(ISSUES.as_bytes(), None, None, today(), utc(), String::new()).unwrap();
        add_panel(&mut s, None, Some(b"rate limited"), None, None, now());
        assert_eq!((s.open_prs, s.oldest_pr_at.clone(), s.review_requests, s.open_issues, s.commits_24h), (None, None, None, None, None));
        let json = serde_json::to_value(&s).unwrap();
        for k in ["open_prs", "oldest_pr_at", "review_requests", "open_issues", "open_by_label", "commits_24h"] {
            assert!(json.get(k).is_none(), "{k} left out");
        }
        // Nothing open is zero, though: it was read.
        add_panel(&mut s, Some(b"[]"), Some(b"[]"), Some(b"[]"), Some(""), now());
        assert_eq!((s.open_prs, s.oldest_pr_at.clone(), s.review_requests, s.open_issues, s.commits_24h), (Some(0), None, Some(0), Some(0), Some(0)));
    }

    #[tokio::test]
    async fn the_panels_fields_ride_the_same_cache() {
        let reads = AtomicUsize::new(0);
        let read = || async {
            reads.fetch_add(1, Ordering::SeqCst);
            let mut s = summarize(ISSUES.as_bytes(), None, Some(LOG), today(), utc(), "2026-11-16T06:00:00Z".into())?;
            add_panel(&mut s, Some(PRS.as_bytes()), Some(REVIEWS.as_bytes()), Some(OPEN.as_bytes()), Some(LOG), now());
            Some(s)
        };
        let cache = Summaries::new(Duration::from_secs(90));
        let first = cache.get_with(Path::new("/src/shop"), "master", read).await.unwrap();
        let again = cache.get_with(Path::new("/src/shop"), "master", read).await.unwrap();
        assert_eq!((first.open_prs, again.review_requests, again.as_of.as_str()), (Some(3), Some(1), "2026-11-16T06:00:00Z"));
        assert_eq!(reads.load(Ordering::SeqCst), 1, "one read for the almanac's days and the panel's counts together");
    }

    #[test]
    fn the_panel_asks_gh_for_little() {
        assert_eq!(PR_LIST.join(" "), "pr list --state open --limit 500 --json number,createdAt");
        assert_eq!(OPEN_ISSUES.join(" "), "issue list --state open --limit 1000 --json number,labels");
    }

    #[test]
    fn the_refresh_is_between_one_and_two_minutes() {
        assert_eq!(ttl_from(None), Duration::from_secs(90));
        assert_eq!(ttl_from(Some("75")), Duration::from_secs(75));
        assert_eq!(ttl_from(Some("0")), Duration::from_secs(60), "never every poll");
        assert_eq!(ttl_from(Some("600")), Duration::from_secs(120));
        assert_eq!(ttl_from(Some("soon")), Duration::from_secs(90));
    }

    #[test]
    fn an_abandoned_claim_is_not_counted_in_the_wait() {
        let issues = r#"[{"number": 9, "state": "CLOSED", "createdAt": "2026-11-01T00:00:00Z", "closedAt": "2026-11-16T05:00:00Z", "stateReason": "COMPLETED",
          "comments": [{"body": "🔒 claim nuc-1 1", "createdAt": "2026-11-06T00:00:00Z"},
                       {"body": "🔒 claim nuc-2 2", "createdAt": "2026-11-16T04:00:00Z"},
                       {"body": "🔒 claim nuc-3 3", "createdAt": "2026-11-16T06:00:00Z"}]}]"#;
        let s = summarize(issues.as_bytes(), None, None, today(), utc(), String::new()).unwrap();
        assert_eq!(s.median_claim_to_land_secs, Some(3600), "the last claim before it landed; one after it is not");
    }

    #[tokio::test]
    async fn polls_arriving_together_wait_for_one_read() {
        let reads = std::sync::Arc::new(AtomicUsize::new(0));
        let cache = std::sync::Arc::new(Summaries::new(Duration::from_secs(90)));
        let polls: Vec<_> = (0..4)
            .map(|_| {
                let (cache, reads) = (cache.clone(), reads.clone());
                tokio::spawn(async move {
                    cache
                        .get_with(Path::new("/src/shop"), "master", || async move {
                            reads.fetch_add(1, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_millis(20)).await;
                            Some(Summary::default())
                        })
                        .await
                })
            })
            .collect();
        for p in polls {
            assert!(p.await.unwrap().is_some());
        }
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        // Another integration branch is another read; forgetting the repo forgets both.
        cache.get_with(Path::new("/src/shop"), "main", || async { Some(Summary::default()) }).await;
        cache.forget(Path::new("/src/shop"));
        let again = reads.clone();
        cache.get_with(Path::new("/src/shop"), "master", || async move { again.fetch_add(1, Ordering::SeqCst); None }).await;
        assert_eq!(reads.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_slow_read_of_one_repo_does_not_hold_up_another() {
        let cache = std::sync::Arc::new(Summaries::new(Duration::from_secs(90)));
        cache.get_with(Path::new("/src/fast"), "master", || async { Some(Summary::default()) }).await;
        let (slow_started, release) = (std::sync::Arc::new(tokio::sync::Notify::new()), std::sync::Arc::new(tokio::sync::Notify::new()));
        let slow = {
            let (cache, started, release) = (cache.clone(), slow_started.clone(), release.clone());
            tokio::spawn(async move {
                cache
                    .get_with(Path::new("/src/slow"), "master", || async move {
                        started.notify_one();
                        release.notified().await;
                        Some(Summary::default())
                    })
                    .await
            })
        };
        slow_started.notified().await;
        // While /src/slow is still being read, /src/fast's cached summary comes straight back.
        let fast = tokio::time::timeout(Duration::from_secs(1), cache.get_with(Path::new("/src/fast"), "master", || async { None })).await;
        assert!(matches!(fast, Ok(Some(_))), "served from the cache without waiting");
        release.notify_one();
        assert!(slow.await.unwrap().is_some());
    }
}
