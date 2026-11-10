//! Frameworks: the neutral report any agent framework can produce, and the
//! one mapping from it to a village.
//!
//! Every framework says the same few things in its own words. A coding swarm
//! (autocoder, modernize, claude-squad, herdr) has a manager, sessions per
//! worktree and issues; a runtime (Temporal, AgentEx, a LangGraph or CrewAI
//! deployment) has a worker fleet, top-level workflows and their children.
//! A [`Report`] says it once, in neutral words:
//!
//! ```text
//! coordinator   the manager, or the worker fleet       → the town hall or castle
//! workstreams   one per worktree or top-level workflow → houses
//!   agents      the sessions or workflow on it         → the house's crew
//!   helpers     sub-agents or child workflows          → villagers working for it
//! tasks         issues, tickets, failed runs           → crates, notes on the board
//! pipeline      stages work passes on its way out      → the canal and its locks
//! builds        versions the workers run               → what a world matches releases against
//! plane         development or runtime                 → which half of a world it is
//! ```
//!
//! Built-in sources (tmux, herdr, Temporal) build a report in Rust. Anything
//! else prints one as JSON (`agents-village/v1`, see `docs/village-protocol.md`)
//! from a command, a URL or a file, and `--framework` reads it, so a new
//! framework needs no change here.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

use super::source::{Reading, TtlCache, VillageSource};
use super::village::{
    Action, Crate, GateEntry, Health, House, HouseKind, HouseStatus, Note, Pipeline, Promotion, Stop, StopKind, StopSource, Subagent, VillageSnapshot, Voyage,
    house_kind,
};
use super::{IssueSnapshot, SwarmSnapshot};
use crate::transport::ServerTransport;

pub const SCHEMA: &str = "agents-village/v1";

/// Which half of the loop a village is: the swarm that writes the system, or
/// the platform that runs it.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Plane {
    #[default]
    Development,
    Runtime,
}

/// What an agent is doing, in words every framework has.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(from = "String", into = "String")]
pub enum AgentState {
    Starting,
    Working,
    Idle,
    /// Waiting on a person: a permission prompt, a question, an approval.
    Waiting,
    /// Supposed to be working but not moving: no output, retrying.
    Stalled,
    Done,
    Stopped,
    #[default]
    Unknown,
}

impl From<String> for AgentState {
    /// Accepts the words frameworks use: herdr's `blocked`, Temporal's
    /// `RUNNING`, a status file's `Working #412`.
    fn from(s: String) -> Self {
        let s = s.trim().to_ascii_lowercase();
        let first = s.split(|c: char| !c.is_ascii_alphanumeric() && c != '_').next().unwrap_or("");
        // `AWAITING_APPROVAL`, `RUN_STATE_RUNNING`: try the whole word, then its parts.
        std::iter::once(first).chain(first.split('_')).map(Self::word).find(|s| *s != Self::Unknown).unwrap_or(Self::Unknown)
    }
}

impl AgentState {
    fn word(w: &str) -> Self {
        match w {
            "starting" | "start" | "launching" | "pending" | "scheduled" | "restarting" | "building" => Self::Starting,
            "working" | "running" | "busy" | "active" | "thinking" | "executing" | "in_progress" => Self::Working,
            "idle" | "ready" | "available" | "sleeping" => Self::Idle,
            "waiting" | "blocked" | "needs_input" | "input" | "waiting_for_human" | "awaiting" | "approval" | "paused" => Self::Waiting,
            "stalled" | "stuck" | "retrying" | "backoff" => Self::Stalled,
            "done" | "completed" | "complete" | "finished" | "succeeded" | "success" => Self::Done,
            "stopped" | "dead" | "exited" | "terminated" | "cancelled" | "canceled" | "failed" | "timed_out" | "crashed" => Self::Stopped,
            _ => Self::Unknown,
        }
    }
}

impl From<AgentState> for String {
    fn from(s: AgentState) -> String {
        match s {
            AgentState::Starting => "starting",
            AgentState::Working => "working",
            AgentState::Idle => "idle",
            AgentState::Waiting => "waiting",
            AgentState::Stalled => "stalled",
            AgentState::Done => "done",
            AgentState::Stopped => "stopped",
            AgentState::Unknown => "unknown",
        }
        .to_string()
    }
}

impl AgentState {
    pub fn status(self) -> HouseStatus {
        match self {
            Self::Starting => HouseStatus::Building,
            Self::Working => HouseStatus::Working,
            Self::Idle => HouseStatus::Idle,
            Self::Waiting => HouseStatus::Blocked,
            Self::Stalled => HouseStatus::Stalled,
            Self::Done => HouseStatus::Done,
            Self::Stopped => HouseStatus::Stopped,
            Self::Unknown => HouseStatus::Unknown,
        }
    }
}

/// Where to drill in from the picture: an agent's session, an issue, a
/// workflow in its platform's UI, a CI run.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Link {
    pub label: String,
    pub url: String,
}

