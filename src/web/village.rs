//! Village view model: the swarm drawn as a 16-bit village.
//!
//! The manager runs the Town Hall and every worker session gets a house. A
//! session can be a crew of several agents working one task, so houses are
//! keyed by worktree, not by agent. Open issues are crates: the backlog piles
//! up at the docks, blocked issues and proposals are pinned to the hall's
//! notice board, and pushed work travels by barge through one lock per
//! pipeline stop (the sentinel's green ref, then each `.autocoder.json`
//! environment) to the castle.
//!
//! A session that spawns sub-agents (Claude Code's Task tool, or a modernize
//! coordinator dispatching its architect/coder/tester/security/documentation
//! specialists) is busy for as long as any of them runs. Claude Code records
//! each one under `~/.claude/projects/<cwd>/<session>/subagents/`, which is
//! where the village counts them.
//!
//! Everything here is a pure function of a `SwarmSnapshot`, its issues, a
//! `Pipeline` read from git, and the sub-agents on disk, so the renderer never
//! has to infer anything.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

use super::deployments::EnvSource;
use super::source::{ActivitySource, IssueTracker, PipelineSource, Reading, TtlCache, VillageSource};
use super::{AgentSnapshot, IssueSnapshot, SwarmSnapshot};
use crate::model::issue::FeatureLink;
use crate::transport::ServerTransport;

/// How long fetched issues and pipeline refs are reused before refetching:
/// 45 s, or `AGENTS_UI_VILLAGE_REFRESH_SECS` (the demo uses 3 so a story
/// plays out in step).
pub(crate) fn extras_ttl() -> Duration {
    std::env::var("AGENTS_UI_VILLAGE_REFRESH_SECS").ok().and_then(|s| s.parse().ok()).map(Duration::from_secs).unwrap_or(Duration::from_secs(45))
}

/// Commits of history searched when placing a pushed tip on a pipeline stop.
const HISTORY_DEPTH: usize = 400;

/// Delivered crates kept for the castle; older ones only count.
const DELIVERED_KEEP: usize = 12;

/// A sub-agent whose transcript hasn't moved for this long is treated as gone
/// (its session crashed or was killed before it could mark it finished).
const SUBAGENT_STALE: Duration = Duration::from_secs(10 * 60);

/// Session directories untouched for this long aren't scanned for sub-agents.
const SESSION_STALE: Duration = Duration::from_secs(2 * 60 * 60);

/// What a house is built as, from the labels on the issue its crew is working.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HouseKind {
    /// No issue: the crew is between tasks.
    Cottage,
    Security,
    Tests,
    Bug,
    Perf,
    Feature,
    Refactor,
    Docs,
    Chores,
    Research,
    /// Working an issue whose labels match nothing above.
    Workshop,
}

/// What the house shows on top of its kind.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HouseStatus {
    /// Scaffolding: the session is still starting.
    Building,
    Working,
    Idle,
    /// An agent in the crew is waiting on a person.
    Blocked,
    Done,
    /// Working, but the pane hasn't moved for a while.
    Stalled,
    Stopped,
    Unknown,
}

/// What an agent is doing this moment, which its villager acts out: a
/// thought cloud, a book, a hammer, a terminal, a magnifying glass, a letter.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// Waiting on the model: composing, planning, reading a tool's result.
    Thinking,
    Reading,
    Writing,
    /// Running commands, tests or builds.
    Running,
    /// Searching or fetching from the web or another system.
    Searching,
    /// Delegating to or messaging another agent or a person.
    Messaging,
}

/// A Claude Code tool's action.
pub fn tool_action(tool: &str) -> Action {
    match tool {
        "Read" | "Grep" | "Glob" | "LS" | "NotebookRead" => Action::Reading,
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => Action::Writing,
        "WebFetch" | "WebSearch" => Action::Searching,
        "Task" | "Agent" | "SendMessage" | "SubagentHandback" => Action::Messaging,
        t if t.starts_with("mcp__") => Action::Searching,
        _ => Action::Running,
    }
}

/// An action guessed from a name such as a Temporal activity type
/// (`call_llm`, `SendEmail`, `fetchVendorQuotes`). Unrecognised work runs.
pub fn name_action(name: &str) -> Action {
    let n = name.to_ascii_lowercase();
    const WORDS: &[(Action, &[&str])] = &[
        (Action::Messaging, &["email", "notify", "send", "slack", "message", "post", "signal", "handoff"]),
        (Action::Searching, &["search", "fetch", "http", "scrape", "browse", "crawl", "lookup", "query"]),
        (Action::Writing, &["write", "commit", "edit", "create", "update", "save", "patch", "draft"]),
        (Action::Thinking, &["llm", "model", "chat", "prompt", "complet", "think", "plan", "reason", "generat", "summar", "classif"]),
        (Action::Reading, &["read", "load", "get", "retriev", "download", "parse", "extract"]),
    ];
    WORDS
        .iter()
        .find(|(_, words)| words.iter().any(|w| n.contains(w)))
        .map(|(a, _)| *a)
        .unwrap_or(Action::Running)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct House {
    /// Stable key: the worktree path, or the role when the path is unknown.
    pub id: String,
    /// Role of the crew's first agent, e.g. `worker-3`.
    pub role: String,
    pub kind: HouseKind,
    pub status: HouseStatus,
    pub issue: Option<u32>,
    pub issue_title: Option<String>,
    pub priority: Option<String>,
    /// Agents living in this house (one session, one task).
    pub crew: Vec<String>,
    /// Sub-agents the crew's sessions are running right now.
    #[serde(default)]
    pub subagents: Vec<Subagent>,
    /// What the house's own agent is doing, when known.
    #[serde(default)]
    pub action: Option<Action>,
    /// Where to drill in: the agent's session, its issue, its workflow.
    #[serde(default)]
    pub links: Vec<super::framework::Link>,
    /// A structured question its agent is waiting on, shown as buttons.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<super::framework::Question>,
    /// Where those buttons send the answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer_url: Option<String>,
    /// The branch its worktree has checked out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// When the server first saw it in its current status (RFC 3339).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Subagent {
    /// Claude Code agent type, e.g. `Explore` or `modernize:coder`, or a
    /// Temporal child workflow's type.
    pub agent_type: String,
    pub description: String,
    /// Set when the source knows it, e.g. a Temporal child waiting on a person.
    #[serde(default)]
    pub status: Option<HouseStatus>,
    #[serde(default)]
    pub action: Option<Action>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Crate {
    pub number: u32,
    pub title: String,
    pub kind: HouseKind,
    pub priority: Option<String>,
    /// The issue in its tracker.
    #[serde(default)]
    pub url: Option<String>,
    /// Open issues this one waits on ("Depends on" / "Blocked by" in its
    /// body). autocoder won't pick it up until they close.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub waits_on: Vec<u32>,
    /// The feature (epic) this issue is a subtask of, from the feature links
    /// or a report task's `parent`. Its crate carries that feature's colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Note {
    pub number: u32,
    pub title: String,
    /// The blocking label that pinned it, e.g. `needs-design` or `proposal`.
    pub label: String,
    #[serde(default)]
    pub url: Option<String>,
}

/// Where a stop's idea of "what runs here" comes from.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StopSource {
    /// The newest successful GitHub Deployment to the environment.
    Deployment,
    /// The latest published GitHub Release (its tag's commit).
    Release,
    /// The newest tag matching the environment's `tag:<glob>`.
    Tag,
    /// A version endpoint the environment's `url:<endpoint>` names.
    Url,
    /// The environment's branch ref (or, for the first stop, the green ref).
    /// Also what a report that doesn't say means.
    #[default]
    Branch,
    /// Neither exists: the village can't tell what runs here, which is not
    /// the same as nothing having reached it.
    Unknown,
}

/// One lock on the canal: issues whose pushed work has reached this stop but
/// not the next one.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Stop {
    pub name: String,
    pub issues: Vec<u32>,
    #[serde(default)]
    pub source: StopSource,
    /// The commit this stop runs, when a deployment names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha: Option<String>,
    /// The release this stop runs (e.g. `v0.5.0`), when its deployment says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// A page about what this stop runs, such as its GitHub Release.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The sentinel's green ref (a lock in the dev village) or a deploy
    /// environment (a village of its own in the environment row).
    #[serde(default)]
    pub kind: StopKind,
    /// When the running version was deployed, RFC 3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployed_at: Option<String>,
    /// Who deployed it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployed_by: Option<String>,
    /// The page listing this environment's deployments, when a report knows it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment_url: Option<String>,
    /// Every issue the stop runs, newest first: cumulative, unlike `issues`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cargo: Vec<u32>,
    /// Older issues it also runs, cut from `cargo`.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cargo_more: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<Health>,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// What a stop is in the environment row (`docs/environment-row.md`).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StopKind {
    /// The sentinel's `<integration>-green` ref: a gate, drawn as a lock.
    Gate,
    /// A configured deploy environment.
    #[default]
    Environment,
}

/// How an environment is doing, where something can tell.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Health {
    pub state: HealthState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HealthState {
    #[default]
    Ok,
    /// The newest deployment failed.
    Failed,
    /// A world's platform has open incidents against it.
    Incident,
    /// Nothing says what it runs.
    Unknown,
}

/// One deployment to one environment, as a ship between two villages.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Voyage {
    /// The village it leaves: the previous environment, or `dev`.
    pub from: String,
    pub to: String,
    pub state: super::deployments::VoyageState,
    pub sha: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default)]
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The issues new to the target: in `sha` but not in what it ran before.
    #[serde(default)]
    pub issues: Vec<u32>,
    /// Some of what it carries is beyond the history the village reads.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub issues_more: bool,
}

/// Issues an environment's `cargo` lists before the rest are counted.
pub const CARGO_KEEP: usize = 40;
/// How long a docked voyage stays reported after it arrives.
pub const DOCKED_FOR_SECS: i64 = 3600;

/// Per stop, every issue it runs (newest first, capped at `CARGO_KEEP`) and
/// how many more: issues whose tip reached past it. `reached` counts stops
/// reached per issue (0: the quay), as `place_tips` leaves it.
pub fn cargo_of(tips: &[(String, u32)], reached: &HashMap<u32, usize>, stops: usize) -> Vec<(Vec<u32>, usize)> {
    let mut order: Vec<u32> = Vec::new();
    for (_, n) in tips {
        if !order.contains(n) {
            order.push(*n);
        }
    }
    (0..stops)
        .map(|i| {
            let all: Vec<u32> = order.iter().copied().filter(|n| reached.get(n).is_some_and(|r| *r > i)).collect();
            let more = all.len().saturating_sub(CARGO_KEEP);
            (all.into_iter().take(CARGO_KEEP).collect(), more)
        })
        .collect()
}

