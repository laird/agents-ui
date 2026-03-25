use std::path::PathBuf;
use super::status::AgentStatus;

/// The type of agent runtime.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentType {
    Claude,
    Codex,
    Droid,
    Gemini,
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentType::Claude => write!(f, "Claude"),
            AgentType::Codex => write!(f, "Codex"),
            AgentType::Droid => write!(f, "Droid"),
            AgentType::Gemini => write!(f, "Gemini"),
        }
    }
}

impl AgentType {
    /// CLI flag value for start-parallel-agents.sh --agent
    pub fn script_flag(&self) -> &str {
        match self {
            AgentType::Claude => "claude",
            AgentType::Codex => "codex",
            AgentType::Droid => "droid",
            AgentType::Gemini => "gemini",
        }
    }

    /// Tmux session prefix (e.g., "claude-myrepo")
    pub fn session_prefix(&self) -> &str {
        match self {
            AgentType::Claude => "claude",
            AgentType::Codex => "codex",
            AgentType::Droid => "droid",
            AgentType::Gemini => "gemini",
        }
    }

    /// Status file directory within a worktree
    pub fn status_dir(&self) -> &str {
        match self {
            AgentType::Claude | AgentType::Codex | AgentType::Gemini => ".codex/loops",
            AgentType::Droid => ".factory/loops",
        }
    }
}

/// The workflow type for a swarm.
#[derive(Debug, Clone, PartialEq)]
pub enum Workflow {
    Autocoder,
    Modernize,
}

impl std::fmt::Display for Workflow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Workflow::Autocoder => write!(f, "Autocoder"),
            Workflow::Modernize => write!(f, "Modernize"),
        }
    }
}

/// Info about a single agent (manager or worker).
#[derive(Debug, Clone)]
pub struct AgentInfo {
    /// Unique ID: "manager" or "worker-0", "worker-1", etc.
    pub id: String,
    /// Path to the worktree (base repo for manager)
    pub worktree_path: PathBuf,
    /// tmux pane target (e.g., "claude-myrepo:0.0")
    pub tmux_target: String,
    /// Current status from status file
    pub status: AgentStatus,
    /// Whether this is the manager agent
    pub is_manager: bool,
    /// Captured pane output (latest snapshot)
    pub pane_content: String,
}

impl AgentInfo {
    /// Check if this agent appears to need human attention based on pane content.
    pub fn needs_attention(&self) -> bool {
        let content = &self.pane_content;
        // Check last 20 lines for attention patterns
        for line in content.lines().rev().take(20) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let lower = trimmed.to_lowercase();
            if lower.contains("interrupted")
                || lower.contains("what should claude do")
                || lower.contains("do you want to")
                || lower.contains("waiting for your")
                || lower.contains("permission denied")
                || lower.contains("? (y/n)")
            {
                return true;
            }
        }
        // Also flag idle agents as needing attention
        matches!(self.status.state, super::status::AgentState::Idle)
    }
}

/// A swarm of agents working on one repo.
#[derive(Debug, Clone)]
pub struct Swarm {
    /// Path to the base repository
    pub repo_path: PathBuf,
    /// Project name (derived from repo directory name)
    pub project_name: String,
    /// Agent runtime type
    pub agent_type: AgentType,
    /// Workflow being executed
    pub workflow: Option<Workflow>,
    /// tmux session name (e.g., "claude-myrepo")
    pub tmux_session: String,
    /// The manager agent (runs in base repo)
    pub manager: AgentInfo,
    /// Worker agents (each in their own worktree)
    pub workers: Vec<AgentInfo>,
}

impl Swarm {
    /// Total agent count (manager + workers)
    pub fn agent_count(&self) -> usize {
        1 + self.workers.len()
    }

    /// Count of busy workers
    pub fn busy_count(&self) -> usize {
        self.workers
            .iter()
            .filter(|w| {
                matches!(
                    w.status.state,
                    super::status::AgentState::Working { .. } | super::status::AgentState::Starting
                )
            })
            .count()
    }