/// A structured question an agent is waiting on: a numbered menu or a y/n
/// prompt. The village shows its options as buttons; anything open-ended is
/// answered in the agent's session instead.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Question {
    pub prompt: String,
    pub kind: QuestionKind,
    pub options: Vec<QuestionOption>,
    /// The menu option the cursor is on now.
    #[serde(default)]
    pub selected: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuestionKind {
    /// Pick with the arrow keys, confirm with Enter (Claude Code's menus).
    Menu,
    /// Type `y` or `n`, then Enter.
    YesNo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuestionOption {
    /// `1`, `2`, ... for a menu; `y` or `n`.
    pub key: String,
    pub label: String,
}

/// Screen text without terminal escapes: colours (`ESC [ … m`) and titles
/// (`ESC ] … BEL`), as `tmux capture-pane -e` returns them.
fn strip_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(c) = chars.next() {
                    if c == '\u{7}' || (c == '\u{1b}' && chars.peek() == Some(&'\\')) {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Strip a TUI box's borders and a menu's cursor from a screen line.
fn screen_line(line: &str) -> &str {
    line.trim().trim_matches(|c: char| "│|╭╮╰╯─┃".contains(c) || c.is_whitespace())
}

/// `❯ 2. Yes, and don't ask again` → (true, 2, "Yes, and don't ask again").
fn menu_option(line: &str) -> Option<(bool, usize, String)> {
    let l = screen_line(line);
    let (cursor, rest) = match l.strip_prefix('❯').or_else(|| l.strip_prefix('>')) {
        Some(r) => (true, r.trim_start()),
        None => (false, l),
    };
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let label = rest[digits.len()..].strip_prefix(". ")?.trim();
    let label = label.strip_suffix("(esc)").unwrap_or(label).trim();
    if label.is_empty() {
        return None;
    }
    Some((cursor, digits.parse().ok()?, label.to_string()))
}

/// The structured question at the bottom of a pane, if any: a numbered menu
/// under a line ending in `?`, or a `(y/n)` prompt.
pub fn parse_question(pane: &str) -> Option<Question> {
    let pane = strip_escapes(pane);
    let lines: Vec<&str> = pane.lines().map(screen_line).filter(|l| !l.is_empty()).collect();
    let tail = &lines[lines.len().saturating_sub(20)..];
    // A menu: skip up to four hint lines under it, then read options upward.
    for skip in 0..=4.min(tail.len()) {
        let mut options: Vec<(bool, usize, String)> = Vec::new();
        let mut i = tail.len() - skip;
        while i > 0 {
            match menu_option(tail[i - 1]) {
                Some(o) => {
                    options.insert(0, o);
                    i -= 1;
                }
                None => break,
            }
        }
        let numbered = options.iter().enumerate().all(|(k, (_, n, _))| *n == k + 1);
        if options.len() >= 2 && numbered {
            let prompt = tail[i.saturating_sub(3)..i].iter().rev().find(|l| l.ends_with('?'))?;
            return Some(Question {
                prompt: prompt.to_string(),
                kind: QuestionKind::Menu,
                selected: options.iter().position(|(c, _, _)| *c).unwrap_or(0),
                options: options.into_iter().map(|(_, n, label)| QuestionOption { key: n.to_string(), label }).collect(),
            });
        }
    }
    let yn = tail[tail.len().saturating_sub(4)..].iter().rev().find(|l| {
        let l = l.to_ascii_lowercase();
        ["(y/n)", "[y/n]"].iter().any(|p| l.contains(p))
    })?;
    Some(Question {
        prompt: yn.to_string(),
        kind: QuestionKind::YesNo,
        selected: 0,
        options: vec![QuestionOption { key: "y".into(), label: "Yes".into() }, QuestionOption { key: "n".into(), label: "No".into() }],
    })
}

/// The tmux keys that pick option `choice`: arrows to it and Enter for a
/// menu, the letter and Enter for y/n.
pub fn answer_keys(q: &Question, choice: usize) -> Option<Vec<String>> {
    let opt = q.options.get(choice)?;
    Some(match q.kind {
        QuestionKind::YesNo => vec![opt.key.clone(), "Enter".to_string()],
        QuestionKind::Menu => {
            let (key, steps) = if choice >= q.selected { ("Down", choice - q.selected) } else { ("Up", q.selected - choice) };
            std::iter::repeat_n(key.to_string(), steps).chain(std::iter::once("Enter".to_string())).collect()
        }
    })
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Agent {
    /// Role or id: `worker-2`, `modernize:coder`, a child workflow's id.
    pub name: String,
    /// Agent or workflow type, when it differs from the name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default)]
    pub state: AgentState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<Action>,
    /// What it is on, in a few words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Workstream {
    /// Stable key: a worktree path, a workflow id. A house keeps its lot
    /// while its key stays the same.
    pub id: String,
    /// Shown as the house's role; defaults to its first agent's name.
    #[serde(default)]
    pub name: String,
    /// The task it is on, by `Task::id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// Shown in place of the task's title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The building, when the task's labels don't say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<HouseKind>,
    /// The house's state, when the framework knows it better than its agents'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<AgentState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<Action>,
    /// The sessions or workflow working this stream: the crew.
    #[serde(default)]
    pub agents: Vec<Agent>,
    /// Sub-agents or child workflows working for it.
    #[serde(default)]
    pub helpers: Vec<Agent>,
    /// Where to drill in: its session, its workflow page. The task's own
    /// `url` is added after these.
    #[serde(default)]
    pub links: Vec<Link>,
    /// A structured question it is waiting on, with where to send the answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<Question>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer_url: Option<String>,
    /// The branch it works on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    /// In the backlog, free to take.
    #[default]
    Open,
    /// Taken, but by nobody this report shows.
    Claimed,
    /// Waiting on a decision; pinned to the board.
    Blocked,
    /// Failed; pinned to the board.
    Failed,
    Done,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Task {
    /// `412`, `#412`, `PROJ-12`, a workflow id.
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub labels: Vec<String>,
    /// `P0` to `P3`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(default)]
    pub state: TaskState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The task this one is a subtask of, by id. Tasks sharing a parent make
    /// a feature, followed from spec to production.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Where the task is in the merge gate, once its work has left the worker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<super::gate::TaskGate>,
    /// Tasks this one waits on, by id. While any of them is still in the
    /// report and not done, the task is waiting, not ready to be picked up.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Stage {
    pub name: String,
    /// Tasks whose work has reached this stage but not the next.
    #[serde(default)]
    pub tasks: Vec<String>,
    /// `deployment`, `branch` (the default) or `unknown`: what says what this stage runs.
    #[serde(default)]
    pub source: StopSource,
    /// The commit this stage runs, when a deployment names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha: Option<String>,
    /// The release this stage runs, such as `v0.5.0`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// A page about that release, such as its GitHub Release.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// `environment` (the default: a village of its own in the environment
    /// row) or `gate` (a check such as a green ref, drawn as a lock).
    #[serde(default)]
    pub kind: StopKind,
    /// When the running version was deployed, RFC 3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployed_by: Option<String>,
    /// The page listing this stage's deployments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment_url: Option<String>,
    /// Every task the stage runs, newest first. When no stage gives one, the
    /// village works it out from the stages' `tasks`: a stage runs its own
    /// tasks and every later stage's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cargo: Vec<String>,
    /// More tasks it runs, cut from `cargo`.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub cargo_more: usize,
    /// `{state: ok | failed | incident | unknown, reason, url}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<Health>,
}

fn is_zero_usize(n: &usize) -> bool {
    *n == 0
}

/// One deployment to one stage, as a ship between two villages.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ReportVoyage {
    /// A stage's name, or `dev` for the village that builds the work.
    pub from: String,
    pub to: String,
    /// `loading`, `sailing`, `docked` or `failed`.
    pub state: super::deployments::VoyageState,
    #[serde(default)]
    pub sha: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default)]
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The tasks it carries: new to the target.
    #[serde(default)]
    pub tasks: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tasks_more: bool,
}

/// The cargo of stage `i` when a report gives none: its own tasks and every
/// later stage's (work only reaches a stage after the ones before it), in
/// stage order from the furthest, once each.
fn derived_cargo(stages: &[Stage], i: usize) -> Vec<u32> {
    let mut out = Vec::new();
    for s in stages[i..].iter().rev() {
        for n in s.tasks.iter().map(|t| task_number(t)) {
            if !out.contains(&n) {
                out.push(n);
            }
        }
    }
    out
}

/// A URL a report gives, if it is a web link.
fn web_url(u: &Option<String>) -> Option<String> {
    u.clone().filter(|u| u.starts_with("https://") || u.starts_with("http://"))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ReportPipeline {
    /// The branch or queue finished work lands on.
    #[serde(default)]
    pub integration: String,
    /// `green`, `pending` or `unknown`: whether the gate has passed the latest
    /// work. `absent` when there is no gate, so the first stage takes work
    /// straight from the integration branch.
    #[serde(default)]
    pub gate: String,
    /// Landed, not yet through the first stage.
    #[serde(default)]
    pub landed: Vec<String>,
    #[serde(default)]
    pub stages: Vec<Stage>,
    /// The version at the last stage: what is released.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
    /// `subtask` (the default) or `feature`: whether a feature's tasks move
    /// through the stages one by one or together.
    #[serde(default)]
    pub promotion: Promotion,
    /// The merge gate on the host that lands the work: its slots and trains.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge: Option<super::gate::MergeGate>,
    /// Deployments between the stages, newest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub voyages: Vec<ReportVoyage>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Report {
    #[serde(default = "schema")]
    pub schema: String,
    pub project: String,
    #[serde(default)]
    pub plane: Plane,
    /// `autocoder`, `modernize`, `herdr`, `temporal`, `agentex`, ...
    #[serde(default)]
    pub framework: String,
    /// A finer name for what the swarm runs, e.g. `Modernize`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<String>,
    /// Where it lives: a repository path, a server URL.
    #[serde(default)]
    pub location: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinator: Option<Workstream>,
    #[serde(default)]
    pub workstreams: Vec<Workstream>,
    #[serde(default)]
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub pipeline: ReportPipeline,
    #[serde(default)]
    pub builds: Vec<String>,
}

fn schema() -> String {
    SCHEMA.to_string()
}

// ---------------------------------------------------------------------------
// Report → village
// ---------------------------------------------------------------------------

/// A task id as the number crates and notes are keyed by: the digits of
/// `412` or `#412`, otherwise a stable hash of the id.
pub fn task_number(id: &str) -> u32 {
    let digits = id.trim().trim_start_matches('#');
    if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(n) = digits.parse() {
            return n;
        }
    }
    id.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619))
}

/// The status a whole crew shows: anyone waiting wins, then any work at all.
pub fn crew_status(statuses: &[HouseStatus]) -> HouseStatus {
    const ORDER: &[HouseStatus] = &[
        HouseStatus::Blocked,
        HouseStatus::Stalled,
        HouseStatus::Working,
        HouseStatus::Building,
        HouseStatus::Done,
        HouseStatus::Idle,
        HouseStatus::Stopped,
    ];
    ORDER.iter().find(|s| statuses.contains(s)).copied().unwrap_or(HouseStatus::Unknown)
}

/// A specialist sub-agent's building, from its type (`modernize:security`).
pub fn specialist_kind(agent_type: &str) -> Option<HouseKind> {
    let t = agent_type.to_ascii_lowercase();
    let role = t.rsplit(':').next().unwrap_or(&t);
    [
        ("architect", HouseKind::Research),
        ("security", HouseKind::Security),
        ("test", HouseKind::Tests),
        ("doc", HouseKind::Docs),
        ("coder", HouseKind::Feature),
    ]
    .iter()
    .find(|(name, _)| role.contains(name))
    .map(|(_, kind)| *kind)
}

fn priority(p: &Option<String>) -> Option<String> {
    p.as_ref().filter(|p| p.starts_with('P')).cloned()
}

