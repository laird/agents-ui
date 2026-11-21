//! Flaky tests, as the merge gate records them (#538, laird/agents#191).
//!
//! When the gate runs a tree it has seen fail and a test that failed then now
//! passes, autocoder appends a `flaky` line to `merge-events.jsonl`
//! (`merge_events.rs` finds the file): `{ts, repo, issue, tree, test,
//! first_failure_excerpt}`. The village shows those sightings for this repo's
//! id from the last week ([`from_events`]). The file is shared by every repo
//! the user merges in, and by laird/agents' own test suite (laird/agents#282),
//! so only this repo's lines count. Once that file is in use, the village's own
//! history (`~/.agents-ui/flaky-<repo id>.json`, below) is deleted.
//!
//! **Fallback** (#501), for an autocoder too old to write the events file, or
//! a repo with no merge recorded in it yet: every autocoder that writes the
//! file writes the sightings too, so the file's absence is the test. The
//! village then works them out itself from each merge job's log (`gate.rs`),
//! which holds one or more gate runs, each starting "🧪 Gating the integrated
//! tree <sha>". A test that fails in one run and is followed by a passing run
//! of the **same tree** is a sighting. Those logs are overwritten when a merge
//! is relaunched, so each finished run is recorded as it is seen, in
//! `~/.agents-ui/flaky-<repo id>.json`, for a week. Recording is idempotent: a
//! run is keyed by its issue, its job's start time and its place in the log,
//! which also order the runs.
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FlakyRun {
    /// The issue whose merge was being gated.
    pub issue: u32,
    pub when: String,
    /// The gate's log, when the village found the failure there (the
    /// fallback); empty for a sighting the gate recorded.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub log: String,
    /// The start of the failure, as the gate recorded it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub excerpt: String,
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
    let by_test = by_test
        .into_iter()
        .map(|(test, fails)| {
            let fails = fails.iter().map(|r| (r.when, FlakyRun { issue: r.issue, when: rfc3339(r.when), log: r.log.clone(), ..Default::default() })).collect();
            (test, fails)
        })
        .collect();
    assemble(by_test, open)
}

fn rfc3339(s: i64) -> String {
    chrono::DateTime::from_timestamp(s, 0).map(|d| d.to_rfc3339()).unwrap_or_default()
}

/// Each test's failures (when, in Unix seconds, and what to show) as the
/// village's `flaky`: the test seen most recently first, each test's
/// failures newest first, with its open `Flaky: <test>` issue from `open`.
fn assemble(by_test: std::collections::BTreeMap<String, Vec<(i64, FlakyRun)>>, open: &[IssueSnapshot]) -> Vec<Flaky> {
    let mut out: Vec<(i64, Flaky)> = by_test
        .into_iter()
        .filter(|(_, fails)| !fails.is_empty())
        .map(|(test, mut fails)| {
            fails.sort_by_key(|(when, _)| std::cmp::Reverse(*when));
            let newest = fails[0].0;
            let title = issue_title(&test);
            let issue = open.iter().find(|i| i.state == "open" && i.title.trim() == title).map(|i| i.number);
            let count = fails.len() as u32;
            let runs = fails.into_iter().map(|(_, r)| r).collect();
            (newest, Flaky { test, count, last_seen: rfc3339(newest), runs, issue })
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.test.cmp(&b.1.test)));
    out.into_iter().map(|(_, f)| f).collect()
}

// ---- The gate's own sightings (#538, laird/agents#191) -------------------

/// What a sighting whose earlier failure named no test is shown as: the
/// gate as a whole failed, then passed (`flaky-report.sh` says the same).
pub const WHOLE_GATE: &str = "(whole gate)";

/// The longest failure excerpt kept: the start of a failure is what tells.
const EXCERPT_MAX: usize = 1500;

/// One line of `merge-events.jsonl`, the fields a sighting has.
#[derive(Debug, Default, Deserialize)]
struct Line {
    #[serde(default)]
    ts: String,
    #[serde(default)]
    event: String,
    #[serde(default)]
    repo: String,
    #[serde(default)]
    issue: Option<u32>,
    #[serde(default)]
    test: Option<String>,
    #[serde(default)]
    first_failure_excerpt: Option<String>,
}

