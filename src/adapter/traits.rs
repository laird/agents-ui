use anyhow::Result;
use std::path::PathBuf;
use crate::model::swarm::{AgentType, Swarm};

/// Configuration for launching a new swarm.
pub struct SwarmConfig {
    pub repo_path: PathBuf,
    pub agent_type: AgentType,
    pub num_workers: u32,
    pub agents_dir: PathBuf,
}

/// Trait abstracting over different agent runtimes.
#[allow(async_fn_in_trait)]
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

    /// Stop a swarm gracefully.
    async fn stop(&self, swarm: &Swarm) -> Result<()>;

    /// Terminate and clean up a swarm.
    async fn teardown(&self, swarm: &Swarm) -> Result<()>;
}