fn priority_rank(p: &Option<String>) -> u8 {
    p.as_deref().and_then(|p| p.strip_prefix('P')).and_then(|n| n.parse().ok()).unwrap_or(9)
}

fn helper(a: &Agent) -> Subagent {
    Subagent {
        agent_type: a.kind.clone().unwrap_or_else(|| a.name.clone()),
        description: a.detail.clone().unwrap_or_else(|| a.name.clone()),
        status: Some(a.state.status()),
        action: if a.state == AgentState::Waiting { None } else { a.action },
    }
}

fn house(id: String, ws: &Workstream, tasks: &HashMap<&str, &Task>) -> House {
    let found = ws.task.as_deref().and_then(|t| tasks.get(t.trim_start_matches('#')).or_else(|| tasks.get(t)));
    let helper_waiting = ws.helpers.iter().any(|h| h.state == AgentState::Waiting);
    let mut status = match ws.state {
        Some(s) => s.status(),
        None => {
            let derived = crew_status(&ws.agents.iter().map(|a| a.state.status()).collect::<Vec<_>>());
            // A crew with helpers running is busy, whatever its own sessions show.
            if !ws.helpers.is_empty()
                && matches!(derived, HouseStatus::Idle | HouseStatus::Done | HouseStatus::Unknown | HouseStatus::Stalled)
            {
                HouseStatus::Working
            } else {
                derived
            }
        }
    };
    if helper_waiting {
        status = HouseStatus::Blocked;
    }
    let kind = match (&ws.task, found, ws.kind) {
        (Some(_), Some(t), _) => house_kind(&t.labels),
        (_, _, Some(hint)) => hint,
        (Some(_), None, None) => HouseKind::Workshop,
        (None, _, None) if !ws.helpers.is_empty() => {
            ws.helpers.iter().find_map(|h| specialist_kind(h.kind.as_deref().unwrap_or(&h.name))).unwrap_or(HouseKind::Workshop)
        }
        (None, _, None) => HouseKind::Cottage,
    };
    let crew: Vec<String> = ws.agents.iter().map(|a| a.name.clone()).collect();
    let mut links = ws.links.clone();
    if let Some(url) = found.and_then(|t| t.url.clone()) {
        links.push(Link { label: task_link_label(&url, ws.task.as_deref().unwrap_or_default()), url });
    }
    House {
        id,
        role: if ws.name.is_empty() { crew.first().cloned().unwrap_or_default() } else { ws.name.clone() },
        kind,
        status,
        issue: ws.task.as_deref().map(task_number),
        issue_title: ws.title.clone().or_else(|| found.map(|t| t.title.clone())),
        priority: found.and_then(|t| priority(&t.priority)),
        crew: if crew.is_empty() { vec![ws.name.clone()] } else { crew },
        subagents: ws.helpers.iter().map(helper).collect(),
        action: if status == HouseStatus::Working { ws.action } else { None },
        links,
        question: if status == HouseStatus::Blocked { ws.question.clone() } else { None },
        answer_url: ws.answer_url.clone(),
        branch: ws.branch.clone(),
        since: None,
    }
}

/// "GitHub issue #43" for a GitHub URL, "Issue PROJ-12" otherwise.
fn task_link_label(url: &str, id: &str) -> String {
    let id = id.trim_start_matches('#');
    let numeric = !id.is_empty() && id.chars().all(|c| c.is_ascii_digit());
    match (url.contains("github.com"), numeric) {
        (true, true) => format!("GitHub issue #{id}"),
        (_, true) => format!("Issue #{id}"),
        _ => format!("Issue {id}"),
    }
}

/// The note label a blocked or failed task is pinned with.
fn note_label(t: &Task) -> String {
    // A gate that handed the work back says why, ahead of any label.
    match t.gate.as_ref().map(|g| g.state) {
        Some(super::gate::GateState::Conflict) => return "conflict".to_string(),
        Some(super::gate::GateState::Failed | super::gate::GateState::RedBase) if t.state == TaskState::Failed => return "failed".to_string(),
        _ => {}
    }
    t.labels
        .iter()
        .find(|l| crate::model::issue::BLOCKING_LABELS.contains(&l.as_str()))
        .cloned()
        .unwrap_or_else(|| if t.state == TaskState::Failed { "failed" } else { "blocked" }.to_string())
}

/// Draw a report as a village, with the open tasks as issues for worlds.
pub fn to_village(report: &Report) -> Reading {
    let tasks: HashMap<&str, &Task> = report.tasks.iter().map(|t| (t.id.trim_start_matches('#'), t)).collect();
    let houses: Vec<House> = report.workstreams.iter().map(|ws| house(ws.id.clone(), ws, &tasks)).collect();
    let manager = match &report.coordinator {
        Some(c) => house("manager".to_string(), c, &tasks),
        None => house(
            "manager".to_string(),
            &Workstream { name: "manager".into(), agents: vec![Agent { name: "manager".into(), ..Agent::default() }], ..Workstream::default() },
            &tasks,
        ),
    };

    let p = &report.pipeline;
    let numbers = |ids: &[String]| ids.iter().map(|i| task_number(i)).collect::<Vec<u32>>();
    // The quay is for landed work: a task still in the merge gate isn't there yet.
    let gates: Vec<GateEntry> = report.tasks.iter().filter_map(|t| t.gate.clone().map(|gate| GateEntry { issue: task_number(&t.id), gate })).collect();
    let gating: HashSet<u32> = gates.iter().filter(|g| g.gate.state.in_flight() || g.gate.state.handed_back()).map(|g| g.issue).collect();
    // A report that lists no cargo at all: a stage runs its own tasks and every later stage's.
    let derive_cargo = p.stages.iter().all(|s| s.cargo.is_empty());
    let pipeline = Pipeline {
        integration: p.integration.clone(),
        sentinel: if p.gate.is_empty() { "unknown".to_string() } else { p.gate.clone() },
        quay: numbers(&p.landed).into_iter().filter(|n| !gating.contains(n)).collect(),
        gates,
        merge: p.merge.clone(),
        stops: p
            .stages
            .iter()
            .enumerate()
            .map(|(i, s)| Stop {
                name: s.name.clone(),
                issues: numbers(&s.tasks),
                source: s.source,
                sha: s.sha.clone(),
                tag: s.tag.clone(),
                url: web_url(&s.url),
                kind: s.kind,
                deployed_at: s.deployed_at.clone(),
                deployed_by: s.deployed_by.clone(),
                deployment_url: web_url(&s.deployment_url),
                cargo: if derive_cargo { derived_cargo(&p.stages, i) } else { numbers(&s.cargo) },
                cargo_more: s.cargo_more,
                health: s.health.clone().map(|h| Health { url: web_url(&h.url), ..h }),
            })
            .collect(),
        release: p.release.clone(),
        promotion: p.promotion,
        voyages: p
            .voyages
            .iter()
            .map(|v| Voyage {
                from: v.from.clone(),
                to: v.to.clone(),
                state: v.state,
                sha: v.sha.clone(),
                tag: v.tag.clone(),
                started_at: v.started_at.clone(),
                finished_at: v.finished_at.clone(),
                url: web_url(&v.url),
                issues: numbers(&v.tasks),
                issues_more: v.tasks_more,
            })
            .collect(),
        ..Pipeline::default()
    };

    let in_houses: HashSet<u32> = houses.iter().chain(std::iter::once(&manager)).filter_map(|h| h.issue).collect();
    let in_transit: HashSet<u32> = pipeline.quay.iter().chain(pipeline.stops.iter().flat_map(|s| s.issues.iter())).copied().collect();
    // A dependency is met once its task is done or gone from the report: one
    // that can't be found counts as met, as autocoder's own check does.
    let open: HashSet<u32> = report.tasks.iter().filter(|t| t.state != TaskState::Done).map(|t| task_number(&t.id)).collect();
    let waits_on = |t: &Task| -> Vec<u32> { t.depends_on.iter().map(|d| task_number(d)).filter(|d| open.contains(d) && *d != task_number(&t.id)).collect() };
    let mut backlog = Vec::new();
    let mut board = Vec::new();
    for t in &report.tasks {
        let n = task_number(&t.id);
        if t.state == TaskState::Done || in_houses.contains(&n) || in_transit.contains(&n) {
            continue;
        }
        match t.state {
            TaskState::Blocked | TaskState::Failed => board.push(Note { number: n, title: t.title.clone(), label: note_label(t), url: t.url.clone() }),
            TaskState::Open => backlog.push(Crate { number: n, title: t.title.clone(), kind: house_kind(&t.labels), priority: priority(&t.priority), url: t.url.clone(), waits_on: waits_on(t), parent: t.parent.as_deref().filter(|p| !p.trim().is_empty()).map(task_number) }),
            TaskState::Claimed | TaskState::Done => {}
        }
    }
    backlog.sort_by_key(|c: &Crate| (priority_rank(&c.priority), c.number));
    board.sort_by_key(|n: &Note| n.number);

    let issues = report
        .tasks
        .iter()
        .filter(|t| t.state != TaskState::Done)
        .map(|t| IssueSnapshot {
            number: task_number(&t.id),
            title: t.title.clone(),
            state: "open".to_string(),
            priority: t.priority.clone().unwrap_or_default(),
            labels: t.labels.clone(),
            is_blocked: t.state == TaskState::Blocked,
            is_working: t.state == TaskState::Claimed,
            assigned_worker: None,
            url: t.url.clone().unwrap_or_default(),
            depends_on: t.depends_on.iter().map(|d| task_number(d)).collect(),
        })
        .collect();

    let mut village = VillageSnapshot {
        project: report.project.clone(),
        plane: report.plane,
        framework: report.framework.clone(),
        workflow: report.workflow.clone(),
        repo_path: report.location.clone(),
        manager,
        houses,
        backlog,
        board,
        pipeline,
        builds: report.builds.clone(),
        features: Vec::new(),
    };
    village.features = super::feature::build(&village, &super::feature::links_from_tasks(&report.tasks));
    Reading { village, issues }
}

