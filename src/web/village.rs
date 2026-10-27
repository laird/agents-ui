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
//! Everything here is a pure function of a `SwarmSnapshot`, its issues, and a
//! `Pipeline` read from git, so the renderer never has to infer anything.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::{AgentSnapshot, IssueSnapshot, SwarmSnapshot};
use crate::transport::ServerTransport;

/// How long fetched issues and pipeline refs are reused before refetching.
const EXTRAS_TTL: Duration = Duration::from_secs(45);

/// Commits of history searched when placing a pushed tip on a pipeline stop.
const HISTORY_DEPTH: usize = 400;

/// Delivered crates kept for the castle; older ones only count.
const DELIVERED_KEEP: usize = 12;

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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Crate {
    pub number: u32,
    pub title: String,
    pub kind: HouseKind,
    pub priority: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Note {
    pub number: u32,
    pub title: String,
    /// The blocking label that pinned it, e.g. `needs-design` or `proposal`.
    pub label: String,
}

/// One lock on the canal: issues whose pushed work has reached this stop but
/// not the next one.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Stop {
    pub name: String,
    pub issues: Vec<u32>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VillageSnapshot {
    pub project: String,
    pub repo_path: String,
    pub manager: House,
    pub houses: Vec<House>,
    pub backlog: Vec<Crate>,
    pub board: Vec<Note>,
    pub pipeline: Pipeline,
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
    if agent.waiting_for_input {
        return HouseStatus::Blocked;
    }
    match agent.health.as_str() {
        "Dead" => return HouseStatus::Stopped,
        "Restarting" => return HouseStatus::Building,
        "Stalled" => return HouseStatus::Stalled,
        _ => {}
    }
    let state = agent.state.as_str();
    if state.starts_with("Working") {
        HouseStatus::Working
    } else if state == "Idle" {
        HouseStatus::Idle
    } else if state.starts_with("Done") {
        HouseStatus::Done
    } else if state == "Starting" {
        HouseStatus::Building
    } else if state == "Stopped" {
        HouseStatus::Stopped
    } else {
        HouseStatus::Unknown
    }
}

/// The status a whole crew shows: anyone blocked wins, then any work at all.
fn crew_status(statuses: &[HouseStatus]) -> HouseStatus {
    const ORDER: &[HouseStatus] = &[
        HouseStatus::Blocked,
        HouseStatus::Stalled,
        HouseStatus::Working,
        HouseStatus::Building,
        HouseStatus::Done,
        HouseStatus::Idle,
        HouseStatus::Stopped,
    ];
    ORDER
        .iter()
        .find(|s| statuses.contains(s))
        .copied()
        .unwrap_or(HouseStatus::Unknown)
}

fn issue_number(agent: &AgentSnapshot) -> Option<u32> {
    agent.current_issue.or_else(|| {
        let rest = agent.state.strip_prefix("Working #")?;
        rest.chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .ok()
    })
}

fn priority(issue: &IssueSnapshot) -> Option<String> {
    issue.priority.starts_with('P').then(|| issue.priority.clone())
}

fn priority_rank(p: &Option<String>) -> u8 {
    p.as_deref()
        .and_then(|p| p.strip_prefix('P'))
        .and_then(|n| n.parse().ok())
        .unwrap_or(9)
}

fn house_for(id: String, crew: &[&AgentSnapshot], issues: &HashMap<u32, &IssueSnapshot>) -> House {
    let statuses: Vec<HouseStatus> = crew.iter().map(|a| agent_status(a)).collect();
    let issue = crew.iter().find_map(|a| issue_number(a));
    let found = issue.and_then(|n| issues.get(&n));
    let kind = match (issue, found) {
        (None, _) => HouseKind::Cottage,
        (Some(_), Some(i)) => house_kind(&i.labels),
        (Some(_), None) => HouseKind::Workshop,
    };
    House {
        id,
        role: crew.first().map(|a| a.role.clone()).unwrap_or_default(),
        kind,
        status: crew_status(&statuses),
        issue,
        issue_title: found
            .map(|i| i.title.clone())
            .or_else(|| crew.iter().find_map(|a| a.current_issue_title.clone())),
        priority: found.and_then(|i| priority(i)),
        crew: crew.iter().map(|a| a.role.clone()).collect(),
    }
}

/// Build the village for one swarm.
///
/// `issues` overrides `swarm.issues` when the snapshot came from tmux
/// discovery, which doesn't fetch issues.
pub fn build(swarm: &SwarmSnapshot, issues: &[IssueSnapshot], pipeline: Pipeline) -> VillageSnapshot {
    let by_number: HashMap<u32, &IssueSnapshot> = issues.iter().map(|i| (i.number, i)).collect();

    // Group workers into houses by worktree, keeping first-seen order so a
    // house never changes lot while the village is on screen.
    let mut order: Vec<String> = Vec::new();
    let mut crews: HashMap<String, Vec<&AgentSnapshot>> = HashMap::new();
    for worker in &swarm.workers {
        let key = if worker.worktree_path.is_empty() {
            worker.role.clone()
        } else {
            worker.worktree_path.clone()
        };
        if !crews.contains_key(&key) {
            order.push(key.clone());
        }
        crews.entry(key).or_default().push(worker);
    }
    let houses: Vec<House> = order
        .into_iter()
        .map(|key| {
            let crew = &crews[&key];
            house_for(key, crew, &by_number)
        })
        .collect();

    let in_houses: HashSet<u32> = houses.iter().filter_map(|h| h.issue).collect();
    let in_transit: HashSet<u32> = pipeline
        .quay
        .iter()
        .chain(pipeline.stops.iter().flat_map(|s| s.issues.iter()))
        .copied()
        .collect();

    let mut backlog: Vec<Crate> = Vec::new();
    let mut board: Vec<Note> = Vec::new();
    for issue in issues.iter().filter(|i| i.state == "open") {
        if in_houses.contains(&issue.number) || in_transit.contains(&issue.number) {
            continue;
        }
        if let Some(label) = issue
            .labels
            .iter()
            .find(|l| crate::model::issue::BLOCKING_LABELS.contains(&l.as_str()))
        {
            board.push(Note {
                number: issue.number,
                title: issue.title.clone(),
                label: label.clone(),
            });
        } else if !issue.is_working {
            backlog.push(Crate {
                number: issue.number,
                title: issue.title.clone(),
                kind: house_kind(&issue.labels),
                priority: priority(issue),
            });
        }
    }
    backlog.sort_by_key(|c| (priority_rank(&c.priority), c.number));
    board.sort_by_key(|n| n.number);

    VillageSnapshot {
        project: swarm.project_name.clone(),
        repo_path: swarm.repo_path.clone(),
        manager: house_for(
            "manager".to_string(),
            &[&swarm.manager],
            &by_number,
        ),
        houses,
        backlog,
        board,
        pipeline,
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
    }
    serde_json::from_str::<Config>(json)
        .map(|c| {
            c.environments
                .into_iter()
                .map(|e| Environment {
                    branch: e.branch.unwrap_or_else(|| format!("env/{}", e.name)),
                    name: e.name,
                })
                .collect()
        })
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

/// Place each pushed tip at the furthest stop that contains it.
///
/// `tips` is newest first. `stops` holds, per stop in pipeline order, the set
/// of commits that stop contains, or `None` when its ref doesn't exist. A tip
/// only reaches a stop after reaching every stop before it, so a missing ref
/// halts it there. Each issue appears once, at its newest tip.
pub fn place_tips(
    tips: &[(String, u32)],
    stop_names: &[String],
    stops: &[Option<HashSet<String>>],
) -> (Vec<u32>, Vec<Stop>) {
    let mut quay = Vec::new();
    let mut placed: Vec<Stop> = stop_names
        .iter()
        .map(|n| Stop { name: n.clone(), issues: Vec::new() })
        .collect();
    let mut seen = HashSet::new();
    for (sha, issue) in tips {
        if !seen.insert(*issue) {
            continue;
        }
        let reached = stops
            .iter()
            .take_while(|s| s.as_ref().is_some_and(|set| set.contains(sha)))
            .count();
        match reached {
            0 => quay.push(*issue),
            n => placed[n - 1].issues.push(*issue),
        }
    }
    if let Some(last) = placed.last_mut() {
        last.issues.truncate(DELIVERED_KEEP);
    }
    (quay, placed)
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

async fn integration_branch(transport: &ServerTransport, repo: &Path) -> String {
    git_lines(transport, repo, &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
        .await
        .and_then(|l| l.first().cloned())
        .and_then(|r| r.strip_prefix("origin/").map(str::to_string))
        .unwrap_or_else(|| "main".to_string())
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

    let history = |branch: String| async move {
        let rev = format!("origin/{branch}");
        let max = format!("--max-count={HISTORY_DEPTH}");
        git_lines(transport, repo, &["rev-list", &max, &rev])
            .await
            .filter(|l| !l.is_empty())
    };
    let mainline = history(integration.clone()).await.unwrap_or_default();
    let green_set = history(green.clone()).await;

    // Order tips newest first by their place on the integration branch. Tip
    // refs are never deleted, so a tip outside the recent window is old
    // history (or not fetched yet, and shows up after the next fetch).
    let position: HashMap<&str, usize> = mainline.iter().enumerate().map(|(i, s)| (s.as_str(), i)).collect();
    let mut tips: Vec<(String, u32)> = tip_refs
        .into_iter()
        .filter(|(sha, _)| position.contains_key(sha.as_str()))
        .collect();
    tips.sort_by_key(|(sha, _)| position[sha.as_str()]);

    let sentinel = match (&green_set, mainline.first()) {
        (None, _) => "unknown",
        (Some(g), Some(tip)) if g.first() == Some(tip) => "green",
        _ => "pending",
    }
    .to_string();

    let mut names = vec![green];
    let mut sets = vec![green_set.map(|v| v.into_iter().collect::<HashSet<_>>())];
    for env in &envs {
        names.push(env.name.clone());
        sets.push(history(env.branch.clone()).await.map(|v| v.into_iter().collect()));
    }
    let (quay, stops) = place_tips(&tips, &names, &sets);
    Pipeline { integration, sentinel, quay, stops }
}

// ---------------------------------------------------------------------------
// Cache of slow-changing extras (issues, pipeline), per repo
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Extras {
    at: Instant,
    issues: Vec<IssueSnapshot>,
    pipeline: Pipeline,
}

/// Shared between requests so `gh` and `git ls-remote` run at most once per
/// `EXTRAS_TTL` per repo however many browsers are watching.
#[derive(Clone, Default)]
pub struct VillageCache {
    inner: Arc<Mutex<HashMap<PathBuf, Extras>>>,
}

impl VillageCache {
    pub async fn villages(&self, transport: &ServerTransport, swarms: &[SwarmSnapshot]) -> Vec<VillageSnapshot> {
        let mut out = Vec::with_capacity(swarms.len());
        for swarm in swarms {
            let repo = PathBuf::from(&swarm.repo_path);
            let cached = self
                .inner
                .lock()
                .ok()
                .and_then(|m| m.get(&repo).cloned())
                .filter(|e| e.at.elapsed() < EXTRAS_TTL);
            let extras = match cached {
                Some(e) => e,
                None => {
                    let e = self.refresh(transport, swarm, &repo).await;
                    if let Ok(mut m) = self.inner.lock() {
                        m.insert(repo.clone(), e.clone());
                    }
                    e
                }
            };
            let issues = if swarm.issues.is_empty() { &extras.issues } else { &swarm.issues };
            out.push(build(swarm, issues, extras.pipeline.clone()));
        }
        out
    }

    async fn refresh(&self, transport: &ServerTransport, swarm: &SwarmSnapshot, repo: &Path) -> Extras {
        if swarm.repo_path.is_empty() {
            return Extras { at: Instant::now(), issues: Vec::new(), pipeline: Pipeline::default() };
        }
        let issues = if swarm.issues.is_empty() {
            crate::github::fetch_issues(transport, repo)
                .await
                .map(|list| {
                    list.iter()
                        .map(|i| IssueSnapshot::from_issue(i, &swarm.project_name))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let pipeline = read_pipeline(transport, repo).await;
        Extras { at: Instant::now(), issues, pipeline }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                Environment { name: "staging".into(), branch: "env/staging".into() },
                Environment { name: "prod".into(), branch: "env/prod".into() },
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

    fn set(items: &[&str]) -> Option<HashSet<String>> {
        Some(items.iter().map(|s| s.to_string()).collect())
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
        let (quay, placed) = place_tips(&tips, &names, &stops);
        assert_eq!(quay, vec![4]);
        assert_eq!(placed[0].issues, vec![3]);
        assert_eq!(placed[1].issues, vec![2]);
        assert_eq!(placed[2].issues, vec![1]);
    }

    #[test]
    fn missing_ref_halts_tips_and_issues_appear_once() {
        let tips = vec![("b".to_string(), 7), ("a".to_string(), 7), ("c".to_string(), 8)];
        let names = vec!["master-green".to_string(), "staging".to_string()];
        let stops = vec![None, set(&["a", "b", "c"])];
        let (quay, placed) = place_tips(&tips, &names, &stops);
        assert_eq!(quay, vec![7, 8], "no green ref: nothing passes the sentinel");
        assert!(placed.iter().all(|s| s.issues.is_empty()));
    }

    #[test]
    fn village_serializes_lowercase_enums() {
        let v = build(&swarm(vec![agent("worker-1", "Idle", "/wt1")]), &[], Pipeline::default());
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains(r#""kind":"cottage""#));
        assert!(json.contains(r#""status":"idle""#));
    }
}
