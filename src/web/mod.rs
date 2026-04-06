pub mod discovery;

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
}

/// Snapshot of a full swarm (manager + workers).
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

/// Shared state between the web server and background discovery task.
pub type SharedWebState = Arc<RwLock<Vec<SwarmSnapshot>>>;
