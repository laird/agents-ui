//! The integration branch's build health: green, red, running or unknown.
//!
//! Read from, in order (the first that knows wins):
//!
//! 1. **CI**: GitHub check runs for the branch tip, per job, so "macOS only"
//!    can be told apart from "everything". When it's red, the branch's
//!    workflow runs say since when and which commit turned it red, and the
//!    failed jobs' logs name the failing tests.
//! 2. **The sentinel** (`main-sentinel.sh`): its state file's `red_issue`,
//!    `last_green` and `last_tested`.
//! 3. **The merge gate**: a merge job whose tests failed on the integration
//!    branch alone ("fails alone too"), not just with its feature merged.
//!
//! An outside framework reports `build` itself (`docs/village-protocol.md`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::transport::ServerTransport;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildState {
    Green,
    Red,
    Running,
    #[default]
    Unknown,
}

/// One CI job that isn't green: failed, or still running.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BuildJob {
    pub name: String,
    /// `Linux`, `macOS` or `Windows`, read from the job's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    pub state: BuildState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The tests this job reports failing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests: Vec<String>,
}

/// The build health of a village's integration branch.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Build {
    pub state: BuildState,
    /// The commit it describes: the branch tip.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sha: String,
    /// Red since (RFC 3339): when the commit that turned it red was built.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// The commit that turned it red.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blame: Option<String>,
    /// The run (or job) to open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The jobs that aren't green.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub jobs: Vec<BuildJob>,
    /// Every failing test, across jobs, once each.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests: Vec<String>,
    /// Red, but some jobs passed: one OS or one job, not everything.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub partial: bool,
    /// Who said so: `ci`, `sentinel`, `gate`, or an outside report's own word.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
}

// ---------------------------------------------------------------------------
// CI: check runs and workflow runs
// ---------------------------------------------------------------------------

/// One entry of `GET /repos/{owner}/{repo}/commits/{sha}/check-runs`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct CheckRun {
    pub id: u64,
    pub name: String,
    /// `queued`, `in_progress` or `completed`.
    pub status: String,
    /// Once completed: `success`, `failure`, `neutral`, `cancelled`,
    /// `skipped`, `timed_out`, `action_required`, `startup_failure`, `stale`.
    #[serde(default)]
    pub conclusion: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
}

pub fn parse_check_runs(json: &str) -> Vec<CheckRun> {
    serde_json::from_str::<Value>(json)
        .ok()
        .and_then(|v| serde_json::from_value(v.get("check_runs")?.clone()).ok())
        .unwrap_or_default()
}

/// A completed run's conclusion that means the build is broken.
fn failed(conclusion: Option<&str>) -> bool {
    matches!(conclusion, Some("failure" | "timed_out" | "startup_failure" | "action_required"))
}

/// A completed run's conclusion that means it passed (or had nothing to do).
fn passed(conclusion: Option<&str>) -> bool {
    matches!(conclusion, Some("success" | "neutral" | "skipped"))
}

/// `test (macos-latest)` → `macOS`.
pub fn job_os(name: &str) -> Option<String> {
    let n = name.to_ascii_lowercase();
    let os = if n.contains("macos") || n.contains("darwin") {
        "macOS"
    } else if n.contains("windows") {
        "Windows"
    } else if n.contains("ubuntu") || n.contains("linux") {
        "Linux"
    } else {
        return None;
    };
    Some(os.to_string())
}