// ---------------------------------------------------------------------------
// Coding swarms (tmux, herdr) → report
// ---------------------------------------------------------------------------

/// An agent's state from a swarm snapshot: waiting first, then health, then
/// what its pane or status file says.
pub fn swarm_agent_state(agent: &super::AgentSnapshot) -> AgentState {
    if agent.waiting_for_input {
        return AgentState::Waiting;
    }
    match agent.health.as_str() {
        "Dead" => return AgentState::Stopped,
        "Restarting" => return AgentState::Starting,
        "Stalled" => return AgentState::Stalled,
        _ => {}
    }
    let s = agent.state.as_str();
    if s.starts_with("Working") {
        AgentState::Working
    } else if s == "Idle" {
        AgentState::Idle
    } else if s.starts_with("Done") {
        AgentState::Done
    } else if s == "Starting" {
        AgentState::Starting
    } else if s == "Stopped" {
        AgentState::Stopped
    } else {
        AgentState::Unknown
    }
}

fn swarm_issue(agent: &super::AgentSnapshot) -> Option<u32> {
    agent.current_issue.or_else(|| {
        let rest = agent.state.strip_prefix("Working #")?;
        rest.chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()
    })
}

/// The dashboard's view of an agent's live session.
pub fn session_link(project: &str, role: &str) -> Link {
    Link { label: "Agent session".to_string(), url: format!("/#/swarm/{}/agent/{}", url_part(project), url_part(role)) }
}

