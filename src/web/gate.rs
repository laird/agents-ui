//! The merge gate: where autocoder's detached merge jobs are.
//!
//! A worker hands a finished branch to `merge-launch.sh`, which runs
//! `merge-to-integration.sh` detached. The job merges onto the integration
//! branch in a throwaway worktree, waits for one of the host's gate slots,
//! runs the regression suite, and then closes the issue, parks it under
//! `awaiting-integration`, or hands it back (a conflict, a failed gate).
//!
//! The job leaves files on the host that runs it, keyed by autocoder's repo
//! id (`<basename>-<8 hex of sha1(git common dir)>`):
//!
//! - `${AUTOCODER_TMP:-/tmp}/autocoder-merge-<repo-id>-<issue>.pid`: the job's pid;
//! - `.log`: what it is doing, phase by phase, and the gate's output;
//! - `.exit`: its exit code once it is done (0 landed, 1 conflict or
//!   setup failure, 2 the gate failed on the integrated tree);
//! - `.run.sh`: the command it runs, which names the branch (a merge train
//!   is a `train/<UTC>` branch);
//! - `autocoder-gate-slot-<k>.lock`: flock-held by whichever job is in slot k.
//!
//! Everything here reads those files. It only works on the host serving the
//! village; a remote host reports nothing rather than guess.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use super::IssueSnapshot;

/// Where a task is in the merge gate.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GateState {
    /// Handed to a merge job that is waiting for a gate slot.
    #[default]
    Queued,
    /// Merging onto the integration branch, re-syncing, or publishing.
    Merging,
    /// The regression suite is running on the integrated tree.
    Gating,
    /// Landed, tested, and closed.
    Passed,
    /// Landed on another basis, waiting for the sentinel to prove a green tip.
    LandedUnproven,
    /// The gate failed on the integrated tree.
    Failed,
    /// The merge onto the integration branch conflicted.
    Conflict,
    /// The integration branch is red without this change.
    RedBase,
}

impl GateState {
    /// Still in the gate: not landed, not handed back.
    pub fn in_flight(self) -> bool {
        matches!(self, GateState::Queued | GateState::Merging | GateState::Gating)
    }
    /// The gate handed the work back without landing it.
    pub fn handed_back(self) -> bool {
        matches!(self, GateState::Failed | GateState::Conflict)
    }
}

/// One task's trip through the merge gate.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TaskGate {
    pub state: GateState,
    /// Whether the work is on the integration branch.
    #[serde(default)]
    pub landed: bool,
    /// The gate slot the job holds while gating.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<u32>,
    /// When the merge job started (RFC 3339).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    /// Seconds since then, while it runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_secs: Option<u64>,
    /// Tests run so far in this gate run, from the test runner's summaries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tests_run: Option<u64>,
    /// A few words on how far the gate has got, e.g. `412 tests run (3 suites)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<String>,
    /// Gate runs in this job: 2 once a re-sync made it gate again.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub attempt: u32,
    /// Where the job wrote what went wrong: the task's issue, which gets its comment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_url: Option<String>,
    /// The merge train this task rides in, by id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub train: Option<String>,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// Several branches gated together as one tree (`train/<UTC>`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Train {
    pub id: String,
    /// The task the train's merge job runs under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub state: GateState,
}

/// The host's merge gate: its slots, which are busy, and the trains in it.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MergeGate {
    /// How many gates may run at once on this host.
    #[serde(default)]
    pub slots: u32,
    /// The slots in use, by number (1-based).
    #[serde(default)]
    pub busy: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trains: Vec<Train>,
}

// ---------------------------------------------------------------------------
// One job, from its files
// ---------------------------------------------------------------------------

