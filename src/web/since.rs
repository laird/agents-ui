//! "Since you last looked" (#479): what closed and merged on GitHub after a
//! moment, for the screensaver's summary card.
//!
//! `GET /api/village/since?t=<unix seconds>` answers, for each village with a
//! repository, the issues closed and the pull requests merged since `t`. The
//! rest of the card (deploys, gate bounces, what waits on you) is already in
//! `/api/village`, so the page reads it from there (`saver.js`).
//!
//! A village whose repository GitHub can't be asked about has `checked:
//! false`, so the card says it couldn't check rather than "nothing happened".
//!
//! Asking costs two GitHub searches per village, and the endpoint has no
//! authentication, so what a caller sends never decides what GitHub is asked
//! ([`SinceCache`]): the server asks about one fixed window, the last
//! [`WINDOW`], at most once per [`FRESH`], and cuts every answer from that.
//! A moment before the window is answered for the window (`from` says so).

use std::path::Path;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::transport::ServerTransport;

/// Most issues or pull requests listed per village.
const LISTED: usize = 50;

/// An issue closed, or a pull request merged, since the moment asked about.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Change {
    pub number: u32,
    pub title: String,
    /// When it closed or merged (RFC 3339).
    pub at: String,
    /// For an issue, GitHub's `stateReason` lowercased: `completed`, `not_planned`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// One village's changes since the moment asked about.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VillageSince {
    pub project: String,
    /// GitHub answered both questions.
    pub checked: bool,
    pub closed: Vec<Change>,
    pub merged: Vec<Change>,
}

/// The moment `t` (unix seconds) as GitHub search wants it, or `None` for
/// nonsense (negative, or past the year 9999).
pub fn search_time(t: i64) -> Option<String> {
    DateTime::<Utc>::from_timestamp(t, 0).filter(|_| t >= 0).map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string())
}

#[derive(Deserialize)]
struct Listed {
    number: u32,
    #[serde(default)]
    title: String,
    #[serde(rename = "closedAt", default)]
    closed_at: Option<String>,
    #[serde(rename = "mergedAt", default)]
    merged_at: Option<String>,
    #[serde(rename = "stateReason", default)]
    state_reason: Option<String>,
}

