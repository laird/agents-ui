//! Flaky tests, from the merge gate's runs (#501, part of #486).
//!
//! Each merge job's log (`gate.rs`) holds one or more gate runs, each starting
//! "🧪 Gating the integrated tree <sha>" and holding the suite's output. A
//! test that fails in one run and is followed by a run of the **same tree**
//! that passes has failed with nothing changed in between: that is a flaky
//! sighting. A pass on a different tree proves nothing, since the code changed.
//!
//! Logs live in `/tmp` and are overwritten when an issue's merge is
//! relaunched, which is when the same tree is usually gated again, so every
//! finished run is recorded as it is seen, in `~/.agents-ui/flaky-<repo id>.json`.
//! Runs older than a week are dropped, so a test clean for 7 days drops out.
//! Recording is idempotent: a run is keyed by its issue, its job's start time
//! and its place in the log, which also order the runs.
//!
//! `/api/village` carries the result as `flaky`, newest first, each with the
//! open `Flaky: <test>` issue when there is one (looked up in the open issues
//! the village already fetched, so no extra `gh` call).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::gate::Job;
use super::IssueSnapshot;

/// How long a run is kept: a test clean for this long is no longer flaky.
pub const WINDOW_SECS: i64 = 7 * 24 * 3600;

const RUN_START: &str = "🧪 Gating the integrated tree ";

/// How a finished gate run ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Passed,
    /// The tests that failed, by name.
    Failed(Vec<String>),
}

/// One finished gate run, as recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub key: String,
    pub issue: u32,
    pub sha: String,
    /// When its job started (its pidfile's time), in Unix seconds; when it
    /// was first seen if that is unknown.
    pub when: i64,
    /// Its place in the job's log: runs of one job are in this order.
    #[serde(default)]
    pub place: usize,
    pub outcome: Outcome,
    pub log: String,
}

/// A flaky test in `/api/village`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Flaky {
    pub test: String,
    pub count: u32,
    /// RFC 3339, the newest failure.
    pub last_seen: String,
    /// Each failure, newest first.
    pub runs: Vec<FlakyRun>,
    /// The open `Flaky: <test>` issue, if there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlakyRun {
    /// The issue whose merge was being gated.
    pub issue: u32,
    pub when: String,
    pub log: String,
}

/// The finished runs in one log: (place in the log, tree sha, outcome). A run
/// is finished once another starts after it, or the job is done; until then
/// it may still fail. A run with no test results (a cached tree, a run cut
/// off) says nothing and is left out.
pub fn runs_in_log(log: &str, job_done: bool) -> Vec<(usize, String, Outcome)> {
    let mut segments: Vec<(String, Vec<&str>)> = Vec::new();
    for line in log.lines() {
        if let Some(rest) = line.trim_start().strip_prefix(RUN_START) {
            let sha = rest.split(|c: char| c == ':' || c.is_whitespace()).next().unwrap_or("").to_string();
            segments.push((sha, Vec::new()));
        } else if let Some((_, lines)) = segments.last_mut() {
            lines.push(line);
        }
    }
    let last = segments.len().saturating_sub(1);
    segments
        .into_iter()
        .enumerate()
        .filter(|(i, (sha, _))| !sha.is_empty() && (*i < last || job_done))
        .filter_map(|(i, (sha, lines))| {
            let failed: Vec<String> = lines
                .iter()
                .filter_map(|l| l.trim().strip_prefix("test ")?.strip_suffix(" ... FAILED").map(str::to_string))
                .collect();
            let results: Vec<&&str> = lines.iter().filter(|l| l.trim_start().starts_with("test result:")).collect();
            if !failed.is_empty() {
                Some((i, sha, Outcome::Failed(failed)))
            } else if !results.is_empty() && results.iter().all(|l| l.contains("test result: ok")) {
                Some((i, sha, Outcome::Passed))
            } else {
                None
            }
        })
        .collect()
}