/// What a merge job left on the host.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Job {
    pub issue: u32,
    /// The job's process is still running.
    pub alive: bool,
    /// Its exit code, once it is done.
    pub exit: Option<i32>,
    pub log: String,
    /// The branch it lands, from `.run.sh`.
    pub feature: Option<String>,
    /// When it started (its pidfile's time).
    pub started: Option<SystemTime>,
    /// The gate slot it holds right now.
    pub slot: Option<u32>,
    /// For a train, the issues of the branches merged onto it.
    pub members: Vec<u32>,
}

/// The phase of a running job, from the last line in its log that names one.
pub fn running_phase(log: &str) -> GateState {
    let mut phase = if log.trim().is_empty() { GateState::Queued } else { GateState::Merging };
    for line in log.lines() {
        let line = line.trim_start();
        if line.starts_with("⏳ All ") && line.contains("waiting for slot") {
            phase = GateState::Queued;
        } else if line.starts_with("▶️") && line.contains("Gate slot") {
            phase = GateState::Gating;
        } else if line.starts_with("🧪 ") || line.starts_with("❌ Tests fail on the integrated tree") {
            // The base is gated alone after a failure: still the gate.
            phase = GateState::Gating;
        } else if (line.starts_with("⚠️") && line.contains("rejected") && line.contains("re-syncing"))
            || line.starts_with("✅ Issue #")
            || line.contains("pushing the already-gated work")
            || line.contains("pushing the gated work")
        {
            phase = GateState::Merging;
        }
    }
    phase
}

/// Gate runs in a log: one per "Gating the integrated tree".
pub fn gate_runs(log: &str) -> u32 {
    log.lines().filter(|l| l.trim_start().starts_with("🧪 Gating the integrated tree")).count() as u32
}

/// Tests run and suites finished in the latest gate run, from the test
/// runner's summaries (`test result: ok. 12 passed; 0 failed; ...`).
pub fn gate_progress(log: &str) -> Option<(u64, u32)> {
    let start = log.rfind("🧪 ").unwrap_or(0);
    let mut tests = 0u64;
    let mut suites = 0u32;
    for line in log[start..].lines() {
        let Some(rest) = line.split("test result:").nth(1) else { continue };
        let words: Vec<&str> = rest.split_whitespace().collect();
        let count = |what: &str| {
            words.windows(2).find(|w| w[1].trim_end_matches([';', ',']) == what).and_then(|w| w[0].parse::<u64>().ok()).unwrap_or(0)
        };
        tests += count("passed") + count("failed");
        suites += 1;
    }
    (suites > 0).then_some((tests, suites))
}

/// The log says the integration branch fails without this change.
fn red_base(log: &str) -> bool {
    log.contains("fails alone too")
}

/// Where a job's issue is in the gate, from its files and the issue's labels.
/// `None` when nothing says: no job and no label that implies one, or a job
/// that died without an exit code.
pub fn job_gate(job: Option<&Job>, labels: &[String], url: Option<&str>, now: SystemTime) -> Option<TaskGate> {
    let parked = labels.iter().any(|l| l == "awaiting-integration");
    let Some(job) = job else {
        return parked.then(|| TaskGate { state: GateState::LandedUnproven, landed: true, ..TaskGate::default() });
    };
    let (state, landed) = if job.alive {
        (running_phase(&job.log), false)
    } else {
        match job.exit? {
            0 if red_base(&job.log) => (GateState::RedBase, true),
            0 if parked => (GateState::LandedUnproven, true),
            0 => (GateState::Passed, true),
            1 if job.log.contains("❌ Merge conflict") => (GateState::Conflict, false),
            _ if red_base(&job.log) => (GateState::RedBase, false),
            _ => (GateState::Failed, false),
        }
    };
    let progress = (state == GateState::Gating).then(|| gate_progress(&job.log)).flatten();
    Some(TaskGate {
        state,
        landed,
        slot: if job.alive { job.slot } else { None },
        started_at: job.started.map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
        elapsed_secs: if job.alive { job.started.and_then(|t| now.duration_since(t).ok()).map(|d| d.as_secs()) } else { None },
        tests_run: progress.map(|(n, _)| n),
        progress: progress.map(|(n, s)| format!("{n} tests run ({s} suite{})", if s == 1 { "" } else { "s" })),
        attempt: gate_runs(&job.log),
        failure_url: (state.handed_back() || state == GateState::RedBase).then(|| url.map(str::to_string)).flatten(),
        train: job.feature.as_deref().and_then(|f| f.strip_prefix("train/")).map(str::to_string),
    })
}