/// The issues a deployment of `new` adds to an environment that ran `old`:
/// tips in the new history but not the old, newest first, once each. With no
/// old version every tip in the new history is new.
pub fn voyage_issues(tips: &[(String, u32)], new: &HashSet<String>, old: Option<&HashSet<String>>) -> Vec<u32> {
    let mut out = Vec::new();
    for (sha, n) in tips {
        if new.contains(sha) && !old.is_some_and(|o| o.contains(sha)) && !out.contains(n) {
            out.push(*n);
        }
    }
    out
}

/// The village a deployment to `env` sails from: the environment it
/// requires, or the dev village for `integration`, `green` or nothing.
pub fn voyage_from(env: &Environment, envs: &[Environment]) -> String {
    match env.requires.as_deref() {
        Some(r) if envs.iter().any(|e| e.name == r && e.name != env.name) => r.to_string(),
        _ => "dev".to_string(),
    }
}

/// An environment's health: failed when its newest deployment failed,
/// unknown when nothing says what it runs, else ok.
pub fn health_of(source: StopSource, voyage: Option<&Voyage>) -> Health {
    match voyage {
        Some(v) if v.state == super::deployments::VoyageState::Failed => Health {
            state: HealthState::Failed,
            reason: Some(format!("deployment of {} failed", &v.sha[..v.sha.len().min(7)])),
            url: v.url.clone(),
        },
        _ if source == StopSource::Unknown => Health { state: HealthState::Unknown, ..Health::default() },
        _ => Health::default(),
    }
}

/// Whether a voyage is still worth showing at `now`: in flight or failed, or
/// docked within `DOCKED_FOR_SECS`.
pub fn voyage_current(v: &Voyage, now: chrono::DateTime<chrono::Utc>) -> bool {
    if v.state != super::deployments::VoyageState::Docked {
        return true;
    }
    let at = v.finished_at.as_deref().unwrap_or(&v.started_at);
    chrono::DateTime::parse_from_rfc3339(at).is_ok_and(|t| (now - t.with_timezone(&chrono::Utc)).num_seconds() <= DOCKED_FOR_SECS)
}

/// How landed work moves through the environments, from `.autocoder.json`'s
/// `promotion`.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Promotion {
    /// Each subtask moves on its own, so a feature can be part-way through.
    #[default]
    Subtask,
    /// A feature's landed subtasks wait at the quay and move together.
    Feature,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Pipeline {
    pub integration: String,
    /// `green` when the green ref is at the integration tip, `pending` while
    /// the sentinel has newer work to test, `unknown` with no green ref.
    pub sentinel: String,
    /// Pushed to the integration branch, waiting at the quay.
    pub quay: Vec<u32>,
    /// The green ref, then each environment in pipeline order.
    pub stops: Vec<Stop>,
    /// The commit at the last stop: what is released.
    #[serde(default)]
    pub release: Option<String>,
    #[serde(default)]
    pub promotion: Promotion,
    /// How many stops each placed issue has reached (0: still at the quay),
    /// before the last stop is cut down to the crates the castle shows.
    /// Counting a feature's subtasks in production needs all of them.
    #[serde(skip)]
    pub reached: HashMap<u32, usize>,
    /// Titles of the issues at the quay and the stops, which have mostly left
    /// the open issues the backlog is drawn from.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub titles: std::collections::BTreeMap<u32, String>,
    /// Where each task handed to the merge gate is: queued, gating, failed...
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gates: Vec<GateEntry>,
    /// The host's merge gate: slots, the busy ones, and merge trains.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge: Option<super::gate::MergeGate>,
    /// Deployments as ships between the villages of the environment row.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub voyages: Vec<Voyage>,
    /// Where the gate handed each issue's work back, oldest first (#431).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub bounces: std::collections::BTreeMap<u32, Vec<super::gate::Bounce>>,
    /// Today at the gate: landed, bounced, and the files that conflict most.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_stats: Option<super::gate::MergeStats>,
}

/// A task's trip through the merge gate, by issue number.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GateEntry {
    pub issue: u32,
    #[serde(flatten)]
    pub gate: super::gate::TaskGate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VillageSnapshot {
    pub project: String,
    /// The swarm that writes the system, or the platform that runs it.
    #[serde(default)]
    pub plane: super::framework::Plane,
    /// The framework it came from: `autocoder`, `herdr`, `temporal`, ...
    #[serde(default)]
    pub framework: String,
    /// `Autocoder` or `Modernize`: from the swarm, or inferred from a
    /// `modernize:*` sub-agent.
    pub workflow: Option<String>,
    pub repo_path: String,
    pub manager: House,
    pub houses: Vec<House>,
    pub backlog: Vec<Crate>,
    pub board: Vec<Note>,
    pub pipeline: Pipeline,
    /// Build IDs the village's workers report running (Temporal pollers).
    #[serde(default)]
    pub builds: Vec<String>,
    /// Parent issues split into subtasks, followed from spec to production.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<super::feature::Feature>,
    /// The integration branch's build health (`build.rs`): CI, the sentinel,
    /// the merge gate, or what an outside report says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<super::build::Build>,
}

/// Labels checked in order; the first that matches decides the house.
const KIND_LABELS: &[(&str, HouseKind)] = &[
    ("security", HouseKind::Security),
    ("test-failure", HouseKind::Tests),
    ("tests", HouseKind::Tests),
    ("test", HouseKind::Tests),
    ("bug", HouseKind::Bug),
    ("performance", HouseKind::Perf),
    ("perf", HouseKind::Perf),
    ("enhancement", HouseKind::Feature),
    ("feature", HouseKind::Feature),
    ("refactor", HouseKind::Refactor),
    ("documentation", HouseKind::Docs),
    ("docs", HouseKind::Docs),
    ("chore", HouseKind::Chores),
    ("dependencies", HouseKind::Chores),
    ("ci", HouseKind::Chores),
    ("proposal", HouseKind::Research),
    ("research", HouseKind::Research),
    ("question", HouseKind::Research),
];

pub fn house_kind(labels: &[String]) -> HouseKind {
    KIND_LABELS
        .iter()
        .find(|(name, _)| labels.iter().any(|l| l.eq_ignore_ascii_case(name)))
        .map(|(_, kind)| *kind)
        .unwrap_or(HouseKind::Workshop)
}

pub fn agent_status(agent: &AgentSnapshot) -> HouseStatus {
    super::framework::swarm_agent_state(agent).status()
}

/// Build the village for one swarm.
///
/// `issues` overrides `swarm.issues` when the snapshot came from tmux
/// discovery, which doesn't fetch issues.
pub fn build(swarm: &SwarmSnapshot, issues: &[IssueSnapshot], pipeline: Pipeline) -> VillageSnapshot {
    build_gated(swarm, issues, pipeline, &super::gate::GateReading::default())
}

/// [`build`], with what the merge gate's files on this host say.
pub fn build_gated(swarm: &SwarmSnapshot, issues: &[IssueSnapshot], pipeline: Pipeline, gate: &super::gate::GateReading) -> VillageSnapshot {
    let mut village = super::framework::to_village(&super::framework::swarm_report_gated(swarm, issues, &pipeline, gate)).village;
    // Not part of the report: only git's placement has it.
    village.pipeline.reached = pipeline.reached;
    village
}

// ---------------------------------------------------------------------------
// Sub-agents from Claude Code's transcripts
// ---------------------------------------------------------------------------

/// Claude Code's per-project directory name: every character that isn't
/// ASCII alphanumeric becomes `-` (`/home/u/repo-wt-1` → `-home-u-repo-wt-1`).
pub fn claude_project_dir_name(cwd: &str) -> String {
    cwd.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn modified_within(path: &Path, now: SystemTime, window: Duration) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| now.duration_since(t).ok().or(Some(Duration::ZERO)))
        .is_some_and(|age| age <= window)
}

/// Bytes read from the end of a transcript to find its latest step.
const TRANSCRIPT_TAIL: u64 = 64 * 1024;

/// What a Claude Code transcript's agent is doing: the tool its latest turn
/// called, or thinking while the model composes or reads a tool's result.
pub fn transcript_action(tail: &str) -> Option<Action> {
    for line in tail.lines().rev() {
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let content = entry.pointer("/message/content").and_then(|c| c.as_array());
        match entry.get("type").and_then(|t| t.as_str()) {
            Some("assistant") => {
                let tool = content
                    .and_then(|items| items.iter().rev().find(|i| i.get("type").and_then(|t| t.as_str()) == Some("tool_use")))
                    .and_then(|i| i.get("name"))
                    .and_then(|n| n.as_str());
                return Some(tool.map(tool_action).unwrap_or(Action::Thinking));
            }
            Some("user") => return Some(Action::Thinking),
            _ => continue,
        }
    }
    None
}

fn last_action(transcript: &Path) -> Option<Action> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(transcript).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(TRANSCRIPT_TAIL))).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    transcript_action(&String::from_utf8_lossy(&buf))
}

/// The action of the most recent live main session under a Claude project
/// dir: the agent living in the house itself, not its sub-agents.
pub fn session_action(project_dir: &Path, now: SystemTime) -> Option<Action> {
    let newest = std::fs::read_dir(project_dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl") && modified_within(p, now, SUBAGENT_STALE))
        .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())?;
    last_action(&newest)
}

/// Sub-agents running in any recent session under one Claude project dir.
///
/// A sub-agent is running when its `.meta.json` isn't marked `finished` and
/// its transcript moved within `SUBAGENT_STALE`.
pub fn active_subagents(project_dir: &Path, now: SystemTime) -> Vec<Subagent> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Meta {
        #[serde(default)]
        agent_type: String,
        #[serde(default)]
        description: String,
        #[serde(default)]
        finished: bool,
    }
    let Ok(sessions) = std::fs::read_dir(project_dir) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for session in sessions.flatten() {
        let dir = session.path().join("subagents");
        if !session.path().is_dir() || !modified_within(&session.path(), now, SESSION_STALE) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let mut metas: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.to_string_lossy().ends_with(".meta.json"))
            .collect();
        metas.sort();
        for meta_path in metas {
            let Some(meta) = std::fs::read_to_string(&meta_path)
                .ok()
                .and_then(|s| serde_json::from_str::<Meta>(&s).ok())
            else {
                continue;
            };
            let transcript = PathBuf::from(meta_path.to_string_lossy().replace(".meta.json", ".jsonl"));
            if meta.finished || !modified_within(&transcript, now, SUBAGENT_STALE) {
                continue;
            }
            found.push(Subagent { agent_type: meta.agent_type, description: meta.description, status: None, action: last_action(&transcript) });
        }
    }
    found
}

/// The house a specialist sub-agent builds when its session has no issue.
/// Move a house's sub-agents in: any running sub-agent makes the house busy,
/// and with no issue of its own the house takes its first specialist's shape.
pub fn apply_subagents(house: &mut House, subagents: Vec<Subagent>) {
    if subagents.is_empty() {
        return;
    }
    if subagents.iter().any(|s| s.status == Some(HouseStatus::Blocked)) {
        house.status = HouseStatus::Blocked;
    } else if matches!(
        house.status,
        HouseStatus::Idle | HouseStatus::Done | HouseStatus::Unknown | HouseStatus::Stalled
    ) {
        house.status = HouseStatus::Working;
    }
    if house.issue.is_none() {
        house.kind = subagents
            .iter()
            .find_map(|s| super::framework::specialist_kind(&s.agent_type))
            .unwrap_or(HouseKind::Workshop);
    }
    house.subagents = subagents;
}

