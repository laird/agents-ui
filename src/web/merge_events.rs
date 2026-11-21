//! autocoder's merge history: `merge-events.jsonl` (laird/agents#184).
//!
//! Every merge job appends one `attempt` line when it ends (its outcome, the
//! conflicting files, the failing suites, how long it took, the commit it
//! landed) and one `push-retry` line per rejected push, to
//! `${AUTOCODER_MERGE_EVENTS:-${XDG_STATE_HOME:-~/.local/state}/autocoder/merge-events.jsonl}`.
//! The format is documented in laird/agents `scripts/README.md` ("Merge
//! history").
//!
//! The file is shared by every repo the user merges in, and laird/agents' own
//! test suite has written to it (laird/agents#282), so everything here keeps
//! to one repo id ([`super::gate::repo_id`]).
//!
//! What it tells: how each finished job ended ([`event_gate`]), the bounces
//! ([`bounces`]) and what landed today ([`landed_today`]). A running job's
//! phase, slot and test progress are not in it; those still come from the
//! job's files (`gate.rs`). An autocoder too old to write the file, or one
//! that has not merged in this repo yet, leaves [`read`] with nothing, and the
//! village reads the job files, the job's comments and the push-retry commits
//! as before.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::gate::{Bounce, BounceKind, GateState, TaskGate};

/// One line of `merge-events.jsonl`. Fields the village doesn't use are left out.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct MergeEvent {
    /// When (ISO-8601 UTC): the end of the attempt, or the rejected push.
    #[serde(default)]
    pub ts: String,
    /// `attempt` or `push-retry` (or `flaky`, which is skipped here).
    #[serde(default)]
    pub event: String,
    /// `landed`, `parked`, `skipped`, `conflict`, `gate-failed`, `escalated`, `failed`, `push-retry`.
    #[serde(default)]
    pub outcome: String,
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub issue: Option<u32>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub duration_s: Option<u64>,
    #[serde(default)]
    pub push_retries: Option<u32>,
}