/// Add the finished runs in `jobs`' logs to `runs`, once each, then drop
/// those older than the window. `log_path` names each job's log.
pub fn record(runs: &mut Vec<Run>, jobs: &[Job], log_path: &dyn Fn(u32) -> String, now: i64) {
    for job in jobs {
        let started = job.started.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64);
        let done = !job.alive || job.exit.is_some();
        for (i, sha, outcome) in runs_in_log(&job.log, done) {
            let key = format!("{}:{}:{i}:{sha}", job.issue, started.unwrap_or(0));
            if runs.iter().any(|r| r.key == key) {
                continue;
            }
            runs.push(Run { key, issue: job.issue, sha, when: started.unwrap_or(now), place: i, outcome, log: log_path(job.issue) });
        }
    }
    runs.retain(|r| now - r.when < WINDOW_SECS);
}

/// The flaky tests in `runs`: each failure followed by a pass of the same
/// tree. `open` are the open issues, for each test's `Flaky: <test>` issue.
pub fn flaky(runs: &[Run], open: &[IssueSnapshot]) -> Vec<Flaky> {
    let mut by_test: std::collections::BTreeMap<String, Vec<&Run>> = Default::default();
    for run in runs {
        let Outcome::Failed(tests) = &run.outcome else { continue };
        // Later: a job started after it, or later in the same job's log.
        let passed_later = runs.iter().any(|r| r.sha == run.sha && r.outcome == Outcome::Passed && (r.when, r.place) > (run.when, run.place));
        if passed_later {
            for t in tests {
                by_test.entry(t.clone()).or_default().push(run);
            }
        }
    }
    let when = |s: i64| chrono::DateTime::from_timestamp(s, 0).map(|d| d.to_rfc3339()).unwrap_or_default();
    let mut out: Vec<(i64, Flaky)> = by_test
        .into_iter()
        .map(|(test, mut fails)| {
            fails.sort_by_key(|r| std::cmp::Reverse(r.when));
            let newest = fails[0].when;
            let title = format!("Flaky: {test}");
            let issue = open.iter().find(|i| i.state == "open" && i.title.trim() == title).map(|i| i.number);
            let runs = fails.iter().map(|r| FlakyRun { issue: r.issue, when: when(r.when), log: r.log.clone() }).collect();
            (newest, Flaky { test, count: fails.len() as u32, last_seen: when(newest), runs, issue })
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.test.cmp(&b.1.test)));
    out.into_iter().map(|(_, f)| f).collect()
}

/// Where a repo's runs are kept: `~/.agents-ui/flaky-<repo id>.json`.
pub fn store_path(repo_id: &str) -> PathBuf {
    #[cfg(test)]
    let dir = crate::config::persistence::config_dir();
    #[cfg(not(test))]
    let dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".agents-ui");
    dir.join(format!("flaky-{repo_id}.json"))
}

pub fn load(path: &Path) -> Vec<Run> {
    std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save(path: &Path, runs: &[Run]) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Whole file or nothing: write beside it, then rename over it.
    let tmp = path.with_extension("json.tmp");
    if let Ok(bytes) = serde_json::to_vec(runs) {
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }
}

