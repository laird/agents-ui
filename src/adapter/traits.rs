use anyhow::Result;
use std::path::PathBuf;
use crate::model::swarm::{AgentType, Swarm};

/// Configuration for launching a new swarm.
#[allow(dead_code)]
pub struct SwarmConfig {
    pub repo_path: PathBuf,
    pub agent_type: AgentType,
    pub num_workers: u32,
    pub agents_dir: PathBuf,
}

/// Trait abstracting over different agent runtimes.
#[allow(async_fn_in_trait, dead_code)]
pub trait AgentRuntime {
    /// Launch a new swarm (manager + workers).
    async fn launch(&self, config: &SwarmConfig) -> Result<Swarm>;

    /// Discover existing swarms from running tmux sessions.
    async fn discover(&self, agents_dir: &std::path::Path) -> Result<Vec<Swarm>>;

    /// Send input to an agent's session.
    async fn send_input(&self, tmux_target: &str, input: &str) -> Result<()>;

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
}