fn infer_workflow(village: &VillageSnapshot) -> Option<String> {
    let modernize = std::iter::once(&village.manager)
        .chain(village.houses.iter())
        .flat_map(|h| h.subagents.iter())
        .any(|s| s.agent_type.to_ascii_lowercase().starts_with("modernize"));
    modernize.then(|| "Modernize".to_string())
}

/// Attach running sub-agents to the hall and every house. Reads the local
/// `~/.claude`, so it is skipped when agents run on a remote `--server`.
fn attach_subagents(village: &mut VillageSnapshot, home: &Path, now: SystemTime) {
    let projects = home.join(".claude").join("projects");
    let scan = |cwd: &str| active_subagents(&projects.join(claude_project_dir_name(cwd)), now);
    if !village.repo_path.is_empty() {
        let subs = scan(&village.repo_path);
        apply_subagents(&mut village.manager, subs);
    }
    for house in &mut village.houses {
        if house.id.starts_with('/') {
            let subs = scan(&house.id);
            apply_subagents(house, subs);
            if house.status == HouseStatus::Working {
                house.action = session_action(&projects.join(claude_project_dir_name(&house.id)), now);
            }
        }
    }
    if village.workflow.is_none() {
        village.workflow = infer_workflow(village);
    }
}

// ---------------------------------------------------------------------------
// Pipeline from git
// ---------------------------------------------------------------------------

/// An environment from `.autocoder.json`, in pipeline order.
#[derive(Debug, Clone, PartialEq)]
pub struct Environment {
    pub name: String,
    pub branch: String,
    /// What it takes work from: the previous stop's name, `green` (the
    /// sentinel), or `integration` (the integration branch, with no sentinel).
    pub requires: Option<String>,
    /// Where to learn what it runs, tried in order (`source` in
    /// `.autocoder.json`). Empty: [`super::deployments::default_chain`].
    pub sources: Vec<EnvSource>,
}

/// The stop source a chain entry reports as.
pub fn stop_source(source: &EnvSource) -> StopSource {
    match source {
        EnvSource::Deployment => StopSource::Deployment,
        EnvSource::Release => StopSource::Release,
        EnvSource::Tag(_) => StopSource::Tag,
        EnvSource::Url(_) => StopSource::Url,
        EnvSource::Branch => StopSource::Branch,
    }
}

/// `requires` value for an environment that takes work straight from the
/// integration branch.
pub const REQUIRES_INTEGRATION: &str = "integration";

/// Whether the pipeline has no sentinel: its first environment takes work
/// straight from the integration branch. A repo with no sentinel has no
/// green ref, and a green stop that never exists would hold everything at
/// the quay.
pub fn gate_absent(envs: &[Environment]) -> bool {
    envs.first().and_then(|e| e.requires.as_deref()) == Some(REQUIRES_INTEGRATION)
}

/// The sentinel's state and its stop, given the green ref's history and the
/// integration tip. `absent` (and no stop) when the environments say there
/// is no sentinel; else `unknown` with no green ref, `green` when it is at
/// the tip, `pending` while newer work waits to be tested.
pub fn sentinel_gate(envs: &[Environment], green: Option<Vec<String>>, tip: Option<&String>) -> (String, Option<StopHistory>) {
    if gate_absent(envs) {
        return ("absent".to_string(), None);
    }
    let state = match (&green, tip) {
        (None, _) => "unknown",
        (Some(g), Some(tip)) if g.first() == Some(tip) => "green",
        _ => "pending",
    };
    let stop = StopHistory { source: StopSource::Branch, commits: green.map(|v| v.into_iter().collect()), ..StopHistory::default() };
    (state.to_string(), Some(stop))
}

/// Parse the `environments` array of `.autocoder.json`.
pub fn parse_environments(json: &str) -> Vec<Environment> {
    #[derive(Deserialize)]
    struct Config {
        #[serde(default)]
        environments: Vec<Env>,
    }
    #[derive(Deserialize)]
    struct Env {
        name: String,
        #[serde(rename = "ref")]
        branch: Option<String>,
        requires: Option<String>,
        source: Option<serde_json::Value>,
    }
    serde_json::from_str::<Config>(json)
        .map(|c| {
            c.environments
                .into_iter()
                .map(|e| Environment {
                    branch: e.branch.unwrap_or_else(|| format!("env/{}", e.name)),
                    name: e.name,
                    requires: e.requires,
                    sources: super::deployments::parse_sources(e.source.as_ref()),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Parse `promotion` from `.autocoder.json`: `subtask` unless it says `feature`.
pub fn parse_promotion(json: &str) -> Promotion {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get("promotion").cloned())
        .and_then(|p| serde_json::from_value(p).ok())
        .unwrap_or_default()
}

/// Parse `refs/autocoder/tips/<integration>/<sha>-<issue>`.
pub fn parse_tip_ref(refname: &str, integration: &str) -> Option<(String, u32)> {
    let rest = refname.strip_prefix(&format!("refs/autocoder/tips/{integration}/"))?;
    let (sha, issue) = rest.rsplit_once('-')?;
    if sha.is_empty() || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some((sha.to_string(), issue.parse().ok()?))
}

/// What one stop runs, as far as placing tips goes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StopHistory {
    pub source: StopSource,
    pub sha: Option<String>,
    /// The deployment's release tag and page, passed on to the stop.
    pub tag: Option<String>,
    pub url: Option<String>,
    pub deployed_at: Option<String>,
    pub deployed_by: Option<String>,
    /// The commits the stop contains, or `None` when nothing says (no
    /// deployment, no ref): the stop is unknown.
    pub commits: Option<HashSet<String>>,
}

/// The stop for the first source in an environment's chain that answered
/// (`None`: none did, so it is unknown), and its head commit.
pub fn stop_from_hit(hit: Option<(EnvSource, (Option<super::deployments::Deployed>, Vec<String>))>) -> (StopHistory, Option<String>) {
    let Some((source, (found, commits))) = hit else {
        return (StopHistory { source: StopSource::Unknown, ..StopHistory::default() }, None);
    };
    let head = commits.first().cloned();
    let found = found.unwrap_or_default();
    let sha = (!found.sha.is_empty()).then_some(found.sha);
    let stop = StopHistory {
        source: stop_source(&source),
        sha,
        tag: found.tag,
        url: found.url,
        deployed_at: found.deployed_at,
        deployed_by: found.deployed_by,
        commits: Some(commits.into_iter().collect()),
    };
    (stop, head)
}

/// Where each pushed tip ended up.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Placement {
    pub quay: Vec<u32>,
    pub stops: Vec<Stop>,
    /// Stops reached per issue (0: the quay), with nothing truncated.
    pub reached: HashMap<u32, usize>,
}

/// Place each pushed tip at the furthest stop that contains it.
///
/// `tips` is newest first. `stops` holds, per stop in pipeline order, what it
/// runs. A tip only reaches a stop after reaching every stop before it, so an
/// unknown stop halts it there. Each issue appears once, at its newest tip.
pub fn place_tips(tips: &[(String, u32)], stop_names: &[String], stops: &[StopHistory]) -> Placement {
    let mut quay = Vec::new();
    let mut placed: Vec<Stop> = stop_names
        .iter()
        .zip(stops)
        .map(|(n, h)| Stop {
            name: n.clone(),
            issues: Vec::new(),
            source: if h.commits.is_some() { h.source } else { StopSource::Unknown },
            sha: h.sha.clone(),
            tag: h.tag.clone(),
            url: h.url.clone(),
            deployed_at: h.deployed_at.clone(),
            deployed_by: h.deployed_by.clone(),
            ..Stop::default()
        })
        .collect();
    let mut reached_by = HashMap::new();
    for (sha, issue) in tips {
        if reached_by.contains_key(issue) {
            continue;
        }
        let reached = stops
            .iter()
            .take_while(|s| s.commits.as_ref().is_some_and(|set| set.contains(sha)))
            .count();
        reached_by.insert(*issue, reached);
        match reached {
            0 => quay.push(*issue),
            n => placed[n - 1].issues.push(*issue),
        }
    }
    if let Some(last) = placed.last_mut() {
        last.issues.truncate(DELIVERED_KEEP);
    }
    Placement { quay, stops: placed, reached: reached_by }
}

async fn git_lines(transport: &ServerTransport, repo: &Path, args: &[&str]) -> Option<Vec<String>> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let out = transport.output("git", &args, Some(repo)).await.ok()?;
    out.status.success().then(|| {
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_string)
            .collect()
    })
}

