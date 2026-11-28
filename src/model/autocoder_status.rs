//! What autocoder says each worker is doing (#542), read only through its
//! documented interface (laird/agents `docs/autocoder-ctl.md` and the
//! scripts README's "Status for UIs"):
//!
//! - `autocoder-ctl.sh status --json`: every worker of the swarm, its
//!   worktree, the issue its branch names, and whether it is busy or idle;
//! - `issue-events.jsonl`, the issue lifecycle log in
//!   `${XDG_STATE_HOME:-~/.local/state}/autocoder/`, filtered to this repo's
//!   id (the log is shared per user): an issue handed to the merge gate,
//!   released or escalated is no longer the worker's, though its branch
//!   still names it.
//!
//! autocoder no longer writes the `/tmp/agents-ui/*.json` files the TUI used
//! to read (laird/agents#193). An autocoder too old to answer (no
//! `autocoder-ctl.sh`, or exit 3) gives no reports, and the TUI infers each
//! worker's state from its pane, as it always has.
//!
//! `status` takes a few seconds (it samples each pane twice), so a [`Poller`]
//! runs it on a thread and the TUI reads the last answer.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How often a repo's status is asked for again.
const POLL_EVERY: Duration = Duration::from_secs(15);
/// A `status` that has not answered by then is killed and counts as failed.
const CTL_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    Busy,
    Idle,
    Unknown,
}

/// A worker row's note on what autocoder itself says (#545b): the activity
/// word, then its reason when one was given. `unknown` is spelled out, never
/// folded into `idle` -- a worker ctl can't read a pane for is not known to
/// be idle.
pub fn activity_note(activity: Activity, reason: Option<&str>) -> String {
    let word = match activity {
        Activity::Busy => "busy",
        Activity::Idle => "idle",
        Activity::Unknown => "unknown",
    };
    match reason {
        Some(r) if !r.is_empty() => format!("{word}: {r}"),
        _ => word.to_string(),
    }
}

/// One worker as `autocoder-ctl.sh status` lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct CtlWorker {
    pub session: String,
    pub number: u32,
    pub worktree: PathBuf,
    pub issue: Option<u32>,
    pub activity: Activity,
    /// worker-idle's reason for the `activity` verdict (`docs/autocoder-ctl.md`
    /// "Status for UIs"), e.g. "activity marker in pane" or "idle sentinel".
    /// `None` when the manifest gave none.
    pub reason: Option<String>,
}

/// What `status` answered.
#[derive(Debug, Clone, PartialEq)]
pub enum CtlStatus {
    Workers(Vec<CtlWorker>),
    /// This autocoder cannot answer: no `autocoder-ctl.sh`, or exit 3.
    Unsupported,
    /// It can, but this call failed (no manifest, a backend error, bad JSON).
    Failed,
}

/// What the TUI shows for one worker.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkerReport {
    pub issue: Option<u32>,
    pub activity: Activity,
    /// autocoder-ctl's reason for `activity` (#545b).
    pub reason: Option<String>,
}

/// Read `status`'s stdout, given its exit code.
pub fn parse_ctl_status(stdout: &str, exit: Option<i32>) -> CtlStatus {
    match exit {
        Some(0) => {}
        Some(3) => return CtlStatus::Unsupported,
        _ => return CtlStatus::Failed,
    }
    let Ok(val) = serde_json::from_str::<serde_json::Value>(stdout.trim()) else {
        return CtlStatus::Failed;
    };
    if val.get("ok").and_then(|v| v.as_bool()) == Some(false) {
        return CtlStatus::Failed;
    }
    // `detail` holds the verb's data; accept it at the top level too.
    let data = val.get("detail").filter(|d| d.is_object()).unwrap_or(&val);
    let Some(list) = data.get("workers").and_then(|w| w.as_array()) else {
        return CtlStatus::Failed;
    };
    let workers = list
        .iter()
        .filter_map(|w| {
            let worktree = w.get("worktree")?.as_str().filter(|s| !s.is_empty())?;
            Some(CtlWorker {
                session: w.get("session").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                number: w.get("number").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                worktree: PathBuf::from(worktree),
                issue: w.get("issue").and_then(|v| v.as_u64()).map(|n| n as u32),
                activity: match w.get("activity").and_then(|v| v.as_str()) {
                    Some("busy") => Activity::Busy,
                    Some("idle") => Activity::Idle,
                    _ => Activity::Unknown,
                },
                reason: w.get("reason").and_then(|v| v.as_str()).map(str::to_string),
            })
        })
        .collect();
    CtlStatus::Workers(workers)
}

