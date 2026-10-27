//! Swarms running under herdr, found with `herdr agent list`.
//!
//! herdr gives one entry per agent pane with its `cwd`, `pane_id`, optional
//! `name` and `agent_status` (`working`, `idle`, `blocked`, `done`,
//! `unknown`). As in autocoder's own `/monitor-workers`, the `cwd` decides
//! who is who and names like `wt3-<project>` are only a hint: an agent in a
//! `<repo>-wt-<N>` worktree is worker N of `<repo>`, and the agent in `<repo>`
//! itself is the manager. Agents anywhere else aren't part of a swarm.
//!
//! herdr doesn't know issues, so a worker's issue comes from its branch
//! (`feature/issue-412`, `fix/412-crash`).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde_json::Value;

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

/// `/src/repo-wt-3` → (`/src/repo`, Some(3)); `/src/repo` → (`/src/repo`, None).
pub fn place(cwd: &str) -> (PathBuf, Option<u32>) {
    let path = Path::new(cwd.trim_end_matches('/'));
    let base = path.file_name().and_then(|b| b.to_str()).unwrap_or_default();
    if let Some((repo, n)) = base.rsplit_once("-wt-")
        && let Ok(n) = n.parse::<u32>()
        && !repo.is_empty()
    {
        return (path.with_file_name(repo), Some(n));
    }
    (path.to_path_buf(), None)
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
    }
}

/// Group herdr agents into swarms. `branches` maps worktree paths to their
/// checked-out branch.
pub fn swarms_from_agents(agents: &[HerdrAgent], branches: &HashMap<PathBuf, String>) -> Vec<SwarmSnapshot> {
    let mut groups: BTreeMap<PathBuf, (Option<&HerdrAgent>, Vec<(u32, &HerdrAgent)>)> = BTreeMap::new();
    for agent in agents {
        let (repo, worker) = place(&agent.cwd);
        let group = groups.entry(repo).or_default();
        match worker {
            Some(n) => group.1.push((n, agent)),
            // Prefer the agent named as the manager when several sit in the repo.
            None if group.0.is_none() || agent.name.as_deref().is_some_and(|n| n.starts_with("manager")) => group.0 = Some(agent),
            None => {}
        }
    }
    groups
        .into_iter()
        .filter(|(_, (manager, workers))| !workers.is_empty() || manager.and_then(|m| m.name.as_deref()).is_some_and(|n| n.starts_with("manager")))
        .map(|(repo, (manager, mut workers))| {
            workers.sort_by_key(|(n, a)| (*n, a.pane_id.clone()));
            let project = repo.file_name().and_then(|b| b.to_str()).unwrap_or("swarm").to_string();
            let branch_of = |cwd: &str| branches.get(Path::new(cwd.trim_end_matches('/')));
            let manager = match manager {
                Some(m) => snapshot(m, &project, "manager", true, branch_of(&m.cwd)),
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
                .map(|(n, a)| snapshot(a, &project, &format!("worker-{n}"), false, branch_of(&a.cwd)))
                .collect();
            let busy = workers.iter().filter(|w| w.state.starts_with("Working")).count();
            let attention = workers.iter().filter(|w| w.waiting_for_input).count();
            SwarmSnapshot {
                agent_type: agents
                    .iter()
                    .find_map(|a| a.agent.clone())
                    .unwrap_or_else(|| "Claude".to_string()),
                project_name: project,
                repo_path: repo.to_string_lossy().into_owned(),
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

/// Swarms under a running herdr server; none when herdr isn't installed or
/// running.
pub async fn collect(transport: &ServerTransport) -> Vec<SwarmSnapshot> {
    let args = ["agent".to_string(), "list".to_string()];
    let Ok(out) = transport.output("herdr", &args, None).await else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    let agents = parse_agents(&String::from_utf8_lossy(&out.stdout));
    let mut branches = HashMap::new();
    let repos: std::collections::BTreeSet<PathBuf> = agents.iter().map(|a| place(&a.cwd).0).collect();
    for repo in repos {
        let args: Vec<String> = ["worktree", "list", "--porcelain"].iter().map(|s| s.to_string()).collect();
        if let Ok(out) = transport.output("git", &args, Some(&repo)).await
            && out.status.success()
        {
            branches.extend(crate::model::swarm::parse_worktree_porcelain(&String::from_utf8_lossy(&out.stdout)));
        }
    }
    swarms_from_agents(&agents, &branches)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn places_worktrees_and_reads_branch_issues() {
        assert_eq!(place("/src/shop-wt-3"), (PathBuf::from("/src/shop"), Some(3)));
        assert_eq!(place("/src/shop"), (PathBuf::from("/src/shop"), None));
        assert_eq!(place("/src/my-wt-x"), (PathBuf::from("/src/my-wt-x"), None));
        assert_eq!(branch_issue("feature/issue-412"), Some(412));
        assert_eq!(branch_issue("issue/88"), Some(88));
        assert_eq!(branch_issue("fix/412-crash"), Some(412));
        assert_eq!(branch_issue("main"), None);
    }

    #[test]
    fn groups_agents_into_swarms_by_cwd() {
        let mut branches = HashMap::new();
        branches.insert(PathBuf::from("/src/shop-wt-2"), "feature/issue-77".to_string());
        let swarms = swarms_from_agents(&parse_agents(LIST), &branches);
        assert_eq!(swarms.len(), 1, "the stray agent in ~/notes is not a swarm");
        let s = &swarms[0];
        assert_eq!(s.project_name, "shop");
        assert_eq!(s.repo_path, "/src/shop");
        assert_eq!(s.manager.state, "Working");
        assert_eq!(s.workers.len(), 2);
        assert_eq!(s.workers[0].role, "worker-1");
        assert!(s.workers[0].waiting_for_input);
        assert_eq!(s.workers[1].current_issue, Some(77));
        assert_eq!(s.workers[1].state, "Working #77");
        assert_eq!(s.workers[1].worktree_path, "/src/shop-wt-2");
        assert_eq!(s.attention_count, 1);
    }

    #[test]
    fn workers_without_a_manager_still_make_a_swarm() {
        let json = r#"{"result":{"agents":[{"pane_id":"w2:p1","agent_status":"idle","cwd":"/src/x-wt-1"}]}}"#;
        let swarms = swarms_from_agents(&parse_agents(json), &HashMap::new());
        assert_eq!(swarms.len(), 1);
        assert_eq!(swarms[0].manager.state, "Missing");
    }
}
