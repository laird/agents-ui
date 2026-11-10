//! Swarms running under herdr, found with `herdr agent list`.
//!
//! herdr gives one entry per agent pane with its `cwd`, `pane_id`, optional
//! `name` and `agent_status` (`working`, `idle`, `blocked`, `done`,
//! `unknown`). The `cwd` decides who is who (`model::recognize`): an agent
//! anywhere inside one of a repo's worktrees belongs to that repo's swarm,
//! as a worker when the worktree isn't the main checkout and as the manager
//! when it is. Names like `wt3-<project>` are only a hint. Agents outside
//! every repo aren't part of a swarm.
//!
//! A manager with no workers is still a swarm (one scaled to zero between
//! waves) when its pane is named `manager-…` or its repo holds an autocoder
//! swarm manifest (`.autocoder/swarm/*.json`). Without either, a lone agent
//! in a repo is just someone's Claude session, not a swarm. An agent with no
//! name of its own takes its workspace's label (`herdr workspace list`), which
//! is where swarm launchers put `manager-<project>`.
//!
//! herdr doesn't know issues, so a worker's issue comes from its branch
//! (`feature/issue-412`, `fix/412-crash`).

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use futures::future::BoxFuture;
use serde_json::Value;

use super::source::SwarmSource;
use crate::model::recognize::{AgentFact, RepoFacts, canonical, gather, recognize};
use super::{AgentSnapshot, SwarmSnapshot};
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

/// Group herdr agents into swarms by repo identity (`recognize`). `repos` are
/// the repos their cwds are in; `manifests` holds the main checkouts with an
/// autocoder swarm manifest, whose manager makes a swarm even with zero
/// workers.
pub fn swarms_from_agents(
    agents: &[HerdrAgent],
    repos: &[RepoFacts],
    manifests: &BTreeSet<PathBuf>,
) -> Vec<SwarmSnapshot> {
    let named_manager = |a: &HerdrAgent| a.name.as_deref().is_some_and(|n| n.starts_with("manager"));
    recognize(&facts(agents), repos)
        .into_iter()
        .filter_map(|swarm| {
            // Prefer the agent named as the manager when several sit in the main checkout.
            let manager = swarm
                .main_members()
                .find(|m| named_manager(&agents[m.agent]))
                .or_else(|| swarm.main_members().next());
            let workers: Vec<_> = swarm.workers().collect();
            let is_swarm = !workers.is_empty()
                || manager.is_some_and(|m| named_manager(&agents[m.agent]) || manifests.contains(&swarm.root));
            if !is_swarm {
                return None;
            }
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
            Some(SwarmSnapshot {
                agent_type: swarm
                    .members
                    .iter()
                    .find_map(|m| agents[m.agent].agent.clone())
                    .unwrap_or_else(|| "Claude".to_string()),
                project_name: project,
                repo_path: swarm.root.to_string_lossy().into_owned(),
                repo_key: swarm.key.to_string_lossy().into_owned(),
                workflow: None,
                tmux_session: "herdr".to_string(),
                stopped: false,
                busy_count: busy,
                idle_count: workers.len() - busy,
                attention_count: attention,
                manager,
                workers,
                issues: Vec::new(),
            })
        })
        .collect()
}

/// Swarms under a running herdr server; none when herdr isn't installed or
/// running.
pub struct HerdrSwarms;

impl SwarmSource for HerdrSwarms {
    fn name(&self) -> &'static str {
        "herdr"
    }
    fn swarms<'a>(&'a self, transport: &'a ServerTransport) -> BoxFuture<'a, Vec<SwarmSnapshot>> {
        Box::pin(collect(transport))
    }
}

async fn collect(transport: &ServerTransport) -> Vec<SwarmSnapshot> {
    let args = ["agent".to_string(), "list".to_string()];
    let Ok(out) = transport.output("herdr", &args, None).await else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    let mut agents = parse_agents(&String::from_utf8_lossy(&out.stdout));
    let args = ["workspace".to_string(), "list".to_string()];
    if let Ok(out) = transport.output("herdr", &args, None).await {
        // herdr exits 1 here even when it prints the list, so read stdout either way.
        name_from_workspaces(&mut agents, &parse_workspace_labels(&String::from_utf8_lossy(&out.stdout)));
    }
    let cwds: Vec<PathBuf> = facts(&agents).into_iter().map(|f| f.cwd).collect();
    let repos = gather(transport, &cwds).await;
    let mut manifests = BTreeSet::new();
    for root in repos.iter().filter_map(|r| r.worktrees.first()).map(|w| &w.path) {
        // An autocoder swarm manifest makes a lone manager a swarm of zero workers.
        let probe = ["-c".to_string(), "ls .autocoder/swarm/*.json >/dev/null 2>&1".to_string()];
        if let Ok(out) = transport.output("sh", &probe, Some(root)).await
            && out.status.success()
        {
            manifests.insert(root.clone());
        }
    }
    swarms_from_agents(&agents, &repos, &manifests)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::recognize::Worktree;

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
        let swarms = swarms_from_agents(&parse_agents(LIST), &repos, &BTreeSet::new());
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
        let swarms = swarms_from_agents(&parse_agents(json), &[repo("/src/x", &[("/src/x-wt-1", None)])], &BTreeSet::new());
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
        assert!(swarms_from_agents(&agents, &repos, &BTreeSet::new()).is_empty());

        let manifests = BTreeSet::from([PathBuf::from("/src/shop")]);
        let swarms = swarms_from_agents(&agents, &repos, &manifests);
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
        let swarms = swarms_from_agents(&agents, &[repo("/src/agents-ui", &[])], &BTreeSet::new());
        assert_eq!(swarms.len(), 1);
        assert_eq!(swarms[0].manager.tmux_target, "w5:p1");
        assert!(swarms[0].workers.is_empty());
    }

    #[test]
    fn a_manager_named_manager_is_a_swarm_without_a_manifest() {
        let json = r#"{"result":{"agents":[{"pane_id":"w1:p1","name":"manager-shop","agent_status":"idle","cwd":"/src/shop"}]}}"#;
        let swarms = swarms_from_agents(&parse_agents(json), &[repo("/src/shop", &[])], &BTreeSet::new());
        assert_eq!(swarms.len(), 1);
        assert!(swarms[0].workers.is_empty());
    }
}
