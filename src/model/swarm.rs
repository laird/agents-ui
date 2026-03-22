use std::path::PathBuf;
use super::status::AgentStatus;

/// The type of agent runtime.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // Codex, Droid, Gemini are planned runtime adapters
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
    #[allow(dead_code)] // Used by multi-runtime discovery
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
#[allow(dead_code)] // Workflow variants are planned for use in swarm configuration
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

/// A swarm of agents working on one repo.
#[derive(Debug, Clone)]
#[allow(dead_code)] // repo_path, tmux_session used by persistence and multi-runtime features
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn all_agents(&self) -> Vec<&AgentInfo> {
        let mut agents = vec![&self.manager];
        agents.extend(self.workers.iter());
        agents
    }
}
