pub mod deployments;
pub mod discovery;
pub mod feature;
pub mod framework;
pub mod gate;
pub mod herdr;
pub mod kubernetes;
pub mod pane;
pub mod server;
pub mod source;
pub mod temporal;
pub mod village;
pub mod world;

use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// Snapshot of a single agent suitable for JSON serialization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentSnapshot {
    pub id: String,
    pub role: String,
    pub state: String,
    pub is_manager: bool,
    pub waiting_for_input: bool,
    pub current_issue: Option<u32>,
    pub current_issue_title: Option<String>,
    /// Latest captured pane output (updated each TUI tick).
    pub pane_content: String,
    /// tmux pane target (e.g., "claude-myrepo:0.0") used to send input.
    pub tmux_target: String,
    /// Health status: "Healthy", "Stalled", "Restarting", or "Dead".
    pub health: String,
    /// Number of completed issues this session.
    pub completed_issue_count: u32,
    /// Number of times this agent has been auto-restarted this session.
    pub resurrection_attempts: u32,
    /// ISO-8601 timestamp of last status update, if available.
    pub status_timestamp: Option<String>,
    /// Absolute path of the worktree this agent is working in.
    #[serde(default)]
    pub worktree_path: String,
    /// Git branch checked out in that worktree. `None` for a detached HEAD.
    #[serde(default)]
    pub branch: Option<String>,
    /// The terminal multiplexer the agent runs in, which decides how its pane
    /// is read and typed into (`tmux_target` is a tmux target or a herdr pane id).
    #[serde(default)]
    pub mux: Mux,
}

/// Where an agent's terminal lives.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mux {
    #[default]
    Tmux,
    Herdr,
}

/// A single GitHub issue suitable for JSON serialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct IssueSnapshot {
    pub number: u32,
    pub title: String,
    pub state: String,
    pub priority: String,
    pub labels: Vec<String>,
    pub is_blocked: bool,
    pub is_working: bool,
    pub assigned_worker: Option<String>,
    /// GitHub URL for linking.
    pub url: String,
    /// Issues this one depends on, from "Depends on" / "Blocked by" lines in its body.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<u32>,
}

/// Snapshot of a full swarm suitable for JSON serialization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwarmSnapshot {
    pub project_name: String,
    pub repo_path: String,
    /// The repo's identity: its git common directory, absolute. Empty when
    /// the source couldn't ask git (a saved swarm, a repo it couldn't find).
    /// Two swarms with the same key are one swarm (`source::discover`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub repo_key: String,
    /// How the manager was chosen (`model::recognize::choose_manager`): the
    /// rule that matched and how many agents it matched. None when the source
    /// didn't choose by evidence (a saved swarm) or found no manager.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manager_by: Option<ManagerPick>,
    pub agent_type: String,
    pub workflow: Option<String>,
    pub tmux_session: String,
    pub stopped: bool,
    pub busy_count: usize,
    pub idle_count: usize,
    pub attention_count: usize,
    pub manager: AgentSnapshot,
    pub workers: Vec<AgentSnapshot>,
    /// Open GitHub issues for this swarm's repository.
    pub issues: Vec<IssueSnapshot>,
}

/// Which evidence picked a swarm's manager.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ManagerPick {
    pub rule: crate::model::recognize::ManagerRule,
    /// How many agents the rule matched; more than one means the choice was a
    /// deterministic pick among equals.
    pub candidates: usize,
    /// Several agents matched the winning rule equally and one was picked.
    pub ambiguous: bool,
}

impl From<crate::model::recognize::ManagerChoice> for ManagerPick {
    fn from(c: crate::model::recognize::ManagerChoice) -> Self {
        Self { rule: c.rule, candidates: c.candidates, ambiguous: c.ambiguous() }
    }
}

/// Thread-safe shared state between the TUI app and the web server.
pub type SharedWebState = Arc<RwLock<Vec<SwarmSnapshot>>>;

/// Create an empty shared web state.
pub fn new_shared_state() -> SharedWebState {
    Arc::new(RwLock::new(Vec::new()))
}

