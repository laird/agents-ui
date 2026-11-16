//! One GitHub summary per repo for the village (#498), served on
//! `/api/village` as `github`.
//!
//! The almanac (#485) and the GitHub panel (#492) both read it, so a wall
//! display makes one set of GitHub calls per refresh, however many features
//! or browsers want it. A refresh is three reads:
//!
//! - one `gh issue list --state all` of the issues updated in the window,
//!   with their comments: what was opened, what landed and when it was
//!   claimed, and where the merge gate handed work back (its comments, as
//!   `gate::comment_bounce` reads them, #431);
//! - one `gh api .../deployments`: what was deployed each day;
//! - the integration branch's `git log`, locally: what was reverted.
//!
//! Each is cached per repo for [`refresh_ttl`] (90 s), and nothing is ever
//! written, so it works on a `--read-only` server. When `gh` is missing or not
//! signed in there is no summary: the village leaves `github` out and draws as
//! before.
//!
//! #492 adds its fields (open PRs, review requests, issues by label, recent
//! merges) to [`Summary`] and to [`fetch`], under the same cache.

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, FixedOffset, NaiveDate};
use serde::{Deserialize, Serialize};

use super::source::TtlCache;
use crate::transport::ServerTransport;

/// Days the summary covers, today included.
pub const DAYS: usize = 14;

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
#[derive(Debug, Clone, Default, Deserialize)]
struct Issue {
    #[serde(default)]
    state: String,
    #[serde(default, rename = "createdAt")]
    created_at: String,
    #[serde(default, rename = "closedAt")]
    closed_at: Option<String>,
    #[serde(default, rename = "stateReason")]
    state_reason: Option<String>,
    #[serde(default)]
    comments: Vec<Comment>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Comment {
    #[serde(default)]
    body: String,
    #[serde(default, rename = "createdAt")]
    created_at: String,
}

/// A comment that claims the issue: the gate's `🔒 claim`, `/fix`'s lock or
/// its "Automated Fix Started".
fn is_claim(body: &str) -> bool {
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
    for issue in &issues {
        if let Some(k) = slot(&issue.created_at) {
            days[k].opened += 1;
        }
        let landed = issue.state.eq_ignore_ascii_case("closed")
            && !matches!(issue.state_reason.as_deref().map(str::to_ascii_uppercase).as_deref(), Some("NOT_PLANNED") | Some("DUPLICATE"));
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
    Some(Summary { as_of, days, median_claim_to_land_secs: median(&mut waits) })
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
/// three reads run together. The issue list stops at 500, about three times
/// what this repo updates in 14 days.
pub async fn fetch(transport: &ServerTransport, repo: &Path, integration: &str) -> Option<Summary> {
    let now = chrono::Local::now();
    let (today, offset) = (now.date_naive(), *now.offset());
    let since = (today - chrono::Duration::days(DAYS as i64)).format("%Y-%m-%d").to_string();
    let args: Vec<String> = [
        "issue", "list", "--state", "all", "--limit", "500",
        "--search", &format!("updated:>={since}"),
        "--json", "number,state,createdAt,closedAt,stateReason,comments",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let issues = async {
        let out = crate::github::gh_repo_output(transport, repo, &args).await.ok()?;
        out.status.success().then_some(out.stdout)
    };
    let deployments = super::deployments::gh_api(transport, repo, "repos/{owner}/{repo}/deployments?per_page=100");
    let log = async {
        if integration.is_empty() {
            return None;
        }
        let args: Vec<String> = vec!["log".into(), format!("origin/{integration}"), format!("--since={since}"), "--format=%cI%x09%s".into()];
        let out = transport.output("git", &args, Some(repo)).await.ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let (issues, deployments, log) = futures::join!(issues, deployments, log);
    summarize(&issues?, deployments.as_deref(), log.as_deref(), today, offset, now.to_rfc3339_opts(chrono::SecondsFormat::Secs, false))
}

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
