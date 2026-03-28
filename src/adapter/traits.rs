use anyhow::Result;
use std::path::PathBuf;
use crate::model::swarm::{AgentType, Swarm};

/// Configuration for launching a new swarm.
pub struct SwarmConfig {
    pub repo_path: PathBuf,
    pub agent_type: AgentType,
    pub num_workers: u32,
    #[allow(dead_code)]
    pub agents_dir: PathBuf,
}

/// Trait abstracting over different agent runtimes.
#[allow(async_fn_in_trait, dead_code)] // Full lifecycle methods for swarm management
pub trait AgentRuntime {
    /// Launch a new swarm (manager + workers).
    async fn launch(&self, config: &SwarmConfig) -> Result<Swarm>;

    /// Discover existing swarms from running tmux sessions.
    async fn discover(&self, agents_dir: &std::path::Path) -> Result<Vec<Swarm>>;

    /// Send input to an agent's session (with Enter appended).
    async fn send_input(&self, tmux_target: &str, input: &str) -> Result<()>;

    /// Send a raw keystroke to an agent's session.
    /// `key` is either a literal character or a tmux named key (e.g., "Enter", "BSpace", "C-c").
    /// `literal` indicates whether to use tmux's -l flag for literal text.
    async fn send_raw_key(&self, tmux_target: &str, key: &str, literal: bool) -> Result<()>;

    /// Capture current pane output for an agent.
    async fn capture_output(&self, tmux_target: &str) -> Result<String>;

    /// Add a new worker to an existing swarm. Returns the new AgentInfo.
    async fn add_worker(&self, swarm: &Swarm) -> Result<crate::model::swarm::AgentInfo>;

    /// Send `/fix-loop` (or equivalent) to a worker to start it working.
    async fn start_worker_loop(&self, tmux_target: &str) -> Result<()>;

    /// Stop a swarm gracefully.
    async fn stop(&self, swarm: &Swarm) -> Result<()>;

    /// Terminate and clean up a swarm.
    async fn teardown(&self, swarm: &Swarm) -> Result<()>;

    /// Re-launch any agents that have dropped back to a shell (e.g. after a self-update).
    async fn revive_agents(&self, swarm: &Swarm) -> Result<()>;

    /// Switch all agents in a swarm to a new runtime. Kills existing agents, updates
    /// swarm.agent_type, relaunches, and restarts worker loops.
    async fn switch_agent(&self, swarm: &mut Swarm, new_runtime: AgentType) -> Result<()>;

    /// Validate and heal worker infrastructure. Returns descriptions of repairs made.
    /// Ensures each worker has a worktree, tmux pane, and active agent.
    async fn heal_workers(&self, swarm: &mut Swarm) -> Result<Vec<String>>;
}