/// The flaky tests the gate recorded for `repo_id` in the week before `now`,
/// from the text of `merge-events.jsonl`. `None` when no line is this repo's:
/// an autocoder too old to record them, or no merge here yet, so the caller
/// falls back to reading the gate's logs. A repo id is matched whole.
pub fn from_events(text: &str, repo_id: &str, now: i64, open: &[IssueSnapshot]) -> Option<Vec<Flaky>> {
    let mut ours = false;
    let mut by_test: std::collections::BTreeMap<String, Vec<(i64, FlakyRun)>> = Default::default();
    for line in text.lines().filter(|l| l.contains(repo_id)) {
        let Ok(e) = serde_json::from_str::<Line>(line) else { continue };
        if e.repo != repo_id {
            continue;
        }
        ours = true;
        if e.event != "flaky" {
            continue;
        }
        let Some(when) = chrono::DateTime::parse_from_rfc3339(&e.ts).ok().map(|d| d.timestamp()) else { continue };
        if now - when >= WINDOW_SECS || when > now + 300 {
            continue;
        }
        let test = e.test.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).unwrap_or_else(|| WHOLE_GATE.to_string());
        let mut excerpt: String = e.first_failure_excerpt.unwrap_or_default().trim_end().to_string();
        if excerpt.len() > EXCERPT_MAX {
            let mut cut = EXCERPT_MAX;
            while !excerpt.is_char_boundary(cut) {
                cut -= 1;
            }
            excerpt.truncate(cut);
            excerpt.push('…');
        }
        let run = FlakyRun { issue: e.issue.unwrap_or(0), when: rfc3339(when), excerpt, ..Default::default() };
        by_test.entry(test).or_default().push((when, run));
    }
    ours.then(|| assemble(by_test, open))
}

/// [`from_events`] on the file autocoder writes on this host. `None` without
/// the file, or without a line for this repo in it.
pub fn from_events_file(file: Option<&Path>, repo_id: &str, now: i64, open: &[IssueSnapshot]) -> Option<Vec<Flaky>> {
    from_events(&std::fs::read_to_string(file?).ok()?, repo_id, now, open)
}

fn events_file() -> Option<PathBuf> {
    super::merge_events::path(&|k| std::env::var(k).ok())
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
    let Some(id) = super::gate::local_repo_id(repo) else { return Vec::new() };
    read_for(&id, events_file().as_deref(), jobs, open, chrono::Utc::now().timestamp())
}