/// Where autocoder writes the file, from the environment (`env` looks a variable up).
pub fn path(env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let set = |k: &str| env(k).filter(|v| !v.is_empty());
    if let Some(p) = set("AUTOCODER_MERGE_EVENTS") {
        return Some(PathBuf::from(p));
    }
    // XDG_STATE_HOME counts only when absolute, as autocoder reads it.
    let state = set("XDG_STATE_HOME").filter(|s| s.starts_with('/')).map(PathBuf::from).or_else(|| set("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(state.join("autocoder/merge-events.jsonl"))
}

/// This repo's `attempt` and `push-retry` lines, oldest first. A line that is
/// not JSON (a write cut short) is skipped.
pub fn parse(text: &str, repo_id: &str) -> Vec<MergeEvent> {
    let mut out: Vec<MergeEvent> = text
        .lines()
        .filter(|l| l.contains(repo_id))
        .filter_map(|l| serde_json::from_str::<MergeEvent>(l).ok())
        .filter(|e| e.repo == repo_id && e.issue.is_some() && (e.event == "attempt" || e.event == "push-retry"))
        .collect();
    out.sort_by(|a, b| a.ts.cmp(&b.ts));
    out
}

/// This repo's events on this host. `None` when there are none to go on: the
/// file is missing (an autocoder older than laird/agents#184) or has no line
/// for this repo yet, so the caller falls back to the job files. Blocking.
pub fn read(repo_id: &str) -> Option<Vec<MergeEvent>> {
    read_file(&path(&|k| std::env::var(k).ok())?, repo_id)
}

/// [`read`], from a given file.
pub fn read_file(file: &Path, repo_id: &str) -> Option<Vec<MergeEvent>> {
    let events = parse(&std::fs::read_to_string(file).ok()?, repo_id);
    (!events.is_empty()).then_some(events)
}

/// Each issue's last finished attempt. `skipped` attempts (nothing to do)
/// say nothing new and are passed over.
pub fn latest_attempts(events: &[MergeEvent]) -> HashMap<u32, &MergeEvent> {
    let mut out = HashMap::new();
    for e in events.iter().filter(|e| e.event == "attempt" && e.outcome != "skipped") {
        if let Some(n) = e.issue {
            out.insert(n, e);
        }
    }
    out
}

/// A conflict, whatever the outcome is called: an escalated attempt with
/// conflicting files was a conflict too.
fn conflicted(e: &MergeEvent) -> bool {
    e.outcome == "conflict" || !e.files.is_empty()
}

/// Where a finished attempt left its issue. `labels` are the issue's: one
/// parked under `awaiting-integration` since landing is landed, not proven.
pub fn event_gate(e: &MergeEvent, labels: &[String], url: Option<&str>) -> Option<TaskGate> {
    let parked = labels.iter().any(|l| l == "awaiting-integration");
    let (state, landed) = match e.outcome.as_str() {
        "landed" if parked => (GateState::LandedUnproven, true),
        "landed" => (GateState::Passed, true),
        "parked" => (GateState::LandedUnproven, true),
        "conflict" | "gate-failed" | "escalated" | "failed" if conflicted(e) => (GateState::Conflict, false),
        "gate-failed" | "escalated" | "failed" => (GateState::Failed, false),
        _ => return None,
    };
    let ended = chrono::DateTime::parse_from_rfc3339(&e.ts).ok().map(|t| t.with_timezone(&chrono::Utc));
    let started = ended.map(|t| t - chrono::Duration::seconds(e.duration_s.unwrap_or(0) as i64));
    Some(TaskGate {
        state,
        landed,
        started_at: started.map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
        // A conflict never reached the gate; anything else gated once, and again after each re-sync.
        attempt: if state == GateState::Conflict { 0 } else { 1 + e.push_retries.unwrap_or(0) },
        failure_url: state.handed_back().then(|| url.map(str::to_string)).flatten(),
        train: e.branch.as_deref().and_then(|b| b.strip_prefix("train/")).map(str::to_string),
        ..TaskGate::default()
    })
}

/// RFC 3339 in UTC, as the bounces from comments and commits are.
fn utc(ts: &str) -> Option<String> {
    chrono::DateTime::parse_from_rfc3339(ts.trim())
        .ok()
        .map(|t| t.with_timezone(&chrono::Utc).to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

/// Each issue's bounces, oldest first: every attempt that handed the work back
/// (a conflict, with its files, or a failed gate) and every rejected push.
pub fn bounces(events: &[MergeEvent]) -> BTreeMap<u32, Vec<Bounce>> {
    let mut out: BTreeMap<u32, Vec<Bounce>> = BTreeMap::new();
    for e in events {
        let Some(n) = e.issue else { continue };
        let kind = match (e.event.as_str(), e.outcome.as_str()) {
            ("push-retry", _) => BounceKind::PushRetry,
            ("attempt", "conflict" | "gate-failed" | "escalated" | "failed") if conflicted(e) => BounceKind::Conflict,
            ("attempt", "gate-failed" | "escalated" | "failed") => BounceKind::GateFailed,
            _ => continue,
        };
        let files = if kind == BounceKind::Conflict { e.files.clone() } else { Vec::new() };
        out.entry(n).or_default().push(Bounce { at: utc(&e.ts), kind, files });
    }
    out
}

/// Attempts that landed today (`today` is `YYYY-MM-DD`, UTC), parked ones included.
pub fn landed_today(events: &[MergeEvent], today: &str) -> u32 {
    events
        .iter()
        .filter(|e| e.event == "attempt" && matches!(e.outcome.as_str(), "landed" | "parked"))
        .filter(|e| utc(&e.ts).is_some_and(|t| t.starts_with(today)))
        .count() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPO: &str = "agents-ui-33dc9185";

    /// Lines as autocoder 4.42 writes them, with another repo's and the test
    /// suite's lines mixed in (laird/agents#282), a flaky line and a torn write.
    const LOG: &str = r#"{"ts":"2026-11-21T01:00:00Z","event":"attempt","outcome":"conflict","repo":"agents-ui-33dc9185","issue":9,"branch":"feature/issue-9","attempt":1,"escalated":false,"files":["src/web/server.rs","src/web/village/main.js"],"gate_tests_failed":[],"duration_s":41,"landed":null,"push_retries":0,"exit":1,"reason":null}
{"ts":"2026-11-21T01:05:00Z","event":"attempt","outcome":"landed","repo":"agents-5ea0ca80","issue":9,"branch":"feature/issue-9","attempt":1,"escalated":false,"files":[],"gate_tests_failed":[],"duration_s":600,"landed":"abc","push_retries":0,"exit":0,"reason":null}
{"ts":"2026-11-21T01:06:00Z","event":"attempt","outcome":"conflict","repo":"worker-0208858f","issue":9,"branch":"feature/issue-9","attempt":1,"escalated":false,"files":["a.sh"],"gate_tests_failed":[],"duration_s":1,"landed":null,"push_retries":0,"exit":1,"reason":null}
{"ts":"2026-11-21T01:50:00Z","event":"push-retry","outcome":"push-retry","repo":"agents-ui-33dc9185","issue":9,"branch":"feature/issue-9","attempt":2,"push_retries":1,"files":[],"gate_tests_failed":[],"escalated":false}
{"ts":"2026-11-21T02:00:00Z","event":"attempt","outcome":"landed","repo":"agents-ui-33dc9185","issue":9,"branch":"feature/issue-9","attempt":2,"escalated":false,"files":[],"gate_tests_failed":[],"duration_s":900,"landed":"def","push_retries":1,"exit":0,"reason":null}
{"ts":"2026-11-21T01:40:00Z","event":"attempt","outcome":"gate-failed","repo":"agents-ui-33dc9185","issue":12,"branch":"feature/issue-12","attempt":1,"escalated":false,"files":[],"gate_tests_failed":["web::gate"],"duration_s":300,"landed":null,"push_retries":0,"exit":2,"reason":"tests fail on the integrated tree"}
{"ts":"2026-11-21T01:45:00Z","event":"flaky","outcome":"flaky","repo":"agents-ui-33dc9185","issue":12,"branch":"feature/issue-12","tree":"aaaa","test":"web::gate::x","first_failure_excerpt":"..."}
{"ts":"2026-11-21T03:00:00Z","event":"attempt","outcome":"skipped","repo":"agents-ui-33dc9185","issue":12,"branch":"feature/issue-12","attempt":2,"escalated":false,"files":[],"gate_tests_failed":[],"duration_s":2,"landed":null,"push_retries":0,"exit":0,"reason":"issue already closed; nothing merged"}
{"ts":"2026-11-20T23:00:00Z","event":"attempt","outcome":"parked","repo":"agents-ui-33dc9185","issue":15,"branch":"train/20261120T2230Z","attempt":1,"escalated":false,"files":[],"gate_tests_failed":[],"duration_s":60,"landed":"123","push_retries":0,"exit":0,"reason":null}
{"ts":"2026-11-21T04:00:00Z","event":"attempt","outcome":"escalated","repo":"agents-ui-33dc9185","issue":20,"branch":"feature/issue-20","attempt":3,"escalated":true,"files":["src/web/gate.rs"],"gate_tests_failed":[],"duration_s":30,"landed":null,"push_retries":0,"exit":1,"reason":null}
{"ts":"2026-11-21T04:10:00Z","event":"attempt","outcome":"escalated","repo":"agents-ui-33dc9185","issue":21,"branch":"feature/issue-21","attempt":1,"escalated":true,"files":[],"gate_tests_failed":[],"duration_s":30,"landed":null,"push_retries":0,"exit":2,"reason":"tests fail on the integrated tree and on origin/master alone"}
{"ts":"2026-11-21T04:20:00Z","event":"attempt","outcome":"landed","repo":"agents-ui-33dc9185","iss
"#;

    fn labels(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn only_this_repos_attempts_and_push_retries_oldest_first() {
        let ev = parse(LOG, REPO);
        assert!(ev.iter().all(|e| e.repo == REPO), "other repos and the test suite's records are dropped");
        assert!(ev.iter().all(|e| e.event != "flaky"));
        assert_eq!(ev.len(), 8, "the torn last line is skipped");
        assert_eq!(ev.first().map(|e| e.issue), Some(Some(15)), "sorted by time, not by line");
        assert!(parse(LOG, "agents-ui-33dc918").is_empty(), "a repo id is matched whole");
    }

    #[test]
    fn the_file_is_where_autocoder_writes_it() {
        let env = |vars: &'static [(&'static str, &'static str)]| move |k: &str| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
        assert_eq!(path(&env(&[("AUTOCODER_MERGE_EVENTS", "/x/e.jsonl"), ("HOME", "/h")])), Some(PathBuf::from("/x/e.jsonl")));
        assert_eq!(path(&env(&[("XDG_STATE_HOME", "/s"), ("HOME", "/h")])), Some(PathBuf::from("/s/autocoder/merge-events.jsonl")));
        assert_eq!(path(&env(&[("XDG_STATE_HOME", "rel"), ("HOME", "/h")])), Some(PathBuf::from("/h/.local/state/autocoder/merge-events.jsonl")), "a relative XDG_STATE_HOME is ignored");
        assert_eq!(path(&env(&[("AUTOCODER_MERGE_EVENTS", ""), ("HOME", "/h")])), Some(PathBuf::from("/h/.local/state/autocoder/merge-events.jsonl")));
        assert_eq!(path(&env(&[])), None);
    }

    #[test]
    fn read_is_none_without_the_file_or_without_this_repo() {
        let dir = std::env::temp_dir().join(format!("agents-ui-merge-events-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("merge-events.jsonl");
        std::fs::write(&file, LOG).unwrap();
        assert_eq!(read_file(&file, REPO).map(|e| e.len()), Some(8));
        assert_eq!(read_file(&file, "elsewhere-00000000"), None, "no line for this repo: the caller falls back");
        assert_eq!(read_file(&dir.join("missing.jsonl"), REPO), None, "no file: an autocoder older than laird/agents#184");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn each_outcome_is_a_gate_state() {
        let ev = parse(LOG, REPO);
        let latest = latest_attempts(&ev);
        let url = Some("https://github.com/o/r/issues/1");
        let gate = |n: u32, l: &[&str]| event_gate(latest[&n], &labels(l), url).map(|g| (g.state, g.landed, g.attempt));
        assert_eq!(gate(9, &[]), Some((GateState::Passed, true, 2)), "the landing after the conflict, gated twice");
        assert_eq!(gate(9, &["awaiting-integration"]), Some((GateState::LandedUnproven, true, 2)));
        assert_eq!(gate(12, &[]), Some((GateState::Failed, false, 1)), "the later skipped attempt says nothing");
        assert_eq!(gate(15, &[]), Some((GateState::LandedUnproven, true, 1)));
        assert_eq!(gate(20, &["needs-clarification"]), Some((GateState::Conflict, false, 0)), "escalated with conflicting files");
        assert_eq!(gate(21, &["needs-clarification"]), Some((GateState::Failed, false, 1)));
        let g = event_gate(latest[&9], &[], url).unwrap();
        assert_eq!(g.started_at.as_deref(), Some("2026-11-21T01:45:00Z"), "the end less its duration");
        assert_eq!((g.failure_url, g.slot, g.elapsed_secs), (None, None, None));
        assert_eq!(event_gate(latest[&20], &[], url).unwrap().failure_url.as_deref(), url);
        assert_eq!(event_gate(latest[&15], &[], url).unwrap().train.as_deref(), Some("20261120T2230Z"));
        let skipped = MergeEvent { outcome: "skipped".into(), event: "attempt".into(), ..MergeEvent::default() };
        assert_eq!(event_gate(&skipped, &[], url), None);
    }

    #[test]
    fn bounces_and_todays_landings_come_from_the_attempts() {
        let ev = parse(LOG, REPO);
        let b = bounces(&ev);
        let nine: Vec<_> = b[&9].iter().map(|b| (b.kind, b.at.clone().unwrap(), b.files.len())).collect();
        assert_eq!(nine, [(BounceKind::Conflict, "2026-11-21T01:00:00Z".to_string(), 2), (BounceKind::PushRetry, "2026-11-21T01:50:00Z".to_string(), 0)]);
        assert_eq!(b[&12].iter().map(|b| b.kind).collect::<Vec<_>>(), [BounceKind::GateFailed]);
        assert_eq!(b[&20][0].files, ["src/web/gate.rs"]);
        assert_eq!(b[&21][0].kind, BounceKind::GateFailed);
        assert!(!b.contains_key(&15), "a parked landing is no bounce");
        assert_eq!(landed_today(&ev, "2026-11-21"), 1);
        assert_eq!(landed_today(&ev, "2026-11-20"), 1, "the parked train");
    }
}