/// Every task's gate, the host's slots, and its trains. Train members ride
/// with their train: they take its state unless their own job says otherwise.
pub fn assess(reading: &GateReading, issues: &[IssueSnapshot], now: SystemTime) -> (HashMap<u32, TaskGate>, Option<MergeGate>) {
    if !reading.local {
        return (HashMap::new(), None);
    }
    let jobs: HashMap<u32, &Job> = reading.jobs.iter().map(|j| (j.issue, j)).collect();
    let mut gates = HashMap::new();
    for i in issues.iter().filter(|i| i.state == "open") {
        let url = Some(i.url.as_str()).filter(|u| !u.is_empty());
        if let Some(g) = job_gate(jobs.get(&i.number).copied(), &i.labels, url, now) {
            gates.insert(i.number, g);
        }
    }
    let mut trains = Vec::new();
    for job in &reading.jobs {
        let Some(id) = job.feature.as_deref().and_then(|f| f.strip_prefix("train/")) else { continue };
        let Some(g) = job_gate(Some(job), &[], None, now) else { continue };
        // A finished train is history once its members are no longer open.
        if !g.state.in_flight() && !job.members.iter().any(|m| gates.contains_key(m) || issues.iter().any(|i| i.number == *m && i.state == "open")) {
            continue;
        }
        for m in &job.members {
            gates.entry(*m).or_insert_with(|| TaskGate { train: Some(id.to_string()), ..g.clone() });
        }
        trains.push(Train {
            id: id.to_string(),
            task: Some(job.issue.to_string()),
            members: job.members.iter().map(u32::to_string).collect(),
            state: g.state,
        });
    }
    (gates, Some(MergeGate { slots: reading.slots, busy: reading.busy.clone(), trains }))
}

// ---------------------------------------------------------------------------
// Gate slots: who holds which lock
// ---------------------------------------------------------------------------

/// The parent pid in a `/proc/<pid>/stat` line (the field after the state,
/// which follows the command's closing parenthesis).
pub fn parent_pid(stat: &str) -> Option<u32> {
    let rest = &stat[stat.rfind(')')? + 1..];
    rest.split_whitespace().nth(1)?.parse().ok()
}

/// The pids holding each inode's lock, from `/proc/locks`. Waiters (`->`
/// lines) are queued for the lock, not holding it.
pub fn lock_holders(proc_locks: &str) -> HashMap<u64, Vec<u32>> {
    let mut held: HashMap<u64, Vec<u32>> = HashMap::new();
    for line in proc_locks.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.get(1) == Some(&"->") || fields.len() < 6 {
            continue;
        }
        let (Some(pid), Some(inode)) = (fields[4].parse::<u32>().ok(), fields[5].rsplit(':').next().and_then(|i| i.parse::<u64>().ok())) else {
            continue;
        };
        held.entry(inode).or_default().push(pid);
    }
    held
}

/// Busy slots, and the slot each job holds: a slot belongs to the job whose
/// pid is the lock holder or one of its ancestors.
pub fn slot_owners(slot_inodes: &[(u32, u64)], holders: &HashMap<u64, Vec<u32>>, parent: &dyn Fn(u32) -> Option<u32>, job_pids: &[(u32, u32)]) -> (Vec<u32>, HashMap<u32, u32>) {
    let jobs: HashMap<u32, u32> = job_pids.iter().map(|(issue, pid)| (*pid, *issue)).collect();
    let mut busy = Vec::new();
    let mut owner = HashMap::new();
    for (slot, inode) in slot_inodes {
        let Some(pids) = holders.get(inode) else { continue };
        busy.push(*slot);
        for pid in pids {
            let (mut p, mut seen) = (Some(*pid), HashSet::new());
            while let Some(cur) = p {
                if !seen.insert(cur) || cur <= 1 {
                    break;
                }
                if let Some(issue) = jobs.get(&cur) {
                    owner.insert(*issue, *slot);
                    break;
                }
                p = parent(cur);
            }
        }
    }
    busy.sort_unstable();
    busy.dedup();
    (busy, owner)
}