impl AgentSnapshot {
    pub fn from_agent_info(info: &crate::model::swarm::AgentInfo) -> Self {
        let health = info.health.status().as_str();
        let status_timestamp = info.status.timestamp.map(|ts| {
            ts.format("%Y-%m-%dT%H:%M:%SZ").to_string()
        });
        Self {
            id: info.id.clone(),
            role: info.role.clone(),
            state: info.status.state.to_string(),
            is_manager: info.is_manager,
            waiting_for_input: info.waiting_for_input,
            current_issue: info.current_issue,
            current_issue_title: info.current_issue_title.clone(),
            pane_content: info.pane_content.clone(),
            tmux_target: info.tmux_target.clone(),
            health: health.to_string(),
            completed_issue_count: info.completed_issue_count,
            resurrection_attempts: info.resurrection_attempts,
            status_timestamp,
            worktree_path: info.worktree_path.to_string_lossy().into_owned(),
            branch: info.branch.clone(),
            mux: Mux::Tmux,
        }
    }
}

impl SwarmSnapshot {
    pub fn from_swarm(swarm: &crate::model::swarm::Swarm) -> Self {
        let issues = swarm
            .issue_cache
            .issues
            .iter()
            .filter(|i| i.state == crate::model::issue::IssueState::Open)
            .map(|i| IssueSnapshot::from_issue(i, None))
            .collect();
        Self {
            project_name: swarm.project_name.clone(),
            repo_path: swarm.repo_path.to_string_lossy().into_owned(),
            repo_key: String::new(),
            manager_by: None,
            agent_type: swarm.agent_type.to_string(),
            workflow: swarm.workflow.as_ref().map(|w| w.to_string()),
            tmux_session: swarm.tmux_session.clone(),
            stopped: swarm.stopped,
            busy_count: swarm.busy_count(),
            idle_count: swarm.idle_count(),
            attention_count: swarm.attention_count(),
            manager: AgentSnapshot::from_agent_info(&swarm.manager),
            workers: swarm
                .workers
                .iter()
                .map(AgentSnapshot::from_agent_info)
                .collect(),
            issues,
        }
    }
}

