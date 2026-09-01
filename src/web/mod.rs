pub mod discovery;
pub mod server;

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
    /// Path to the agent's worktree (base repo for manager), if known.
    pub worktree_path: String,
    /// Git branch checked out in the agent's worktree, if resolvable.
    pub branch: Option<String>,
    /// Health status: "Healthy", "Stalled", "Restarting", or "Dead".
    pub health: String,
    /// Number of completed issues this session.
    pub completed_issue_count: u32,
    /// Number of times this agent has been auto-restarted this session.
    pub resurrection_attempts: u32,
    /// ISO-8601 timestamp of last status update, if available.
    pub status_timestamp: Option<String>,
}

/// A single GitHub issue suitable for JSON serialization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
}

/// Snapshot of a full swarm suitable for JSON serialization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwarmSnapshot {
    pub project_name: String,
    pub repo_path: String,
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

/// Thread-safe shared state between the TUI app and the web server.
pub type SharedWebState = Arc<RwLock<Vec<SwarmSnapshot>>>;

/// Create an empty shared web state.
pub fn new_shared_state() -> SharedWebState {
    Arc::new(RwLock::new(Vec::new()))
}

impl AgentSnapshot {
    pub fn from_agent_info(info: &crate::model::swarm::AgentInfo) -> Self {
        use crate::model::swarm::HealthStatus;
        let health = match info.health.status() {
            HealthStatus::Healthy    => "Healthy",
            HealthStatus::Stalled    => "Stalled",
            HealthStatus::Restarting => "Restarting",
            HealthStatus::Dead       => "Dead",
        };
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
            worktree_path: info.worktree_path.to_string_lossy().into_owned(),
            branch: info.branch.clone(),
            health: health.to_string(),
            completed_issue_count: info.completed_issue_count,
            resurrection_attempts: info.resurrection_attempts,
            status_timestamp,
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
            .map(|i| IssueSnapshot::from_issue(i, &swarm.project_name))
            .collect();
        Self {
            project_name: swarm.project_name.clone(),
            repo_path: swarm.repo_path.to_string_lossy().into_owned(),
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
    pub fn from_issue(issue: &crate::model::issue::GitHubIssue, project_name: &str) -> Self {
        // Derive GitHub URL from project name (owner/repo format expected in project_name or
        // use a best-effort URL; the actual repo URL is in the gh remote).
        let url = format!("https://github.com/{project_name}/issues/{}", issue.number);
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
            worktree_path: PathBuf::from("/tmp/test"),
            branch: None,
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