/// The integration branch: origin's default, or whichever of `main` and
/// `master` exists when the clone has no `origin/HEAD`.
async fn integration_branch(transport: &ServerTransport, repo: &Path) -> String {
    if let Some(branch) = git_lines(transport, repo, &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
        .await
        .and_then(|l| l.first().cloned())
        .and_then(|r| r.strip_prefix("origin/").map(str::to_string))
    {
        return branch;
    }
    for candidate in ["main", "master"] {
        let rev = format!("refs/remotes/origin/{candidate}");
        if git_lines(transport, repo, &["rev-parse", "--verify", "--quiet", &rev]).await.is_some() {
            return candidate.to_string();
        }
    }
    "main".to_string()
}

/// Titles for the issues at the quay and the stops: from the open issues,
/// else the recently closed ones (landed work's issue has usually closed).
pub fn pipeline_titles(p: &Pipeline, open: &[IssueSnapshot], closed: &[IssueSnapshot]) -> std::collections::BTreeMap<u32, String> {
    let known: HashMap<u32, &str> = closed.iter().chain(open).map(|i| (i.number, i.title.as_str())).collect();
    p.quay
        .iter()
        .chain(p.stops.iter().flat_map(|s| s.issues.iter()))
        .filter_map(|n| known.get(n).filter(|t| !t.is_empty()).map(|t| (*n, t.to_string())))
        .collect()
}

/// When each house was first seen in its current status, per village: the
/// house dialog says how long it has been working, blocked or idle. Only as
/// long as this server has been watching, so it says "since", not "for".
#[derive(Default)]
pub struct SinceMemory {
    seen: std::sync::Mutex<HashMap<String, HashMap<String, (HouseStatus, String)>>>,
}

impl SinceMemory {
    pub fn stamp(&self, village: &str, houses: &mut [House], now: chrono::DateTime<chrono::Utc>) {
        let Ok(mut seen) = self.seen.lock() else { return };
        stamp_since(seen.entry(village.to_string()).or_default(), houses, now);
    }
}

/// Set each house's `since` from `memory`, starting the clock for a house
/// that is new or has changed status, and forgetting houses that are gone.
pub fn stamp_since(memory: &mut HashMap<String, (HouseStatus, String)>, houses: &mut [House], now: chrono::DateTime<chrono::Utc>) {
    let stamp = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    memory.retain(|id, _| houses.iter().any(|h| &h.id == id));
    for h in houses.iter_mut() {
        let entry = memory.entry(h.id.clone()).or_insert_with(|| (h.status, stamp.clone()));
        if entry.0 != h.status {
            *entry = (h.status, stamp.clone());
        }
        h.since = Some(entry.1.clone());
    }
}

/// Read where pushed work is in the pipeline. Uses local remote-tracking
/// refs for history and one `ls-remote` for the tip refs, which a plain
/// `git fetch` doesn't bring down.
pub async fn read_pipeline(transport: &ServerTransport, repo: &Path) -> Pipeline {
    let integration = integration_branch(transport, repo).await;
    let green = format!("{integration}-green");
    let config = transport
        .output("cat", &[repo.join(".autocoder.json").to_string_lossy().into_owned()], None)
        .await
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let envs = parse_environments(&config);
    let promotion = parse_promotion(&config);

    let tip_glob = format!("refs/autocoder/tips/{integration}/*");
    let remote = git_lines(transport, repo, &["ls-remote", "origin", &tip_glob])
        .await
        .unwrap_or_default();
    // The ref name carries the issue; the object id is the exact commit.
    let tip_refs: Vec<(String, u32)> = remote
        .iter()
        .filter_map(|line| line.split_once(char::is_whitespace))
        .filter_map(|(oid, name)| parse_tip_ref(name.trim(), &integration).map(|(_, n)| (oid.to_string(), n)))
        .collect();

    let history = |rev: String| async move {
        let max = format!("--max-count={HISTORY_DEPTH}");
        git_lines(transport, repo, &["rev-list", &max, &rev])
            .await
            .filter(|l| !l.is_empty())
    };
    let mainline = history(format!("origin/{integration}")).await.unwrap_or_default();
    let green_set = history(format!("origin/{green}")).await;

    // Order tips newest first by their place on the integration branch. Tip
    // refs are never deleted, so a tip outside the recent window is old
    // history (or not fetched yet, and shows up after the next fetch).
    let position: HashMap<&str, usize> = mainline.iter().enumerate().map(|(i, s)| (s.as_str(), i)).collect();
    let mut tips: Vec<(String, u32)> = tip_refs
        .into_iter()
        .filter(|(sha, _)| position.contains_key(sha.as_str()))
        .collect();
    tips.sort_by_key(|(sha, _)| position[sha.as_str()]);

    let mut release = green_set.as_ref().and_then(|v| v.first().cloned());
    let (sentinel, gate) = sentinel_gate(&envs, green_set, mainline.first());
    let (mut names, mut stops) = match gate {
        Some(stop) => (vec![green], vec![stop]),
        None => {
            release = None;
            (Vec::new(), Vec::new())
        }
    };
    let gated = !stops.is_empty();
    let mut voyages = Vec::new();
    for (i, env) in envs.iter().enumerate() {
        names.push(env.name.clone());
        // The environment's sources, in order: the first that names a commit
        // this clone has wins. A commit it hasn't fetched can't place
        // anything, so that source falls through to the next one too.
        let chain = if env.sources.is_empty() { super::deployments::default_chain(i + 1 == envs.len()) } else { env.sources.clone() };
        // Deployments are read once: what runs, and the newest deployment as a voyage.
        let deployments = match chain.contains(&EnvSource::Deployment) {
            true => super::deployments::deployment_state(transport, repo, &env.name).await,
            false => None,
        };
        let deployments = &deployments;
        let hit = super::deployments::first_hit(&chain, |source| async move {
            match source {
                EnvSource::Branch => history(format!("origin/{}", env.branch)).await.map(|commits| (None, commits)),
                EnvSource::Deployment => {
                    let found = deployments.as_ref()?.deployed.clone()?;
                    let commits = history(found.sha.clone()).await?;
                    Some((Some(found), commits))
                }
                other => {
                    let found = super::deployments::locate(transport, repo, &env.name, &other).await?;
                    let commits = history(found.sha.clone()).await?;
                    Some((Some(found), commits))
                }
            }
        })
        .await;
        let (stop, head) = stop_from_hit(hit);
        release = head;
        stops.push(stop);
        // The voyage's cargo is what it adds to what the target ran before.
        if let Some(v) = deployments.as_ref().and_then(|d| d.voyage.clone()) {
            let new = history(v.sha.clone()).await.map(|c| c.into_iter().collect::<HashSet<String>>());
            let old = match &v.previous {
                Some(p) => history(p.clone()).await.map(|c| c.into_iter().collect::<HashSet<String>>()),
                None => None,
            };
            let short = new.as_ref().is_some_and(|c| c.len() >= HISTORY_DEPTH) || (v.previous.is_some() && old.is_none());
            voyages.push(Voyage {
                from: voyage_from(env, &envs),
                to: env.name.clone(),
                state: v.state,
                issues: new.as_ref().map(|c| voyage_issues(&tips, c, old.as_ref())).unwrap_or_default(),
                issues_more: new.is_none() || short,
                sha: v.sha,
                tag: v.tag,
                started_at: v.started_at,
                finished_at: v.finished_at,
                url: v.url,
            });
        }
    }
    let placement = place_tips(&tips, &names, &stops);
    let now = chrono::Utc::now();
    voyages.retain(|v| voyage_current(v, now));
    let cargo = cargo_of(&tips, &placement.reached, placement.stops.len());
    let mut placed = placement.stops;
    for (i, (stop, (cargo, more))) in placed.iter_mut().zip(cargo).enumerate() {
        stop.kind = if gated && i == 0 { StopKind::Gate } else { StopKind::Environment };
        stop.cargo = cargo;
        stop.cargo_more = more;
        if stop.kind == StopKind::Environment {
            stop.health = Some(health_of(stop.source, voyages.iter().find(|v| v.to == stop.name)));
        }
    }
    Pipeline {
        integration,
        sentinel,
        quay: placement.quay,
        stops: placed,
        release,
        promotion,
        reached: placement.reached,
        voyages,
        ..Pipeline::default()
    }
}

// ---------------------------------------------------------------------------
// Sources: autocoder's pipeline, Claude Code's activity, and swarm villages
// ---------------------------------------------------------------------------

/// The pipeline autocoder records in git: tip refs, the green sentinel, and
/// the environments in `.autocoder.json`.
pub struct AutocoderPipeline;

impl PipelineSource for AutocoderPipeline {
    fn name(&self) -> &'static str {
        "autocoder"
    }
    fn pipeline<'a>(&'a self, transport: &'a ServerTransport, repo: &'a Path) -> BoxFuture<'a, Pipeline> {
        Box::pin(read_pipeline(transport, repo))
    }
}

/// Sub-agents and actions from Claude Code's transcripts. They're local
/// files, so a village read through `--server` shows none.
pub struct ClaudeActivity;

impl ActivitySource for ClaudeActivity {
    fn name(&self) -> &'static str {
        "claude"
    }
    fn attach(&self, transport: &ServerTransport, village: &mut VillageSnapshot) {
        if transport.server().is_none()
            && let Some(home) = dirs::home_dir()
        {
            attach_subagents(village, &home, SystemTime::now());
        }
    }
}

/// A village per discovered swarm, with its issues, pipeline and activity
/// from whichever sources are plugged in.
pub struct SwarmVillages {
    issues: Box<dyn IssueTracker>,
    pipeline: Box<dyn PipelineSource>,
    activity: Box<dyn ActivitySource>,
    extras: TtlCache<PathBuf, (Vec<IssueSnapshot>, Pipeline, Vec<FeatureLink>, Vec<IssueSnapshot>, Vec<(u32, String, String)>)>,
    since: SinceMemory,
    /// Each repo's conflicted files by issue, kept after the job's log is overwritten.
    conflicts: std::sync::Mutex<HashMap<String, HashMap<u32, Vec<String>>>>,
    /// CI answers per repo, re-asked when the tip moves (`build.rs`).
    ci: super::build::CiCache,
}

impl SwarmVillages {
    pub fn new(issues: Box<dyn IssueTracker>, pipeline: Box<dyn PipelineSource>, activity: Box<dyn ActivitySource>) -> Self {
        Self { issues, pipeline, activity, extras: TtlCache::new(extras_ttl()), since: Default::default(), conflicts: Default::default(), ci: Default::default() }
    }

    async fn village(&self, transport: &ServerTransport, swarm: &SwarmSnapshot) -> Reading {
        let repo = PathBuf::from(&swarm.repo_path);
        let (fetched, pipeline, links, closed, merge_comments) = if swarm.repo_path.is_empty() {
            (Vec::new(), Pipeline::default(), Vec::new(), Vec::new(), Vec::new())
        } else {
            self.extras
                .get_or(repo.clone(), || async {
                    // Swarms that bring their own issues (the TUI's) aren't refetched.
                    let issues = match swarm.issues.is_empty() {
                        true => self.issues.issues(transport, &repo, &swarm.project_name).await,
                        false => Vec::new(),
                    };
                    let pipeline = self.pipeline.pipeline(transport, &repo).await;
                    let features = self.issues.features(transport, &repo).await;
                    let comments = self.issues.merge_comments(transport, &repo).await;
                    (issues, pipeline, features, self.issues.closed(transport, &repo).await, comments)
                })
                .await
        };
        let issues = if swarm.issues.is_empty() { fetched } else { swarm.issues.clone() };
        // The merge gate's files are on the host that runs the merges: read
        // them only for a local village, and fresh each time (they change by
        // the second while a gate runs).
        let (gate, retries) = if transport.server().is_none() && !swarm.repo_path.is_empty() {
            let (repo, integration) = (repo.clone(), pipeline.integration.clone());
            tokio::task::spawn_blocking(move || (super::gate::read_local(&repo), super::gate::read_push_retries(&repo, &integration)))
                .await
                .unwrap_or_default()
        } else {
            (super::gate::GateReading::default(), String::new())
        };
        // Bounces (#431): the job's comments and the push retries outlive its files.
        let bounces = {
            let mut memory = self.conflicts.lock().unwrap_or_else(|e| e.into_inner());
            let remembered = memory.entry(swarm.repo_path.clone()).or_default();
            super::gate::remember_conflicts(&gate.jobs, remembered);
            super::gate::bounce_history(&merge_comments, &retries, &gate.jobs, remembered)
        };
        let merge_stats = (gate.local || !bounces.is_empty())
            .then(|| super::gate::merge_stats(&bounces, &gate.jobs, &chrono::Utc::now().format("%Y-%m-%d").to_string()));
        let integration = pipeline.integration.clone();
        let mut village = build_gated(swarm, &issues, pipeline, &gate);
        if !swarm.repo_path.is_empty() {
            village.build = Some(super::build::read(&self.ci, transport, &repo, &integration, &gate).await);
        }
        village.pipeline.titles = pipeline_titles(&village.pipeline, &issues, &closed);
        village.pipeline.bounces = bounces;
        village.pipeline.merge_stats = merge_stats;
        self.since.stamp(&swarm.repo_path, &mut village.houses, chrono::Utc::now());
        super::feature::attach_parents(&mut village.backlog, &links);
        village.features = super::feature::build(&village, &links);
        self.activity.attach(transport, &mut village);
        Reading { village, issues }
    }
}