impl IssueSnapshot {
    /// `repo_url` is the repo's GitHub page (`https://github.com/<owner>/<repo>`),
    /// used only when the tracker didn't report the issue's own URL. With
    /// neither, `url` is empty and pages show the number without a link.
    pub fn from_issue(issue: &crate::model::issue::GitHubIssue, repo_url: Option<&str>) -> Self {
        let url = issue
            .url
            .clone()
            .or_else(|| repo_url.map(|r| format!("{}/issues/{}", r.trim_end_matches('/'), issue.number)))
            .unwrap_or_default();
        let state = match issue.state {
            crate::model::issue::IssueState::Open => "open",
            crate::model::issue::IssueState::Closed => "closed",
        };
        Self {
            number: issue.number,
            title: issue.title.clone(),
            state: state.to_string(),
            priority: issue.priority_label(),
            labels: issue.labels.clone(),
            is_blocked: issue.is_blocked(),
            is_working: issue.is_being_worked(),
            assigned_worker: issue.assigned_worker.clone(),
            url,
            depends_on: issue.depends_on.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::status::{AgentState, AgentStatus};
    use crate::model::swarm::{AgentInfo, AgentType, Swarm};
    use std::path::PathBuf;

    fn make_agent_info(role: &str, is_manager: bool, state: AgentState) -> AgentInfo {
        AgentInfo {
            id: format!("test-repo/{role}"),
            role: role.to_string(),
            branch: None,
            worktree_path: PathBuf::from("/tmp/test"),
            tmux_target: "test:0.0".to_string(),
            status: AgentStatus {
                timestamp: None,
                state,
            },
            is_manager,
            pane_content: String::new(),
            dispatched_issue: None,
            current_issue: None,
            current_issue_title: None,
            waiting_for_input: false,
            resurrection_attempts: 0,
            completed_issue_count: 0,
            health: crate::model::swarm::WorkerHealth::default(),
        }
    }

    fn make_swarm() -> Swarm {
        Swarm {
            repo_path: PathBuf::from("/repos/test-repo"),
            project_name: "test-repo".to_string(),
            agent_type: AgentType::Claude,
            workflow: None,
            tmux_session: "claude-test-repo".to_string(),
            manager: make_agent_info("manager", true, AgentState::Working { issue: Some(42) }),
            workers: vec![
                make_agent_info("worker-1", false, AgentState::Working { issue: Some(7) }),
                make_agent_info("worker-2", false, AgentState::Idle),
            ],
            issue_cache: crate::model::issue::IssueCache::default(),
            stopped: false,
        }
    }

    #[test]
    fn agent_snapshot_from_working_manager() {
        let info = make_agent_info("manager", true, AgentState::Working { issue: Some(42) });
        let snap = AgentSnapshot::from_agent_info(&info);
        assert_eq!(snap.role, "manager");
        assert!(snap.is_manager);
        assert_eq!(snap.state, "Working #42");
        assert_eq!(snap.current_issue, None); // AgentInfo.current_issue not set in test
        assert!(!snap.waiting_for_input);
    }

    #[test]
    fn agent_snapshot_from_idle_worker() {
        let info = make_agent_info("worker-1", false, AgentState::Idle);
        let snap = AgentSnapshot::from_agent_info(&info);
        assert_eq!(snap.role, "worker-1");
        assert!(!snap.is_manager);
        assert_eq!(snap.state, "Idle");
    }

    #[test]
    fn swarm_snapshot_from_swarm() {
        let swarm = make_swarm();
        let snap = SwarmSnapshot::from_swarm(&swarm);
        assert_eq!(snap.project_name, "test-repo");
        assert_eq!(snap.agent_type, "Claude");
        assert_eq!(snap.repo_path, "/repos/test-repo");
        assert_eq!(snap.tmux_session, "claude-test-repo");
        assert!(!snap.stopped);
        assert_eq!(snap.busy_count, 1); // 1 working worker
        assert_eq!(snap.idle_count, 1); // 1 idle worker
        assert_eq!(snap.workers.len(), 2);
    }

    #[test]
    fn swarm_snapshot_serializes_to_json() {
        let swarm = make_swarm();
        let snap = SwarmSnapshot::from_swarm(&swarm);
        let json = serde_json::to_string(&snap).unwrap();
        assert!(json.contains("test-repo"));
        assert!(json.contains("Claude"));
        assert!(json.contains("manager"));
        assert!(json.contains("worker-1"));
    }

    fn open_issue(number: u32, url: Option<&str>) -> crate::model::issue::GitHubIssue {
        crate::model::issue::GitHubIssue {
            number,
            title: "t".into(),
            state: crate::model::issue::IssueState::Open,
            priority: crate::model::issue::IssuePriority::P2,
            issue_type: crate::model::issue::IssueType::Bug,
            labels: vec![],
            is_working: false,
            assigned_worker: None,
            updated_at: None,
            url: url.map(String::from),
            depends_on: Vec::new(),
        }
    }

    #[test]
    fn issue_snapshot_prefers_the_url_the_tracker_reported() {
        let issue = open_issue(358, Some("https://github.com/laird/agents-ui/issues/358"));
        let snap = IssueSnapshot::from_issue(&issue, Some("https://github.com/someone/else"));
        assert_eq!(snap.url, "https://github.com/laird/agents-ui/issues/358");
    }

    #[test]
    fn issue_snapshot_falls_back_to_the_repo_page() {
        let snap = IssueSnapshot::from_issue(&open_issue(7, None), Some("https://github.com/laird/agents-ui"));
        assert_eq!(snap.url, "https://github.com/laird/agents-ui/issues/7");
    }

    #[test]
    fn issue_snapshot_without_a_github_repo_has_no_link() {
        // Not `https://github.com/<dir>/issues/N` built from the checkout's name.
        let snap = IssueSnapshot::from_issue(&open_issue(2116, None), None);
        assert_eq!(snap.url, "");
        let mut swarm = make_swarm();
        swarm.issue_cache.issues = vec![open_issue(2116, None)];
        assert_eq!(SwarmSnapshot::from_swarm(&swarm).issues[0].url, "");
    }

    #[test]
    fn shared_state_can_be_written_and_read() {
        let state = new_shared_state();
        let swarm = make_swarm();
        let snap = SwarmSnapshot::from_swarm(&swarm);
        {
            let mut guard = state.write().unwrap();
            guard.push(snap.clone());
        }
        let read = state.read().unwrap();
        assert_eq!(read.len(), 1);
        assert_eq!(read[0], snap);
    }
}