/// The gate's sightings for `id` when `events` has any line of this repo's
/// (deleting the village's own history, now unused), else the fallback.
fn read_for(id: &str, events: Option<&Path>, jobs: &[Job], open: &[IssueSnapshot], now: i64) -> Vec<Flaky> {
    // One at a time: every browser's poll lands here, and two that load, add
    // and save together would each drop what the other added.
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _held = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = store_path(id);
    if let Some(found) = from_events_file(events, id, now, open) {
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        return found;
    }
    let id = id.to_string();
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
    record(&mut runs, changed, &log_path, now);
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

/// Failures one line each: when, whose merge was gated, then the log the
/// village found it in, or the start of the failure as the gate recorded it.
fn failure_lines<'a>(runs: impl IntoIterator<Item = &'a FlakyRun>) -> String {
    runs.into_iter()
        .map(|r| {
            let mut line = failure_head(r);
            if !r.log.is_empty() {
                line.push_str(&format!("; log: `{}`", r.log));
            }
            if !r.excerpt.is_empty() {
                // Indented under its item; a fence the excerpt can't close.
                let body: String = r.excerpt.lines().map(|l| format!("  {}\n", l.replace("~~~~", "~ ~ ~ ~"))).collect();
                line.push_str(&format!("\n\n  ~~~~\n{body}  ~~~~\n"));
            }
            line
        })
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
         {source}\n\n\
         🤖 Filed from the village's ghost at the merge gate",
        test = f.test,
        n = times(f.count),
        lines = failure_lines(&f.runs),
        source = if f.runs.iter().any(|r| !r.log.is_empty()) {
            "The logs are the merge gate's (`/tmp/autocoder-merge-*.log`) and are overwritten when a merge is relaunched."
        } else {
            "The merge gate recorded these sightings (`merge-events.jsonl`; `flaky-report.sh` lists them)."
        },
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
            let now = chrono::Utc::now().timestamp();
            let found = from_events_file(events_file().as_deref(), &id, now, &[]).unwrap_or_else(|| {
                // Only this week's runs: the store is pruned when a village is
                // polled, which may not have happened since a run aged out.
                let runs: Vec<Run> = load(&store_path(&id)).into_iter().filter(|r| r.when >= now - WINDOW_SECS).collect();
                flaky(&runs, &[])
            });
            found.into_iter().find(|f| f.test == test)
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
                FlakyRun { issue: 480, when: "2026-11-15T10:00:00Z".into(), log: "/tmp/autocoder-merge-x-480.log".into(), ..Default::default() },
                FlakyRun { issue: 470, when: "2026-11-14T09:00:00Z".into(), log: "/tmp/autocoder-merge-x-470.log".into(), ..Default::default() },
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
        IssueSnapshot { number, title: title.into(), state: "open".into(), ..Default::default() }
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

    // ---- The gate's own sightings (#538) ----

    const REPO: &str = "agents-ui-33dc9185";
    fn ts(secs: i64) -> String {
        chrono::DateTime::from_timestamp(NOW + secs, 0).unwrap().format("%Y-%m-%dT%H:%M:%SZ").to_string()
    }
    /// Lines as autocoder 4.44 writes them, with another repo's, the test
    /// suite's and a repo whose id merely starts the same.
    fn events() -> String {
        [
            format!(r#"{{"ts":"{}","event":"attempt","outcome":"gate-failed","repo":"{REPO}","issue":9,"branch":"feature/issue-9","attempt":1,"escalated":false,"files":[],"gate_tests_failed":["web::a"]}}"#, ts(-50)),
            format!(r#"{{"ts":"{}","event":"flaky","outcome":"flaky","repo":"{REPO}","issue":9,"branch":"feature/issue-9","tree":"aaaa","test":"web::a","first_failure_excerpt":"thread 'web::a' panicked\nleft: 1"}}"#, ts(-40)),
            format!(r#"{{"ts":"{}","event":"flaky","outcome":"flaky","repo":"{REPO}","issue":12,"branch":"feature/issue-12","tree":"bbbb","test":"web::a","first_failure_excerpt":null}}"#, ts(-10)),
            format!(r#"{{"ts":"{}","event":"flaky","outcome":"flaky","repo":"{REPO}","issue":13,"branch":"feature/issue-13","tree":"cccc","test":null,"first_failure_excerpt":"FAIL"}}"#, ts(-20)),
            format!(r#"{{"ts":"{}","event":"flaky","outcome":"flaky","repo":"{REPO}","issue":3,"branch":"feature/issue-3","tree":"dddd","test":"web::old","first_failure_excerpt":""}}"#, ts(-WINDOW_SECS - 1)),
            format!(r#"{{"ts":"{}","event":"flaky","outcome":"flaky","repo":"agents-5ea0ca80","issue":9,"tree":"eeee","test":"test_x.sh"}}"#, ts(-5)),
            format!(r#"{{"ts":"{}","event":"flaky","outcome":"flaky","repo":"{REPO}0","issue":9,"tree":"ffff","test":"web::other"}}"#, ts(-5)),
            "not json".to_string(),
        ]
        .join("\n")
    }

    #[test]
    fn the_gates_sightings_for_this_repo_are_the_flaky_tests() {
        let f = from_events(&events(), REPO, NOW, &[open(341, "Flaky: web::a")]).expect("this repo is in the file");
        assert_eq!(f.iter().map(|f| f.test.as_str()).collect::<Vec<_>>(), vec!["web::a", WHOLE_GATE], "newest first; other repos and last week's dropped");
        assert_eq!(f[0].count, 2);
        assert_eq!(f[0].issue, Some(341));
        assert_eq!(f[0].last_seen, rfc3339(NOW - 10));
        assert_eq!(f[0].runs.iter().map(|r| r.issue).collect::<Vec<_>>(), vec![12, 9]);
        assert_eq!(f[0].runs[1].excerpt, "thread 'web::a' panicked\nleft: 1");
        assert!(f[0].runs.iter().all(|r| r.log.is_empty()), "no log: the gate recorded it");
        assert_eq!(f[1].issue, None);
    }

    #[test]
    fn no_line_for_this_repo_means_fall_back() {
        assert_eq!(from_events(&events(), "elsewhere-00000000", NOW, &[]), None);
        assert_eq!(from_events(&events(), "agents-ui-33dc918", NOW, &[]), None, "a repo id is matched whole");
        assert_eq!(from_events("", REPO, NOW, &[]), None);
        // This repo merges, but nothing flaked: no flaky tests, and no fallback.
        let quiet = events().lines().take(1).collect::<String>();
        assert_eq!(from_events(&quiet, REPO, NOW, &[]), Some(Vec::new()));
        assert_eq!(from_events_file(None, REPO, NOW, &[]), None);
        assert_eq!(from_events_file(Some(Path::new("/nonexistent/agents-ui/merge-events.jsonl")), REPO, NOW, &[]), None);
    }

    #[test]
    fn a_long_excerpt_is_cut_on_a_character() {
        let long = "é".repeat(EXCERPT_MAX);
        let line = format!(r#"{{"ts":"{}","event":"flaky","repo":"{REPO}","issue":1,"test":"t","first_failure_excerpt":"{long}"}}"#, ts(-1));
        let f = from_events(&line, REPO, NOW, &[]).unwrap();
        let ex = &f[0].runs[0].excerpt;
        assert!(ex.len() <= EXCERPT_MAX + '…'.len_utf8() && ex.ends_with('…'), "{}", ex.len());
    }

    #[test]
    fn a_sighting_files_with_its_excerpt_not_a_log() {
        let f = from_events(&events(), REPO, NOW, &[]).unwrap().remove(0);
        let body = report_body(&f);
        assert!(body.contains(&format!("- {} — while gating #9's merge\n\n  ~~~~\n  thread 'web::a' panicked\n  left: 1\n  ~~~~", rfc3339(NOW - 40))), "{body}");
        assert!(!body.contains("log: `"), "{body}");
        assert!(body.contains("merge-events.jsonl") && !body.contains("/tmp/autocoder-merge"), "{body}");
        // Filed once, nothing more to add.
        let filed = serde_json::to_vec(&serde_json::json!({ "body": body, "comments": [] })).unwrap();
        assert!(unreported(&f, &filed).is_empty());
    }

    /// The events file in use: its sightings, and the old history deleted.
    /// Without it: the logs, kept in the history as before.
    #[test]
    fn the_events_file_replaces_the_local_history_and_its_absence_keeps_it() {
        let id = format!("flaky-read-{}", std::process::id());
        let dir = std::env::temp_dir().join(format!("agents-ui-flaky-events-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("merge-events.jsonl");
        let jobs = [job(9, 100, FAIL_A, true), job(10, 150, PASS_A, true)];

        // No file: the fallback reads the logs and keeps them.
        let old = read_for(&id, Some(&file), &jobs, &[], NOW + 200);
        assert_eq!(old.len(), 1);
        assert!(old[0].runs[0].log.ends_with(&format!("/autocoder-merge-{id}-9.log")), "{}", old[0].runs[0].log);
        assert!(store_path(&id).exists(), "the fallback keeps its history");

        // A file with no line of this repo's: still the fallback.
        std::fs::write(&file, events()).unwrap();
        assert_eq!(read_for(&id, Some(&file), &jobs, &[], NOW + 200).len(), 1);
        assert!(store_path(&id).exists());

        // The gate records this repo: its sightings, and the history is gone.
        std::fs::write(&file, events().replace(REPO, &id)).unwrap();
        let new = read_for(&id, Some(&file), &jobs, &[], NOW);
        assert_eq!(new.iter().map(|f| f.test.as_str()).collect::<Vec<_>>(), vec!["web::a", WHOLE_GATE]);
        assert!(!store_path(&id).exists(), "the local history is deleted once unused");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