impl VillageSource for SwarmVillages {
    fn name(&self) -> String {
        format!("swarms ({}, {}, {})", self.issues.name(), self.pipeline.name(), self.activity.name())
    }
    fn read<'a>(&'a self, transport: &'a ServerTransport, swarms: &'a [SwarmSnapshot]) -> BoxFuture<'a, Vec<Reading>> {
        Box::pin(async move {
            let mut out = Vec::with_capacity(swarms.len());
            for swarm in swarms {
                out.push(self.village(transport, swarm).await);
            }
            out
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::framework::specialist_kind;

    fn agent(role: &str, state: &str, worktree: &str) -> AgentSnapshot {
        AgentSnapshot {
            id: format!("proj/{role}"),
            role: role.to_string(),
            state: state.to_string(),
            is_manager: role == "manager",
            waiting_for_input: false,
            current_issue: None,
            current_issue_title: None,
            pane_content: String::new(),
            tmux_target: String::new(),
            health: "Healthy".to_string(),
            completed_issue_count: 0,
            resurrection_attempts: 0,
            status_timestamp: None,
            worktree_path: worktree.to_string(),
            branch: None,
            mux: Default::default(),
        }
    }

    fn issue(number: u32, labels: &[&str]) -> IssueSnapshot {
        let labels: Vec<String> = labels.iter().map(|s| s.to_string()).collect();
        let priority = labels
            .iter()
            .find(|l| l.len() == 2 && l.starts_with('P'))
            .cloned()
            .unwrap_or_else(|| "—".to_string());
        IssueSnapshot {
            number,
            title: format!("Issue {number}"),
            state: "open".to_string(),
            priority,
            is_blocked: labels.iter().any(|l| crate::model::issue::BLOCKING_LABELS.contains(&l.as_str())),
            is_working: labels.iter().any(|l| l == "working"),
            labels,
            assigned_worker: None,
            url: String::new(),
            depends_on: Vec::new(),
        }
    }

    /// #431: the gate's bounces and today's tally ride on the pipeline, and
    /// are left out of the JSON when there are none.
    #[test]
    fn bounces_and_merge_stats_reach_the_village_json() {
        use super::super::gate::{Bounce, BounceKind, Hotspot, MergeStats};
        let mut v = build(&swarm(vec![]), &[], Pipeline::default());
        let quiet = serde_json::to_value(&v).unwrap();
        assert!(quiet["pipeline"].get("bounces").is_none() && quiet["pipeline"].get("merge_stats").is_none());
        v.pipeline.bounces.insert(9, vec![Bounce { at: Some("2026-11-10T01:00:00Z".into()), kind: BounceKind::Conflict, files: vec!["src/web/server.rs".into()] }]);
        v.pipeline.merge_stats = Some(MergeStats { landed_today: 3, bounced_today: 1, hotspots: vec![Hotspot { file: "src/web/server.rs".into(), count: 1 }] });
        let j = serde_json::to_value(&v).unwrap();
        assert_eq!(j["pipeline"]["bounces"]["9"][0]["kind"], "conflict");
        assert_eq!(j["pipeline"]["bounces"]["9"][0]["files"][0], "src/web/server.rs");
        assert_eq!(j["pipeline"]["merge_stats"]["bounced_today"], 1);
        assert_eq!(j["pipeline"]["merge_stats"]["hotspots"][0]["count"], 1);
        let back: VillageSnapshot = serde_json::from_value(j).unwrap();
        assert_eq!(back.pipeline.bounces[&9][0].kind, BounceKind::Conflict);
    }

    #[test]
    fn landed_issues_keep_their_titles_from_open_or_closed_issues() {
        let titled = |n: u32, t: &str| IssueSnapshot { number: n, title: t.into(), ..IssueSnapshot::default() };
        let p = Pipeline {
            quay: vec![1, 2, 3],
            stops: vec![Stop { name: "nuc-1".into(), issues: vec![4], ..Stop::default() }],
            ..Pipeline::default()
        };
        let titles = pipeline_titles(&p, &[titled(1, "Still open"), titled(9, "Not landed")], &[titled(2, "Closed when it landed"), titled(4, "Deployed"), titled(3, "")]);
        assert_eq!(titles, [(1, "Still open".to_string()), (2, "Closed when it landed".to_string()), (4, "Deployed".to_string())].into());
        let json = serde_json::to_value(Pipeline { titles, ..Pipeline::default() }).unwrap();
        assert_eq!(json["titles"]["2"], "Closed when it landed");
        assert!(serde_json::to_value(Pipeline::default()).unwrap().get("titles").is_none());
    }

    #[test]
    fn a_house_remembers_since_when_it_has_been_in_its_status() {
        let at = |m: u32| chrono::DateTime::parse_from_rfc3339(&format!("2026-11-09T12:{m:02}:00Z")).unwrap().with_timezone(&chrono::Utc);
        let mut memory = HashMap::new();
        let mut houses = build(&swarm(vec![agent("worker-1", "Working #7", "/wt1"), agent("worker-2", "Idle", "/wt2")]), &[], Pipeline::default()).houses;
        stamp_since(&mut memory, &mut houses, at(0));
        assert_eq!(houses[0].since.as_deref(), Some("2026-11-09T12:00:00Z"));
        // Still working later: the clock keeps its start.
        stamp_since(&mut memory, &mut houses, at(5));
        assert_eq!(houses[0].since.as_deref(), Some("2026-11-09T12:00:00Z"));
        // A new status starts it again.
        houses[1].status = HouseStatus::Blocked;
        stamp_since(&mut memory, &mut houses, at(9));
        assert_eq!((houses[0].since.as_deref(), houses[1].since.as_deref()), (Some("2026-11-09T12:00:00Z"), Some("2026-11-09T12:09:00Z")));
        // A house that went away is forgotten.
        let mut one = vec![houses[0].clone()];
        stamp_since(&mut memory, &mut one, at(10));
        assert_eq!(memory.len(), 1);
    }

    #[test]
    fn a_house_carries_its_worktrees_branch() {
        let mut a = agent("worker-1", "Working #7", "/repos/proj-wt-1");
        a.branch = Some("feature/issue-7".into());
        let v = build(&swarm(vec![a, agent("worker-2", "Idle", "/repos/proj-wt-2")]), &[], Pipeline::default());
        assert_eq!(v.houses[0].branch.as_deref(), Some("feature/issue-7"));
        assert_eq!(v.houses[1].branch, None, "a detached HEAD has no branch");
        let json = serde_json::to_value(&v.houses[0]).unwrap();
        assert_eq!(json["branch"], "feature/issue-7");
        assert!(serde_json::to_value(&v.houses[1]).unwrap().get("branch").is_none());
    }

    fn swarm(workers: Vec<AgentSnapshot>) -> SwarmSnapshot {
        SwarmSnapshot {
            project_name: "proj".to_string(),
            repo_path: "/repos/proj".to_string(),
            repo_key: String::new(),
            manager_by: None,
            agent_type: "Claude".to_string(),
            workflow: None,
            tmux_session: "claude-proj".to_string(),
            stopped: false,
            busy_count: 0,
            idle_count: 0,
            attention_count: 0,
            manager: agent("manager", "Working", "/repos/proj"),
            workers,
            issues: Vec::new(),
        }
    }

    fn labels(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn house_kind_follows_label_precedence() {
        assert_eq!(house_kind(&labels(&["bug", "security", "P0"])), HouseKind::Security);
        assert_eq!(house_kind(&labels(&["bug", "test-failure"])), HouseKind::Tests);
        assert_eq!(house_kind(&labels(&["enhancement", "P2"])), HouseKind::Feature);
        assert_eq!(house_kind(&labels(&["Bug"])), HouseKind::Bug);
        assert_eq!(house_kind(&labels(&["P3"])), HouseKind::Workshop);
    }

    #[test]
    fn agent_status_prefers_waiting_then_health_then_state() {
        let mut a = agent("worker-1", "Working #4", "/wt1");
        assert_eq!(agent_status(&a), HouseStatus::Working);
        a.health = "Stalled".to_string();
        assert_eq!(agent_status(&a), HouseStatus::Stalled);
        a.waiting_for_input = true;
        assert_eq!(agent_status(&a), HouseStatus::Blocked);
        assert_eq!(agent_status(&agent("w", "Idle", "")), HouseStatus::Idle);
        assert_eq!(agent_status(&agent("w", "Done: merged", "")), HouseStatus::Done);
        assert_eq!(agent_status(&agent("w", "Starting", "")), HouseStatus::Building);
        assert_eq!(agent_status(&agent("w", "???", "")), HouseStatus::Unknown);
    }

    #[test]
    fn agents_sharing_a_worktree_are_one_house() {
        let mut lead = agent("worker-1", "Working #7", "/wt1");
        lead.current_issue = Some(7);
        let helper = agent("worker-1b", "Idle", "/wt1");
        let other = agent("worker-2", "Idle", "/wt2");
        let v = build(&swarm(vec![lead, helper, other]), &[issue(7, &["bug", "P1", "working"])], Pipeline::default());
        assert_eq!(v.houses.len(), 2);
        assert_eq!(v.houses[0].crew, vec!["worker-1", "worker-1b"]);
        assert_eq!(v.houses[0].status, HouseStatus::Working);
        assert_eq!(v.houses[0].kind, HouseKind::Bug);
        assert_eq!(v.houses[0].priority.as_deref(), Some("P1"));
        assert_eq!(v.houses[1].kind, HouseKind::Cottage);
    }

    #[test]
    fn one_blocked_agent_blocks_the_house() {
        let a = agent("worker-1", "Working", "/wt1");
        let mut b = agent("worker-1b", "Working", "/wt1");
        b.waiting_for_input = true;
        let v = build(&swarm(vec![a, b]), &[], Pipeline::default());
        assert_eq!(v.houses[0].status, HouseStatus::Blocked);
    }

    #[test]
    fn issue_number_falls_back_to_state_text() {
        let a = agent("worker-1", "Working #42", "/wt1");
        let v = build(&swarm(vec![a]), &[issue(42, &["enhancement"])], Pipeline::default());
        assert_eq!(v.houses[0].issue, Some(42));
        assert_eq!(v.houses[0].kind, HouseKind::Feature);
    }

    #[test]
    fn unknown_issue_is_a_workshop() {
        let mut a = agent("worker-1", "Working", "/wt1");
        a.current_issue = Some(99);
        let v = build(&swarm(vec![a]), &[], Pipeline::default());
        assert_eq!(v.houses[0].kind, HouseKind::Workshop);
    }

    #[test]
    fn issues_sort_into_backlog_board_and_houses() {
        let mut a = agent("worker-1", "Working", "/wt1");
        a.current_issue = Some(1);
        let issues = vec![
            issue(1, &["bug", "working"]),
            issue(2, &["bug", "P2"]),
            issue(3, &["enhancement", "P0"]),
            issue(4, &["needs-design"]),
            issue(5, &["proposal"]),
            issue(6, &["working"]),
            issue(7, &["bug", "P3"]),
        ];
        let pipeline = Pipeline { quay: vec![7], ..Pipeline::default() };
        let v = build(&swarm(vec![a]), &issues, pipeline);
        let backlog: Vec<u32> = v.backlog.iter().map(|c| c.number).collect();
        assert_eq!(backlog, vec![3, 2], "P0 first, claimed and in-transit excluded");
        let board: Vec<(u32, &str)> = v.board.iter().map(|n| (n.number, n.label.as_str())).collect();
        assert_eq!(board, vec![(4, "needs-design"), (5, "proposal")]);
    }

    #[test]
    fn manager_is_the_hall() {
        let v = build(&swarm(vec![]), &[], Pipeline::default());
        assert_eq!(v.manager.role, "manager");
        assert_eq!(v.manager.status, HouseStatus::Working);
        assert!(v.houses.is_empty());
    }

    #[test]
    fn parses_environments_with_default_refs() {
        let envs = parse_environments(
            r#"{"environments":[{"name":"staging","ref":"env/staging","requires":"green"},{"name":"prod","requires":"staging"}]}"#,
        );
        assert_eq!(
            envs,
            vec![
                Environment { name: "staging".into(), branch: "env/staging".into(), requires: Some("green".into()), sources: vec![] },
                Environment { name: "prod".into(), branch: "env/prod".into(), requires: Some("staging".into()), sources: vec![] },
            ]
        );
        assert!(parse_environments("").is_empty());
        assert!(parse_environments(r#"{"issueSource":"github"}"#).is_empty());
    }

    #[test]
    fn parses_tip_refs() {
        assert_eq!(
            parse_tip_ref("refs/autocoder/tips/master/3f2a9c1-412", "master"),
            Some(("3f2a9c1".to_string(), 412))
        );
        assert_eq!(parse_tip_ref("refs/autocoder/tips/main/3f2a9c1-412", "master"), None);
        assert_eq!(parse_tip_ref("refs/autocoder/tips/master/zz-412", "master"), None);
        assert_eq!(parse_tip_ref("refs/autocoder/tips/master/abc-", "master"), None);
    }

    fn set(items: &[&str]) -> StopHistory {
        StopHistory { source: StopSource::Branch, commits: Some(items.iter().map(|s| s.to_string()).collect()), ..StopHistory::default() }
    }

    fn deployed(sha: &str, items: &[&str]) -> StopHistory {
        StopHistory { source: StopSource::Deployment, sha: Some(sha.to_string()), ..set(items) }
    }

    fn unknown() -> StopHistory {
        StopHistory { source: StopSource::Unknown, ..StopHistory::default() }
    }

    fn env(name: &str, requires: Option<&str>) -> Environment {
        Environment { name: name.into(), branch: format!("env/{name}"), requires: requires.map(str::to_string), sources: Vec::new() }
    }

    fn shas(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn an_environment_carries_everything_it_runs() {
        // nuc-1 runs c, b, a; release runs a. Cargo is cumulative, unlike `issues`.
        let tips = vec![("d".to_string(), 4), ("c".to_string(), 3), ("b".to_string(), 2), ("a".to_string(), 1)];
        let names = vec!["nuc-1".to_string(), "release".to_string()];
        let p = place_tips(&tips, &names, &[set(&["c", "b", "a"]), set(&["a"])]);
        let cargo = cargo_of(&tips, &p.reached, 2);
        assert_eq!(cargo, vec![(vec![3, 2, 1], 0), (vec![1], 0)]);
        assert_eq!((p.stops[0].issues.clone(), p.stops[1].issues.clone()), (vec![3, 2], vec![1]));
        // Past CARGO_KEEP the rest are counted, newest kept.
        let many: Vec<(String, u32)> = (0..50).map(|k| (format!("s{k}"), 100 + k)).collect();
        let reached: HashMap<u32, usize> = many.iter().map(|(_, n)| (*n, 1)).collect();
        let (kept, more) = &cargo_of(&many, &reached, 1)[0];
        assert_eq!((kept.len(), *more, kept[0]), (CARGO_KEEP, 10, 100));
    }

    #[test]
    fn a_promotion_carries_what_is_new_to_the_target() {
        // release ran a; the promotion deploys what nuc-1 runs (c, b, a): it carries 3 and 2.
        let tips = vec![("d".to_string(), 4), ("c".to_string(), 3), ("b".to_string(), 2), ("a".to_string(), 1)];
        assert_eq!(voyage_issues(&tips, &shas(&["c", "b", "a"]), Some(&shas(&["a"]))), vec![3, 2]);
        // A first deployment carries everything in it.
        assert_eq!(voyage_issues(&tips, &shas(&["b", "a"]), None), vec![2, 1]);
        // An issue with two tips is carried once.
        let twice = vec![("c".to_string(), 3), ("b".to_string(), 3), ("a".to_string(), 1)];
        assert_eq!(voyage_issues(&twice, &shas(&["c", "b", "a"]), None), vec![3, 1]);
    }

    #[test]
    fn a_voyage_sails_from_the_environment_it_requires() {
        let envs = vec![env("nuc-1", Some("integration")), env("release", Some("nuc-1")), env("canary", None), env("odd", Some("green"))];
        let from: Vec<String> = envs.iter().map(|e| voyage_from(e, &envs)).collect();
        assert_eq!(from, vec!["dev", "nuc-1", "dev", "dev"]);
        // A requires naming nothing configured (or itself) sails from the dev village.
        assert_eq!(voyage_from(&env("x", Some("x")), &[env("x", Some("x"))]), "dev");
    }

    #[test]
    fn health_says_failed_unknown_or_ok() {
        use super::super::deployments::VoyageState;
        let v = |state| Voyage { to: "release".into(), state, sha: "9b1e4401234".into(), url: Some("https://x/log".into()), ..Voyage::default() };
        let failed = health_of(StopSource::Deployment, Some(&v(VoyageState::Failed)));
        assert_eq!((failed.state, failed.reason.as_deref(), failed.url.as_deref()), (HealthState::Failed, Some("deployment of 9b1e440 failed"), Some("https://x/log")));
        assert_eq!(health_of(StopSource::Deployment, Some(&v(VoyageState::Sailing))).state, HealthState::Ok, "in flight is not a failure");
        assert_eq!(health_of(StopSource::Unknown, None).state, HealthState::Unknown);
        assert_eq!(health_of(StopSource::Release, None).state, HealthState::Ok);
    }

    #[test]
    fn docked_voyages_are_reported_for_an_hour_others_until_superseded() {
        use super::super::deployments::VoyageState;
        let now = chrono::DateTime::parse_from_rfc3339("2026-11-09T12:00:00Z").unwrap().with_timezone(&chrono::Utc);
        let at = |state, finished: &str| Voyage { state, started_at: "2026-11-09T09:00:00Z".into(), finished_at: Some(finished.into()), ..Voyage::default() };
        assert!(voyage_current(&at(VoyageState::Docked, "2026-11-09T11:30:00Z"), now));
        assert!(!voyage_current(&at(VoyageState::Docked, "2026-11-09T10:30:00Z"), now));
        assert!(voyage_current(&at(VoyageState::Failed, "2026-11-01T10:30:00Z"), now), "a failure stays until a newer deployment");
        assert!(voyage_current(&Voyage { state: VoyageState::Sailing, ..Voyage::default() }, now));
    }

    #[test]
    fn the_environment_row_serializes_as_the_design_note_says() {
        use super::super::deployments::VoyageState;
        let p = Pipeline {
            stops: vec![
                Stop { name: "master-green".into(), kind: StopKind::Gate, ..Stop::default() },
                Stop {
                    name: "release".into(),
                    tag: Some("v0.6.0".into()),
                    deployed_at: Some("2026-11-09T17:40:02Z".into()),
                    deployed_by: Some("laird".into()),
                    cargo: vec![362, 361],
                    cargo_more: 41,
                    health: Some(Health::default()),
                    ..Stop::default()
                },
            ],
            voyages: vec![Voyage { from: "nuc-1".into(), to: "release".into(), state: VoyageState::Sailing, sha: "c0ffee1".into(), issues: vec![363], ..Voyage::default() }],
            ..Pipeline::default()
        };
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(json["stops"][0]["kind"], "gate");
        assert_eq!(json["stops"][1]["kind"], "environment");
        assert_eq!(json["stops"][1]["cargo"], serde_json::json!([362, 361]));
        assert_eq!(json["stops"][1]["cargo_more"], 41);
        assert_eq!(json["stops"][1]["health"], serde_json::json!({"state": "ok"}));
        assert_eq!(json["stops"][1]["deployed_by"], "laird");
        assert_eq!(json["voyages"][0], serde_json::json!({"from": "nuc-1", "to": "release", "state": "sailing", "sha": "c0ffee1", "started_at": "", "issues": [363]}));
        assert!(json["stops"][0].get("cargo").is_none() && json["stops"][0].get("health").is_none(), "empty fields stay out");
        let back: Pipeline = serde_json::from_value(json).unwrap();
        assert_eq!(back.voyages, p.voyages);
        // An older snapshot without the new fields still reads: every stop an environment.
        let old: Stop = serde_json::from_str(r#"{"name": "prod", "issues": [1]}"#).unwrap();
        assert_eq!((old.kind, old.cargo.len(), old.health), (StopKind::Environment, 0, None));
    }

    #[test]
    fn tips_stop_at_the_furthest_consecutive_stop() {
        let tips = vec![
            ("d".to_string(), 4),
            ("c".to_string(), 3),
            ("b".to_string(), 2),
            ("a".to_string(), 1),
        ];
        let names = vec!["master-green".to_string(), "staging".to_string(), "prod".to_string()];
        let stops = vec![set(&["c", "b", "a"]), set(&["b", "a"]), set(&["a"])];
        let Placement { quay, stops: placed, reached } = place_tips(&tips, &names, &stops);
        assert_eq!(quay, vec![4]);
        assert_eq!(placed[0].issues, vec![3]);
        assert_eq!(placed[1].issues, vec![2]);
        assert_eq!(placed[2].issues, vec![1]);
        assert!(placed.iter().all(|s| s.source == StopSource::Branch && s.sha.is_none()));
        assert_eq!(reached, HashMap::from([(4, 0), (3, 1), (2, 2), (1, 3)]));
    }

    #[test]
    fn a_deployment_places_tips_by_what_it_runs() {
        // staging's branch has c, but what is deployed there is b.
        let tips = vec![("c".to_string(), 3), ("b".to_string(), 2), ("a".to_string(), 1)];
        let names = vec!["master-green".to_string(), "staging".to_string(), "prod".to_string()];
        let stops = vec![set(&["c", "b", "a"]), deployed("b", &["b", "a"]), set(&["a"])];
        let p = place_tips(&tips, &names, &stops);
        assert_eq!(p.stops[0].issues, vec![3]);
        assert_eq!(p.stops[1].issues, vec![2]);
        assert_eq!(p.stops[1].source, StopSource::Deployment);
        assert_eq!(p.stops[1].sha.as_deref(), Some("b"));
        assert_eq!(p.stops[2].issues, vec![1]);
        assert_eq!(p.stops[2].source, StopSource::Branch);
    }

    fn commits(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    /// A stop placed by a deployment of `sha` whose history is `items`.
    fn dep(sha: &str, items: &[&str]) -> (StopHistory, Option<String>) {
        let found = super::super::deployments::Deployed { sha: sha.into(), ..Default::default() };
        stop_from_hit(Some((EnvSource::Deployment, (Some(found), commits(items)))))
    }

    /// The default chain, with each source answering from `answers`.
    async fn place(answers: Vec<(EnvSource, (Option<super::super::deployments::Deployed>, Vec<String>))>, last: bool) -> (StopHistory, Option<String>) {
        let chain = super::super::deployments::default_chain(last);
        let hit = super::super::deployments::first_hit(&chain, |s| {
            let a = answers.iter().find(|(k, _)| *k == s).map(|(_, v)| v.clone());
            async move { a }
        })
        .await;
        stop_from_hit(hit)
    }

    fn found(sha: &str, tag: Option<&str>) -> Option<super::super::deployments::Deployed> {
        Some(super::super::deployments::Deployed { sha: sha.into(), tag: tag.map(str::to_string), url: None, ..super::super::deployments::Deployed::default() })
    }

    #[tokio::test]
    async fn a_deployment_beats_the_branch() {
        let (stop, head) = place(vec![(EnvSource::Deployment, (found("b", None), commits(&["b", "a"]))), (EnvSource::Branch, (None, commits(&["c", "b", "a"])))], true).await;
        assert_eq!(stop, deployed("b", &["b", "a"]));
        assert_eq!(head.as_deref(), Some("b"));
    }

    #[tokio::test]
    async fn the_branch_is_used_without_a_deployment() {
        let (stop, head) = place(vec![(EnvSource::Branch, (None, commits(&["c", "b"])))], false).await;
        assert_eq!(stop, set(&["c", "b"]));
        assert_eq!(head.as_deref(), Some("c"));
    }

    #[tokio::test]
    async fn neither_deployment_nor_branch_is_unknown() {
        let (stop, head) = place(vec![], true).await;
        assert_eq!(stop, unknown());
        assert_eq!(head, None);
    }

    #[tokio::test]
    async fn the_harbour_falls_back_to_the_latest_release() {
        // No Deployments, a published Release: the last stop is at its commit.
        let answers = vec![(EnvSource::Release, (found("a1b2c3d", Some("v1.2.0")), commits(&["a1b2c3d", "0000000"]))), (EnvSource::Branch, (None, commits(&["fffffff"])))];
        let (stop, head) = place(answers.clone(), true).await;
        assert_eq!(stop.source, StopSource::Release);
        assert_eq!(stop.sha.as_deref(), Some("a1b2c3d"));
        assert_eq!(stop.tag.as_deref(), Some("v1.2.0"));
        assert_eq!(head.as_deref(), Some("a1b2c3d"));
        // Only the last stop looks at Releases by default; others use their branch.
        let (mid, _) = place(answers, false).await;
        assert_eq!(mid.source, StopSource::Branch);
    }

    #[tokio::test]
    async fn a_configured_chain_falls_through_on_a_miss() {
        let envs = parse_environments(r#"{"environments":[{"name":"prod","source":["deployment","tag:v*","url:https://app/version"]}]}"#);
        let chain = envs[0].sources.clone();
        assert_eq!(chain, vec![EnvSource::Deployment, EnvSource::Tag("v*".into()), EnvSource::Url("https://app/version".into())]);
        // The deployment misses; the tag answers, so the url is never asked.
        let mut asked = Vec::new();
        let hit = super::super::deployments::first_hit(&chain, |s| {
            asked.push(s.clone());
            let a = matches!(s, EnvSource::Tag(_)).then(|| (found("abcdef1", Some("v2.0.0")), commits(&["abcdef1"])));
            async move { a }
        })
        .await;
        let (stop, _) = stop_from_hit(hit);
        assert_eq!(stop.source, StopSource::Tag);
        assert_eq!(stop.tag.as_deref(), Some("v2.0.0"));
        assert_eq!(asked.len(), 2);
        // Everything missing is unknown, as before.
        let none = super::super::deployments::first_hit(&chain, |_| async { None::<(Option<super::super::deployments::Deployed>, Vec<String>)> }).await;
        assert_eq!(stop_from_hit(none).0, unknown());
    }

    #[test]
    fn source_is_parsed_from_a_string_or_a_list() {
        let envs = parse_environments(
            r#"{"environments":[{"name":"a"},{"name":"b","source":"release"},{"name":"c","source":["branch","nonsense","tag:"]},{"name":"d","source":"url:ftp://x"}]}"#,
        );
        assert!(envs[0].sources.is_empty(), "unset: the default chain");
        assert_eq!(envs[1].sources, vec![EnvSource::Release]);
        assert_eq!(envs[2].sources, vec![EnvSource::Branch], "unreadable entries are dropped");
        assert!(envs[3].sources.is_empty(), "only http(s) endpoints");
        assert_eq!(super::super::deployments::default_chain(false), vec![EnvSource::Deployment, EnvSource::Branch]);
        assert_eq!(super::super::deployments::default_chain(true), vec![EnvSource::Deployment, EnvSource::Release, EnvSource::Branch]);
        // Each reports as itself.
        let json = serde_json::to_value([StopSource::Release, StopSource::Tag, StopSource::Url]).unwrap();
        assert_eq!(json, serde_json::json!(["release", "tag", "url"]));
    }

    #[test]
    fn an_unknown_environment_halts_tips_and_says_so() {
        let tips = vec![("b".to_string(), 2), ("a".to_string(), 1)];
        let names = vec!["master-green".to_string(), "uat".to_string(), "prod".to_string()];
        let stops = vec![set(&["b", "a"]), unknown(), set(&["a"])];
        let p = place_tips(&tips, &names, &stops);
        assert_eq!(p.stops[0].issues, vec![2, 1], "halted before the unknown stop");
        assert_eq!(p.stops[1].source, StopSource::Unknown);
        assert!(p.stops[2].issues.is_empty());
        assert_eq!(p.reached, HashMap::from([(2, 1), (1, 1)]));
    }

    #[test]
    fn reached_counts_every_delivered_issue_the_castle_drops() {
        let tips: Vec<(String, u32)> = (1..=20).map(|n| (format!("s{n}"), n)).collect();
        let all: Vec<String> = tips.iter().map(|(s, _)| s.clone()).collect();
        let all: Vec<&str> = all.iter().map(String::as_str).collect();
        let names = vec!["master-green".to_string(), "prod".to_string()];
        let p = place_tips(&tips, &names, &[set(&all), set(&all)]);
        assert_eq!(p.stops[1].issues.len(), DELIVERED_KEEP);
        assert_eq!(p.reached.len(), 20);
        assert!(p.reached.values().all(|&n| n == 2));
    }

    #[test]
    fn parses_promotion_with_a_default() {
        assert_eq!(parse_promotion(r#"{"promotion":"feature"}"#), Promotion::Feature);
        assert_eq!(parse_promotion(r#"{"promotion":"subtask"}"#), Promotion::Subtask);
        assert_eq!(parse_promotion(r#"{"environments":[]}"#), Promotion::Subtask);
        assert_eq!(parse_promotion(r#"{"promotion":"sideways"}"#), Promotion::Subtask);
        assert_eq!(parse_promotion(""), Promotion::Subtask);
    }

    #[test]
    fn stop_source_serializes_and_old_stops_still_parse() {
        let stop = Stop { name: "uat".into(), issues: vec![461], source: StopSource::Unknown, ..Stop::default() };
        let json = serde_json::to_value(&stop).unwrap();
        assert_eq!(json, serde_json::json!({"name": "uat", "issues": [461], "source": "unknown", "kind": "environment"}));
        let deployed = Stop { source: StopSource::Deployment, sha: Some("9b1e440".into()), ..stop.clone() };
        assert_eq!(serde_json::to_value(&deployed).unwrap()["sha"], "9b1e440");

        let old: Stop = serde_json::from_str(r#"{"name": "prod", "issues": [7]}"#).unwrap();
        assert_eq!(old.source, StopSource::Branch);
        assert_eq!(old.sha, None);
        let old: Pipeline = serde_json::from_str(r#"{"integration": "main", "sentinel": "green", "quay": [], "stops": []}"#).unwrap();
        assert_eq!(old.promotion, Promotion::Subtask);
    }

    #[test]
    fn build_keeps_stop_sources_promotion_and_reached() {
        let pipeline = Pipeline {
            stops: vec![
                Stop { name: "master-green".into(), issues: vec![], ..Stop::default() },
                Stop { name: "uat".into(), issues: vec![3], source: StopSource::Deployment, sha: Some("abc1234".into()), ..Stop::default() },
                Stop { name: "prod".into(), issues: vec![], source: StopSource::Unknown, ..Stop::default() },
            ],
            promotion: Promotion::Feature,
            reached: HashMap::from([(3, 2)]),
            ..Pipeline::default()
        };
        let v = build(&swarm(vec![]), &[], pipeline.clone());
        // With no cargo given, each stop runs its own issues and every later stop's.
        let mut want = pipeline.stops.clone();
        want[0].cargo = vec![3];
        want[1].cargo = vec![3];
        assert_eq!(v.pipeline.stops, want);
        assert_eq!(v.pipeline.promotion, Promotion::Feature);
        assert_eq!(v.pipeline.reached, pipeline.reached);
    }

    #[test]
    fn missing_ref_halts_tips_and_issues_appear_once() {
        let tips = vec![("b".to_string(), 7), ("a".to_string(), 7), ("c".to_string(), 8)];
        let names = vec!["master-green".to_string(), "staging".to_string()];
        let stops = vec![unknown(), set(&["a", "b", "c"])];
        let Placement { quay, stops: placed, .. } = place_tips(&tips, &names, &stops);
        assert_eq!(quay, vec![7, 8], "no green ref: nothing passes the sentinel");
        assert!(placed.iter().all(|s| s.issues.is_empty()));
    }

    /// This repo's own `.autocoder.json`: no sentinel, an install on nuc-1,
    /// then GitHub Releases.
    const NO_SENTINEL: &str = r#"{"issueSource":"github","environments":[
        {"name":"nuc-1","requires":"integration"},
        {"name":"release","requires":"nuc-1"}]}"#;

    #[test]
    fn the_repo_config_has_no_sentinel_and_two_environments() {
        let config = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(".autocoder.json")).unwrap();
        let envs = parse_environments(&config);
        let names: Vec<_> = envs.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["nuc-1", "release"]);
        assert!(gate_absent(&envs));
        assert_eq!(envs, parse_environments(NO_SENTINEL));
    }

    #[test]
    fn the_gate_is_absent_only_when_the_first_environment_follows_integration() {
        assert!(gate_absent(&parse_environments(NO_SENTINEL)));
        assert!(!gate_absent(&[]));
        assert!(!gate_absent(&parse_environments(r#"{"environments":[{"name":"prod","requires":"green"}]}"#)));
        assert!(!gate_absent(&parse_environments(r#"{"environments":[{"name":"prod"}]}"#)));
        // Only the first stop decides: a later one following integration is a typo, not "no sentinel".
        assert!(!gate_absent(&parse_environments(
            r#"{"environments":[{"name":"dev"},{"name":"prod","requires":"integration"}]}"#
        )));
    }

    #[test]
    fn sentinel_gate_states() {
        let envs = parse_environments(r#"{"environments":[{"name":"prod"}]}"#);
        let tip = "t".to_string();
        let (state, stop) = sentinel_gate(&envs, None, Some(&tip));
        assert_eq!(state, "unknown");
        assert_eq!(stop.map(|s| s.commits), Some(None), "a missing green ref is an unknown stop, not no stop");
        assert_eq!(sentinel_gate(&envs, Some(vec!["t".into(), "s".into()]), Some(&tip)).0, "green");
        assert_eq!(sentinel_gate(&envs, Some(vec!["s".into()]), Some(&tip)).0, "pending");
        let (state, stop) = sentinel_gate(&parse_environments(NO_SENTINEL), Some(vec!["t".into()]), Some(&tip));
        assert_eq!((state.as_str(), stop), ("absent", None));
    }

    #[test]
    fn without_a_sentinel_deployed_work_moves_past_the_quay() {
        // Newest first: #9 landed after the nuc-1 install, #8 and #7 before it,
        // and the release was cut before #8.
        let tips = vec![("c".to_string(), 9), ("b".to_string(), 8), ("a".to_string(), 7)];
        let envs = parse_environments(NO_SENTINEL);
        let (sentinel, gate) = sentinel_gate(&envs, None, Some(&"c".to_string()));
        assert_eq!(sentinel, "absent");
        assert!(gate.is_none());
        let names: Vec<String> = envs.iter().map(|e| e.name.clone()).collect();
        let (nuc, _) = dep("b", &["b", "a"]);
        let (release, head) = dep("a", &["a"]);
        assert_eq!(head.as_deref(), Some("a"));
        let p = place_tips(&tips, &names, &[nuc, release]);
        assert_eq!(p.quay, vec![9], "landed after the last deploy: still at the quay");
        assert_eq!(p.stops[0].name, "nuc-1");
        assert_eq!(p.stops[0].issues, vec![8]);
        assert_eq!(p.stops[0].source, StopSource::Deployment);
        assert_eq!(p.stops[1].name, "release");
        assert_eq!(p.stops[1].issues, vec![7]);
    }

    #[test]
    fn with_a_sentinel_but_no_green_ref_deployments_still_wait() {
        // The same deployments behind a sentinel that never ran hold everything.
        let tips = vec![("b".to_string(), 8), ("a".to_string(), 7)];
        let envs = parse_environments(r#"{"environments":[{"name":"nuc-1"}]}"#);
        let (_, gate) = sentinel_gate(&envs, None, Some(&"b".to_string()));
        let (nuc, _) = dep("b", &["b", "a"]);
        let p = place_tips(&tips, &["master-green".into(), "nuc-1".into()], &[gate.unwrap(), nuc]);
        assert_eq!(p.quay, vec![8, 7]);
    }

    fn temp_project(name: &str) -> PathBuf {
        let nanos = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("agents-ui-village-{name}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_subagent(project: &Path, session: &str, id: &str, agent_type: &str, finished: bool) {
        let dir = project.join(session).join("subagents");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("agent-{id}.meta.json")),
            format!(r#"{{"agentType":"{agent_type}","description":"do {id}","finished":{finished}}}"#),
        )
        .unwrap();
        std::fs::write(dir.join(format!("agent-{id}.jsonl")), "{}\n").unwrap();
    }

    #[test]
    fn claude_project_dir_name_replaces_non_alphanumerics() {
        assert_eq!(claude_project_dir_name("/home/user/agents-ui"), "-home-user-agents-ui");
        assert_eq!(claude_project_dir_name("/src/my.repo-wt-2"), "-src-my-repo-wt-2");
    }

    #[test]
    fn active_subagents_skips_finished_and_stale_ones() {
        let project = temp_project("subagents");
        write_subagent(&project, "s1", "a", "modernize:coder", false);
        write_subagent(&project, "s1", "b", "Explore", true);
        write_subagent(&project, "s2", "c", "modernize:tester", false);
        let now = SystemTime::now();
        let mut found: Vec<String> = active_subagents(&project, now).into_iter().map(|s| s.agent_type).collect();
        found.sort();
        assert_eq!(found, vec!["modernize:coder", "modernize:tester"]);
        // An hour later every transcript is stale, so nothing is running.
        assert!(active_subagents(&project, now + Duration::from_secs(3600)).is_empty());
        assert!(active_subagents(&project.join("missing"), now).is_empty());
        std::fs::remove_dir_all(&project).ok();
    }

    fn sub(agent_type: &str) -> Subagent {
        Subagent { agent_type: agent_type.to_string(), description: String::new(), status: None, action: None }
    }

    #[test]
    fn subagents_make_an_idle_house_busy_and_shape_it() {
        let v = build(&swarm(vec![agent("worker-1", "Idle", "/wt1")]), &[], Pipeline::default());
        let mut house = v.houses[0].clone();
        apply_subagents(&mut house, vec![sub("modernize:security"), sub("modernize:coder")]);
        assert_eq!(house.status, HouseStatus::Working);
        assert_eq!(house.kind, HouseKind::Security);
        assert_eq!(house.subagents.len(), 2);

        let mut generic = v.houses[0].clone();
        apply_subagents(&mut generic, vec![sub("Explore")]);
        assert_eq!(generic.kind, HouseKind::Workshop);
    }

    #[test]
    fn subagents_keep_blocked_and_issue_shapes() {
        let mut a = agent("worker-1", "Working #5", "/wt1");
        a.waiting_for_input = true;
        let v = build(&swarm(vec![a]), &[issue(5, &["bug"])], Pipeline::default());
        let mut house = v.houses[0].clone();
        apply_subagents(&mut house, vec![sub("modernize:architect")]);
        assert_eq!(house.status, HouseStatus::Blocked, "a person is still needed");
        assert_eq!(house.kind, HouseKind::Bug, "the issue decides the shape");
    }

    #[test]
    fn specialists_map_to_buildings() {
        assert_eq!(specialist_kind("modernize:architect"), Some(HouseKind::Research));
        assert_eq!(specialist_kind("modernize:tester"), Some(HouseKind::Tests));
        assert_eq!(specialist_kind("modernize:documentation"), Some(HouseKind::Docs));
        assert_eq!(specialist_kind("general-purpose"), None);
    }

    #[test]
    fn attach_subagents_finds_sessions_by_worktree_and_infers_modernize() {
        let home = temp_project("home");
        let projects = home.join(".claude").join("projects");
        write_subagent(&projects.join(claude_project_dir_name("/repos/proj")), "s", "m", "modernize:architect", false);
        write_subagent(&projects.join(claude_project_dir_name("/wt1")), "s", "w", "Explore", false);
        let mut v = build(
            &swarm(vec![agent("worker-1", "Idle", "/wt1"), agent("worker-2", "Idle", "/wt2")]),
            &[],
            Pipeline::default(),
        );
        attach_subagents(&mut v, &home, SystemTime::now());
        assert_eq!(v.manager.subagents.len(), 1);
        assert_eq!(v.houses[0].status, HouseStatus::Working);
        assert_eq!(v.houses[1].status, HouseStatus::Idle);
        assert_eq!(v.workflow.as_deref(), Some("Modernize"));
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn transcript_action_follows_the_latest_step() {
        let tool = |name: &str| format!(r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"x"}},{{"type":"tool_use","name":"{name}"}}]}}}}"#);
        let result = r#"{"type":"user","message":{"content":[{"type":"tool_result"}]}}"#;
        let noise = r#"{"type":"attachment"}"#;
        assert_eq!(transcript_action(&format!("{}\n{noise}", tool("Edit"))), Some(Action::Writing));
        assert_eq!(transcript_action(&format!("{}\n{result}", tool("Bash"))), Some(Action::Thinking), "tool done, model's turn");
        assert_eq!(transcript_action(&tool("Grep")), Some(Action::Reading));
        assert_eq!(transcript_action(&tool("WebSearch")), Some(Action::Searching));
        assert_eq!(transcript_action(&tool("Task")), Some(Action::Messaging));
        assert_eq!(transcript_action(r#"{"type":"assistant","message":{"content":[{"type":"text","text":"plan"}]}}"#), Some(Action::Thinking));
        // A truncated first line from reading the tail is skipped.
        assert_eq!(transcript_action(&format!("ol\"}}]}}\n{}", tool("Read"))), Some(Action::Reading));
        assert_eq!(transcript_action(noise), None);
    }

    #[test]
    fn name_action_reads_activity_names() {
        assert_eq!(name_action("call_llm"), Action::Thinking);
        assert_eq!(name_action("SendApprovalEmail"), Action::Messaging);
        assert_eq!(name_action("fetchVendorQuotes"), Action::Searching);
        assert_eq!(name_action("commit_changes"), Action::Writing);
        assert_eq!(name_action("load_contract"), Action::Reading);
        assert_eq!(name_action("run_tests"), Action::Running);
        assert_eq!(name_action("frobnicate"), Action::Running);
    }

    #[test]
    fn sub_agent_actions_come_from_their_transcripts() {
        let project = temp_project("actions");
        write_subagent(&project, "s", "a", "Explore", false);
        std::fs::write(
            project.join("s").join("subagents").join("agent-a.jsonl"),
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Grep"}]}}"#,
        )
        .unwrap();
        std::fs::write(project.join("s.jsonl"), r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash"}]}}"#).unwrap();
        let now = SystemTime::now();
        assert_eq!(active_subagents(&project, now)[0].action, Some(Action::Reading));
        assert_eq!(session_action(&project, now), Some(Action::Running));
        std::fs::remove_dir_all(&project).ok();
    }

    #[test]
    fn village_serializes_lowercase_enums() {
        let v = build(&swarm(vec![agent("worker-1", "Idle", "/wt1")]), &[], Pipeline::default());
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains(r#""kind":"cottage""#));
        assert!(json.contains(r#""status":"idle""#));
    }
}
