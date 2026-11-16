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
//!
//! `POST /api/swarms/{project}/flaky` (#503, the ghost's button) files a test
//! as `Flaky: <test>` with its failures, or comments the ones not yet on the
//! open issue. A write: a `--read-only` server refuses it; GitHub only.

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
/// off) or one the gate says failed without naming a test says nothing and
/// is left out. Each run comes with its place in the log and which time its
/// tree is gated there (0 for the first): the second is what keys it, as it
/// stays the same when `tail` cuts the log's start off.
pub fn runs_in_log(log: &str, job_done: bool) -> Vec<(usize, usize, String, Outcome)> {
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
    let mut seen: std::collections::HashMap<String, usize> = Default::default();
    let nth: Vec<usize> = segments
        .iter()
        .map(|(sha, _)| {
            let n = seen.entry(sha.clone()).or_default();
            *n += 1;
            *n - 1
        })
        .collect();
    segments
        .into_iter()
        .enumerate()
        .filter(|(i, (sha, _))| !sha.is_empty() && (*i < last || job_done))
        .filter_map(|(i, (sha, lines))| {
            let nth = nth[i];
            let failed: Vec<String> = lines
                .iter()
                .filter_map(|l| l.trim().strip_prefix("test ")?.strip_suffix(" ... FAILED").map(str::to_string))
                .collect();
            let results: Vec<&&str> = lines.iter().filter(|l| l.trim_start().starts_with("test result:")).collect();
            let gate_failed = lines.iter().any(|l| l.trim_start().starts_with("❌ Tests fail"));
            if !failed.is_empty() {
                Some((i, nth, sha, Outcome::Failed(failed)))
            } else if !gate_failed && !results.is_empty() && results.iter().all(|l| l.contains("test result: ok")) {
                Some((i, nth, sha, Outcome::Passed))
            } else {
                None
            }
        })
        .collect()
}

