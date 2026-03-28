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

    /// The shell command to launch this agent with autonomous permissions.
    pub fn launch_cmd(&self) -> &str {
        match self {
            AgentType::Claude => "claude code --dangerously-skip-permissions .",
            AgentType::Codex => "codex",
            AgentType::Droid => "droid",
            AgentType::Gemini => "gemini --sandbox=false",
        }
    }

    /// The slash command to start the worker fix-loop.
    #[allow(dead_code)]
    pub fn worker_loop_cmd(&self) -> &str {
        match self {
            AgentType::Claude => "/autocoder:fix-loop",
            AgentType::Codex | AgentType::Droid => "",
            AgentType::Gemini => "/fix-loop",
        }
    }

    pub fn from_name(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "claude" => Some(AgentType::Claude),
            "codex" => Some(AgentType::Codex),
            "droid" => Some(AgentType::Droid),
            "gemini" => Some(AgentType::Gemini),
            _ => None,
        }
    }
}

impl std::str::FromStr for AgentType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        AgentType::from_name(s).ok_or(())
    }
}

/// The workflow type for a swarm.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
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
    /// Globally unique ID: "nextgen-CDD/manager" or "agents-ui/worker-1"
    pub id: String,
    /// Role within the swarm: "manager", "worker-1", "tester", etc.
    pub role: String,
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
    /// Issue number currently assigned by the TUI dispatcher (None = unassigned)
    pub dispatched_issue: Option<u32>,
    /// Current issue number from JSON status file
    pub current_issue: Option<u32>,
    /// Current issue title from JSON status file
    pub current_issue_title: Option<String>,
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

    /// Get a specific agent by role (e.g., "manager", "worker-1")
    pub fn agent(&self, role: &str) -> Option<&AgentInfo> {
        if self.manager.role == role {
            Some(&self.manager)
        } else {
            self.workers.iter().find(|w| w.role == role)
        }
    }

    /// Get a mutable reference to a specific agent by role
    #[allow(dead_code)]
    pub fn agent_mut(&mut self, role: &str) -> Option<&mut AgentInfo> {
        if self.manager.role == role {
            Some(&mut self.manager)
        } else {
            self.workers.iter_mut().find(|w| w.role == role)
        }
    }

    /// Get a specific agent by globally unique ID (e.g., "nextgen-CDD/manager")
    #[allow(dead_code)]
    pub fn agent_by_id(&self, id: &str) -> Option<&AgentInfo> {
        if self.manager.id == id {
            Some(&self.manager)
        } else {
            self.workers.iter().find(|w| w.id == id)
        }
    }

    /// Get a mutable reference to a specific agent by globally unique ID
    pub fn agent_by_id_mut(&mut self, id: &str) -> Option<&mut AgentInfo> {
        if self.manager.id == id {
            Some(&mut self.manager)
        } else {
            self.workers.iter_mut().find(|w| w.id == id)
        }
    }

}

#[cfg(test)]
mod tests {
    use super::AgentType;

    #[test]
    fn codex_and_droid_launch_interactive_sessions() {
        assert_eq!(AgentType::Codex.launch_cmd(), "codex");
        assert_eq!(AgentType::Droid.launch_cmd(), "droid");
    }

    #[test]
    fn claude_and_gemini_keep_inline_launch_commands() {
        assert!(AgentType::Claude.launch_cmd().contains("claude code"));
        assert!(AgentType::Gemini.launch_cmd().contains("gemini"));
    }

    #[test]
    fn worker_loop_commands_match_runtime_model() {
        assert_eq!(AgentType::Claude.worker_loop_cmd(), "/autocoder:fix-loop");
        assert_eq!(AgentType::Gemini.worker_loop_cmd(), "/fix-loop");
        assert_eq!(AgentType::Codex.worker_loop_cmd(), "");
        assert_eq!(AgentType::Droid.worker_loop_cmd(), "");
    }

    #[test]
    fn status_directories_match_runtime_storage() {
        assert_eq!(AgentType::Codex.status_dir(), ".codex/loops");
        assert_eq!(AgentType::Claude.status_dir(), ".codex/loops");
        assert_eq!(AgentType::Droid.status_dir(), ".factory/loops");
    }
}