/// The issues of the branches merged onto a train, from its merge commits'
/// subjects (`Merge branch 'feature/issue-412' into train/...`).
pub fn train_members(subjects: &str) -> Vec<u32> {
    let mut out = Vec::new();
    for line in subjects.lines() {
        let Some(at) = line.find("issue-") else { continue };
        let n: String = line[at + 6..].chars().take_while(char::is_ascii_digit).collect();
        if let Ok(n) = n.parse() {
            if !out.contains(&n) {
                out.push(n);
            }
        }
    }
    out
}

/// The branch a job's `.run.sh` lands: the word after `--feature`.
pub fn run_feature(run_sh: &str) -> Option<String> {
    let mut words = run_sh.split_whitespace();
    words.find(|w| *w == "--feature")?;
    words.next().map(|w| w.trim_matches(['\'', '"']).to_string())
}

/// How many gates may run at once: `AUTOCODER_MAX_GATES`, else a quarter of the CPUs, at least one.
pub fn max_gates(env: Option<&str>, cpus: usize) -> u32 {
    env.and_then(|v| v.trim().parse::<u32>().ok()).filter(|n| *n >= 1).unwrap_or(((cpus / 4) as u32).max(1))
}

// ---------------------------------------------------------------------------
// Reading the host
// ---------------------------------------------------------------------------

/// What the merge gate's files say on this host, for one repo.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GateReading {
    /// Read on this host. A remote village reports no gate at all.
    pub local: bool,
    pub jobs: Vec<Job>,
    pub slots: u32,
    pub busy: Vec<u32>,
}

/// autocoder's repo id: the main checkout's name and the first 8 hex of the
/// sha1 of its git common dir, as `repo-id-fns.sh` makes it.
pub fn repo_id(common_dir: &Path, sha1_hex: &str) -> String {
    let mut name = common_dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if name == ".git" {
        name = common_dir.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    }
    format!("{name}-{}", &sha1_hex[..sha1_hex.len().min(8)])
}

