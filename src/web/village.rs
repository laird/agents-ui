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
    /// Where each task handed to the merge gate is: queued, gating, failed...
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gates: Vec<GateEntry>,
    /// The host's merge gate: slots, the busy ones, and merge trains.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge: Option<super::gate::MergeGate>,
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
    }
    serde_json::from_str::<Config>(json)
        .map(|c| {
            c.environments
                .into_iter()
                .map(|e| Environment {
                    branch: e.branch.unwrap_or_else(|| format!("env/{}", e.name)),
                    name: e.name,
                    requires: e.requires,
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
    /// The commits the stop contains, or `None` when nothing says (no
    /// deployment, no ref): the stop is unknown.
    pub commits: Option<HashSet<String>>,
}

/// Choose what a stop runs: the deployed commit's history when there is one,
/// else the branch's, else unknown. Histories are newest first; the first
/// element of the chosen one is the stop's head (what it releases).
pub fn stop_history(deployed: Option<(String, Vec<String>)>, branch: Option<Vec<String>>) -> (StopHistory, Option<String>) {
    let (source, sha, commits) = match (deployed, branch) {
        (Some((sha, commits)), _) => (StopSource::Deployment, Some(sha), Some(commits)),
        (None, Some(commits)) => (StopSource::Branch, None, Some(commits)),
        (None, None) => (StopSource::Unknown, None, None),
    };
    let head = commits.as_ref().and_then(|v| v.first().cloned());
    (StopHistory { source, sha, commits: commits.map(|v| v.into_iter().collect()), ..StopHistory::default() }, head)
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
    for env in &envs {
        names.push(env.name.clone());
        // A deployment says what the environment runs; its branch ref is the
        // fallback. A deployed commit this clone hasn't fetched can't place
        // anything, so it falls back too.
        let record = super::deployments::deployed(transport, repo, &env.name).await;
        let deployed = match &record {
            Some(d) => history(d.sha.clone()).await.map(|commits| (d.sha.clone(), commits)),
            None => None,
        };
        let branch = match deployed {
            Some(_) => None,
            None => history(format!("origin/{}", env.branch)).await,
        };
        let (mut stop, head) = stop_history(deployed, branch);
        if stop.source == StopSource::Deployment {
            if let Some(d) = record {
                stop.tag = d.tag;
                stop.url = d.url;
            }
        }
        release = head;
        stops.push(stop);
    }
    let placement = place_tips(&tips, &names, &stops);
    Pipeline {
        integration,
        sentinel,
        quay: placement.quay,
        stops: placement.stops,
        release,
        promotion,
        reached: placement.reached,
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
    extras: TtlCache<PathBuf, (Vec<IssueSnapshot>, Pipeline, Vec<FeatureLink>)>,
    live: super::feature::LiveMemory,
}

impl SwarmVillages {
    pub fn new(issues: Box<dyn IssueTracker>, pipeline: Box<dyn PipelineSource>, activity: Box<dyn ActivitySource>) -> Self {
        Self { issues, pipeline, activity, extras: TtlCache::new(extras_ttl()), live: Default::default() }
    }

    async fn village(&self, transport: &ServerTransport, swarm: &SwarmSnapshot) -> Reading {
        let repo = PathBuf::from(&swarm.repo_path);
        let (fetched, pipeline, links) = if swarm.repo_path.is_empty() {
            (Vec::new(), Pipeline::default(), Vec::new())
        } else {
            self.extras
                .get_or(repo.clone(), || async {
                    // Swarms that bring their own issues (the TUI's) aren't refetched.
                    let issues = match swarm.issues.is_empty() {
                        true => self.issues.issues(transport, &repo, &swarm.project_name).await,
                        false => Vec::new(),
                    };
                    let pipeline = self.pipeline.pipeline(transport, &repo).await;
                    (issues, pipeline, self.issues.features(transport, &repo).await)
                })
                .await
        };
        let issues = if swarm.issues.is_empty() { fetched } else { swarm.issues.clone() };
        // The merge gate's files are on the host that runs the merges: read
        // them only for a local village, and fresh each time (they change by
        // the second while a gate runs).
        let gate = if transport.server().is_none() && !swarm.repo_path.is_empty() {
            let repo = repo.clone();
            tokio::task::spawn_blocking(move || super::gate::read_local(&repo)).await.unwrap_or_default()
        } else {
            super::gate::GateReading::default()
        };
        let mut village = build_gated(swarm, &issues, pipeline, &gate);
        let features = super::feature::build(&village, &links);
        village.features = self.live.keep(&swarm.repo_path, features, std::time::Instant::now());
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
        }
    }

    fn swarm(workers: Vec<AgentSnapshot>) -> SwarmSnapshot {
        SwarmSnapshot {
            project_name: "proj".to_string(),
            repo_path: "/repos/proj".to_string(),
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
                Environment { name: "staging".into(), branch: "env/staging".into(), requires: Some("green".into()) },
                Environment { name: "prod".into(), branch: "env/prod".into(), requires: Some("staging".into()) },
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

    #[test]
    fn a_deployment_beats_the_branch() {
        let (stop, head) = stop_history(Some(("b".into(), commits(&["b", "a"]))), Some(commits(&["c", "b", "a"])));
        assert_eq!(stop, deployed("b", &["b", "a"]));
        assert_eq!(head.as_deref(), Some("b"));
    }

    #[test]
    fn the_branch_is_used_without_a_deployment() {
        let (stop, head) = stop_history(None, Some(commits(&["c", "b"])));
        assert_eq!(stop, set(&["c", "b"]));
        assert_eq!(head.as_deref(), Some("c"));
    }

    #[test]
    fn neither_deployment_nor_branch_is_unknown() {
        let (stop, head) = stop_history(None, None);
        assert_eq!(stop, unknown());
        assert_eq!(head, None);
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
        assert_eq!(json, serde_json::json!({"name": "uat", "issues": [461], "source": "unknown"}));
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
        assert_eq!(v.pipeline.stops, pipeline.stops);
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
        let (nuc, _) = stop_history(Some(("b".into(), vec!["b".into(), "a".into()])), None);
        let (release, head) = stop_history(Some(("a".into(), vec!["a".into()])), None);
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
        let (nuc, _) = stop_history(Some(("b".into(), vec!["b".into(), "a".into()])), None);
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