    /// Count of items needing attention (idle workers, blocked states)
    pub fn attention_count(&self) -> usize {
        self.workers
            .iter()
            .filter(|w| matches!(w.status.state, super::status::AgentState::Idle))
            .count()
    }

    /// Get a specific agent by ID
    pub fn agent(&self, id: &str) -> Option<&AgentInfo> {
        if self.manager.id == id {
            Some(&self.manager)
        } else {
            self.workers.iter().find(|w| w.id == id)
        }
    }

    /// Get a mutable reference to a specific agent by ID
    pub fn agent_mut(&mut self, id: &str) -> Option<&mut AgentInfo> {
        if self.manager.id == id {
            Some(&mut self.manager)
        } else {
            self.workers.iter_mut().find(|w| w.id == id)
        }
    }

    /// Get all agents (manager + workers) as a flat list
    pub fn all_agents(&self) -> Vec<&AgentInfo> {
        let mut agents = vec![&self.manager];
        agents.extend(self.workers.iter());
        agents
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::status::{AgentState, AgentStatus};

    fn make_agent(id: &str, state: AgentState, pane_content: &str) -> AgentInfo {
        AgentInfo {
            id: id.to_string(),
            worktree_path: PathBuf::from("/tmp/test"),
            tmux_target: format!("test:0.0"),
            status: AgentStatus {
                timestamp: None,
                state,
            },
            is_manager: id == "manager",
            pane_content: pane_content.to_string(),
        }
    }

    fn make_swarm(workers: Vec<AgentInfo>) -> Swarm {
        Swarm {
            repo_path: PathBuf::from("/tmp/repo"),
            project_name: "test".to_string(),
            agent_type: AgentType::Claude,
            workflow: Some(Workflow::Autocoder),
            tmux_session: "claude-test".to_string(),
            manager: make_agent("manager", AgentState::Working { issue: None }, ""),
            workers,
        }
    }

    // --- AgentType tests ---

    #[test]
    fn agent_type_display() {
        assert_eq!(AgentType::Claude.to_string(), "Claude");
        assert_eq!(AgentType::Codex.to_string(), "Codex");
        assert_eq!(AgentType::Droid.to_string(), "Droid");
        assert_eq!(AgentType::Gemini.to_string(), "Gemini");
    }

    #[test]
    fn agent_type_script_flag() {
        assert_eq!(AgentType::Claude.script_flag(), "claude");
        assert_eq!(AgentType::Codex.script_flag(), "codex");
        assert_eq!(AgentType::Droid.script_flag(), "droid");
        assert_eq!(AgentType::Gemini.script_flag(), "gemini");
    }

    #[test]
    fn agent_type_session_prefix() {
        assert_eq!(AgentType::Claude.session_prefix(), "claude");
        assert_eq!(AgentType::Droid.session_prefix(), "droid");
    }

    #[test]
    fn agent_type_status_dir() {
        assert_eq!(AgentType::Claude.status_dir(), ".codex/loops");
        assert_eq!(AgentType::Codex.status_dir(), ".codex/loops");
        assert_eq!(AgentType::Gemini.status_dir(), ".codex/loops");
        assert_eq!(AgentType::Droid.status_dir(), ".factory/loops");
    }

    // --- Workflow Display ---

    #[test]
    fn workflow_display() {
        assert_eq!(Workflow::Autocoder.to_string(), "Autocoder");
        assert_eq!(Workflow::Modernize.to_string(), "Modernize");
    }

    // --- AgentInfo::needs_attention tests ---

    #[test]
    fn needs_attention_idle_agent() {
        let agent = make_agent("w-0", AgentState::Idle, "some output");
        assert!(agent.needs_attention());
    }

    #[test]
    fn needs_attention_working_agent() {
        let agent = make_agent("w-0", AgentState::Working { issue: Some(42) }, "doing stuff");
        assert!(!agent.needs_attention());
    }

    #[test]
    fn needs_attention_permission_prompt() {
        let agent = make_agent(
            "w-0",
            AgentState::Working { issue: None },
            "some output\nWhat should Claude do? (y/n)\n",
        );
        assert!(agent.needs_attention());
    }

    #[test]
    fn needs_attention_interrupted() {
        let agent = make_agent(
            "w-0",
            AgentState::Working { issue: None },
            "output\nProcess was interrupted\n",
        );
        assert!(agent.needs_attention());
    }

    #[test]
    fn needs_attention_permission_denied() {
        let agent = make_agent(
            "w-0",
            AgentState::Working { issue: None },
            "trying stuff\npermission denied for file\n",
        );
        assert!(agent.needs_attention());
    }

    #[test]
    fn needs_attention_do_you_want() {
        let agent = make_agent(
            "w-0",
            AgentState::Working { issue: None },
            "stuff\nDo you want to continue?\n",
        );
        assert!(agent.needs_attention());
    }

    #[test]
    fn needs_attention_empty_pane() {
        let agent = make_agent("w-0", AgentState::Working { issue: None }, "");
        assert!(!agent.needs_attention());
    }

    // --- Swarm method tests ---

    #[test]
    fn swarm_agent_count() {
        let swarm = make_swarm(vec![
            make_agent("w-0", AgentState::Idle, ""),
            make_agent("w-1", AgentState::Working { issue: None }, ""),
        ]);
        assert_eq!(swarm.agent_count(), 3); // manager + 2 workers
    }

    #[test]
    fn swarm_agent_count_no_workers() {
        let swarm = make_swarm(vec![]);
        assert_eq!(swarm.agent_count(), 1); // just manager
    }

    #[test]
    fn swarm_busy_count() {
        let swarm = make_swarm(vec![
            make_agent("w-0", AgentState::Idle, ""),
            make_agent("w-1", AgentState::Working { issue: Some(1) }, ""),
            make_agent("w-2", AgentState::Starting, ""),
            make_agent("w-3", AgentState::Stopped, ""),
        ]);
        assert_eq!(swarm.busy_count(), 2); // Working + Starting
    }

    #[test]
    fn swarm_busy_count_none_busy() {
        let swarm = make_swarm(vec![
            make_agent("w-0", AgentState::Idle, ""),
            make_agent("w-1", AgentState::Stopped, ""),
        ]);
        assert_eq!(swarm.busy_count(), 0);
    }

    #[test]
    fn swarm_attention_count() {
        let swarm = make_swarm(vec![
            make_agent("w-0", AgentState::Idle, ""),
            make_agent("w-1", AgentState::Working { issue: None }, ""),
            make_agent("w-2", AgentState::Idle, ""),
        ]);
        assert_eq!(swarm.attention_count(), 2);
    }

    #[test]
    fn swarm_agent_lookup_manager() {
        let swarm = make_swarm(vec![make_agent("w-0", AgentState::Idle, "")]);
        let agent = swarm.agent("manager");
        assert!(agent.is_some());
        assert!(agent.unwrap().is_manager);
    }

    #[test]
    fn swarm_agent_lookup_worker() {
        let swarm = make_swarm(vec![make_agent("w-0", AgentState::Idle, "")]);
        let agent = swarm.agent("w-0");
        assert!(agent.is_some());
        assert_eq!(agent.unwrap().id, "w-0");
    }

    #[test]
    fn swarm_agent_lookup_missing() {
        let swarm = make_swarm(vec![]);
        assert!(swarm.agent("nonexistent").is_none());
    }

    #[test]
    fn swarm_agent_mut_worker() {
        let mut swarm = make_swarm(vec![make_agent("w-0", AgentState::Idle, "")]);
        let agent = swarm.agent_mut("w-0");
        assert!(agent.is_some());
        agent.unwrap().pane_content = "updated".to_string();
        assert_eq!(swarm.agent("w-0").unwrap().pane_content, "updated");
    }

    #[test]
    fn swarm_all_agents() {
        let swarm = make_swarm(vec![
            make_agent("w-0", AgentState::Idle, ""),
            make_agent("w-1", AgentState::Working { issue: None }, ""),
        ]);
        let all = swarm.all_agents();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].id, "manager");
        assert_eq!(all[1].id, "w-0");
        assert_eq!(all[2].id, "w-1");
    }
}