/// The build at `sha` from its check runs. Red when any job failed (even
/// with others still running), running while any job hasn't finished,
/// green when all passed, unknown with no runs at all.
pub fn from_check_runs(sha: &str, runs: &[CheckRun]) -> Build {
    if runs.is_empty() {
        return Build { sha: sha.to_string(), source: "ci".into(), ..Build::default() };
    }
    let job = |r: &CheckRun, state| BuildJob { name: r.name.clone(), os: job_os(&r.name), state, url: r.html_url.clone(), tests: Vec::new() };
    let red: Vec<BuildJob> = runs
        .iter()
        .filter(|r| r.status == "completed" && failed(r.conclusion.as_deref()))
        .map(|r| job(r, BuildState::Red))
        .collect();
    let running: Vec<BuildJob> = runs.iter().filter(|r| r.status != "completed").map(|r| job(r, BuildState::Running)).collect();
    let green = runs.iter().filter(|r| r.status == "completed" && passed(r.conclusion.as_deref())).count();
    let state = if !red.is_empty() {
        BuildState::Red
    } else if !running.is_empty() {
        BuildState::Running
    } else {
        BuildState::Green
    };
    let partial = state == BuildState::Red && green > 0;
    let url = red.first().or(running.first()).and_then(|j| j.url.clone()).map(|u| run_url(&u));
    Build {
        state,
        sha: sha.to_string(),
        url,
        jobs: red.into_iter().chain(running).collect(),
        partial,
        source: "ci".into(),
        ..Build::default()
    }
}

/// A job's page → its run's page: `…/actions/runs/42/job/7` → `…/actions/runs/42`.
fn run_url(job_url: &str) -> String {
    match job_url.find("/job/") {
        Some(i) => job_url[..i].to_string(),
        None => job_url.to_string(),
    }
}

/// One entry of `GET /repos/{owner}/{repo}/actions/runs?branch=…`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct WorkflowRun {
    pub head_sha: String,
    pub status: String,
    #[serde(default)]
    pub conclusion: Option<String>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub html_url: Option<String>,
}

pub fn parse_workflow_runs(json: &str) -> Vec<WorkflowRun> {
    serde_json::from_str::<Value>(json)
        .ok()
        .and_then(|v| serde_json::from_value(v.get("workflow_runs")?.clone()).ok())
        .unwrap_or_default()
}

/// When the branch turned red, and the commit that did it, from its workflow
/// runs (newest first). Commits are walked back from the newest while they
/// are red; the oldest of that streak is the blame, its earliest run the
/// since. Commits whose runs haven't finished are skipped. None when the
/// newest finished commit isn't red.
pub fn red_since(runs: &[WorkflowRun]) -> Option<(String, String)> {
    let mut order: Vec<&str> = Vec::new();
    let mut by_sha: HashMap<&str, Vec<&WorkflowRun>> = HashMap::new();
    for r in runs {
        if !by_sha.contains_key(r.head_sha.as_str()) {
            order.push(&r.head_sha);
        }
        by_sha.entry(&r.head_sha).or_default().push(r);
    }
    let mut streak: Option<(String, String)> = None;
    for sha in order {
        let done: Vec<&&WorkflowRun> = by_sha[sha].iter().filter(|r| r.status == "completed").collect();
        if done.is_empty() {
            continue;
        }
        if !done.iter().any(|r| failed(r.conclusion.as_deref())) {
            break;
        }
        let first = done.iter().map(|r| r.created_at.as_str()).min().unwrap_or_default();
        streak = Some((first.to_string(), sha.to_string()));
    }
    streak
}

/// The tests a `cargo test` log reports failing: `test a::b ... FAILED`.
/// GitHub prefixes each log line with a timestamp, which is skipped.
pub fn failing_tests(log: &str) -> Vec<String> {
    let mut tests: Vec<String> = Vec::new();
    for line in log.lines() {
        let Some(rest) = line.trim_end().strip_suffix(" ... FAILED") else { continue };
        let Some(i) = rest.find("test ") else { continue };
        let name = rest[i + "test ".len()..].trim().to_string();
        if !name.is_empty() && !tests.contains(&name) {
            tests.push(name);
        }
    }
    tests
}

// ---------------------------------------------------------------------------
// The sentinel and the merge gate
// ---------------------------------------------------------------------------

