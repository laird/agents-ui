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

#[allow(dead_code)]
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
            AgentType::Codex => "codex --dangerously-skip-permissions",
            AgentType::Droid => "droid",
            AgentType::Gemini => "gemini --sandbox=false",
        }
    }

    /// The slash command to start the worker fix-loop.
    pub fn worker_loop_cmd(&self) -> &str {
        match self {
            AgentType::Claude => "/autocoder:fix-loop",
            AgentType::Codex => "use autocoder to fix-loop",
            _ => "/fix-loop",
        }
    }

    /// The slash command to run manager dispatch/monitor workflow.
    pub fn monitor_workers_cmd(&self) -> &str {
        match self {
            AgentType::Claude => "/autocoder:monitor-workers",
            AgentType::Codex => "use autocoder to monitor-workers",
            _ => "/monitor-workers",
        }
    }

    /// Normalize direct user input for runtimes that do not support Claude-style slash commands.
    pub fn normalize_input(&self, input: &str) -> String {
        if !matches!(self, AgentType::Codex) {
            return input.to_string();
        }

        let trimmed = input.trim();
        let mapped = match trimmed {
            "/monitor-workers" | "/autocoder:monitor-workers" => {
                Some("monitor-workers".to_string())
            }
            "/fix-loop" | "/autocoder:fix-loop" => Some("fix-loop".to_string()),
            "/fix" | "/autocoder:fix" => Some("fix".to_string()),
            _ => trimmed
                .strip_prefix("/fix ")
                .or_else(|| trimmed.strip_prefix("/autocoder:fix "))
                .map(|rest| format!("fix {}", rest.trim())),
        };

        match mapped {
            Some(cmd) => format!("use autocoder to {cmd}"),
            None => input.to_string(),
        }
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

#[cfg(test)]
mod tests {
    use super::AgentType;

    #[test]
    fn command_mapping_for_worker_and_monitor_matches_runtime() {
        assert_eq!(AgentType::Claude.worker_loop_cmd(), "/autocoder:fix-loop");
        assert_eq!(
            AgentType::Claude.monitor_workers_cmd(),
            "/autocoder:monitor-workers"
        );

        assert_eq!(AgentType::Codex.worker_loop_cmd(), "use autocoder to fix-loop");
        assert_eq!(
            AgentType::Codex.monitor_workers_cmd(),
            "use autocoder to monitor-workers"
        );

        for agent_type in [AgentType::Droid, AgentType::Gemini] {
            assert_eq!(agent_type.worker_loop_cmd(), "/fix-loop");
            assert_eq!(agent_type.monitor_workers_cmd(), "/monitor-workers");
        }
    }

    #[test]
    fn codex_normalizes_slash_inputs_to_skill_prompts() {
        let codex = AgentType::Codex;
        assert_eq!(
            codex.normalize_input("/monitor-workers"),
            "use autocoder to monitor-workers"
        );
        assert_eq!(
            codex.normalize_input("/autocoder:monitor-workers"),
            "use autocoder to monitor-workers"
        );
        assert_eq!(codex.normalize_input("/fix"), "use autocoder to fix");
        assert_eq!(
            codex.normalize_input("/fix 240"),
            "use autocoder to fix 240"
        );
        assert_eq!(
            codex.normalize_input("/autocoder:fix 240"),
            "use autocoder to fix 240"
        );
        assert_eq!(
            codex.normalize_input("plain text"),
            "plain text"
        );
    }
}

#[allow(dead_code)]
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