/// Add the finished runs in `jobs`' logs to `runs`, once each, then drop
/// those older than the window. `log_path` names each job's log.
pub fn record<'a>(runs: &mut Vec<Run>, jobs: impl IntoIterator<Item = &'a Job>, log_path: &dyn Fn(u32) -> String, now: i64) {
    for job in jobs {
        let started = job.started.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64);
        if started.is_some_and(|s| now - s >= WINDOW_SECS) {
            continue; // its runs would be dropped below anyway
        }
        let done = !job.alive || job.exit.is_some();
        for (i, nth, sha, outcome) in runs_in_log(&job.log, done) {
            let key = format!("{}:{}:{sha}:{nth}", job.issue, started.unwrap_or(0));
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
            let title = issue_title(&test);
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
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = path.with_extension(format!("json.{}.{n}.tmp", std::process::id()));
    if let Ok(bytes) = serde_json::to_vec(runs) {
        if std::fs::write(&tmp, bytes).is_ok() && std::fs::rename(&tmp, path).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

/// Record what `jobs` show for `repo`, and say which tests are flaky. Local
/// only, like the gate's files; a repo with no id has none.
pub fn read_local(repo: &Path, jobs: &[Job], open: &[IssueSnapshot]) -> Vec<Flaky> {
    // One at a time: every browser's poll lands here, and two that load, add
    // and save together would each drop what the other added.
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _held = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some(id) = super::gate::local_repo_id(repo) else { return Vec::new() };
    let path = store_path(&id);
    let mut runs = load(&path);
    let before = runs.clone();
    let tmp = std::env::var("AUTOCODER_TMP").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/tmp".to_string());
    let log_path = |issue: u32| format!("{tmp}/autocoder-merge-{id}-{issue}.log");
    // A log that hasn't grown since the last poll has nothing new to say.
    static READ: std::sync::Mutex<Option<std::collections::HashMap<(String, u32, Option<std::time::SystemTime>), usize>>> = std::sync::Mutex::new(None);
    let mut read = READ.lock().unwrap_or_else(|e| e.into_inner());
    let read = read.get_or_insert_with(Default::default);
    let changed: Vec<&Job> = jobs
        .iter()
        .filter(|j| {
            // Its length, and one more once it is done (its last run then counts).
            let size = j.log.len() + usize::from(!j.alive || j.exit.is_some());
            read.insert((id.clone(), j.issue, j.started), size) != Some(size)
        })
        .collect();
    record(&mut runs, changed, &log_path, chrono::Utc::now().timestamp());
    if runs != before {
        save(&path, &runs);
    }
    flaky(&runs, open)
}

// ---- Filing a flaky test (#503) ------------------------------------------

/// The issue a flaky test is filed as, named as the ones filed by hand (#341, #358).
pub fn issue_title(test: &str) -> String {
    format!("Flaky: {test}")
}

/// How a failure starts its line in an issue: what tells, in a comment
/// already posted, that this failure was reported.
fn failure_head(r: &FlakyRun) -> String {
    format!("- {} — while gating #{}'s merge", r.when, r.issue)
}

/// Failures one line each: when, whose merge was gated, the log.
fn failure_lines<'a>(runs: impl IntoIterator<Item = &'a FlakyRun>) -> String {
    runs.into_iter()
        .map(|r| format!("{}; log: `{}`", failure_head(r), r.log))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The failures not yet in an issue's body or comments (`gh issue view --json
/// body,comments`), so pressing the button again never repeats them.
pub fn unreported<'a>(f: &'a Flaky, view_json: &[u8]) -> Vec<&'a FlakyRun> {
    let v: serde_json::Value = serde_json::from_slice(view_json).unwrap_or_default();
    let mut text = v["body"].as_str().unwrap_or("").to_string();
    for c in v["comments"].as_array().into_iter().flatten() {
        text.push('\n');
        text.push_str(c["body"].as_str().unwrap_or(""));
    }
    f.runs.iter().filter(|r| !text.contains(&failure_head(r))).collect()
}

/// The new issue's body: what flaked, how often, and each failure.
pub fn report_body(f: &Flaky) -> String {
    format!(
        "`{test}` failed and then passed on the same tree {n} this week, so the failure was not caused by the change being gated.\n\n\
         Failures (newest first):\n{lines}\n\n\
         The logs are the merge gate's (`/tmp/autocoder-merge-*.log`) and are overwritten when a merge is relaunched.\n\n\
         🤖 Filed from the village's ghost at the merge gate",
        test = f.test,
        n = times(f.count),
        lines = failure_lines(&f.runs),
    )
}

/// A comment for the open issue: the failures it does not list yet.
pub fn update_body(f: &Flaky, new: &[&FlakyRun]) -> String {
    format!(
        "Flaked again ({n} this week), failing and then passing on the same tree:\n\n{lines}\n\n🤖 From the village's ghost at the merge gate",
        n = times(f.count),
        lines = failure_lines(new.iter().copied()),
    )
}

fn times(n: u32) -> String {
    if n == 1 { "once".to_string() } else { format!("{n} times") }
}

/// The open issue already filed for `test`, from `gh issue list --json number,title`:
/// the title must match exactly, so `Flaky: a::b` is not `Flaky: a::bc`.
pub fn open_issue_for(list_json: &[u8], test: &str) -> Option<u32> {
    let want = issue_title(test);
    let issues: Vec<serde_json::Value> = serde_json::from_slice(list_json).ok()?;
    issues
        .iter()
        .find(|i| i["title"].as_str().map(str::trim) == Some(want.as_str()))
        .and_then(|i| i["number"].as_u64())
        .and_then(|n| u32::try_from(n).ok())
}

#[derive(Debug, Deserialize)]
pub struct FileBody {
    pub test: String,
}

/// POST /api/swarms/{project}/flaky: file a flaky test as `Flaky: <test>`, or
/// comment its failures on the open one. A write, so a `--read-only` server
/// refuses it (403) before it gets here. Only a test this repo's gate has
/// seen flake this week can be filed, so the body is the gate's record.
pub async fn api_file_handler(
    axum::extract::Path(project): axum::extract::Path<String>,
    axum::extract::State(state): axum::extract::State<super::server::WebServerState>,
    axum::Json(body): axum::Json<FileBody>,
) -> Result<axum::Json<serde_json::Value>, (axum::http::StatusCode, axum::Json<serde_json::Value>)> {
    use axum::http::StatusCode;
    use serde_json::json;
    let fail = |code: StatusCode, msg: &str| (code, axum::Json(json!({ "error": msg })));
    let repo = state
        .swarms
        .read()
        .ok()
        .and_then(|s| s.iter().find(|s| s.project_name == project).map(|s| s.repo_path.clone()))
        .filter(|r| !r.is_empty())
        .ok_or_else(|| fail(StatusCode::NOT_FOUND, "no such swarm"))?;
    let repo = PathBuf::from(repo);
    let test = body.test.trim().to_string();
    let client = crate::project_management::ProjectManagementClient::from_env();
    if client.backend() != crate::project_management::ProjectManagementBackend::GitHub {
        return Err(fail(StatusCode::NOT_IMPLEMENTED, "filing a flaky test needs the GitHub tracker"));
    }
    let id = super::gate::local_repo_id(&repo)
        .ok_or_else(|| fail(StatusCode::INTERNAL_SERVER_ERROR, "can't tell which repo this swarm's gate records"))?;
    let found = {
        let test = test.clone();
        tokio::task::spawn_blocking(move || {
            // Only this week's runs: the store is pruned when a village is
            // polled, which may not have happened since a run aged out.
            let week = chrono::Utc::now().timestamp() - WINDOW_SECS;
            let runs: Vec<Run> = load(&store_path(&id)).into_iter().filter(|r| r.when >= week).collect();
            flaky(&runs, &[]).into_iter().find(|f| f.test == test)
        })
        .await
        .map_err(|_| fail(StatusCode::INTERNAL_SERVER_ERROR, "could not read the gate's record"))?
    };
    let f = found.ok_or_else(|| fail(StatusCode::NOT_FOUND, "that test has not flaked here this week"))?;

    // One filing at a time: two clicks that both find no issue would both create one.
    static FILING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _one = FILING.lock().await;
    let run = |cmd: crate::project_management::ProjectCommand| {
        let (transport, repo) = (state.transport.clone(), repo.clone());
        async move { transport.output(cmd.program, &cmd.args, Some(&repo)).await }
    };
    let ask = |out: anyhow::Result<std::process::Output>, what: &str| match out {
        Ok(o) if o.status.success() => Ok(o.stdout),
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
            tracing::warn!("filing flaky {test}: {what} failed: {err}");
            Err(fail(StatusCode::BAD_GATEWAY, if err.is_empty() { "the tracker refused it" } else { &err }))
        }
        Err(_) => Err(fail(StatusCode::BAD_GATEWAY, "could not reach the tracker")),
    };
    let title = issue_title(&test);
    let list = ask(run(client.search_open_issues(&format!("\"{title}\" in:title"), "number,title", 20)).await, "the search")?;
    let (action, number) = match open_issue_for(&list, &test) {
        Some(n) => {
            let view = ask(run(client.view_issue_json(n, "body,comments")).await, "reading the issue")?;
            let new = unreported(&f, &view);
            if new.is_empty() {
                return Ok(axum::Json(json!({ "ok": true, "action": "unchanged", "number": n })));
            }
            ask(run(client.comment_issue(n, &update_body(&f, &new))).await, "the comment")?;
            ("commented", Some(n))
        }
        None => {
            let out = ask(run(client.create_issue(&title, Some(&report_body(&f)), &[])).await, "creating it")?;
            ("created", super::newissue::created(&String::from_utf8_lossy(&out)).map(|(_, n)| n))
        }
    };
    // The next poll's village carries the issue, so the ghost shows its link.
    state.village.forget(&repo);
    Ok(axum::Json(json!({ "ok": true, "action": action, "number": number })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_flaky() -> Flaky {
        Flaky {
            test: "web::gate::tests::a_slow_one".into(),
            count: 2,
            last_seen: "2026-11-15T10:00:00Z".into(),
            runs: vec![
                FlakyRun { issue: 480, when: "2026-11-15T10:00:00Z".into(), log: "/tmp/autocoder-merge-x-480.log".into() },
                FlakyRun { issue: 470, when: "2026-11-14T09:00:00Z".into(), log: "/tmp/autocoder-merge-x-470.log".into() },
            ],
            issue: None,
        }
    }

    /// #503: the issue is named as the ones filed by hand, and lists each failure.
    #[test]
    fn a_flaky_test_is_filed_with_its_failures() {
        let f = sample_flaky();
        assert_eq!(issue_title(&f.test), "Flaky: web::gate::tests::a_slow_one");
        let body = report_body(&f);
        assert!(body.contains("failed and then passed on the same tree 2 times this week"), "{body}");
        assert!(body.contains("- 2026-11-15T10:00:00Z — while gating #480's merge; log: `/tmp/autocoder-merge-x-480.log`"), "{body}");
        assert!(body.contains("#470"));
        let newest = [&f.runs[0]];
        let again = update_body(&Flaky { count: 1, ..f.clone() }, &newest);
        assert!(again.starts_with("Flaked again (once this week)"), "{again}");
        assert!(again.contains("#480") && !again.contains("#470"), "only the failures given: {again}");
    }

    /// A second press comments only what the issue does not list yet.
    #[test]
    fn only_failures_not_yet_on_the_issue_are_commented() {
        let f = sample_flaky();
        let filed = serde_json::to_vec(&serde_json::json!({ "body": report_body(&Flaky { runs: vec![f.runs[1].clone()], ..f.clone() }), "comments": [] })).unwrap();
        assert_eq!(unreported(&f, &filed).iter().map(|r| r.issue).collect::<Vec<_>>(), vec![480]);
        let both = serde_json::to_vec(&serde_json::json!({ "body": "", "comments": [{ "body": report_body(&f) }] })).unwrap();
        assert!(unreported(&f, &both).is_empty(), "everything reported: nothing to say");
        assert_eq!(unreported(&f, b"not json").len(), 2);
    }

    /// The open issue is found by its exact title: a test whose name merely
    /// starts the same is another test.
    #[test]
    fn the_open_flaky_issue_is_matched_by_its_exact_title() {
        let list = br#"[{"number":341,"title":"Flaky: a::bc"},{"number":358,"title":"Flaky: a::b "},{"number":9,"title":"Fix a::b"}]"#;
        assert_eq!(open_issue_for(list, "a::b"), Some(358));
        assert_eq!(open_issue_for(list, "a::bc"), Some(341));
        assert_eq!(open_issue_for(list, "a"), None);
        assert_eq!(open_issue_for(b"not json", "a::b"), None);
    }


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
        assert_eq!(runs, vec![(0, 0, "44c8402c".to_string(), Outcome::Failed(vec!["web::village_tests::notify::notify_and_widget_model_tests_pass".into()]))]);
        assert_eq!(runs_in_log(PASS_B, true), vec![(0, 0, "5ede65f1".to_string(), Outcome::Passed)]);
        // Two runs in one log: the first is finished even while the job runs on.
        let both = format!("{PASS_B}{FAIL_A}");
        assert_eq!(runs_in_log(&both, false), vec![(0, 0, "5ede65f1".to_string(), Outcome::Passed)]);
        assert_eq!(runs_in_log(&both, true).len(), 2);
        // A run still going, or with no results (a cached tree), says nothing.
        assert!(runs_in_log("🧪 Gating the integrated tree aa: cargo test\ngate-cache: tree aa passed before\n", true).is_empty());
        // The gate says it failed but no test is named: not a pass.
        assert!(runs_in_log("🧪 Gating the integrated tree aa: x\ntest result: ok. 3 passed\n❌ Tests fail on the integrated tree aa.\n", true).is_empty());
    }

    #[test]
    fn a_run_keeps_its_key_when_the_logs_start_is_cut_off() {
        // `tail` drops the first run once the log is long: the rest are not new.
        let mut runs = Vec::new();
        let long = format!("{PASS_B}{FAIL_A}{PASS_A}");
        record(&mut runs, &[job(9, 100, &long, true)], &path, NOW + 300);
        let cut = format!("{FAIL_A}{PASS_A}");
        record(&mut runs, &[job(9, 100, &cut, true)], &path, NOW + 360);
        assert_eq!(runs.len(), 3);
        assert_eq!(flaky(&runs, &[])[0].count, 1);
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
