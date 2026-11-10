//! Swarms running under herdr, found with `herdr agent list`.
//!
//! herdr gives one entry per agent pane with its `cwd`, `pane_id`, optional
//! `name` and `agent_status` (`working`, `idle`, `blocked`, `done`,
//! `unknown`). The `cwd` decides who is where (`model::recognize`): an agent
//! anywhere inside one of a repo's worktrees belongs to that repo's swarm, as
//! a worker when the worktree isn't the main checkout. Names like
//! `wt3-<project>` are only a hint. Agents outside every repo aren't part of a
//! swarm.
//!
//! The manager is chosen by evidence (`choose_manager`): the pane the repo's
//! swarm manifest names, then an agent whose processes carry
//! `AUTOCODER_MANAGER` (`herdr pane process-info`, then `/proc`), then a pane
//! or workspace labelled `manager-…`, then any agent in the main checkout.
//! An agent with no name of its own takes its workspace's label (`herdr
//! workspace list`), which is where swarm launchers put `manager-<project>`.
//!
//! A manager with no workers is still a swarm (one scaled to zero between
//! waves) when its repo holds a manifest or anything but its place in the main
//! checkout picked it. Otherwise a lone agent in a repo is just someone's
//! Claude session, not a swarm.
//!
//! herdr doesn't know issues, so a worker's issue comes from its branch
//! (`feature/issue-412`, `fix/412-crash`).

use std::collections::HashMap;
use std::path::PathBuf;

use futures::future::BoxFuture;
use serde_json::Value;

use super::source::SwarmSource;
use crate::model::recognize::{
    AgentExplanation, AgentFact, Evidence, Judged, LONE_AGENT, ManagerRule, RepoFacts, SwarmManifest, canonical,
    choose_manager, explain, gather, process_marker, read_manifests, recognize,
};
use super::{AgentSnapshot, ManagerPick, SwarmSnapshot};
use crate::transport::ServerTransport;

#[derive(Debug, Clone, PartialEq)]
pub struct HerdrAgent {
    pub pane_id: String,
    pub name: Option<String>,
    pub status: String,
    pub cwd: String,
    pub agent: Option<String>,
}