/// The build from `main-sentinel.sh`'s state file (`key=value` lines) for
/// the branch at `tip`: red while it tracks a red streak (`red_issue`),
/// green when it last passed this tip, otherwise unknown.
pub fn from_sentinel(state: &str, tip: &str) -> Build {
    let fields: HashMap<&str, &str> = state.lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.trim(), v.trim())).collect();
    let get = |k: &str| fields.get(k).copied().filter(|v| !v.is_empty());
    let state = if get("red_issue").is_some() {
        BuildState::Red
    } else if !tip.is_empty() && get("last_green") == Some(tip) {
        BuildState::Green
    } else {
        BuildState::Unknown
    };
    Build { state, sha: tip.to_string(), source: "sentinel".into(), ..Build::default() }
}

/// The sentinel's state file for `repo_id` (as `gate::repo_id` makes it) and
/// `integration`.
pub fn sentinel_state_file(repo_id: &str, integration: &str) -> Option<PathBuf> {
    let dir = match std::env::var("AUTOCODER_SENTINEL_STATE_DIR").ok().filter(|s| !s.is_empty()) {
        Some(d) => PathBuf::from(d),
        None => {
            let state = std::env::var("XDG_STATE_HOME")
                .ok()
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .or_else(|| dirs::home_dir().map(|h| h.join(".local/state")))?;
            state.join("autocoder/sentinel")
        }
    };
    Some(dir.join(format!("{repo_id}-{integration}")))
}

/// Red when the newest finished merge job failed on the integration branch
/// alone: its tests fail without the feature, so the branch itself is broken.
/// None when the gate says nothing about the branch.
pub fn from_gate(reading: &super::gate::GateReading) -> Option<Build> {
    let newest = reading.jobs.iter().filter(|j| !j.alive && j.exit.is_some()).max_by_key(|j| j.started)?;
    super::gate::red_base(&newest.log).then(|| Build { state: BuildState::Red, source: "gate".into(), ..Build::default() })
}

