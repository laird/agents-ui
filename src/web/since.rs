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
//! Each fresh answer costs two GitHub searches per village, and the endpoint
//! has no authentication, so answers are kept for [`FRESH`] ([`SinceCache`]):
//! a later moment is answered from the kept one, an earlier one waits (429).

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

/// How long an answer is kept.
pub const FRESH: Duration = Duration::from_secs(60);

/// The last answer, for [`FRESH`]: (when, the moment it was asked about, the villages).
#[derive(Default)]
pub struct SinceCache(tokio::sync::Mutex<Option<(Instant, i64, Vec<VillageSince>)>>);

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
    /// The answer for `t`: the kept one when it is fresh and asked about `t`
    /// or earlier, `None` (too soon) when it is fresh but later than `t`,
    /// else a new one from `fetch`. Requests wait on each other, so a burst
    /// costs one fetch.
    pub async fn answer<F, Fut>(&self, t: i64, now: Instant, fetch: F) -> Option<Vec<VillageSince>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Vec<VillageSince>>,
    {
        let mut kept = self.0.lock().await;
        if let Some((at, kept_t, villages)) = kept.as_ref() {
            if now.saturating_duration_since(*at) < FRESH {
                return (*kept_t <= t).then(|| narrow(villages, t));
            }
        }
        let villages = fetch().await;
        *kept = Some((now, t, villages.clone()));
        Some(villages)
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
    async fn answers_are_kept_a_minute_and_narrowed_to_a_later_moment() {
        let cache = SinceCache::default();
        let t0 = 1_794_800_000; // 2026-11-16T03:33:20Z
        let start = Instant::now();
        let village = VillageSince {
            project: "shop".into(),
            checked: true,
            closed: vec![change(5, "2026-11-16T03:40:00Z"), change(4, "2026-11-16T03:35:00Z")],
            merged: vec![],
        };
        let calls = std::sync::atomic::AtomicU32::new(0);
        let fetch = || async {
            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            vec![village.clone()]
        };
        assert_eq!(cache.answer(t0, start, fetch).await.unwrap()[0].closed.len(), 2);
        // Ten minutes later in the moment asked about, a second later in time: from the kept answer.
        let later = cache.answer(t0 + 300, start + Duration::from_secs(1), fetch).await.unwrap();
        assert_eq!(later[0].closed.iter().map(|c| c.number).collect::<Vec<_>>(), vec![5]);
        // An earlier moment can't be cut from it: too soon.
        assert!(cache.answer(t0 - 3600, start + Duration::from_secs(2), fetch).await.is_none());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        // Once it is stale, anything is asked again.
        assert!(cache.answer(t0 - 3600, start + FRESH, fetch).await.is_some());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_village_without_a_repository_is_unchecked() {
        let v = village_since(&ServerTransport::default(), "ghost", "", 1_794_800_000).await;
        assert!(!v.checked && v.closed.is_empty() && v.merged.is_empty());
        let v = village_since(&ServerTransport::default(), "ghost", "/no/such/dir/479", 1_794_800_000).await;
        assert!(!v.checked);
    }
}