/// Run `<script> status --json` for the swarm whose main checkout is `repo`.
/// Blocking. `None` for the script means none is installed.
pub fn run_ctl_status(script: Option<&Path>, repo: &Path) -> CtlStatus {
    let Some(script) = script.filter(|s| s.exists()) else {
        return CtlStatus::Unsupported;
    };
    run_with_timeout(
        std::process::Command::new("bash")
            .arg(script)
            .args(["status", "--json"])
            .current_dir(repo)
            .env("AUTOCODER_CTL_PROJECT_ROOT", repo),
        CTL_TIMEOUT,
    )
    .map_or(CtlStatus::Failed, |(stdout, code)| parse_ctl_status(&stdout, code))
}

/// Run `cmd`, killing it after `limit`: its stdout and exit code, or `None`
/// when it could not start or was killed.
fn run_with_timeout(cmd: &mut std::process::Command, limit: Duration) -> Option<(String, Option<i32>)> {
    use std::io::Read;
    use std::process::Stdio;
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    // Drain stdout on its own thread, so a large answer cannot fill the pipe.
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let _ = stdout.read_to_string(&mut out);
        out
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < limit => std::thread::sleep(Duration::from_millis(100)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    Some((reader.join().ok()?, status.code()))
}

/// `${AUTOCODER_ISSUE_EVENTS:-${XDG_STATE_HOME:-~/.local/state}/autocoder/issue-events.jsonl}`.
/// `XDG_STATE_HOME` counts only when absolute, as autocoder reads it.
pub fn events_path(env: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let set = |k: &str| env(k).filter(|v| !v.is_empty());
    if let Some(p) = set("AUTOCODER_ISSUE_EVENTS") {
        return Some(PathBuf::from(p));
    }
    let state = set("XDG_STATE_HOME")
        .filter(|s| s.starts_with('/'))
        .map(PathBuf::from)
        .or_else(|| set("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(state.join("autocoder/issue-events.jsonl"))
}

/// What the lifecycle log says about this repo's issues.
#[derive(Debug, Default, PartialEq)]
pub struct Lifecycle {
    /// Issues whose latest step is a claim: issue -> the `worker` that claimed it.
    pub claimed: HashMap<u32, String>,
    /// Issues whose latest step took them from their worker: released,
    /// escalated, decomposed, sent to the merge gate or landed.
    pub left: HashSet<u32>,
}

/// The lifecycle of `repo_id`'s issues, from the log. Only an issue's latest
/// step counts, so one bounced back by the gate (`gate-end`, no new claim)
/// has not left its worker. `None` when the log has no line for this repo.
pub fn lifecycle(log: &str, repo_id: &str) -> Option<Lifecycle> {
    let mut events: Vec<(String, u32, String, String)> = log
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|e| e.get("v").and_then(|v| v.as_u64()) == Some(1))
        .filter(|e| e.get("repo").and_then(|v| v.as_str()) == Some(repo_id))
        .filter_map(|e| {
            Some((
                e.get("ts")?.as_str()?.to_string(),
                e.get("issue")?.as_u64()? as u32,
                e.get("event")?.as_str()?.to_string(),
                e.get("worker").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            ))
        })
        .collect();
    if events.is_empty() {
        return None;
    }
    // ISO-8601 UTC sorts as text; the sort is stable for equal stamps.
    events.sort_by(|a, b| a.0.cmp(&b.0));
    let mut life = Lifecycle::default();
    for (_, issue, event, worker) in events {
        match event.as_str() {
            "claimed" => {
                life.left.remove(&issue);
                life.claimed.insert(issue, worker);
            }
            "released" | "escalated" | "decomposed" | "gate-queued" | "landed" => {
                life.claimed.remove(&issue);
                life.left.insert(issue);
            }
            // Any other step (the gate's start and end, a ctl command) leaves
            // the issue where it was, but no longer just claimed.
            "first-commit" | "deployed" | "released-tag" | "ctl" => {}
            _ => {
                life.left.remove(&issue);
            }
        }
    }
    Some(life)
}

/// Each worker's report, by worktree. The branch's issue counts unless the
/// log says it has since left the worker; a worker whose branch names no
/// issue takes the one the log says it claimed (`worker` is `<session>:<n>`
/// in a swarm, else the worktree's directory name).
pub fn reports(workers: &[CtlWorker], life: Option<&Lifecycle>) -> HashMap<PathBuf, WorkerReport> {
    workers
        .iter()
        .map(|w| {
            let issue = match life {
                None => w.issue,
                Some(life) => w.issue.filter(|n| !life.left.contains(n)).or_else(|| {
                    let tag = format!("{}:{}", w.session, w.number);
                    let dir = w.worktree.file_name().map(|d| d.to_string_lossy().into_owned());
                    life.claimed
                        .iter()
                        .filter(|(_, who)| **who == tag || Some(who.as_str()) == dir.as_deref())
                        .map(|(n, _)| *n)
                        .max()
                }),
            };
            (w.worktree.clone(), WorkerReport { issue, activity: w.activity, reason: w.reason.clone() })
        })
        .collect()
}

#[derive(Default)]
struct Polled {
    asked: Option<Instant>,
    running: bool,
    reports: Option<HashMap<PathBuf, WorkerReport>>,
}

/// Asks autocoder for each repo's workers in the background.
#[derive(Clone, Default)]
pub struct Poller {
    repos: Arc<Mutex<HashMap<PathBuf, Polled>>>,
}

impl Poller {
    /// The last reports for the swarm in `repo`, by worktree, or `None` when
    /// autocoder has not answered (yet, or ever). Asks again in the
    /// background once the answer is older than [`POLL_EVERY`].
    pub fn reports(&self, repo: &Path) -> Option<HashMap<PathBuf, WorkerReport>> {
        let mut repos = self.repos.lock().ok()?;
        let polled = repos.entry(repo.to_path_buf()).or_default();
        let due = polled.asked.map_or(true, |t| t.elapsed() >= POLL_EVERY);
        if due && !polled.running {
            polled.running = true;
            polled.asked = Some(Instant::now());
            let (repos, repo) = (self.repos.clone(), repo.to_path_buf());
            std::thread::spawn(move || {
                // A panic must still clear `running`, or the repo is never asked again.
                let answer = std::panic::catch_unwind(|| ask(&repo)).ok().flatten();
                if let Ok(mut repos) = repos.lock() {
                    let polled = repos.entry(repo).or_default();
                    polled.running = false;
                    polled.reports = answer;
                }
            });
        }
        polled.reports.clone()
    }
}

fn ask(repo: &Path) -> Option<HashMap<PathBuf, WorkerReport>> {
    let script = crate::scripts::launcher::find_script("autocoder-ctl.sh");
    let CtlStatus::Workers(workers) = run_ctl_status(script.as_deref(), repo) else {
        return None;
    };
    let life = crate::web::gate::local_repo_id(repo).and_then(|id| {
        let log = std::fs::read_to_string(events_path(|k| std::env::var(k).ok())?).ok()?;
        lifecycle(&log, &id)
    });
    // Key by the canonical path too, so a symlinked manifest path still matches.
    let mut by_path = reports(&workers, life.as_ref());
    let canonical: Vec<_> = by_path
        .iter()
        .filter_map(|(p, r)| Some((std::fs::canonicalize(p).ok().filter(|c| c != p)?, r.clone())))
        .collect();
    by_path.extend(canonical);
    Some(by_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_note_spells_out_unknown_instead_of_folding_it_into_idle() {
        assert_eq!(activity_note(Activity::Unknown, Some("no pane")), "unknown: no pane");
        assert_eq!(activity_note(Activity::Idle, Some("idle sentinel")), "idle: idle sentinel");
        assert_eq!(activity_note(Activity::Busy, Some("spinner")), "busy: spinner");
    }

    #[test]
    fn activity_note_without_a_reason_is_just_the_word() {
        assert_eq!(activity_note(Activity::Unknown, None), "unknown");
        assert_eq!(activity_note(Activity::Idle, Some("")), "idle");
    }

    const STATUS: &str = r#"{"ok": true, "action": "status", "target": "/src/r", "detail": {"swarms": 1, "workers": [
        {"session": "claude-r", "number": 1, "worktree": "/src/r-wt-1", "pane": "%3", "state": "running",
         "branch": "feature/issue-41", "issue": 41, "activity": "busy", "reason": "spinner"},
        {"session": "claude-r", "number": 2, "worktree": "/src/r-wt-2", "pane": null, "state": "running",
         "branch": "master-wt-2", "issue": null, "activity": "unknown", "reason": "no pane"},
        {"session": "claude-r", "number": 3, "worktree": "/src/r-wt-3", "pane": "%5", "state": "running",
         "branch": "master-wt-3", "issue": null, "activity": "idle", "reason": "idle sentinel"}]}}"#;

    fn worker(n: u32, issue: Option<u32>, activity: Activity) -> CtlWorker {
        worker_with_reason(n, issue, activity, None)
    }

    fn worker_with_reason(n: u32, issue: Option<u32>, activity: Activity, reason: Option<&str>) -> CtlWorker {
        CtlWorker {
            session: "claude-r".into(),
            number: n,
            worktree: PathBuf::from(format!("/src/r-wt-{n}")),
            issue,
            activity,
            reason: reason.map(str::to_string),
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("agents-ui-ctl-{name}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_each_worker_of_the_status_verb() {
        let CtlStatus::Workers(ws) = parse_ctl_status(STATUS, Some(0)) else { panic!("no workers") };
        assert_eq!(
            ws,
            vec![
                worker_with_reason(1, Some(41), Activity::Busy, Some("spinner")),
                worker_with_reason(2, None, Activity::Unknown, Some("no pane")),
                worker_with_reason(3, None, Activity::Idle, Some("idle sentinel"))
            ]
        );
    }

    #[test]
    fn no_swarm_is_no_workers_not_a_failure() {
        let empty = r#"{"ok": true, "action": "status", "target": "/src/r", "detail": {"swarms": 0, "workers": []}}"#;
        assert_eq!(parse_ctl_status(empty, Some(0)), CtlStatus::Workers(vec![]));
    }

    #[test]
    fn exit_3_is_an_autocoder_too_old_to_answer() {
        let old = r#"{"ok": false, "action": "status", "target": null, "detail": "not implemented"}"#;
        assert_eq!(parse_ctl_status(old, Some(3)), CtlStatus::Unsupported);
    }

    #[test]
    fn a_failed_call_or_garbage_is_a_failure() {
        assert_eq!(parse_ctl_status(r#"{"ok": false, "detail": "no manifest"}"#, Some(1)), CtlStatus::Failed);
        assert_eq!(parse_ctl_status("not json", Some(0)), CtlStatus::Failed);
        assert_eq!(parse_ctl_status(r#"{"ok": true, "detail": "hm"}"#, Some(0)), CtlStatus::Failed);
        assert_eq!(parse_ctl_status(STATUS, None), CtlStatus::Failed, "killed by a signal");
    }

    #[test]
    fn no_script_is_an_autocoder_too_old_to_answer() {
        let dir = temp_dir("none");
        assert_eq!(run_ctl_status(None, &dir), CtlStatus::Unsupported);
        assert_eq!(run_ctl_status(Some(&dir.join("autocoder-ctl.sh")), &dir), CtlStatus::Unsupported);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn runs_the_script_in_the_repo_and_reads_its_answer() {
        let dir = temp_dir("run");
        let script = dir.join("autocoder-ctl.sh");
        // Answers only `status --json`, and names the project root it was given.
        std::fs::write(
            &script,
            r#"[ "$1 $2" = "status --json" ] || exit 2
printf '{"ok":true,"action":"status","target":"%s","detail":{"workers":[{"session":"s","number":1,"worktree":"%s","issue":7,"activity":"busy"}]}}' "$AUTOCODER_CTL_PROJECT_ROOT" "$AUTOCODER_CTL_PROJECT_ROOT""#,
        )
        .unwrap();
        let CtlStatus::Workers(ws) = run_ctl_status(Some(&script), &dir) else { panic!("no workers") };
        assert_eq!((ws[0].worktree.as_path(), ws[0].issue), (dir.as_path(), Some(7)));

        std::fs::write(&script, r#"echo '{"ok":false,"detail":"not implemented"}'; exit 3"#).unwrap();
        assert_eq!(run_ctl_status(Some(&script), &dir), CtlStatus::Unsupported);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_events_log_is_where_autocoder_writes_it() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
        };
        assert_eq!(events_path(env(&[("AUTOCODER_ISSUE_EVENTS", "/x.jsonl"), ("HOME", "/h")])), Some(PathBuf::from("/x.jsonl")));
        assert_eq!(events_path(env(&[("XDG_STATE_HOME", "/s"), ("HOME", "/h")])), Some(PathBuf::from("/s/autocoder/issue-events.jsonl")));
        assert_eq!(
            events_path(env(&[("XDG_STATE_HOME", "rel"), ("HOME", "/h")])),
            Some(PathBuf::from("/h/.local/state/autocoder/issue-events.jsonl")),
            "a relative XDG_STATE_HOME is ignored"
        );
        assert_eq!(events_path(env(&[])), None);
    }

    const LOG: &str = r#"{"v":1,"ts":"2026-11-21T10:00:00Z","repo":"r-1234","issue":41,"worker":"claude-r:1","branch":"feature/issue-41","event":"claimed","detail":{}}
{"v":1,"ts":"2026-11-21T10:00:00Z","repo":"other-9999","issue":50,"worker":"claude-r:2","branch":"x","event":"claimed","detail":{}}
{"v":1,"ts":"2026-11-21T09:00:00Z","repo":"r-1234","issue":38,"worker":"claude-r:1","branch":"feature/issue-38","event":"claimed","detail":{}}
{"v":1,"ts":"2026-11-21T09:30:00Z","repo":"r-1234","issue":38,"worker":"claude-r:1","branch":"feature/issue-38","event":"gate-queued","detail":{"attempt":1}}
{"v":1,"ts":"2026-11-21T10:05:00Z","repo":"r-1234","issue":60,"worker":"r-wt-3","branch":"master-wt-3","event":"claimed","detail":{}}
{"v":2,"ts":"2026-11-21T10:06:00Z","repo":"r-1234","issue":60,"worker":"r-wt-3","event":"released","detail":{}}
{"v":1,"ts":"2026-11-21T10:07:00Z","repo":"r-1234","issue":null,"worker":"r-wt-3","event":"deployed","detail":{}}
not json"#;

    #[test]
    fn only_an_issues_latest_step_counts() {
        let life = lifecycle(LOG, "r-1234").unwrap();
        // 38 went to the gate (its lines are out of order in the file); 50 is
        // another repo's; the v2 release of 60 is a format this reader skips.
        let mut claimed: Vec<_> = life.claimed.iter().map(|(n, w)| (*n, w.as_str())).collect();
        claimed.sort();
        assert_eq!(claimed, vec![(41, "claude-r:1"), (60, "r-wt-3")]);
        assert_eq!(life.left, HashSet::from([38]));
    }

    #[test]
    fn a_repo_with_no_events_says_nothing() {
        assert_eq!(lifecycle(LOG, "unseen-0000"), None);
        assert_eq!(lifecycle("", "r-1234"), None);
    }

    #[test]
    fn the_log_overrules_a_branch_whose_issue_has_left_the_worker() {
        let life = lifecycle(LOG, "r-1234").unwrap();
        let ws = [
            worker(1, Some(38), Activity::Busy), // its branch still says 38, handed to the gate
            worker(2, Some(70), Activity::Busy), // 70 predates the log: the branch still counts
            worker(3, None, Activity::Idle),     // claimed 60 before switching branch
        ];
        let r = reports(&ws, Some(&life));
        assert_eq!(r[Path::new("/src/r-wt-1")].issue, Some(41), "takes the issue it claimed instead");
        assert_eq!(r[Path::new("/src/r-wt-2")].issue, Some(70));
        assert_eq!(r[Path::new("/src/r-wt-3")], WorkerReport { issue: Some(60), activity: Activity::Idle, reason: None });
    }

    #[test]
    fn an_issue_the_gate_bounced_back_is_its_workers_again() {
        let log = format!(
            "{LOG}\n{}",
            r#"{"v":1,"ts":"2026-11-21T09:40:00Z","repo":"r-1234","issue":38,"worker":"claude-r:1","event":"gate-end","detail":{"outcome":"conflict"}}"#
        );
        let life = lifecycle(&log, "r-1234").unwrap();
        assert!(!life.left.contains(&38));
        let r = reports(&[worker(4, Some(38), Activity::Busy)], Some(&life));
        assert_eq!(r[Path::new("/src/r-wt-4")].issue, Some(38));
    }

    #[test]
    fn a_status_that_hangs_is_killed_and_counts_as_failed() {
        let dir = temp_dir("hang");
        let script = dir.join("autocoder-ctl.sh");
        std::fs::write(&script, "sleep 30").unwrap();
        let start = Instant::now();
        let got = run_with_timeout(std::process::Command::new("bash").arg(&script), Duration::from_millis(300));
        assert_eq!(got, None);
        assert!(start.elapsed() < Duration::from_secs(10));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_workers_reason_passes_through_to_its_report() {
        let r = reports(&[worker_with_reason(1, Some(38), Activity::Idle, Some("idle sentinel"))], None);
        assert_eq!(r[Path::new("/src/r-wt-1")].reason, Some("idle sentinel".to_string()));
    }

    #[test]
    fn without_the_log_the_branch_names_the_issue() {
        let r = reports(&[worker(1, Some(38), Activity::Busy)], None);
        assert_eq!(r[Path::new("/src/r-wt-1")], WorkerReport { issue: Some(38), activity: Activity::Busy, reason: None });
    }
}