/// `herdr workspace list` output → workspace id (`w5`) to its label
/// (`manager-agents-ui`).
pub fn parse_workspace_labels(json: &str) -> HashMap<String, String> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return HashMap::new();
    };
    v.pointer("/result/workspaces")
        .or_else(|| v.get("workspaces"))
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|w| {
                    let id = w.get("workspace_id")?.as_str()?;
                    let label = w.get("label")?.as_str()?;
                    Some((id.to_string(), label.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Name each unnamed agent after its workspace's label. Swarm launchers label
/// the manager's workspace `manager-<project>` while `herdr agent list` leaves
/// the pane itself unnamed.
pub fn name_from_workspaces(agents: &mut [HerdrAgent], labels: &HashMap<String, String>) {
    for agent in agents.iter_mut().filter(|a| a.name.is_none()) {
        let workspace = agent.pane_id.split(':').next().unwrap_or_default();
        agent.name = labels.get(workspace).cloned();
    }
}

/// `herdr agent list` output: `{"result": {"agents": [...]}}`.
pub fn parse_agents(json: &str) -> Vec<HerdrAgent> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let list = v.pointer("/result/agents").or_else(|| v.get("agents"));
    list.and_then(Value::as_array)
        .map(|agents| {
            agents
                .iter()
                .filter_map(|a| {
                    let s = |k: &str| a.get(k).and_then(Value::as_str).map(str::to_string);
                    Some(HerdrAgent {
                        pane_id: s("pane_id")?,
                        name: s("name"),
                        status: s("agent_status").unwrap_or_else(|| "unknown".to_string()),
                        cwd: s("cwd").or_else(|| s("foreground_cwd"))?,
                        agent: s("agent"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The issue a branch is for: `feature/issue-412`, `issue/412`, `fix/412-crash`.
pub fn branch_issue(branch: &str) -> Option<u32> {
    let b = branch.to_ascii_lowercase();
    let digits_after = |s: &str| -> Option<u32> {
        let rest = s.trim_start_matches(|c: char| !c.is_ascii_digit() && !c.is_ascii_alphabetic());
        let n: String = rest.chars().take_while(char::is_ascii_digit).collect();
        n.parse().ok()
    };
    if let Some(i) = b.rfind("issue") {
        return digits_after(&b[i + "issue".len()..]);
    }
    b.rsplit('/').next().and_then(|last| {
        let n: String = last.chars().take_while(char::is_ascii_digit).collect();
        n.parse().ok()
    })
}

fn snapshot(agent: &HerdrAgent, project: &str, role: &str, is_manager: bool, branch: Option<&String>) -> AgentSnapshot {
    let (state, waiting) = match agent.status.as_str() {
        "working" => ("Working".to_string(), false),
        "idle" => ("Idle".to_string(), false),
        "blocked" => ("Idle".to_string(), true),
        "done" => ("Done: finished".to_string(), false),
        _ => ("Unknown".to_string(), false),
    };
    let issue = branch.and_then(|b| branch_issue(b));
    AgentSnapshot {
        id: format!("{project}/{role}"),
        role: role.to_string(),
        state: match (issue, state.as_str()) {
            (Some(n), "Working") => format!("Working #{n}"),
            _ => state,
        },
        is_manager,
        waiting_for_input: waiting,
        current_issue: if is_manager { None } else { issue },
        current_issue_title: None,
        pane_content: String::new(),
        tmux_target: agent.pane_id.clone(),
        health: "Healthy".to_string(),
        completed_issue_count: 0,
        resurrection_attempts: 0,
        status_timestamp: None,
        worktree_path: agent.cwd.trim_end_matches('/').to_string(),
        branch: branch.cloned(),
        mux: super::Mux::Herdr,
    }
}

/// The plain facts the recognizer needs from herdr's agents.
pub fn facts(agents: &[HerdrAgent]) -> Vec<AgentFact> {
    agents
        .iter()
        .map(|a| AgentFact { source: "herdr".to_string(), pane: a.pane_id.clone(), cwd: canonical(&a.cwd) })
        .collect()
}

/// The swarm manifests in each repo, by main checkout.
pub type Manifests = HashMap<PathBuf, Vec<SwarmManifest>>;

/// What each agent shows that could make it the manager: its pane, the
/// `AUTOCODER_MANAGER` marker probed from its processes (`markers`, by pane),
/// and its name or workspace label.
pub fn evidence(agents: &[HerdrAgent], markers: &HashMap<String, String>) -> Vec<Evidence> {
    agents
        .iter()
        .map(|a| Evidence {
            panes: vec![a.pane_id.clone()],
            marker: markers.get(&a.pane_id).cloned(),
            label: a.name.clone(),
        })
        .collect()
}

/// The panes worth probing for a process marker: agents in a main checkout
/// whose swarm a live manifest pane hasn't already settled.
pub fn marker_candidates(agents: &[HerdrAgent], repos: &[RepoFacts], manifests: &Manifests) -> Vec<String> {
    let evidence = evidence(agents, &HashMap::new());
    recognize(&facts(agents), repos)
        .iter()
        .filter(|swarm| {
            let found = manifests.get(&swarm.root).map(Vec::as_slice).unwrap_or(&[]);
            choose_manager(swarm, &evidence, found).is_none_or(|c| c.rule != ManagerRule::ManifestPane)
        })
        .flat_map(|swarm| swarm.main_members().map(|m| agents[m.agent].pane_id.clone()).collect::<Vec<_>>())
        .collect()
}

/// `herdr pane process-info` output → the pane's shell and foreground pids.
pub fn parse_pane_pids(json: &str) -> Vec<u32> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let Some(info) = v.pointer("/result/process_info") else {
        return Vec::new();
    };
    let pid = |v: &Value| v.as_u64().and_then(|n| u32::try_from(n).ok());
    let mut pids: Vec<u32> = info
        .get("foreground_processes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| p.get("pid").and_then(pid))
        .collect();
    pids.extend(info.get("shell_pid").and_then(pid));
    pids
}

/// Group herdr agents into swarms by repo identity (`recognize`), each with
/// its manager chosen by evidence (`choose_manager`). `repos` are the repos
/// their cwds are in; `manifests` the swarm manifests in each; `markers` the
/// `AUTOCODER_MANAGER` marker found in each probed pane's processes.
///
/// A repo is a swarm when it has a worker, a manifest, or a manager picked
/// by more than being in the main checkout. Without any of those, a lone
/// agent in a repo is just someone's Claude session.
pub fn swarms_from_agents(
    agents: &[HerdrAgent],
    repos: &[RepoFacts],
    manifests: &Manifests,
    markers: &HashMap<String, String>,
) -> Vec<SwarmSnapshot> {
    judge(agents, repos, manifests, markers)
        .into_iter()
        .filter(|j| j.left_out.is_none())
        .map(|Judged { swarm, choice, .. }| {
            let manager = choice.as_ref().map(|c| &swarm.members[c.member]);
            // An agent picked as manager from a worker's worktree isn't also a worker.
            let workers: Vec<_> = swarm.workers().filter(|(_, m)| Some(m.agent) != manager.map(|x| x.agent)).collect();
            let project = swarm.name();
            let manager = match manager {
                Some(m) => snapshot(&agents[m.agent], &project, "manager", true, m.branch.as_ref()),
                None => {
                    let mut missing = snapshot(
                        &HerdrAgent { pane_id: String::new(), name: None, status: "unknown".into(), cwd: String::new(), agent: None },
                        &project,
                        "manager",
                        true,
                        None,
                    );
                    missing.state = "Missing".to_string();
                    missing
                }
            };
            let workers: Vec<AgentSnapshot> = workers
                .iter()
                .map(|(n, m)| snapshot(&agents[m.agent], &project, &format!("worker-{n}"), false, m.branch.as_ref()))
                .collect();
            let busy = workers.iter().filter(|w| w.state.starts_with("Working")).count();
            let attention = workers.iter().filter(|w| w.waiting_for_input).count();
            SwarmSnapshot {
                agent_type: swarm
                    .members
                    .iter()
                    .find_map(|m| agents[m.agent].agent.clone())
                    .unwrap_or_else(|| "Claude".to_string()),
                project_name: project,
                repo_path: swarm.root.to_string_lossy().into_owned(),
                repo_key: swarm.key.to_string_lossy().into_owned(),
                manager_by: choice.map(ManagerPick::from),
                workflow: None,
                tmux_session: "herdr".to_string(),
                stopped: false,
                busy_count: busy,
                idle_count: workers.len() - busy,
                attention_count: attention,
                manager,
                workers,
                issues: Vec::new(),
            }
        })
        .collect()
}

/// Each repo's agents as the recognizer judged them: the manager chosen by
/// evidence (`choose_manager`), and whether the repo is a swarm at all. A
/// repo is a swarm when it has a worker, a manifest, or a manager picked by
/// more than being in the main checkout. Without any of those, a lone agent
/// in a repo is just someone's Claude session.
pub fn judge(agents: &[HerdrAgent], repos: &[RepoFacts], manifests: &Manifests, markers: &HashMap<String, String>) -> Vec<Judged> {
    let evidence = evidence(agents, markers);
    recognize(&facts(agents), repos)
        .into_iter()
        .map(|swarm| {
            let found = manifests.get(&swarm.root).map(Vec::as_slice).unwrap_or(&[]);
            let choice = choose_manager(&swarm, &evidence, found);
            let manager = choice.as_ref().map(|c| swarm.members[c.member].agent);
            let has_worker = swarm.workers().any(|(_, m)| Some(m.agent) != manager);
            let is_swarm = has_worker
                || !found.is_empty()
                || choice.as_ref().is_some_and(|c| c.rule != ManagerRule::MainCheckout);
            Judged { swarm, choice, left_out: (!is_swarm).then(|| LONE_AGENT.to_string()) }
        })
        .collect()
}

/// Every herdr agent and what became of it (`GET /api/swarms?explain=1`).
pub fn explain_agents(
    agents: &[HerdrAgent],
    repos: &[RepoFacts],
    manifests: &Manifests,
    markers: &HashMap<String, String>,
) -> Vec<AgentExplanation> {
    let names: Vec<Option<String>> = agents.iter().map(|a| a.name.clone()).collect();
    explain(&facts(agents), &names, &judge(agents, repos, manifests, markers))
}

/// Swarms under a running herdr server; none when herdr isn't installed or
/// running.
pub struct HerdrSwarms;

impl SwarmSource for HerdrSwarms {
    fn name(&self) -> &'static str {
        "herdr"
    }
    fn swarms<'a>(&'a self, transport: &'a ServerTransport) -> BoxFuture<'a, Vec<SwarmSnapshot>> {
        Box::pin(async move {
            let (agents, repos, manifests, markers) = look(transport).await;
            swarms_from_agents(&agents, &repos, &manifests, &markers)
        })
    }
    fn explain<'a>(&'a self, transport: &'a ServerTransport) -> BoxFuture<'a, Vec<AgentExplanation>> {
        Box::pin(async move {
            let (agents, repos, manifests, markers) = look(transport).await;
            explain_agents(&agents, &repos, &manifests, &markers)
        })
    }
}

/// What the recognizer needs from herdr and git: the agents (named after
/// their workspaces), the repos holding them, each repo's manifests, and the
/// process markers of the panes worth probing. Nothing when herdr isn't there.
async fn look(transport: &ServerTransport) -> (Vec<HerdrAgent>, Vec<RepoFacts>, Manifests, HashMap<String, String>) {
    let nothing = || (Vec::new(), Vec::new(), Manifests::new(), HashMap::new());
    let args = ["agent".to_string(), "list".to_string()];
    let Ok(out) = transport.output("herdr", &args, None).await else {
        return nothing();
    };
    if !out.status.success() {
        return nothing();
    }
    let mut agents = parse_agents(&String::from_utf8_lossy(&out.stdout));
    let args = ["workspace".to_string(), "list".to_string()];
    if let Ok(out) = transport.output("herdr", &args, None).await {
        // herdr exits 1 here even when it prints the list, so read stdout either way.
        name_from_workspaces(&mut agents, &parse_workspace_labels(&String::from_utf8_lossy(&out.stdout)));
    }
    let cwds: Vec<PathBuf> = facts(&agents).into_iter().map(|f| f.cwd).collect();
    let repos = gather(transport, &cwds).await;
    let mut manifests = Manifests::new();
    for root in repos.iter().filter_map(|r| r.worktrees.first()).map(|w| &w.path) {
        manifests.insert(root.clone(), read_manifests(transport, root).await);
    }
    let mut markers = HashMap::new();
    for pane in marker_candidates(&agents, &repos, &manifests) {
        let args: Vec<String> = ["pane", "process-info", "--pane", &pane].iter().map(|s| s.to_string()).collect();
        let Ok(out) = transport.output("herdr", &args, None).await else { continue };
        let pids = parse_pane_pids(&String::from_utf8_lossy(&out.stdout));
        if let Some(marker) = process_marker(transport, &pids).await {
            markers.insert(pane, marker);
        }
    }
    (agents, repos, manifests, markers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::recognize::{ManagerRule, OUTSIDE_ANY_REPO, Worktree};

    fn repo(root: &str, worktrees: &[(&str, Option<&str>)]) -> RepoFacts {
        RepoFacts {
            common_dir: PathBuf::from(format!("{root}/.git")),
            worktrees: std::iter::once((root, None))
                .chain(worktrees.iter().copied())
                .map(|(p, b)| Worktree { path: p.into(), branch: b.map(str::to_string) })
                .collect(),
        }
    }

    const LIST: &str = r#"{"result":{"agents":[
      {"pane_id":"w1:p1","name":"manager-shop","agent_status":"working","cwd":"/src/shop","agent":"claude"},
      {"pane_id":"w2:p1","name":"wt1-shop","agent_status":"blocked","cwd":"/src/shop-wt-1"},
      {"pane_id":"w3:p1","name":"wt2-shop","agent_status":"working","cwd":"/src/shop-wt-2/"},
      {"pane_id":"w4:p1","agent_status":"idle","cwd":"/home/me/notes"}
    ]}}"#;

    #[test]
    fn parses_agent_list() {
        let agents = parse_agents(LIST);
        assert_eq!(agents.len(), 4);
        assert_eq!(agents[1].status, "blocked");
        assert_eq!(agents[3].name, None);
        assert!(parse_agents("not json").is_empty());
    }

    #[test]
    fn reads_branch_issues() {
        assert_eq!(branch_issue("feature/issue-412"), Some(412));
        assert_eq!(branch_issue("issue/88"), Some(88));
        assert_eq!(branch_issue("fix/412-crash"), Some(412));
        assert_eq!(branch_issue("main"), None);
    }

    #[test]
    fn groups_agents_into_swarms_by_cwd() {
        let repos = [repo("/src/shop", &[("/src/shop-wt-1", None), ("/src/shop-wt-2", Some("feature/issue-77"))])];
        let swarms = swarms_from_agents(&parse_agents(LIST), &repos, &Manifests::new(), &HashMap::new());
        assert_eq!(swarms.len(), 1, "the stray agent in ~/notes is not a swarm");
        let s = &swarms[0];
        assert_eq!(s.project_name, "shop");
        assert_eq!(s.repo_path, "/src/shop");
        assert_eq!(s.repo_key, "/src/shop/.git");
        assert_eq!(s.manager.state, "Working");
        assert_eq!(s.workers.len(), 2);
        assert_eq!(s.workers[0].role, "worker-1");
        assert!(s.workers[0].waiting_for_input);
        assert_eq!(s.workers[1].current_issue, Some(77));
        assert_eq!(s.workers[1].state, "Working #77");
        assert_eq!(s.workers[1].worktree_path, "/src/shop-wt-2");
        assert_eq!(s.attention_count, 1);
        // Their panes are herdr's, so the session view reads them through herdr, not tmux.
        assert!([&s.manager].into_iter().chain(&s.workers).all(|a| a.mux == crate::web::Mux::Herdr));
    }

    #[test]
    fn workers_without_a_manager_still_make_a_swarm() {
        let json = r#"{"result":{"agents":[{"pane_id":"w2:p1","agent_status":"idle","cwd":"/src/x-wt-1"}]}}"#;
        let swarms = swarms_from_agents(&parse_agents(json), &[repo("/src/x", &[("/src/x-wt-1", None)])], &Manifests::new(), &HashMap::new());
        assert_eq!(swarms.len(), 1);
        assert_eq!(swarms[0].manager.state, "Missing");
    }

    #[test]
    fn a_manager_with_zero_workers_is_a_swarm_when_its_repo_has_a_manifest() {
        // An unnamed agent alone in a repo: a swarm scaled to zero, but only if
        // autocoder left a swarm manifest there.
        let json = r#"{"result":{"agents":[{"pane_id":"w5:p1","agent_status":"working","cwd":"/src/shop"}]}}"#;
        let agents = parse_agents(json);
        let repos = [repo("/src/shop", &[])];
        assert!(swarms_from_agents(&agents, &repos, &Manifests::new(), &HashMap::new()).is_empty());

        let manifests = Manifests::from([(PathBuf::from("/src/shop"), vec![SwarmManifest::default()])]);
        let swarms = swarms_from_agents(&agents, &repos, &manifests, &HashMap::new());
        assert_eq!(swarms.len(), 1);
        assert_eq!(swarms[0].project_name, "shop");
        assert_eq!(swarms[0].manager.tmux_target, "w5:p1");
        assert_eq!(swarms[0].manager.state, "Working");
        assert!(swarms[0].workers.is_empty());
        assert_eq!((swarms[0].busy_count, swarms[0].idle_count), (0, 0));
    }

    #[test]
    fn unnamed_agents_take_their_workspace_label() {
        let ws = r#"{"id":"cli:workspace:list","result":{"type":"workspace_list","workspaces":[
            {"label":"agents-ui","workspace_id":"w1"},{"label":"manager-agents-ui","workspace_id":"w5"}]}}"#;
        let labels = parse_workspace_labels(ws);
        assert_eq!(labels.get("w5").map(String::as_str), Some("manager-agents-ui"));
        let json = r#"{"result":{"agents":[
            {"pane_id":"w1:p1","agent_status":"done","cwd":"/src/agents-ui"},
            {"pane_id":"w5:p1","agent_status":"working","cwd":"/src/agents-ui"},
            {"pane_id":"w6:p1","name":"kept","agent_status":"idle","cwd":"/elsewhere"}]}}"#;
        let mut agents = parse_agents(json);
        name_from_workspaces(&mut agents, &labels);
        assert_eq!(agents[1].name.as_deref(), Some("manager-agents-ui"));
        assert_eq!(agents[2].name.as_deref(), Some("kept"));
        // The labelled manager wins over the other session in the repo, and makes a swarm.
        let swarms = swarms_from_agents(&agents, &[repo("/src/agents-ui", &[])], &Manifests::new(), &HashMap::new());
        assert_eq!(swarms.len(), 1);
        assert_eq!(swarms[0].manager.tmux_target, "w5:p1");
        assert!(swarms[0].workers.is_empty());
    }

    #[test]
    fn a_manager_named_manager_is_a_swarm_without_a_manifest() {
        let json = r#"{"result":{"agents":[{"pane_id":"w1:p1","name":"manager-shop","agent_status":"idle","cwd":"/src/shop"}]}}"#;
        let swarms = swarms_from_agents(&parse_agents(json), &[repo("/src/shop", &[])], &Manifests::new(), &HashMap::new());
        assert_eq!(swarms.len(), 1);
        assert!(swarms[0].workers.is_empty());
    }

    /// Two agents in the main checkout of /src/agents-ui: w1:p1 (an earlier
    /// session) and w5:p1 (the manager), plus a worker.
    const TWO_IN_MAIN: &str = r#"{"result":{"agents":[
        {"pane_id":"w1:p1","agent_status":"done","cwd":"/src/agents-ui"},
        {"pane_id":"w5:p1","agent_status":"working","cwd":"/src/agents-ui"},
        {"pane_id":"w2:p1","agent_status":"working","cwd":"/src/agents-ui-wt-1"}]}}"#;

    fn agents_ui() -> [RepoFacts; 1] {
        [repo("/src/agents-ui", &[("/src/agents-ui-wt-1", None)])]
    }

    #[test]
    fn the_manifest_pane_is_the_manager() {
        let manifests = Manifests::from([(
            PathBuf::from("/src/agents-ui"),
            vec![SwarmManifest { session: "claude-agents-ui".into(), manager_panes: vec!["w5:p1".into()], state: None }],
        )]);
        let agents = parse_agents(TWO_IN_MAIN);
        // Settled by the manifest: no process needs probing.
        assert!(marker_candidates(&agents, &agents_ui(), &manifests).is_empty());
        let s = &swarms_from_agents(&agents, &agents_ui(), &manifests, &HashMap::new())[0];
        assert_eq!(s.manager.tmux_target, "w5:p1");
        let by = s.manager_by.unwrap();
        assert_eq!((by.rule, by.ambiguous), (ManagerRule::ManifestPane, false));
    }

    #[test]
    fn the_process_marker_picks_the_manager_among_agents_in_the_main_checkout() {
        let agents = parse_agents(TWO_IN_MAIN);
        // No manifest: both main-checkout agents get probed.
        assert_eq!(marker_candidates(&agents, &agents_ui(), &Manifests::new()), vec!["w1:p1", "w5:p1"]);
        let markers = HashMap::from([("w5:p1".to_string(), "claude-agents-ui".to_string())]);
        let s = &swarms_from_agents(&agents, &agents_ui(), &Manifests::new(), &markers)[0];
        assert_eq!(s.manager.tmux_target, "w5:p1");
        assert_eq!(s.manager_by.unwrap().rule, ManagerRule::ProcessMarker);
    }

    #[test]
    fn two_unmarked_agents_in_the_main_checkout_make_an_ambiguous_manager() {
        let s = &swarms_from_agents(&parse_agents(TWO_IN_MAIN), &agents_ui(), &Manifests::new(), &HashMap::new())[0];
        assert_eq!(s.manager.tmux_target, "w1:p1", "picked deterministically: the lowest pane");
        let by = s.manager_by.unwrap();
        assert_eq!((by.rule, by.candidates, by.ambiguous), (ManagerRule::MainCheckout, 2, true));
        assert_eq!(s.workers.len(), 1);
    }

    #[test]
    fn parses_pane_pids() {
        let json = r#"{"result":{"process_info":{"foreground_processes":[{"pid":481923,"name":"claude"}],"pane_id":"w5:p1","shell_pid":476081}}}"#;
        assert_eq!(parse_pane_pids(json), vec![481923, 476081]);
        assert!(parse_pane_pids("nope").is_empty());
    }

    /// `/api/swarms?explain=1` for herdr: the bug of #393 (a lone manager in a
    /// repo with no manifest or label) shows up in one request.
    #[test]
    fn explains_who_was_grouped_and_why_not() {
        let shop = [repo("/src/shop", &[("/src/shop-wt-1", Some("feature/issue-77")), ("/src/shop-wt-2", None)])];
        let out = explain_agents(&parse_agents(LIST), &shop, &Manifests::new(), &HashMap::new());
        assert_eq!(out.len(), 4, "every agent herdr saw, kept or not");
        let by = |pane: &str| out.iter().find(|e| e.pane == pane).unwrap();

        let m = by("w1:p1");
        assert_eq!((m.source.as_str(), m.role.as_deref(), m.swarm.as_deref()), ("herdr", Some("manager"), Some("shop")));
        assert_eq!(m.name.as_deref(), Some("manager-shop"));
        assert_eq!((m.manager_rule, m.ambiguous), (Some(ManagerRule::Label), Some(false)));
        assert_eq!(m.manager_because.as_deref(), Some("its manager label"));
        assert_eq!(m.repo.as_deref(), Some("/src/shop/.git"));

        assert_eq!(by("w2:p1").role.as_deref(), Some("worker-1"));
        assert_eq!(by("w2:p1").manager_rule, None);

        let stray = by("w4:p1");
        assert_eq!((stray.swarm.as_deref(), stray.role.as_deref()), (None, None));
        assert_eq!(stray.left_out.as_deref(), Some(OUTSIDE_ANY_REPO));
        assert_eq!(stray.cwd, "/home/me/notes");
    }

    #[test]
    fn explains_a_lone_unmarked_agent_and_the_runner_up_in_the_main_checkout() {
        // A lone unnamed agent in a repo with no manifest: not a swarm, and says so.
        let lone = r#"{"result":{"agents":[{"pane_id":"w5:p1","agent_status":"working","cwd":"/src/agents-ui"}]}}"#;
        let out = explain_agents(&parse_agents(lone), &agents_ui(), &Manifests::new(), &HashMap::new());
        assert_eq!(out[0].left_out.as_deref(), Some(LONE_AGENT));
        assert_eq!((out[0].swarm.as_deref(), out[0].role.as_deref()), (None, None));

        // Two unmarked agents in the main checkout: one manager (an ambiguous
        // pick) and one left without a role, naming who won and why.
        let out = explain_agents(&parse_agents(TWO_IN_MAIN), &agents_ui(), &Manifests::new(), &HashMap::new());
        let m = out.iter().find(|e| e.role.as_deref() == Some("manager")).unwrap();
        assert_eq!((m.pane.as_str(), m.manager_rule, m.ambiguous), ("w1:p1", Some(ManagerRule::MainCheckout), Some(true)));
        let other = out.iter().find(|e| e.pane == "w5:p1").unwrap();
        assert_eq!(other.role, None);
        assert_eq!(other.swarm.as_deref(), Some("agents-ui"));
        assert_eq!(other.left_out.as_deref(), Some("in the main checkout, but w1:p1 is the manager (it is the agent in the main checkout)"));

        // With the manifest naming w5:p1, it is the manager, unambiguously, by that rule.
        let manifests = Manifests::from([(
            PathBuf::from("/src/agents-ui"),
            vec![SwarmManifest { session: "claude-agents-ui".into(), manager_panes: vec!["w5:p1".into()], state: None }],
        )]);
        let out = explain_agents(&parse_agents(TWO_IN_MAIN), &agents_ui(), &manifests, &HashMap::new());
        let m = out.iter().find(|e| e.pane == "w5:p1").unwrap();
        assert_eq!((m.role.as_deref(), m.manager_rule, m.ambiguous), (Some("manager"), Some(ManagerRule::ManifestPane), Some(false)));
        assert_eq!(m.manager_because.as_deref(), Some("the pane the swarm manifest names"));
    }

    /// The swarms and their explanation agree: the explanation is made from
    /// the same judgement.
    #[test]
    fn explanation_matches_the_swarms() {
        let shop = [repo("/src/shop", &[("/src/shop-wt-1", None), ("/src/shop-wt-2", None)])];
        let agents = parse_agents(LIST);
        let swarms = swarms_from_agents(&agents, &shop, &Manifests::new(), &HashMap::new());
        let out = explain_agents(&agents, &shop, &Manifests::new(), &HashMap::new());
        let grouped: Vec<&str> = out.iter().filter(|e| e.role.is_some()).map(|e| e.pane.as_str()).collect();
        let shown: Vec<&str> = swarms
            .iter()
            .flat_map(|s| std::iter::once(&s.manager).chain(&s.workers))
            .map(|a| a.tmux_target.as_str())
            .collect();
        assert_eq!(grouped, shown);
    }
}
