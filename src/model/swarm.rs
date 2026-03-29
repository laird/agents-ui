use std::path::PathBuf;
use super::issue::IssueCache;
use super::status::AgentStatus;

/// The type of agent runtime.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentType {
    Claude,
    Codex,
    Droid,
    Gemini,
}

/// All supported agent types, in display order.
pub const ALL_AGENT_TYPES: &[AgentType] = &[
    AgentType::Claude,
    AgentType::Codex,
    AgentType::Droid,
    AgentType::Gemini,
];

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

    /// Parse agent type from tmux session prefix (e.g., "claude" → Claude).
    #[allow(dead_code)] // Available for future use in dynamic session parsing
    pub fn from_prefix(prefix: &str) -> Option<AgentType> {
        match prefix {
            "claude" => Some(AgentType::Claude),
            "codex" => Some(AgentType::Codex),
            "droid" => Some(AgentType::Droid),
            "gemini" => Some(AgentType::Gemini),
            _ => None,
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
    pub fn worker_loop_cmd(&self) -> &str {
        match self {
            AgentType::Claude => "/autocoder:fix-loop",
            AgentType::Codex | AgentType::Droid => "",
            AgentType::Gemini => "/fix-loop",
        }
    }

    /// Status file directory within a worktree
    pub fn status_dir(&self) -> &str {
        match self {
            AgentType::Claude | AgentType::Codex | AgentType::Gemini => ".codex/loops",
            AgentType::Droid => ".factory/loops",
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
#[allow(dead_code)] // Planned for workflow display in repos list
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
    /// Whether the agent is waiting for user input (detected from pane content)
    pub waiting_for_input: bool,
    /// Number of times the TUI has attempted to revive this agent in the current session.
    pub resurrection_attempts: u32,
}

/// Detect if pane content indicates the session is waiting for user input.
pub fn detect_waiting_for_input(content: &str) -> bool {
    // Look at the last ~15 lines for waiting indicators
    let tail: Vec<&str> = content.lines().rev().take(15).collect();
    let tail_text = tail.iter().rev().copied().collect::<Vec<_>>().join("\n");

    // Permission prompts
    if tail_text.contains("bypass permissions")
        || tail_text.contains("Allow?")
        || tail_text.contains("allow this action")
        || tail_text.contains("(y/n)")
        || tail_text.contains("[Y/n]")
        || tail_text.contains("[y/N]")
    {
        return true;
    }

    // Interrupted state
    if tail_text.contains("What should Claude do instead?") {
        return true;
    }

    // AskUserQuestion or similar prompts
    if tail_text.contains("Interrupted") && tail_text.contains("❯") {
        return true;
    }

    // Bare prompt at end with no active work (idle at prompt after interruption)
    // Check if the very last non-empty line is just a prompt
    let last_lines: Vec<&str> = content
        .lines()
        .rev()
        .filter(|l| !l.trim().is_empty())
        .take(3)
        .collect();

    if let Some(last) = last_lines.first() {
        let trimmed = last.trim();
        // Permission bypass prompt line
        if trimmed.contains("bypass permissions on") && trimmed.contains("shift+tab") {
            return true;
        }
    }

    false
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
    /// Cached GitHub issues
    pub issue_cache: IssueCache,
}

#[allow(dead_code)] // Utility methods for future UI enhancements
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

    /// Count of items needing human attention (blocked issues in the issue cache).
    pub fn attention_count(&self) -> usize {
        self.issue_cache
            .issues
            .iter()
            .filter(|i| i.is_blocked())
            .count()
    }

    /// Count of idle workers.
    pub fn idle_count(&self) -> usize {
        self.workers
            .iter()
            .filter(|w| matches!(w.status.state, super::status::AgentState::Idle))
            .count()
    }

    /// Count of agents waiting for user input
    pub fn waiting_count(&self) -> usize {
        let mut count = 0;
        if self.manager.waiting_for_input {
            count += 1;
        }
        count += self.workers.iter().filter(|w| w.waiting_for_input).count();
        count
    }

    /// Get all agents (manager first, then workers).
    pub fn all_agents(&self) -> Vec<&AgentInfo> {
        let mut all = vec![&self.manager];
        all.extend(self.workers.iter());
        all
    }

    /// Get the next agent waiting for input, starting after `after_id`.
    /// Returns None if no agent is waiting.
    pub fn next_waiting_agent(&self, after_id: Option<&str>) -> Option<&AgentInfo> {
        let all = self.all_agents();
        let start_idx = after_id
            .and_then(|id| all.iter().position(|a| a.id == id))
            .map(|i| i + 1)
            .unwrap_or(0);

        // Search from start_idx, wrapping around
        for i in 0..all.len() {
            let idx = (start_idx + i) % all.len();
            if all[idx].waiting_for_input {
                return Some(all[idx]);
            }
        }
        None
    }

    /// Get a specific agent by ID
    pub fn agent(&self, id: &str) -> Option<&AgentInfo> {
        if self.manager.id == id || self.manager.role == id {
            Some(&self.manager)
        } else {
            self.workers.iter().find(|w| w.id == id || w.role == id)
        }
    }

    /// Get a mutable reference to a specific agent by role
    pub fn agent_mut(&mut self, role: &str) -> Option<&mut AgentInfo> {
        if self.manager.role == role || self.manager.id == role {
            Some(&mut self.manager)
        } else {
            self.workers.iter_mut().find(|w| w.role == role || w.id == role)
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
    use super::*;
    use crate::model::issue::{GitHubIssue, IssueCache, IssueState, IssuePriority, IssueType};
    use crate::model::status::AgentStatus;

    fn make_swarm_with_issues(issues: Vec<GitHubIssue>) -> Swarm {
        let manager = AgentInfo {
            id: "test/manager".to_string(),
            role: "manager".to_string(),
            worktree_path: PathBuf::from("/tmp/test"),
            tmux_target: "test:0.0".to_string(),
            status: AgentStatus::default(),
            is_manager: true,
            pane_content: String::new(),
            dispatched_issue: None,
            current_issue: None,
            current_issue_title: None,
            waiting_for_input: false,
            resurrection_attempts: 0,
        };
        let mut cache = IssueCache::default();
        cache.issues = issues;
        Swarm {
            repo_path: PathBuf::from("/tmp/test"),
            project_name: "test".to_string(),
            agent_type: AgentType::Claude,
            workflow: None,
            tmux_session: "claude-test".to_string(),
            manager,
            workers: Vec::new(),
            issue_cache: cache,
        }
    }

    fn blocked_issue(number: u32) -> GitHubIssue {
        GitHubIssue {
            number,
            title: format!("Blocked issue #{number}"),
            state: IssueState::Open,
            priority: IssuePriority::P2,
            issue_type: IssueType::Other,
            labels: vec!["needs-design".to_string()],
            is_working: false,
            assigned_worker: None,
        }
    }

    fn open_issue(number: u32) -> GitHubIssue {
        GitHubIssue {
            number,
            title: format!("Open issue #{number}"),
            state: IssueState::Open,
            priority: IssuePriority::P2,
            issue_type: IssueType::Bug,
            labels: vec!["bug".to_string()],
            is_working: false,
            assigned_worker: None,
        }
    }

    #[test]
    fn attention_count_returns_blocked_issue_count() {
        let swarm = make_swarm_with_issues(vec![
            blocked_issue(1),
            open_issue(2),
            blocked_issue(3),
        ]);
        assert_eq!(swarm.attention_count(), 2);
    }

    #[test]
    fn attention_count_zero_when_no_blocked_issues() {
        let swarm = make_swarm_with_issues(vec![open_issue(1), open_issue(2)]);
        assert_eq!(swarm.attention_count(), 0);
    }

    #[test]
    fn attention_count_zero_when_issue_cache_empty() {
        let swarm = make_swarm_with_issues(vec![]);
        assert_eq!(swarm.attention_count(), 0);
    }

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