fn url_part(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

fn swarm_workstream(id: String, crew: &[&super::AgentSnapshot], project: &str) -> Workstream {
    // Only a waiting tmux agent can be answered from the village: herdr panes
    // carry no screen text, and tmux is what the answer endpoint types into.
    let asking = crew.iter().find(|a| a.waiting_for_input).and_then(|a| parse_question(&a.pane_content).map(|q| (a, q)));
    Workstream {
        answer_url: asking.as_ref().map(|(a, _)| format!("/api/swarms/{}/agents/{}/answer", url_part(project), url_part(&a.role))),
        question: asking.map(|(_, q)| q),
        links: crew.first().map(|a| vec![session_link(project, &a.role)]).unwrap_or_default(),
        id,
        name: crew.first().map(|a| a.role.clone()).unwrap_or_default(),
        task: crew.iter().find_map(|a| swarm_issue(a)).map(|n| n.to_string()),
        title: crew.iter().find_map(|a| a.current_issue_title.clone()),
        branch: crew.iter().find_map(|a| a.branch.clone()),
        agents: crew
            .iter()
            .map(|a| Agent { name: a.role.clone(), state: swarm_agent_state(a), ..Agent::default() })
            .collect(),
        ..Workstream::default()
    }
}

/// autocoder's labels for an issue whose fix has left the worker.
pub const HANDED_OFF_LABELS: &[&str] = &["merging", "awaiting-integration"];

/// A coding swarm's report. Agents sharing a worktree are one workstream;
/// `issues` are the swarm's open issues and `pipeline` where pushed work is.
pub fn swarm_report(swarm: &SwarmSnapshot, issues: &[IssueSnapshot], pipeline: &Pipeline) -> Report {
    swarm_report_gated(swarm, issues, pipeline, &super::gate::GateReading::default())
}

/// [`swarm_report`] with what the merge gate's files on this host say.
pub fn swarm_report_gated(swarm: &SwarmSnapshot, issues: &[IssueSnapshot], pipeline: &Pipeline, gate: &super::gate::GateReading) -> Report {
    let (gates, merge) = super::gate::assess(gate, issues, std::time::SystemTime::now());
    let mut order: Vec<String> = Vec::new();
    let mut crews: HashMap<String, Vec<&super::AgentSnapshot>> = HashMap::new();
    for worker in &swarm.workers {
        let key = if worker.worktree_path.is_empty() { worker.role.clone() } else { worker.worktree_path.clone() };
        if !crews.contains_key(&key) {
            order.push(key.clone());
        }
        crews.entry(key).or_default().push(worker);
    }
    let mut coordinator = swarm_workstream("manager".to_string(), &[&swarm.manager], &swarm.project_name);
    // A house's title falls back to its agent's; a found issue's title wins.
    let known: HashSet<u32> = issues.iter().map(|i| i.number).collect();
    let mut workstreams: Vec<Workstream> = order.into_iter().map(|key| { let crew = &crews[&key]; swarm_workstream(key, crew, &swarm.project_name) }).collect();
    for ws in std::iter::once(&mut coordinator).chain(workstreams.iter_mut()) {
        if ws.task.as_deref().and_then(|t| t.parse().ok()).is_some_and(|n: u32| known.contains(&n)) {
            ws.title = None;
        }
    }
    let ids = |v: &[u32]| v.iter().map(u32::to_string).collect::<Vec<_>>();
    // autocoder hands an issue to a merge job (`merging`) and parks it once
    // landed but not yet proven green (`awaiting-integration`): either way it
    // has left the worker and waits at the quay, if git hasn't placed it yet.
    // When the merge job's files say more, they decide: work still in the
    // gate, or handed back by it, has not landed.
    let placed: HashSet<u32> = pipeline.quay.iter().chain(pipeline.stops.iter().flat_map(|s| s.issues.iter())).copied().collect();
    let mut landed = ids(&pipeline.quay);
    landed.extend(
        issues
            .iter()
            .filter(|i| i.state == "open" && !placed.contains(&i.number))
            .filter(|i| match gates.get(&i.number) {
                Some(g) => g.landed,
                None => i.labels.iter().any(|l| HANDED_OFF_LABELS.contains(&l.as_str())),
            })
            .map(|i| i.number.to_string()),
    );
    Report {
        schema: schema(),
        project: swarm.project_name.clone(),
        plane: Plane::Development,
        framework: swarm.workflow.clone().unwrap_or_else(|| "autocoder".to_string()).to_ascii_lowercase(),
        workflow: swarm.workflow.clone(),
        location: swarm.repo_path.clone(),
        coordinator: Some(coordinator),
        workstreams,
        tasks: issues
            .iter()
            .filter(|i| i.state == "open")
            .map(|i| Task {
                id: i.number.to_string(),
                title: i.title.clone(),
                labels: i.labels.clone(),
                priority: Some(i.priority.clone()).filter(|p| !p.is_empty()),
                state: if gates.get(&i.number).is_some_and(|g| g.state.handed_back() || (g.state == super::gate::GateState::RedBase && !g.landed)) {
                    TaskState::Failed
                } else if i.labels.iter().any(|l| crate::model::issue::BLOCKING_LABELS.contains(&l.as_str())) {
                    TaskState::Blocked
                } else if i.is_working || i.labels.iter().any(|l| l == "decomposed") || gates.get(&i.number).is_some_and(|g| g.state.in_flight()) {
                    TaskState::Claimed
                } else {
                    TaskState::Open
                },
                url: Some(i.url.clone()).filter(|u| !u.is_empty()),
                parent: None,
                gate: gates.get(&i.number).cloned(),
                depends_on: i.depends_on.iter().map(u32::to_string).collect(),
            })
            .collect(),
        pipeline: ReportPipeline {
            integration: pipeline.integration.clone(),
            gate: pipeline.sentinel.clone(),
            landed,
            stages: pipeline
                .stops
                .iter()
                .map(|s| Stage {
                    name: s.name.clone(),
                    tasks: ids(&s.issues),
                    source: s.source,
                    sha: s.sha.clone(),
                    tag: s.tag.clone(),
                    url: s.url.clone(),
                    kind: s.kind,
                    deployed_at: s.deployed_at.clone(),
                    deployed_by: s.deployed_by.clone(),
                    deployment_url: s.deployment_url.clone(),
                    cargo: ids(&s.cargo),
                    cargo_more: s.cargo_more,
                    health: s.health.clone(),
                })
                .collect(),
            release: pipeline.release.clone(),
            promotion: pipeline.promotion,
            merge,
            voyages: pipeline
                .voyages
                .iter()
                .map(|v| ReportVoyage {
                    from: v.from.clone(),
                    to: v.to.clone(),
                    state: v.state,
                    sha: v.sha.clone(),
                    tag: v.tag.clone(),
                    started_at: v.started_at.clone(),
                    finished_at: v.finished_at.clone(),
                    url: v.url.clone(),
                    tasks: ids(&v.issues),
                    tasks_more: v.issues_more,
                })
                .collect(),
        },
        builds: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Reports from outside: a command, a URL, a file
// ---------------------------------------------------------------------------

/// Where `--framework` reads reports from.
#[derive(Debug, Clone, PartialEq)]
pub enum ReportSpec {
    /// `exec:<command>`: run it; it prints reports on stdout.
    Command(String),
    /// `http://` or `https://`: GET it.
    Url(String),
    /// `file:<path>`, or a path ending `.json`.
    File(PathBuf),
}

impl ReportSpec {
    pub fn parse(spec: &str) -> Option<Self> {
        let spec = spec.trim();
        if let Some(cmd) = spec.strip_prefix("exec:") {
            return (!cmd.trim().is_empty()).then(|| Self::Command(cmd.trim().to_string()));
        }
        if spec.starts_with("http://") || spec.starts_with("https://") {
            return Some(Self::Url(spec.to_string()));
        }
        if let Some(path) = spec.strip_prefix("file:") {
            return (!path.is_empty()).then(|| Self::File(PathBuf::from(path)));
        }
        spec.ends_with(".json").then(|| Self::File(PathBuf::from(spec)))
    }

    fn label(&self) -> String {
        match self {
            Self::Command(c) => format!("exec:{c}"),
            Self::Url(u) => u.clone(),
            Self::File(p) => format!("file:{}", p.display()),
        }
    }
}

/// Reports from JSON: one report, a list of them, or `{"reports": [...]}`.
pub fn parse_reports(json: &str) -> Result<Vec<Report>, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("not JSON: {e}"))?;
    let list = match v {
        serde_json::Value::Array(items) => items,
        serde_json::Value::Object(ref m) if m.contains_key("reports") => match m.get("reports") {
            Some(serde_json::Value::Array(items)) => items.clone(),
            _ => return Err("\"reports\" is not a list".to_string()),
        },
        other => vec![other],
    };
    list.into_iter()
        .map(|item| {
            let r: Report = serde_json::from_value(item).map_err(|e| format!("not an {SCHEMA} report: {e}"))?;
            if !r.schema.starts_with("agents-village/") {
                return Err(format!("unknown schema {}", r.schema));
            }
            Ok(r)
        })
        .collect()
}

/// How long reports from outside are reused before reading them again.
const REPORT_TTL: Duration = Duration::from_secs(5);

/// Villages from an outside adapter, read through the transport, so a
/// command or file on a `--server` host is read there.
pub struct ReportVillages {
    spec: ReportSpec,
    cache: TtlCache<(), Vec<Reading>>,
}

impl ReportVillages {
    pub fn new(spec: ReportSpec) -> Self {
        Self { spec, cache: TtlCache::new(REPORT_TTL) }
    }

    async fn fetch(&self, transport: &ServerTransport) -> Result<String, String> {
        let (program, args): (&str, Vec<String>) = match &self.spec {
            ReportSpec::Command(c) => ("sh", vec!["-c".to_string(), c.clone()]),
            ReportSpec::Url(u) => ("curl", ["-sS", "--fail", "--max-time", "10", "-H", "Accept: application/json", u].iter().map(|s| s.to_string()).collect()),
            ReportSpec::File(p) => ("cat", vec![p.to_string_lossy().into_owned()]),
        };
        let out = transport.output(program, &args, None).await.map_err(|e| format!("{e:#}"))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(err.lines().last().unwrap_or("failed").trim().to_string());
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Shown in place of the adapter's villages when it can't be read, so a
    /// broken adapter is visible instead of silently missing.
    fn broken(&self, why: &str) -> Reading {
        let mut r = to_village(&Report { project: self.spec.label(), framework: "unknown".into(), ..Report::default() });
        r.village.manager.status = HouseStatus::Unknown;
        r.village.board.push(Note { number: 0, title: format!("Can't read {}: {why}", self.spec.label()), label: "unreachable".to_string(), url: None });
        r
    }
}

impl VillageSource for ReportVillages {
    fn name(&self) -> String {
        format!("reports from {}", self.spec.label())
    }
    fn read<'a>(&'a self, transport: &'a ServerTransport, _: &'a [SwarmSnapshot]) -> BoxFuture<'a, Vec<Reading>> {
        Box::pin(async move {
            self.cache
                .get_or((), || async {
                    match self.fetch(transport).await.and_then(|json| parse_reports(&json)) {
                        Ok(reports) => reports.iter().map(to_village).collect(),
                        Err(why) => vec![self.broken(&why)],
                    }
                })
                .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = include_str!("../../tools/adapters/example-report.json");

    fn agent(name: &str, state: &str) -> Agent {
        Agent { name: name.into(), state: AgentState::from(state.to_string()), ..Agent::default() }
    }

    #[test]
    fn states_accept_every_frameworks_words() {
        for (word, want) in [
            ("blocked", AgentState::Waiting),
            ("waiting_for_human", AgentState::Waiting),
            ("RUNNING", AgentState::Working),
            ("Working #412", AgentState::Working),
            ("COMPLETED", AgentState::Done),
            ("retrying", AgentState::Stalled),
            ("TIMED_OUT", AgentState::Stopped),
            ("pending", AgentState::Starting),
            ("AWAITING_APPROVAL", AgentState::Waiting),
            ("RUN_STATE_RUNNING", AgentState::Working),
            ("whatever", AgentState::Unknown),
        ] {
            assert_eq!(AgentState::from(word.to_string()), want, "{word}");
        }
        assert_eq!(serde_json::to_string(&AgentState::Waiting).unwrap(), "\"waiting\"");
    }

    #[test]
    fn task_numbers_keep_issue_numbers_and_hash_the_rest() {
        assert_eq!(task_number("412"), 412);
        assert_eq!(task_number("#412"), 412);
        assert_eq!(task_number("PROJ-12"), task_number("PROJ-12"));
        assert_ne!(task_number("PROJ-12"), 12);
    }

    #[test]
    fn a_task_with_an_open_dependency_waits_on_it() {
        let report = Report {
            project: "shop".into(),
            tasks: vec![
                // Depends on #8 (open), #10 (done) and #99 (not in the report): only #8 holds it.
                Task { id: "7".into(), title: "Ship it".into(), depends_on: vec!["8".into(), "10".into(), "99".into(), "7".into()], ..Task::default() },
                Task { id: "8".into(), title: "Build it".into(), ..Task::default() },
                Task { id: "10".into(), title: "Plan it".into(), state: TaskState::Done, ..Task::default() },
                Task { id: "11".into(), title: "After the plan".into(), depends_on: vec!["10".into()], ..Task::default() },
            ],
            ..Report::default()
        };
        let v = to_village(&report).village;
        let crate_of = |n: u32| v.backlog.iter().find(|c| c.number == n).unwrap();
        assert_eq!(crate_of(7).waits_on, vec![8], "a done or unknown dependency is met; itself never counts");
        assert!(crate_of(8).waits_on.is_empty());
        assert!(crate_of(11).waits_on.is_empty(), "its only dependency is done");
        // Reported only when there is something to wait on.
        let json = serde_json::to_value(&v.backlog).unwrap();
        assert!(json.as_array().unwrap().iter().filter(|c| c.get("waits_on").is_some()).count() == 1);
        // A swarm's issues carry their body's dependencies into the report.
        let swarm: SwarmSnapshot = serde_json::from_value(serde_json::json!({
            "project_name": "shop", "repo_path": "", "agent_type": "claude", "workflow": null, "tmux_session": "",
            "stopped": false, "busy_count": 0, "idle_count": 0, "attention_count": 0, "workers": [], "issues": [],
            "manager": {"id": "m", "role": "manager", "state": "Idle", "is_manager": true, "waiting_for_input": false,
                "current_issue": null, "current_issue_title": null, "pane_content": "", "tmux_target": "", "health": "Healthy",
                "completed_issue_count": 0, "resurrection_attempts": 0, "status_timestamp": null}
        }))
        .unwrap();
        let issues = [
            IssueSnapshot { number: 5, state: "open".into(), depends_on: vec![4], ..IssueSnapshot::default() },
            IssueSnapshot { number: 4, state: "open".into(), ..IssueSnapshot::default() },
        ];
        let report = swarm_report(&swarm, &issues, &Pipeline::default());
        assert_eq!(report.tasks[0].depends_on, vec!["4".to_string()]);
        let v = to_village(&report).village;
        assert_eq!(v.backlog.iter().find(|c| c.number == 5).unwrap().waits_on, vec![4]);
    }

    #[test]
    fn a_subtask_crate_names_its_parent() {
        let report = Report {
            project: "shop".into(),
            tasks: vec![
                Task { id: "395".into(), title: "Epic".into(), state: TaskState::Claimed, ..Task::default() },
                Task { id: "396".into(), title: "Part".into(), parent: Some("395".into()), ..Task::default() },
                Task { id: "397".into(), title: "Loose".into(), parent: Some(" ".into()), ..Task::default() },
                Task { id: "PROJ-2".into(), title: "Jira part".into(), parent: Some("PROJ-1".into()), ..Task::default() },
            ],
            ..Report::default()
        };
        let v = to_village(&report).village;
        let crate_of = |n: u32| v.backlog.iter().find(|c| c.number == n).unwrap().clone();
        assert_eq!(crate_of(396).parent, Some(395));
        assert_eq!(crate_of(397).parent, None, "a blank parent is none");
        assert_eq!(crate_of(task_number("PROJ-2")).parent, Some(task_number("PROJ-1")));
        // Serialized only when there is one.
        let json = serde_json::to_value(&v.backlog).unwrap();
        let by = |n: u32| json.as_array().unwrap().iter().find(|c| c["number"] == n).unwrap().clone();
        assert_eq!(by(396)["parent"], 395);
        assert!(by(397).get("parent").is_none());
    }

    #[test]
    fn workstreams_become_houses() {
        let report = Report {
            project: "shop".into(),
            tasks: vec![
                Task { id: "7".into(), title: "Fix login".into(), labels: vec!["bug".into()], priority: Some("P1".into()), ..Task::default() },
                Task { id: "8".into(), title: "Dark mode".into(), labels: vec!["enhancement".into()], ..Task::default() },
                Task { id: "9".into(), title: "Pick a queue".into(), labels: vec!["needs-design".into()], state: TaskState::Blocked, ..Task::default() },
                Task { id: "10".into(), title: "Old".into(), state: TaskState::Done, ..Task::default() },
            ],
            workstreams: vec![
                Workstream { id: "/src/shop-wt-1".into(), task: Some("#7".into()), agents: vec![agent("worker-1", "working"), agent("worker-1b", "blocked")], ..Workstream::default() },
                Workstream { id: "/src/shop-wt-2".into(), agents: vec![agent("worker-2", "idle")], helpers: vec![Agent { name: "a1".into(), kind: Some("modernize:security".into()), state: AgentState::Working, ..Agent::default() }], ..Workstream::default() },
                Workstream { id: "wf-1".into(), name: "PaymentWorkflow".into(), kind: Some(HouseKind::Perf), state: Some(AgentState::Stalled), ..Workstream::default() },
            ],
            ..Report::default()
        };
        let r = to_village(&report);
        let v = &r.village;
        assert_eq!(v.houses[0].role, "worker-1");
        assert_eq!((v.houses[0].kind, v.houses[0].status, v.houses[0].issue), (HouseKind::Bug, HouseStatus::Blocked, Some(7)));
        assert_eq!(v.houses[0].issue_title.as_deref(), Some("Fix login"));
        assert_eq!(v.houses[0].priority.as_deref(), Some("P1"));
        assert_eq!((v.houses[1].kind, v.houses[1].status), (HouseKind::Security, HouseStatus::Working), "helpers make an idle house busy and shape it");
        assert_eq!((v.houses[2].kind, v.houses[2].status, v.houses[2].crew.clone()), (HouseKind::Perf, HouseStatus::Stalled, vec!["PaymentWorkflow".to_string()]));
        assert_eq!(v.backlog.iter().map(|c| c.number).collect::<Vec<_>>(), vec![8], "a task in a house is not in the backlog");
        assert_eq!((v.board[0].number, v.board[0].label.as_str()), (9, "needs-design"));
        assert_eq!(r.issues.len(), 3, "done tasks aren't open issues");
        assert_eq!(v.manager.status, HouseStatus::Unknown, "no coordinator: an empty hall");
    }

    #[test]
    fn handed_off_issues_wait_at_the_quay_and_decomposed_ones_leave_the_backlog() {
        let issue = |n: u32, labels: &[&str]| IssueSnapshot {
            number: n,
            title: format!("#{n}"),
            state: "open".into(),
            priority: "P2".into(),
            labels: labels.iter().map(|l| l.to_string()).collect(),
            is_blocked: false,
            is_working: false,
            assigned_worker: None,
            url: String::new(),
            depends_on: Vec::new(),
        };
        let swarm: SwarmSnapshot = serde_json::from_value(serde_json::json!({
            "project_name": "shop", "repo_path": "", "agent_type": "claude", "workflow": null, "tmux_session": "",
            "stopped": false, "busy_count": 0, "idle_count": 0, "attention_count": 0, "workers": [], "issues": [],
            "manager": {"id": "m", "role": "manager", "state": "Idle", "is_manager": true, "waiting_for_input": false,
                "current_issue": null, "current_issue_title": null, "pane_content": "", "tmux_target": "", "health": "Healthy",
                "completed_issue_count": 0, "resurrection_attempts": 0, "status_timestamp": null}
        }))
        .unwrap();
        let issues = vec![issue(1, &["merging"]), issue(2, &["awaiting-integration"]), issue(3, &["decomposed"]), issue(4, &["bug"]), issue(5, &["needs-feedback"])];
        let pipeline = Pipeline { stops: vec![Stop { name: "main-green".into(), issues: vec![2], ..Stop::default() }], ..Pipeline::default() };
        let v = to_village(&swarm_report(&swarm, &issues, &pipeline)).village;
        assert_eq!(v.pipeline.quay, vec![1], "#2 is already placed by git");
        assert_eq!(v.backlog.iter().map(|c| c.number).collect::<Vec<_>>(), vec![4]);
        assert_eq!(v.board.iter().map(|n| (n.number, n.label.as_str())).collect::<Vec<_>>(), vec![(5, "needs-feedback")]);
    }

    #[test]
    fn the_merge_gate_decides_what_is_at_the_quay() {
        use super::super::gate::{GateReading, GateState, Job};
        let issue = |n: u32, labels: &[&str]| IssueSnapshot {
            number: n,
            state: "open".into(),
            labels: labels.iter().map(|l| l.to_string()).collect(),
            url: format!("https://github.com/acme/shop/issues/{n}"),
            depends_on: Vec::new(),
            ..IssueSnapshot::default()
        };
        let swarm: SwarmSnapshot = serde_json::from_value(serde_json::json!({
            "project_name": "shop", "repo_path": "", "agent_type": "claude", "workflow": null, "tmux_session": "",
            "stopped": false, "busy_count": 0, "idle_count": 0, "attention_count": 0, "workers": [], "issues": [],
            "manager": {"id": "m", "role": "manager", "state": "Idle", "is_manager": true, "waiting_for_input": false,
                "current_issue": null, "current_issue_title": null, "pane_content": "", "tmux_target": "", "health": "Healthy",
                "completed_issue_count": 0, "resurrection_attempts": 0, "status_timestamp": null}
        }))
        .unwrap();
        let job = |n: u32, alive: bool, exit: Option<i32>, log: &str| Job { issue: n, alive, exit, log: log.into(), slot: alive.then_some(1), ..Job::default() };
        let reading = GateReading {
            local: true,
            slots: 2,
            busy: vec![1],
            jobs: vec![
                job(1, true, None, "🧪 Gating the integrated tree a: cargo test\n"),
                job(2, true, None, "⏳ All 2 gate slot(s) on this host are busy; waiting for slot 2 (12:00:00)\n"),
                job(3, false, Some(2), "❌ Tests fail on the integrated tree a.\n❌ origin/master passes alone, so this change broke it. NOT pushing.\n"),
                job(4, false, Some(1), "❌ Merge conflict integrating 'master' into feature/issue-4.\n"),
                job(5, false, Some(0), "✅ Issue #5 merged into 'master' (disjoint re-sync; ...)\n"),
            ],
        };
        let issues = vec![
            issue(1, &["merging"]),
            issue(2, &["working"]),
            issue(3, &["needs-clarification"]),
            issue(4, &["needs-clarification"]),
            issue(5, &["awaiting-integration"]),
            issue(6, &["merging"]),
        ];
        let report = swarm_report_gated(&swarm, &issues, &Pipeline::default(), &reading);
        let task = |n: &str| report.tasks.iter().find(|t| t.id == n).unwrap();
        assert_eq!(task("1").gate.as_ref().map(|g| (g.state, g.slot)), Some((GateState::Gating, Some(1))));
        assert_eq!(task("2").state, TaskState::Claimed, "queued for the gate, not in the backlog");
        assert_eq!(task("3").state, TaskState::Failed);
        assert_eq!(report.pipeline.merge.as_ref().map(|m| (m.slots, m.busy.clone())), Some((2, vec![1])));

        let v = to_village(&report).village;
        // Landed work only: #5 landed unproven; #6 has no job here, so its label still counts.
        assert_eq!(v.pipeline.quay, vec![5, 6]);
        let states: Vec<(u32, GateState)> = v.pipeline.gates.iter().map(|g| (g.issue, g.gate.state)).collect();
        assert_eq!(states, vec![(1, GateState::Gating), (2, GateState::Queued), (3, GateState::Failed), (4, GateState::Conflict), (5, GateState::LandedUnproven)]);
        // Handed back: on the board, saying why, with the issue holding the gate's comment.
        assert_eq!(v.board.iter().map(|n| (n.number, n.label.as_str())).collect::<Vec<_>>(), vec![(3, "failed"), (4, "conflict")]);
        assert_eq!(v.pipeline.gates[2].gate.failure_url.as_deref(), Some("https://github.com/acme/shop/issues/3"));
        assert!(v.backlog.is_empty());
        let json = serde_json::to_value(&v.pipeline).unwrap();
        assert_eq!(json["gates"][0], serde_json::json!({"issue": 1, "state": "gating", "landed": false, "slot": 1, "attempt": 1}));
        assert_eq!(json["merge"], serde_json::json!({"slots": 2, "busy": [1]}));

        // An outside report says the same with `gate` on its tasks: in-flight work leaves the quay.
        let outside: Report = serde_json::from_value(serde_json::json!({
            "project": "x", "tasks": [{"id": "9", "gate": {"state": "merging"}}, {"id": "10"}],
            "pipeline": {"landed": ["9", "10"], "merge": {"slots": 1, "busy": [], "trains": [{"id": "t1", "members": ["9"], "state": "merging"}]}}
        }))
        .unwrap();
        let v = to_village(&outside).village;
        assert_eq!(v.pipeline.quay, vec![10]);
        assert_eq!(v.pipeline.merge.unwrap().trains[0].members, vec!["9".to_string()]);
    }

    #[test]
    fn an_outside_report_describes_the_environment_row() {
        let report: Report = serde_json::from_value(serde_json::json!({
            "project": "x",
            "pipeline": {
                "stages": [
                    {"name": "green", "kind": "gate", "tasks": ["5"]},
                    {"name": "staging", "tasks": ["3"], "deployed_at": "2026-11-09T10:00:00Z", "deployed_by": "ci",
                     "deployment_url": "https://ci.example/staging", "health": {"state": "failed", "reason": "smoke test", "url": "javascript:x"}},
                    {"name": "prod", "tasks": ["1", "2"]}
                ],
                "voyages": [{"from": "staging", "to": "prod", "state": "sailing", "sha": "abc1234", "tasks": ["3"], "url": "https://ci.example/run/9"},
                            {"from": "dev", "to": "staging", "state": "failed", "sha": "def5678", "tasks": ["4"], "tasks_more": true}]
            }
        }))
        .unwrap();
        let p = to_village(&report).village.pipeline;
        assert_eq!(p.stops[0].kind, StopKind::Gate);
        assert_eq!(p.stops[1].kind, StopKind::Environment, "the default");
        // No stage gave cargo: each runs its own tasks and every later stage's.
        let cargo: Vec<Vec<u32>> = p.stops.iter().map(|s| s.cargo.clone()).collect();
        assert_eq!(cargo, vec![vec![1, 2, 3, 5], vec![1, 2, 3], vec![1, 2]]);
        assert_eq!((p.stops[1].deployed_by.as_deref(), p.stops[1].deployment_url.as_deref()), (Some("ci"), Some("https://ci.example/staging")));
        let health = p.stops[1].health.clone().unwrap();
        assert_eq!((health.state, health.url), (super::super::village::HealthState::Failed, None), "a page that isn't a web link is dropped");
        let voyages: Vec<(&str, &str, super::super::deployments::VoyageState, Vec<u32>, bool)> =
            p.voyages.iter().map(|v| (v.from.as_str(), v.to.as_str(), v.state, v.issues.clone(), v.issues_more)).collect();
        use super::super::deployments::VoyageState::{Failed, Sailing};
        assert_eq!(voyages, vec![("staging", "prod", Sailing, vec![3], false), ("dev", "staging", Failed, vec![4], true)]);
        // A report that lists cargo keeps it as given.
        let mut given = report.clone();
        given.pipeline.stages[2].cargo = vec!["2".into()];
        assert_eq!(to_village(&given).village.pipeline.stops[2].cargo, vec![2]);
    }

    #[test]
    fn a_release_tag_and_page_reach_the_village() {
        let swarm: SwarmSnapshot = serde_json::from_value(serde_json::json!({
            "project_name": "shop", "repo_path": "", "agent_type": "claude", "workflow": null, "tmux_session": "",
            "stopped": false, "busy_count": 0, "idle_count": 0, "attention_count": 0, "workers": [], "issues": [],
            "manager": {"id": "m", "role": "manager", "state": "Idle", "is_manager": true, "waiting_for_input": false,
                "current_issue": null, "current_issue_title": null, "pane_content": "", "tmux_target": "", "health": "Healthy",
                "completed_issue_count": 0, "resurrection_attempts": 0, "status_timestamp": null}
        }))
        .unwrap();
        let release = Stop {
            name: "release".into(),
            issues: vec![7],
            source: StopSource::Deployment,
            sha: Some("abc1234".into()),
            tag: Some("v0.5.0".into()),
            url: Some("https://github.com/acme/shop/releases/tag/v0.5.0".into()),
            ..Stop::default()
        };
        let voyage = Voyage { from: "nuc-1".into(), to: "release".into(), sha: "abc1234".into(), issues: vec![7], ..Voyage::default() };
        let pipeline = Pipeline {
            stops: vec![Stop { name: "nuc-1".into(), issues: vec![8], cargo: vec![8, 7], ..Stop::default() }, release.clone()],
            voyages: vec![voyage.clone()],
            ..Pipeline::default()
        };
        let report = swarm_report(&swarm, &[], &pipeline);
        // Autocoder's own cargo and voyages go through the report unchanged.
        let through = to_village(&report).village.pipeline;
        assert_eq!((through.stops[0].cargo.clone(), through.voyages.clone()), (vec![8, 7], vec![voyage]));
        assert_eq!(report.pipeline.stages[1].tag.as_deref(), Some("v0.5.0"));
        assert_eq!(to_village(&report).village.pipeline.stops[1], release);
        // An outside adapter's page that isn't a web link is dropped.
        let mut odd = report.clone();
        odd.pipeline.stages[1].url = Some("javascript:alert(1)".into());
        assert_eq!(to_village(&odd).village.pipeline.stops[1].url, None);
    }

    #[test]
    fn houses_link_to_their_session_and_issue() {
        let report = Report {
            project: "shop".into(),
            tasks: vec![
                Task { id: "43".into(), title: "Checkout 500".into(), url: Some("https://github.com/acme/shop/issues/43".into()), ..Task::default() },
                Task { id: "45".into(), title: "Which provider?".into(), labels: vec!["needs-design".into()], state: TaskState::Blocked, url: Some("https://github.com/acme/shop/issues/45".into()), ..Task::default() },
            ],
            workstreams: vec![Workstream { id: "/src/shop-wt-2".into(), task: Some("43".into()), links: vec![session_link("shop", "worker-2")], agents: vec![agent("worker-2", "blocked")], ..Workstream::default() }],
            ..Report::default()
        };
        let v = to_village(&report).village;
        let links: Vec<(&str, &str)> = v.houses[0].links.iter().map(|l| (l.label.as_str(), l.url.as_str())).collect();
        assert_eq!(links, vec![("Agent session", "/#/swarm/shop/agent/worker-2"), ("GitHub issue #43", "https://github.com/acme/shop/issues/43")]);
        assert_eq!(v.board[0].url.as_deref(), Some("https://github.com/acme/shop/issues/45"));
        assert_eq!(session_link("my repo", "worker-1").url, "/#/swarm/my%20repo/agent/worker-1");
    }

    const PERMISSION: &str = "\u{1b}[2m✓ 214 tests passed\u{1b}[0m\n╭──────────────────────────────╮\n│ Bash command                 │\n│   npm publish                │\n│ Do you want to proceed?      │\n│ \u{1b}[36m❯ 1. Yes\u{1b}[0m                    │\n│   2. Yes, and don't ask again │\n│   3. No, and tell Claude what to do differently (esc) │\n╰──────────────────────────────╯\n";

    #[test]
    fn reads_a_numbered_menu_through_colours_and_box_borders() {
        let q = parse_question(PERMISSION).expect("a menu");
        assert_eq!(q.prompt, "Do you want to proceed?");
        assert_eq!(q.kind, QuestionKind::Menu);
        assert_eq!(q.selected, 0);
        let labels: Vec<&str> = q.options.iter().map(|o| o.label.as_str()).collect();
        assert_eq!(labels, vec!["Yes", "Yes, and don't ask again", "No, and tell Claude what to do differently"]);
        assert_eq!(answer_keys(&q, 2).unwrap(), vec!["Down", "Down", "Enter"]);
        assert_eq!(answer_keys(&q, 0).unwrap(), vec!["Enter"]);
        assert!(answer_keys(&q, 3).is_none());
    }

    #[test]
    fn reads_a_yes_no_prompt_and_the_cursor_position() {
        let q = parse_question("Running migration 0042\nApply it now? (y/n) ").expect("y/n");
        assert_eq!((q.kind, q.prompt.as_str()), (QuestionKind::YesNo, "Apply it now? (y/n)"));
        assert_eq!(answer_keys(&q, 1).unwrap(), vec!["n", "Enter"]);
        let q = parse_question("Which store?\n  1. Postgres\n❯ 2. SQLite\n  3. Files\nEnter to select · Esc to cancel").unwrap();
        assert_eq!(q.selected, 1);
        assert_eq!(answer_keys(&q, 0).unwrap(), vec!["Up", "Enter"]);
    }

    #[test]
    fn ignores_lists_that_are_not_questions() {
        assert!(parse_question("Plan:\n1. Read the code\n2. Fix the bug\n3. Test").is_none(), "no question above the list");
        assert!(parse_question("Steps?\n2. second\n3. third").is_none(), "not numbered from 1");
        assert!(parse_question("All 214 tests passed.").is_none());
    }

    #[test]
    fn the_example_report_parses_and_draws() {
        let reports = parse_reports(EXAMPLE).expect("example parses");
        assert_eq!(reports.len(), 2);
        let dev = to_village(&reports[0]).village;
        assert_eq!(dev.plane, Plane::Development);
        // The environment row: a gate, two environments with what they run, and a voyage under sail.
        let kinds: Vec<StopKind> = dev.pipeline.stops.iter().map(|s| s.kind).collect();
        assert_eq!(kinds, vec![StopKind::Gate, StopKind::Environment, StopKind::Environment]);
        assert_eq!((dev.pipeline.stops[2].tag.as_deref(), dev.pipeline.stops[2].deployed_by.as_deref()), (Some("v2.3.0"), Some("deploy-bot")));
        assert_eq!(dev.pipeline.stops[2].cargo, vec![37], "derived: prod runs only what reached it");
        assert_eq!(dev.pipeline.voyages.len(), 1);
        assert_eq!((dev.pipeline.voyages[0].from.as_str(), dev.pipeline.voyages[0].issues.clone()), ("staging", vec![39]));
        assert!(dev.houses.iter().any(|h| h.status == HouseStatus::Blocked));
        let run = to_village(&reports[1]).village;
        assert_eq!(run.plane, Plane::Runtime);
        assert_eq!(run.builds, vec!["1f2e3d4c5b6a"]);
        assert!(!run.manager.subagents.is_empty());
    }

    #[test]
    fn report_tasks_with_a_parent_become_features() {
        let json = r#"{"project":"shop","tasks":[
            {"id":"PROJ-1","title":"Saved carts","state":"claimed"},
            {"id":"PROJ-2","title":"Cart table","parent":"PROJ-1","state":"done"},
            {"id":"PROJ-3","title":"Cart API","parent":"PROJ-1"}
        ],"pipeline":{"stages":[{"name":"prod","tasks":["PROJ-2"]}]}}"#;
        let v = to_village(&parse_reports(json).unwrap()[0]).village;
        assert_eq!(v.features.len(), 1);
        let f = &v.features[0];
        assert_eq!((f.number, f.title.as_str()), (task_number("PROJ-1"), "Saved carts"));
        assert_eq!((f.build.landed, f.build.total), (1, 2));
        assert_eq!(f.stages[0].reached, 1);
        assert!(!f.live);
        // Reports without parents draw no features, and serialize without the field.
        let plain = to_village(&parse_reports(r#"{"project":"a","tasks":[{"id":"1"}]}"#).unwrap()[0]).village;
        assert!(plain.features.is_empty());
        assert!(serde_json::to_value(&plain).unwrap().get("features").is_none());
    }

    #[test]
    fn reports_come_singly_listed_or_wrapped() {
        let one = r#"{"project":"a"}"#;
        assert_eq!(parse_reports(one).unwrap()[0].schema, SCHEMA);
        assert_eq!(parse_reports(r#"[{"project":"a"},{"project":"b"}]"#).unwrap().len(), 2);
        assert_eq!(parse_reports(r#"{"reports":[{"project":"a"}]}"#).unwrap().len(), 1);
        assert!(parse_reports("nope").is_err());
        assert!(parse_reports(r#"{"schema":"other/v9","project":"a"}"#).is_err());
        assert!(parse_reports(r#"{"workstreams":[]}"#).is_err(), "a report names its project");
    }

    #[test]
    fn report_specs() {
        assert_eq!(ReportSpec::parse("exec:./adapter.py --json"), Some(ReportSpec::Command("./adapter.py --json".into())));
        assert_eq!(ReportSpec::parse("https://agentex.local/village"), Some(ReportSpec::Url("https://agentex.local/village".into())));
        assert_eq!(ReportSpec::parse("file:/tmp/r"), Some(ReportSpec::File("/tmp/r".into())));
        assert_eq!(ReportSpec::parse("/tmp/r.json"), Some(ReportSpec::File("/tmp/r.json".into())));
        assert_eq!(ReportSpec::parse("exec:"), None);
        assert_eq!(ReportSpec::parse("shop"), None);
    }

    #[tokio::test]
    async fn a_command_adapter_is_read_and_a_broken_one_shows() {
        let ok = ReportVillages::new(ReportSpec::Command(r#"printf '{"project":"crew","plane":"runtime","workstreams":[{"id":"w","agents":[{"name":"a","state":"blocked"}]}]}'"#.into()));
        let got = ok.read(&ServerTransport::default(), &[]).await;
        assert_eq!(got[0].village.project, "crew");
        assert_eq!(got[0].village.houses[0].status, HouseStatus::Blocked);

        let bad = ReportVillages::new(ReportSpec::Command("echo nope".into()));
        let got = bad.read(&ServerTransport::default(), &[]).await;
        assert_eq!(got[0].village.board[0].label, "unreachable");
    }
}