/// The first source that knows, in the order given; unknown when none does.
pub fn combine(sources: impl IntoIterator<Item = Option<Build>>) -> Build {
    sources
        .into_iter()
        .flatten()
        .find(|b| b.state != BuildState::Unknown)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Reading it
// ---------------------------------------------------------------------------

async fn gh_api(transport: &ServerTransport, repo: &Path, path: &str) -> Option<String> {
    let args = vec!["api".to_string(), "--allow-escape-sequences".to_string(), path.to_string()];
    let out = transport.output("gh", &args, Some(repo)).await.ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The build at `tip` of `integration`, from GitHub. None when GitHub can't
/// be asked. A red build also gets since/blame from the branch's workflow
/// runs and the failing tests from its failed jobs' logs.
pub async fn read_ci(transport: &ServerTransport, repo: &Path, integration: &str, tip: &str) -> Option<Build> {
    let runs = parse_check_runs(&gh_api(transport, repo, &format!("repos/{{owner}}/{{repo}}/commits/{tip}/check-runs?per_page=100")).await?);
    let mut build = from_check_runs(tip, &runs);
    if build.state != BuildState::Red {
        return Some(build);
    }
    let path = format!("repos/{{owner}}/{{repo}}/actions/runs?branch={integration}&per_page=30");
    if let Some(json) = gh_api(transport, repo, &path).await
        && let Some((since, blame)) = red_since(&parse_workflow_runs(&json))
    {
        build.since = Some(since);
        build.blame = Some(blame);
    }
    for job in build.jobs.iter_mut().filter(|j| j.state == BuildState::Red) {
        let Some(id) = job.url.as_deref().and_then(|u| u.rsplit("/job/").next()).and_then(|id| id.parse::<u64>().ok()) else {
            continue;
        };
        if let Some(log) = gh_api(transport, repo, &format!("repos/{{owner}}/{{repo}}/actions/jobs/{id}/logs")).await {
            job.tests = failing_tests(&log);
        }
    }
    let mut tests: Vec<String> = Vec::new();
    for t in build.jobs.iter().flat_map(|j| &j.tests) {
        if !tests.contains(t) {
            tests.push(t.clone());
        }
    }
    build.tests = tests;
    Some(build)
}

/// How long a finished (green or red) answer for a tip is kept before CI is
/// asked again (a job can be re-run); a running or unknown one is re-asked
/// sooner.
const SETTLED: Duration = Duration::from_secs(300);
const UNSETTLED: Duration = Duration::from_secs(30);

/// CI answers per repo, kept until the tip moves or they go stale, so the
/// village's poll doesn't ask GitHub every few seconds.
#[derive(Default)]
pub struct CiCache {
    entries: Mutex<HashMap<PathBuf, (String, Instant, Option<Build>)>>,
}

impl CiCache {
    pub async fn get(&self, transport: &ServerTransport, repo: &Path, integration: &str, tip: &str) -> Option<Build> {
        if let Ok(entries) = self.entries.lock()
            && let Some((sha, at, build)) = entries.get(repo)
        {
            let settled = build.as_ref().is_some_and(|b| matches!(b.state, BuildState::Green | BuildState::Red));
            if sha == tip && at.elapsed() < if settled { SETTLED } else { UNSETTLED } {
                return build.clone();
            }
        }
        let build = read_ci(transport, repo, integration, tip).await;
        if let Ok(mut entries) = self.entries.lock() {
            entries.insert(repo.to_path_buf(), (tip.to_string(), Instant::now(), build.clone()));
        }
        build
    }
}

/// The build of a local swarm's integration branch: CI, then the sentinel,
/// then the merge gate. `gate` is what the merge gate's files already said.
pub async fn read(cache: &CiCache, transport: &ServerTransport, repo: &Path, integration: &str, gate: &super::gate::GateReading) -> Build {
    if integration.is_empty() {
        return Build::default();
    }
    let rev = ["rev-parse".to_string(), format!("origin/{integration}")];
    let tip = match transport.output("git", &rev, Some(repo)).await {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => String::new(),
    };
    let ci = if tip.is_empty() { None } else { cache.get(transport, repo, integration, &tip).await };
    let sentinel = if transport.server().is_none() {
        let repo = repo.to_path_buf();
        let (integration, tip) = (integration.to_string(), tip.clone());
        tokio::task::spawn_blocking(move || {
            let id = super::gate::local_repo_id(&repo)?;
            let text = std::fs::read_to_string(sentinel_state_file(&id, &integration)?).ok()?;
            Some(from_sentinel(&text, &tip))
        })
        .await
        .ok()
        .flatten()
    } else {
        None
    };
    let mut build = combine([ci, sentinel, from_gate(gate)]);
    if build.sha.is_empty() {
        build.sha = tip;
    }
    build
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recorded from this repo's CI: `test (ubuntu-latest)` and
    /// `test (macos-latest)`, trimmed to the fields read.
    fn runs(ubuntu: (&str, Option<&str>), macos: (&str, Option<&str>)) -> String {
        let run = |id: u64, os: &str, (status, conclusion): (&str, Option<&str>)| {
            serde_json::json!({
                "id": id, "name": format!("test ({os}-latest)"), "status": status, "conclusion": conclusion,
                "html_url": format!("https://github.com/laird/agents-ui/actions/runs/42738481866/job/{id}"),
                "app": {"slug": "github-actions"}
            })
        };
        serde_json::json!({"total_count": 2, "check_runs": [run(129573438761, "ubuntu", ubuntu), run(129573438336, "macos", macos)]})
            .to_string()
    }

    const DONE_OK: (&str, Option<&str>) = ("completed", Some("success"));
    const DONE_FAIL: (&str, Option<&str>) = ("completed", Some("failure"));
    const RUNNING: (&str, Option<&str>) = ("in_progress", None);

    fn ci(json: &str) -> Build {
        from_check_runs("e4c77f6c", &parse_check_runs(json))
    }

    #[test]
    fn green_when_every_job_passed() {
        let b = ci(&runs(DONE_OK, DONE_OK));
        assert_eq!((b.state, b.partial, b.jobs.len()), (BuildState::Green, false, 0));
        assert_eq!(b.source, "ci");
    }

    #[test]
    fn red_on_every_os_is_not_partial() {
        let b = ci(&runs(DONE_FAIL, DONE_FAIL));
        assert_eq!(b.state, BuildState::Red);
        assert!(!b.partial);
        let os: Vec<Option<&str>> = b.jobs.iter().map(|j| j.os.as_deref()).collect();
        assert_eq!(os, vec![Some("Linux"), Some("macOS")]);
        assert_eq!(b.url.as_deref(), Some("https://github.com/laird/agents-ui/actions/runs/42738481866"));
    }

    #[test]
    fn red_on_one_os_is_partial() {
        let b = ci(&runs(DONE_OK, DONE_FAIL));
        assert_eq!((b.state, b.partial), (BuildState::Red, true));
        assert_eq!(b.jobs.len(), 1);
        assert_eq!(b.jobs[0].os.as_deref(), Some("macOS"));
    }

    #[test]
    fn running_until_every_job_finishes_unless_one_already_failed() {
        let b = ci(&runs(RUNNING, DONE_OK));
        assert_eq!(b.state, BuildState::Running);
        assert_eq!(b.jobs[0].state, BuildState::Running);
        // A failure is already a red build, whatever is still running.
        let b = ci(&runs(RUNNING, DONE_FAIL));
        assert_eq!(b.state, BuildState::Red);
        assert_eq!(b.jobs.iter().map(|j| j.state).collect::<Vec<_>>(), vec![BuildState::Red, BuildState::Running]);
    }

    #[test]
    fn unknown_with_no_check_runs() {
        assert_eq!(ci(r#"{"total_count":0,"check_runs":[]}"#).state, BuildState::Unknown);
        assert_eq!(ci("not json").state, BuildState::Unknown);
    }

    #[test]
    fn job_oses() {
        assert_eq!(job_os("test (windows-2022)").as_deref(), Some("Windows"));
        assert_eq!(job_os("build-linux").as_deref(), Some("Linux"));
        assert_eq!(job_os("lint"), None);
    }

    fn wf(sha: &str, status: &str, conclusion: Option<&str>, at: &str) -> serde_json::Value {
        serde_json::json!({"head_sha": sha, "status": status, "conclusion": conclusion, "created_at": at, "html_url": "https://x/runs/1"})
    }

    #[test]
    fn red_since_the_oldest_commit_of_the_red_streak() {
        // Recorded shape: newest first, the newest still running.
        let json = serde_json::json!({"workflow_runs": [
            wf("912a9be3", "in_progress", None, "2026-11-10T01:53:18Z"),
            wf("e4c77f6c", "completed", Some("failure"), "2026-11-10T01:49:26Z"),
            wf("fe17fed2", "completed", Some("failure"), "2026-11-10T01:45:57Z"),
            wf("6c5e44c7", "completed", Some("failure"), "2026-11-10T01:23:07Z"),
            wf("33aa8cb0", "completed", Some("success"), "2026-11-10T00:50:00Z"),
            wf("11111111", "completed", Some("failure"), "2026-11-09T20:00:00Z"),
        ]})
        .to_string();
        let runs = parse_workflow_runs(&json);
        assert_eq!(red_since(&runs), Some(("2026-11-10T01:23:07Z".to_string(), "6c5e44c7".to_string())));
        // Green at the newest finished commit: not red since anything.
        assert_eq!(red_since(&runs[4..]), None);
        assert_eq!(red_since(&[]), None);
    }

    #[test]
    fn failing_tests_from_a_job_log() {
        let log = "2026-11-10T01:51:12.0285866Z test web::server::tests::screensaver_install_and_page_tests_pass ... FAILED\n\
2026-11-10T01:51:12.1Z test web::gate::tests::slots ... ok\n\
2026-11-10T01:51:44.3Z failures:\n\
2026-11-10T01:51:44.4Z test web::server::tests::screensaver_install_and_page_tests_pass ... FAILED\n\
test plain::one ... FAILED\n";
        assert_eq!(
            failing_tests(log),
            vec!["web::server::tests::screensaver_install_and_page_tests_pass".to_string(), "plain::one".to_string()]
        );
    }

    #[test]
    fn the_sentinel_says_red_green_or_nothing() {
        let red = "last_tested=bbb\nlast_green=aaa\nred_issue=408\npending_revert=\n";
        assert_eq!(from_sentinel(red, "bbb").state, BuildState::Red);
        let green = "last_tested=bbb\nlast_green=bbb\nred_issue=\n";
        assert_eq!(from_sentinel(green, "bbb").state, BuildState::Green);
        // Not tested at this tip yet.
        assert_eq!(from_sentinel(green, "ccc").state, BuildState::Unknown);
        assert_eq!(from_sentinel("", "ccc").source, "sentinel");
    }

    #[test]
    fn sentinel_state_file_follows_the_sentinel_s_own_rule() {
        // SAFETY: tests in this module are the only readers of this variable.
        unsafe { std::env::set_var("AUTOCODER_SENTINEL_STATE_DIR", "/tmp/sentinel-state") };
        assert_eq!(sentinel_state_file("agents-ui-33dc9185", "master"), Some(PathBuf::from("/tmp/sentinel-state/agents-ui-33dc9185-master")));
        unsafe { std::env::remove_var("AUTOCODER_SENTINEL_STATE_DIR") };
    }

    fn job(started: u64, exit: i32, log: &str) -> super::super::gate::Job {
        super::super::gate::Job {
            issue: 1,
            alive: false,
            exit: Some(exit),
            log: log.to_string(),
            started: Some(std::time::UNIX_EPOCH + Duration::from_secs(started)),
            ..Default::default()
        }
    }

    #[test]
    fn the_gate_says_red_when_its_newest_job_failed_on_the_branch_alone() {
        use super::super::gate::GateReading;
        let red_alone = "❌ Tests fail on the integrated tree\n… and master fails alone too\n";
        let reading = GateReading { local: true, jobs: vec![job(10, 0, "ok"), job(20, 1, red_alone)], ..Default::default() };
        assert_eq!(from_gate(&reading).map(|b| (b.state, b.source)), Some((BuildState::Red, "gate".to_string())));
        // A newer job that landed cleanly: the gate no longer says red.
        let reading = GateReading { local: true, jobs: vec![job(20, 1, red_alone), job(30, 0, "landed")], ..Default::default() };
        assert_eq!(from_gate(&reading), None);
        assert_eq!(from_gate(&GateReading::default()), None);
    }

    #[test]
    fn the_first_source_that_knows_wins() {
        let b = |state, source: &str| Some(Build { state, source: source.into(), ..Build::default() });
        assert_eq!(combine([b(BuildState::Unknown, "ci"), b(BuildState::Red, "sentinel"), b(BuildState::Green, "gate")]).source, "sentinel");
        assert_eq!(combine([None, None, b(BuildState::Red, "gate")]).state, BuildState::Red);
        assert_eq!(combine([None, None, None]), Build::default());
    }

    #[test]
    fn serialises_compactly() {
        let green = serde_json::to_value(Build { state: BuildState::Green, sha: "abc".into(), source: "ci".into(), ..Build::default() }).unwrap();
        assert_eq!(green, serde_json::json!({"state": "green", "sha": "abc", "source": "ci"}));
        let red: Build = serde_json::from_value(serde_json::json!({"state": "red", "partial": true, "jobs": [{"name": "test (macos)", "state": "red"}]})).unwrap();
        assert!(red.partial);
        assert_eq!(red.jobs[0].state, BuildState::Red);
    }
}