/// Record what `jobs` show for `repo`, and say which tests are flaky. Local
/// only, like the gate's files; a repo with no id has none.
pub fn read_local(repo: &Path, jobs: &[Job], open: &[IssueSnapshot]) -> Vec<Flaky> {
    let Some(id) = super::gate::local_repo_id(repo) else { return Vec::new() };
    let path = store_path(&id);
    let mut runs = load(&path);
    let before = runs.clone();
    let tmp = std::env::var("AUTOCODER_TMP").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/tmp".to_string());
    let log_path = |issue: u32| format!("{tmp}/autocoder-merge-{id}-{issue}.log");
    record(&mut runs, jobs, &log_path, chrono::Utc::now().timestamp());
    if runs != before {
        save(&path, &runs);
    }
    flaky(&runs, open)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    const NOW: i64 = 1_794_800_000;

    /// A job started `secs` after NOW.
    fn job(issue: u32, secs: u64, log: &str, done: bool) -> Job {
        let started = NOW as u64 + secs;
        Job {
            issue,
            alive: !done,
            exit: done.then_some(0),
            log: log.to_string(),
            feature: None,
            started: Some(UNIX_EPOCH + Duration::from_secs(started)),
            slot: None,
            members: Vec::new(),
        }
    }
    fn path(issue: u32) -> String {
        format!("/tmp/autocoder-merge-r-{issue}.log")
    }
    fn open(number: u32, title: &str) -> IssueSnapshot {
        IssueSnapshot { number, title: title.into(), state: "open".into(), priority: String::new(), labels: vec![], is_blocked: false, is_working: false, assigned_worker: None, url: String::new(), depends_on: vec![] }
    }

    // Recorded cargo test output, trimmed.
    const FAIL_A: &str = "🧪 Gating the integrated tree 44c8402c: source \"$HOME/.cargo/env\" && cargo test\ngate-cache: no usable record for tree 44c8402c; running gate\nrunning 3 tests\ntest web::a ... ok\ntest web::village_tests::notify::notify_and_widget_model_tests_pass ... FAILED\ntest web::c ... ok\n\nfailures:\n    web::village_tests::notify::notify_and_widget_model_tests_pass\n\ntest result: FAILED. 2 passed; 1 failed; 0 ignored\n❌ Tests fail on the integrated tree 44c8402c.\n";
    const PASS_A: &str = "🧪 Gating the integrated tree 44c8402c: cargo test\nrunning 3 tests\ntest web::a ... ok\ntest result: ok. 3 passed; 0 failed; 0 ignored\n✅ Issue #9 merged into 'master'\n";
    const PASS_B: &str = "🧪 Gating the integrated tree 5ede65f1: cargo test\nrunning 3 tests\ntest result: ok. 3 passed; 0 failed; 0 ignored\n✅ Issue #9 merged into 'master'\n";

    #[test]
    fn reads_each_finished_runs_tree_and_outcome() {
        let runs = runs_in_log(FAIL_A, true);
        assert_eq!(runs, vec![(0, "44c8402c".to_string(), Outcome::Failed(vec!["web::village_tests::notify::notify_and_widget_model_tests_pass".into()]))]);
        assert_eq!(runs_in_log(PASS_B, true), vec![(0, "5ede65f1".to_string(), Outcome::Passed)]);
        // Two runs in one log: the first is finished even while the job runs on.
        let both = format!("{PASS_B}{FAIL_A}");
        assert_eq!(runs_in_log(&both, false), vec![(0, "5ede65f1".to_string(), Outcome::Passed)]);
        assert_eq!(runs_in_log(&both, true).len(), 2);
        // A run still going, or with no results (a cached tree), says nothing.
        assert!(runs_in_log("🧪 Gating the integrated tree aa: cargo test\ngate-cache: tree aa passed before\n", true).is_empty());
    }

    #[test]
    fn a_failure_then_a_pass_of_the_same_tree_is_flaky() {
        let mut runs = Vec::new();
        record(&mut runs, &[job(9, 100, FAIL_A, true)], &path, NOW);
        assert!(flaky(&runs, &[]).is_empty(), "no pass yet");
        // The relaunched job overwrote the log: same issue, a later job, same tree.
        record(&mut runs, &[job(9, 200, PASS_A, true)], &path, NOW + 60);
        let f = flaky(&runs, &[]);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].test, "web::village_tests::notify::notify_and_widget_model_tests_pass");
        assert_eq!(f[0].count, 1);
        assert_eq!(f[0].runs[0].issue, 9);
        assert_eq!(f[0].runs[0].log, "/tmp/autocoder-merge-r-9.log");
        assert_eq!(f[0].last_seen, chrono::DateTime::from_timestamp(NOW + 100, 0).unwrap().to_rfc3339());
    }

    #[test]
    fn a_pass_of_a_different_tree_is_not_flaky() {
        let mut runs = Vec::new();
        record(&mut runs, &[job(9, 100, FAIL_A, true)], &path, NOW);
        record(&mut runs, &[job(9, 200, PASS_B, true)], &path, NOW + 60);
        assert!(flaky(&runs, &[]).is_empty());
    }

    #[test]
    fn a_pass_before_the_failure_is_not_flaky() {
        let mut runs = Vec::new();
        record(&mut runs, &[job(9, 100, PASS_A, true)], &path, NOW);
        record(&mut runs, &[job(9, 200, FAIL_A, true)], &path, NOW + 60);
        assert!(flaky(&runs, &[]).is_empty());
    }

    #[test]
    fn order_comes_from_the_jobs_not_from_reading_order() {
        // One pass reads both jobs, the passing one first (directory order).
        let mut runs = Vec::new();
        record(&mut runs, &[job(10, 200, PASS_A, true), job(9, 100, FAIL_A, true)], &path, NOW + 300);
        assert_eq!(flaky(&runs, &[]).len(), 1);
        // A retry later in the same job's log counts too.
        let mut one = Vec::new();
        record(&mut one, &[job(9, 100, &format!("{FAIL_A}{PASS_A}"), true)], &path, NOW + 300);
        assert_eq!(flaky(&one, &[]).len(), 1);
    }

    #[test]
    fn rereading_a_log_records_nothing_twice() {
        let mut runs = Vec::new();
        for t in 0..3 {
            record(&mut runs, &[job(9, 100, FAIL_A, true), job(10, 150, PASS_A, true)], &path, NOW + t);
        }
        assert_eq!(runs.len(), 2);
        assert_eq!(flaky(&runs, &[])[0].count, 1);
    }

    #[test]
    fn sightings_survive_the_logs_and_age_out_after_a_week() {
        let file = std::env::temp_dir().join(format!("agents-ui-flaky-test-{}.json", std::process::id()));
        let mut runs = Vec::new();
        record(&mut runs, &[job(9, 100, FAIL_A, true), job(10, 150, PASS_A, true)], &path, NOW);
        save(&file, &runs);
        // The logs are gone; the file still says it.
        let mut kept = load(&file);
        record(&mut kept, &[], &path, NOW + 100 + WINDOW_SECS - 1);
        assert_eq!(flaky(&kept, &[]).len(), 1);
        record(&mut kept, &[], &path, NOW + 150 + WINDOW_SECS);
        assert!(kept.is_empty() && flaky(&kept, &[]).is_empty(), "clean for a week");
        let _ = std::fs::remove_file(&file);
        assert!(load(&file).is_empty(), "no file is no runs");
    }

    #[test]
    fn names_the_open_flaky_issue_and_lists_newest_first() {
        let mut runs = Vec::new();
        let other = FAIL_A.replace("notify::notify_and_widget_model_tests_pass", "fit::fits");
        record(&mut runs, &[job(9, 100, FAIL_A, true), job(10, 150, PASS_A, true)], &path, NOW);
        record(&mut runs, &[job(11, 300, &other, true), job(12, 350, PASS_A, true)], &path, NOW + 600);
        let f = flaky(&runs, &[open(341, "Flaky: web::village_tests::fit::fits"), open(342, "Flaky: something else")]);
        assert_eq!(f.iter().map(|f| f.test.as_str()).collect::<Vec<_>>(), vec!["web::village_tests::fit::fits", "web::village_tests::notify::notify_and_widget_model_tests_pass"]);
        assert_eq!(f[0].issue, Some(341));
        assert_eq!(f[1].issue, None);
        let j = serde_json::to_value(&f[1]).unwrap();
        assert!(j.get("issue").is_none());
    }
}