fn command(cmd: &str, args: &[&str], dir: &Path, stdin: Option<&str>) -> Option<String> {
    use std::io::Write;
    let mut child = std::process::Command::new(cmd)
        .args(args)
        .current_dir(dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    if let Some(input) = stdin {
        child.stdin.take()?.write_all(input.as_bytes()).ok()?;
    }
    drop(child.stdin.take());
    let out = child.wait_with_output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The last `max` bytes of a file, cut at a line start.
fn tail(path: &Path, max: u64) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(path) else { return String::new() };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let from = len.saturating_sub(max);
    if f.seek(SeekFrom::Start(from)).is_err() {
        return String::new();
    }
    let mut buf = Vec::new();
    let _ = f.read_to_end(&mut buf);
    let text = String::from_utf8_lossy(&buf).into_owned();
    match (from > 0, text.find('\n')) {
        (true, Some(nl)) => text[nl + 1..].to_string(),
        _ => text,
    }
}

/// Log bytes read per job. A gate's test output can run to megabytes; the
/// phase lines and the latest summaries are near the end.
const LOG_TAIL: u64 = 512 * 1024;

/// Read the merge jobs for `repo` on this host. Blocking: it reads a few
/// small files and runs `git` twice.
pub fn read_local(repo: &Path) -> GateReading {
    let tmp = PathBuf::from(std::env::var("AUTOCODER_TMP").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/tmp".to_string()));
    let slots = max_gates(std::env::var("AUTOCODER_MAX_GATES").ok().as_deref(), std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4));
    let mut reading = GateReading { local: true, slots, ..GateReading::default() };
    let Some(common) = command("git", &["rev-parse", "--git-common-dir"], repo, None) else { return reading };
    let common = std::fs::canonicalize(repo.join(&common)).unwrap_or_else(|_| repo.join(&common));
    let Some(sha) = command("sha1sum", &[], repo, Some(&common.to_string_lossy())).and_then(|s| s.split_whitespace().next().map(str::to_string)) else {
        return reading;
    };
    let prefix = format!("autocoder-merge-{}-", repo_id(&common, &sha));

    let mut jobs = Vec::new();
    for entry in std::fs::read_dir(&tmp).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(issue) = name.strip_prefix(&prefix).and_then(|r| r.strip_suffix(".pid")).and_then(|n| n.parse::<u32>().ok()) else { continue };
        let base = tmp.join(format!("{prefix}{issue}"));
        let pid = std::fs::read_to_string(base.with_extension("pid")).ok().and_then(|s| s.trim().parse::<u32>().ok());
        let alive = pid.is_some_and(|p| Path::new(&format!("/proc/{p}")).exists());
        let run = std::fs::read_to_string(format!("{}.run.sh", base.display())).unwrap_or_default();
        let feature = run_feature(&run);
        let members = match feature.as_deref() {
            Some(f) if f.starts_with("train/") => {
                let range = format!("origin/HEAD..{f}");
                command("git", &["log", "--merges", "--format=%s", &range], repo, None).map(|s| train_members(&s)).unwrap_or_default()
            }
            _ => Vec::new(),
        };
        jobs.push((
            pid,
            Job {
                issue,
                alive,
                exit: std::fs::read_to_string(base.with_extension("exit")).ok().and_then(|s| s.trim().parse().ok()),
                log: tail(&base.with_extension("log"), LOG_TAIL),
                feature,
                started: entry.metadata().and_then(|m| m.modified()).ok(),
                slot: None,
                members,
            },
        ));
    }

    // Which job holds which slot: /proc/locks names the holder's pid, and the
    // holder is a subshell several levels under the job's wrapper.
    let slot_inodes: Vec<(u32, u64)> = (1..=slots)
        .filter_map(|k| {
            use std::os::unix::fs::MetadataExt;
            std::fs::metadata(tmp.join(format!("autocoder-gate-slot-{k}.lock"))).ok().map(|m| (k, m.ino()))
        })
        .collect();
    let holders = lock_holders(&std::fs::read_to_string("/proc/locks").unwrap_or_default());
    let parent = |pid: u32| std::fs::read_to_string(format!("/proc/{pid}/stat")).ok().and_then(|s| parent_pid(&s));
    let job_pids: Vec<(u32, u32)> = jobs.iter().filter(|(_, j)| j.alive).filter_map(|(p, j)| p.map(|p| (j.issue, p))).collect();
    let (busy, owner) = slot_owners(&slot_inodes, &holders, &parent, &job_pids);
    reading.busy = busy;
    reading.jobs = jobs
        .into_iter()
        .map(|(_, mut j)| {
            j.slot = owner.get(&j.issue).copied();
            j
        })
        .collect();
    reading.jobs.sort_by_key(|j| j.issue);
    reading
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const T0: u64 = 1_794_240_000;

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn job(issue: u32, alive: bool, exit: Option<i32>, log: &str) -> Job {
        Job { issue, alive, exit, log: log.to_string(), started: Some(at(T0)), ..Job::default() }
    }

    fn labels(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    const PREPARING: &str = "ℹ️  Issue #364 handed off: working → merging.\nPreparing worktree (detached HEAD 00dfca9)\nMerge made by the 'ort' strategy.\n";
    const WAITING: &str = "🧪 Gating the integrated tree 2a278bb5: cargo test\n⏳ All 3 gate slot(s) on this host are busy; waiting for slot 3 (12:01:02)\n";
    const GATING: &str = "🧪 Gating the integrated tree 2a278bb5: cargo test\ngate-cache: no usable record for tree 2a278bb5; running gate\nrunning 12 tests\ntest result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\nrunning 700 tests\ntest a ... ok\n";
    const CONFLICT: &str = "Auto-merging src/web/server.rs\nCONFLICT (content): Merge conflict in src/web/server.rs\n❌ Merge conflict integrating 'master' into feature/issue-9.\n";
    const FAILED: &str = "🧪 Gating the integrated tree 2a278bb5: cargo test\ntest result: FAILED. 698 passed; 2 failed; 0 ignored\n❌ Tests fail on the integrated tree 2a278bb5.\n❌ origin/master passes alone, so this change broke it. NOT pushing.\n";
    const RED_REFUSED: &str = "❌ Tests fail on the integrated tree 2a278bb5.\n❌ origin/master fails alone too (tree 9c9c9c9c), and no green ref yet. NOT pushing.\n";
    const RED_LANDED: &str = "⚠️  origin/master fails alone too (tree 9c9c9c9c): the integration branch is red independent of this change.\n✅ Issue #9 merged into 'master' on a red base (...)\n";
    const RESYNC: &str = "🧪 Gating the integrated tree aaaa: cargo test\ntest result: ok. 5 passed; 0 failed\n⚠️  Push to 'master' rejected (a sibling worker likely pushed first); re-syncing (attempt 1/5)…\nℹ️  Re-sync brings in src/web/village.rs; re-gating the merged tree\n🧪 Gating the integrated tree bbbb: cargo test\nrunning 3 tests\n";
    const PASSED: &str = "🧪 Gating the integrated tree aaaa: cargo test\n✅ Issue #9 merged into 'master' and verified: tests passed on the published tree aaaa.\n✅ Issue #9 closed.\n";

    #[test]
    fn running_jobs_are_queued_merging_or_gating() {
        assert_eq!(running_phase(""), GateState::Queued, "launched, nothing logged yet");
        assert_eq!(running_phase(PREPARING), GateState::Merging);
        assert_eq!(running_phase(WAITING), GateState::Queued, "waiting for a slot");
        assert_eq!(running_phase(&format!("{WAITING}▶️  Gate slot 3 acquired (12:04:00)\n")), GateState::Gating);
        assert_eq!(running_phase(GATING), GateState::Gating);
        assert_eq!(running_phase("🧪 Gating the integrated tree a\n❌ Tests fail on the integrated tree a.\n"), GateState::Gating, "the base is being gated alone");
        assert_eq!(running_phase(RESYNC), GateState::Gating, "a re-sync that re-gates");
        assert_eq!(running_phase("🧪 Gating the integrated tree a\n⚠️  Push to 'master' rejected (a sibling worker likely pushed first); re-syncing (attempt 1/5)…\n"), GateState::Merging);
        assert_eq!(running_phase(PASSED), GateState::Merging, "landed, closing up");
    }

    #[test]
    fn progress_counts_the_latest_gate_run() {
        assert_eq!(gate_progress(GATING), Some((12, 1)));
        assert_eq!(gate_progress(FAILED), Some((700, 1)));
        assert_eq!(gate_progress(RESYNC), None, "the re-gate has no summaries yet; the first run's don't count");
        assert_eq!(gate_progress(PREPARING), None);
        assert_eq!(gate_runs(RESYNC), 2);
        assert_eq!(gate_runs(PREPARING), 0);
    }

    #[test]
    fn every_state_from_files_and_labels() {
        let now = at(T0 + 125);
        let url = Some("https://github.com/o/r/issues/9");
        let state = |j: Option<&Job>, l: &[&str]| job_gate(j, &labels(l), url, now).map(|g| (g.state, g.landed));

        assert_eq!(state(Some(&job(9, true, None, WAITING)), &["merging"]), Some((GateState::Queued, false)));
        assert_eq!(state(Some(&job(9, true, None, PREPARING)), &["merging"]), Some((GateState::Merging, false)));
        assert_eq!(state(Some(&job(9, true, None, GATING)), &["merging"]), Some((GateState::Gating, false)));
        // The hand-off can fail (the backend refused) and leave `working`: the job still decides.
        assert_eq!(state(Some(&job(9, true, None, GATING)), &["working"]), Some((GateState::Gating, false)));
        assert_eq!(state(Some(&job(9, false, Some(0), PASSED)), &[]), Some((GateState::Passed, true)));
        assert_eq!(state(Some(&job(9, false, Some(0), "✅ Issue #9 merged into 'master' (disjoint re-sync; ...)\n")), &["awaiting-integration"]), Some((GateState::LandedUnproven, true)));
        assert_eq!(state(None, &["awaiting-integration"]), Some((GateState::LandedUnproven, true)), "parked by an earlier job whose files are gone");
        assert_eq!(state(Some(&job(9, false, Some(2), FAILED)), &["needs-clarification"]), Some((GateState::Failed, false)));
        assert_eq!(state(Some(&job(9, false, Some(1), CONFLICT)), &["needs-clarification"]), Some((GateState::Conflict, false)));
        assert_eq!(state(Some(&job(9, false, Some(1), RED_REFUSED)), &["needs-clarification"]), Some((GateState::RedBase, false)));
        assert_eq!(state(Some(&job(9, false, Some(0), RED_LANDED)), &["awaiting-integration"]), Some((GateState::RedBase, true)));
        assert_eq!(state(Some(&job(9, false, Some(1), "❌ Could not fetch origin/master\n")), &[]), Some((GateState::Failed, false)));
        // Nothing to go on: no guess.
        assert_eq!(state(None, &["merging"]), None, "no job files on this host");
        assert_eq!(state(Some(&job(9, false, None, GATING)), &["merging"]), None, "the job died without an exit code");
    }

    #[test]
    fn a_running_gate_carries_slot_timing_attempt_and_progress() {
        let mut j = job(9, true, None, RESYNC);
        j.slot = Some(2);
        let g = job_gate(Some(&j), &labels(&["merging"]), None, at(T0 + 125)).unwrap();
        assert_eq!(g.slot, Some(2));
        assert_eq!(g.started_at.as_deref(), Some("2026-11-09T16:00:00Z"));
        assert_eq!(g.elapsed_secs, Some(125));
        assert_eq!(g.attempt, 2);
        assert_eq!(g.failure_url, None);
        let g = job_gate(Some(&job(9, true, None, GATING)), &[], None, at(T0)).unwrap();
        assert_eq!((g.tests_run, g.progress.as_deref()), (Some(12), Some("12 tests run (1 suite)")));
        // A finished job keeps when it started, but not a slot or a clock.
        let mut done = job(9, false, Some(2), FAILED);
        done.slot = Some(1);
        let g = job_gate(Some(&done), &[], Some("https://github.com/o/r/issues/9"), at(T0 + 999)).unwrap();
        assert_eq!((g.slot, g.elapsed_secs, g.progress), (None, None, None));
        assert_eq!(g.failure_url.as_deref(), Some("https://github.com/o/r/issues/9"), "the job's comment with the gate tail is on the issue");
    }

    #[test]
    fn slots_belong_to_the_job_above_the_lock_holder() {
        // /proc/locks: slot 1 held by pid 501 (a subshell of job 9's wrapper, 400),
        // slot 2 free, slot 3 held by 777 with 801 waiting on it.
        let locks = "1: FLOCK  ADVISORY  WRITE 501 00:1f:1001 0 EOF\n\
                     2: FLOCK  ADVISORY  WRITE 777 00:1f:1003 0 EOF\n\
                     2: -> FLOCK  ADVISORY  WRITE 801 00:1f:1003 0 EOF\n\
                     3: POSIX  ADVISORY  WRITE 1234 fd:01:5555 0 EOF\n";
        let holders = lock_holders(locks);
        assert_eq!(holders.get(&1003), Some(&vec![777]), "the waiter is not a holder");
        let tree: HashMap<u32, u32> = [(501, 450), (450, 400), (400, 1), (777, 700), (700, 1)].into();
        let parent = |p: u32| tree.get(&p).copied();
        let (busy, owner) = slot_owners(&[(1, 1001), (2, 1002), (3, 1003)], &holders, &parent, &[(9, 400), (12, 650)]);
        assert_eq!(busy, vec![1, 3]);
        assert_eq!(owner, [(9, 1)].into(), "slot 3's holder is no job of this repo");
        assert_eq!(parent_pid("4242 (bash (wrapper)) S 400 4242 4242 0 -1"), Some(400));
        assert_eq!(max_gates(Some("2"), 16), 2);
        assert_eq!(max_gates(Some("0"), 16), 4);
        assert_eq!(max_gates(None, 3), 1);
    }

    #[test]
    fn trains_and_their_members() {
        assert_eq!(
            train_members("Merge branch 'feature/issue-412' into train/20261109T1700Z\nMerge branch 'enhancement/issue-415-auto' into train/20261109T1700Z\nMerge branch 'feature/issue-412'\n"),
            vec![412, 415]
        );
        assert_eq!(run_feature("/x/merge-to-integration.sh --feature train/20261109T1700Z --feature-sha abc --issue 400 > /tmp/l 2>&1\n").as_deref(), Some("train/20261109T1700Z"));
        let issue = |n: u32, l: &[&str]| IssueSnapshot { number: n, state: "open".into(), labels: labels(l), ..IssueSnapshot::default() };
        let mut train = job(400, true, None, GATING);
        train.feature = Some("train/20261109T1700Z".into());
        train.members = vec![412, 415];
        train.slot = Some(1);
        let reading = GateReading { local: true, jobs: vec![train, job(9, false, Some(1), CONFLICT)], slots: 3, busy: vec![1] };
        let (gates, merge) = assess(&reading, &[issue(400, &["merging"]), issue(412, &["merging"]), issue(415, &[]), issue(9, &["needs-clarification"])], at(T0 + 60));
        let merge = merge.unwrap();
        assert_eq!((merge.slots, merge.busy.clone()), (3, vec![1]));
        assert_eq!(merge.trains, vec![Train { id: "20261109T1700Z".into(), task: Some("400".into()), members: vec!["412".into(), "415".into()], state: GateState::Gating }]);
        assert_eq!(gates[&412].state, GateState::Gating);
        assert_eq!(gates[&412].train.as_deref(), Some("20261109T1700Z"));
        assert_eq!(gates[&415].slot, Some(1));
        assert_eq!(gates[&9].state, GateState::Conflict);
        // Read from another host: nothing at all.
        assert_eq!(assess(&GateReading::default(), &[issue(412, &["merging"])], at(T0)), (HashMap::new(), None));
    }

    #[test]
    fn repo_ids_match_autocoders() {
        assert_eq!(repo_id(Path::new("/home/u/src/agents-ui/.git"), "33dc9185f00d"), "agents-ui-33dc9185");
        assert_eq!(repo_id(Path::new("/srv/mirror.git"), "0123456789"), "mirror.git-01234567");
    }

    #[test]
    fn states_serialize_as_the_protocol_names_them() {
        let words: Vec<String> = [GateState::Queued, GateState::LandedUnproven, GateState::RedBase]
            .iter()
            .map(|s| serde_json::to_string(s).unwrap())
            .collect();
        assert_eq!(words, vec!["\"queued\"", "\"landed-unproven\"", "\"red-base\""]);
        let g: TaskGate = serde_json::from_str(r#"{"state": "gating", "slot": 2, "attempt": 1}"#).unwrap();
        assert_eq!((g.state, g.slot, g.attempt), (GateState::Gating, Some(2), 1));
    }
}