/// `gh issue list` / `gh pr list` JSON as changes at or after `since`,
/// newest first. GitHub's search is by day, so the time is checked here too.
pub fn parse_changes(json: &str, since: &DateTime<Utc>) -> Option<Vec<Change>> {
    let listed: Vec<Listed> = serde_json::from_str(json).ok()?;
    let mut out: Vec<(DateTime<Utc>, Change)> = listed
        .into_iter()
        .filter_map(|l| {
            let at = l.merged_at.or(l.closed_at).filter(|s| !s.is_empty())?;
            let when = DateTime::parse_from_rfc3339(&at).ok()?.with_timezone(&Utc);
            (when >= *since).then(|| {
                let reason = l.state_reason.filter(|r| !r.is_empty()).map(|r| r.to_lowercase());
                (when, Change { number: l.number, title: l.title, at, reason })
            })
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    Some(out.into_iter().map(|(_, c)| c).take(LISTED).collect())
}

async fn gh_list(transport: &ServerTransport, repo: &Path, what: &str, query: &str, fields: &str, since: &DateTime<Utc>) -> Option<Vec<Change>> {
    let args: Vec<String> = [what, "list", "--state", if what == "pr" { "merged" } else { "closed" }, "--search", query, "--json", fields, "--limit", "100"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let out = transport.output("gh", &args, Some(repo)).await.ok()?;
    if !out.status.success() {
        return None;
    }
    parse_changes(&String::from_utf8_lossy(&out.stdout), since)
}

/// What closed and merged in `repo` since `t` (unix seconds).
pub async fn village_since(transport: &ServerTransport, project: &str, repo: &str, t: i64) -> VillageSince {
    let mut v = VillageSince { project: project.to_string(), checked: false, closed: vec![], merged: vec![] };
    let (Some(at), Some(since)) = (search_time(t), DateTime::<Utc>::from_timestamp(t, 0)) else { return v };
    // Over ssh the repository is on the other host: let gh say whether it is one.
    if repo.is_empty() || (!transport.is_remote() && !Path::new(repo).is_dir()) {
        return v;
    }
    let repo = Path::new(repo);
    let closed = gh_list(transport, repo, "issue", &format!("closed:>={at}"), "number,title,closedAt,stateReason", &since).await;
    let merged = gh_list(transport, repo, "pr", &format!("merged:>={at}"), "number,title,mergedAt", &since).await;
    v.checked = closed.is_some() && merged.is_some();
    v.closed = closed.unwrap_or_default();
    v.merged = merged.unwrap_or_default();
    v
}

/// How long an answer is kept: GitHub is asked at most this often.
pub const FRESH: Duration = Duration::from_secs(60);
/// How far back GitHub is asked about: a spell longer than this is answered
/// for this much of it.
pub const WINDOW: i64 = 3 * 24 * 3600;

/// The longest GitHub is waited on. An asking that runs over is kept as an
/// empty answer (every village unchecked) until it is [`FRESH`] no longer.
pub const ASK_LIMIT: Duration = Duration::from_secs(60);

type Kept = Option<(Instant, i64, Vec<VillageSince>)>;

/// The last answer: (when it was fetched, the start of its window in unix
/// seconds, the villages).
#[derive(Default)]
pub struct SinceCache(std::sync::Arc<tokio::sync::Mutex<Kept>>);

/// The kept answer cut down to changes at or after `t`.
pub fn narrow(villages: &[VillageSince], t: i64) -> Vec<VillageSince> {
    let Some(since) = DateTime::<Utc>::from_timestamp(t, 0) else { return villages.to_vec() };
    let keep = |c: &&Change| DateTime::parse_from_rfc3339(&c.at).is_ok_and(|a| a.with_timezone(&Utc) >= since);
    villages
        .iter()
        .map(|v| VillageSince {
            closed: v.closed.iter().filter(keep).cloned().collect(),
            merged: v.merged.iter().filter(keep).cloned().collect(),
            ..v.clone()
        })
        .collect()
}

impl SinceCache {
    /// The answer for `t`, as (the moment it covers from, the villages). The
    /// kept answer serves while it is fresh; otherwise `fetch(window start)`
    /// asks GitHub about the last [`WINDOW`] before `now` (unix seconds) and
    /// is kept. `t` only cuts the answer down, so no caller can make GitHub
    /// be asked more than once per [`FRESH`]. `clock` is read once the lock
    /// is held, and requests wait on each other.
    ///
    /// The asking runs as a task of its own, holding the lock: a caller that
    /// hangs up mid-way does not cancel it, so what GitHub was asked is kept
    /// and the next caller waits for it instead of asking again. It is given
    /// [`ASK_LIMIT`], so a `gh` that never answers can't hold the lock.
    pub async fn answer<F, Fut>(&self, t: i64, now: i64, clock: impl Fn() -> Instant, fetch: F) -> (i64, Vec<VillageSince>)
    where
        F: FnOnce(i64) -> Fut,
        Fut: std::future::Future<Output = Vec<VillageSince>> + Send + 'static,
    {
        let mut kept = self.0.clone().lock_owned().await;
        let at = clock();
        if !kept.as_ref().is_some_and(|(fetched, _, _)| at.saturating_duration_since(*fetched) < FRESH) {
            let from = now - WINDOW;
            let ask = fetch(from);
            let task = tokio::spawn(async move {
                let villages = tokio::time::timeout(ASK_LIMIT, ask).await.unwrap_or_default();
                *kept = Some((at, from, villages));
                kept
            });
            match task.await {
                Ok(guard) => kept = guard,
                // The asking panicked: nothing is kept, and this answer is empty.
                Err(_) => return (t.max(from), vec![]),
            }
        }
        let (_, window, villages) = kept.as_ref().expect("kept above");
        let from = t.max(*window);
        (from, narrow(villages, from))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn search_time_is_utc_and_refuses_nonsense() {
        assert_eq!(search_time(0).as_deref(), Some("1970-01-01T00:00:00Z"));
        assert_eq!(search_time(1_794_800_000).as_deref(), Some("2026-11-16T03:33:20Z"));
        assert_eq!(search_time(-5), None);
        assert_eq!(search_time(i64::MAX), None);
    }

    #[test]
    fn closed_issues_since_the_moment_newest_first() {
        let json = r#"[
            {"number": 3, "title": "Old", "closedAt": "2026-11-15T09:00:00Z", "stateReason": "COMPLETED"},
            {"number": 5, "title": "Landed", "closedAt": "2026-11-15T22:00:00Z", "stateReason": "COMPLETED"},
            {"number": 7, "title": "Dropped", "closedAt": "2026-11-15T23:00:00Z", "stateReason": "NOT_PLANNED"},
            {"number": 8, "title": "No time"}
        ]"#;
        let got = parse_changes(json, &at("2026-11-15T20:00:00Z")).unwrap();
        assert_eq!(got.iter().map(|c| c.number).collect::<Vec<_>>(), vec![7, 5]);
        assert_eq!(got[0].reason.as_deref(), Some("not_planned"));
        assert_eq!(got[1].reason.as_deref(), Some("completed"));
    }

    #[test]
    fn merged_pull_requests_use_merged_at() {
        let json = r#"[{"number": 12, "title": "PR", "mergedAt": "2026-11-15T21:00:00Z"}]"#;
        let got = parse_changes(json, &at("2026-11-15T20:00:00Z")).unwrap();
        assert_eq!(got, vec![Change { number: 12, title: "PR".into(), at: "2026-11-15T21:00:00Z".into(), reason: None }]);
    }

    #[test]
    fn garbage_is_no_answer_and_an_empty_list_is_an_answer() {
        assert_eq!(parse_changes("not json", &at("2026-11-15T20:00:00Z")), None);
        assert_eq!(parse_changes("[]", &at("2026-11-15T20:00:00Z")), Some(vec![]));
    }

    fn change(n: u32, at: &str) -> Change {
        Change { number: n, title: String::new(), at: at.into(), reason: None }
    }

    #[tokio::test]
    async fn github_is_asked_about_the_window_once_a_minute_whatever_is_asked() {
        let cache = SinceCache::default();
        let now = 1_794_800_000; // 2026-11-16T03:33:20Z
        let start = Instant::now();
        let at = |s: u64| move || start + Duration::from_secs(s);
        let village = VillageSince {
            project: "shop".into(),
            checked: true,
            closed: vec![change(5, "2026-11-16T03:00:00Z"), change(4, "2026-11-15T00:00:00Z"), change(3, "2026-11-10T00:00:00Z")],
            merged: vec![],
        };
        let asked = std::sync::Mutex::new(Vec::new());
        let fetch = |from: i64| {
            asked.lock().unwrap().push(from);
            let v = village.clone();
            async move { vec![v] }
        };
        let numbers = |a: &(i64, Vec<VillageSince>)| a.1[0].closed.iter().map(|c| c.number).collect::<Vec<_>>();
        // An hour ago: GitHub is asked about the whole window, the answer is cut to the hour.
        let a = cache.answer(now - 3600, now, at(0), fetch).await;
        assert_eq!((a.0, numbers(&a)), (now - 3600, vec![5]));
        // A day and a half ago, then a week ago, then the future, all within the minute: no more asking.
        assert_eq!(numbers(&cache.answer(now - 36 * 3600, now, at(5), fetch).await), vec![5, 4]);
        let week = cache.answer(now - 7 * 86400, now, at(10), fetch).await;
        assert_eq!(week.0, now - WINDOW, "a spell longer than the window is answered for the window");
        assert_eq!(numbers(&week), vec![5, 4]);
        assert!(cache.answer(now + 99_999, now, at(20), fetch).await.1[0].closed.is_empty());
        assert_eq!(*asked.lock().unwrap(), vec![now - WINDOW]);
        // A minute on, it is asked again.
        cache.answer(now - 3600, now + 60, at(FRESH.as_secs()), fetch).await;
        assert_eq!(*asked.lock().unwrap(), vec![now - WINDOW, now + 60 - WINDOW]);
    }

    #[tokio::test]
    async fn a_caller_that_hangs_up_does_not_cancel_the_asking() {
        let cache = SinceCache::default();
        let now = 1_794_800_000;
        let asked = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let fetch = |asked: std::sync::Arc<std::sync::atomic::AtomicUsize>| {
            move |_from: i64| {
                asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                async move {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    vec![VillageSince { project: "shop".into(), checked: true, closed: vec![change(5, "2026-11-16T03:00:00Z")], merged: vec![] }]
                }
            }
        };
        // The first caller gives up long before GitHub answers.
        let hung_up = tokio::time::timeout(Duration::from_millis(20), cache.answer(now - 3600, now, Instant::now, fetch(asked.clone()))).await;
        assert!(hung_up.is_err());
        // The next waits for that asking and gets its answer, without asking again.
        let got = cache.answer(now - 3600, now, Instant::now, fetch(asked.clone())).await;
        assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(got.1[0].checked);
        assert_eq!(got.1[0].closed.len(), 1);
    }

    #[tokio::test]
    async fn a_village_without_a_repository_is_unchecked() {
        let v = village_since(&ServerTransport::default(), "ghost", "", 1_794_800_000).await;
        assert!(!v.checked && v.closed.is_empty() && v.merged.is_empty());
        let v = village_since(&ServerTransport::default(), "ghost", "/no/such/dir/479", 1_794_800_000).await;
        assert!(!v.checked);
    }
}
