use super::issue::IssueCache;
use super::status::AgentStatus;
use std::path::PathBuf;

/// Health status of a worker agent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Stalled,
    Restarting,
    Dead,
}

/// Per-agent health tracking for auto-recovery.
#[derive(Debug, Clone)]
pub struct WorkerHealth {
    /// Whether the tmux pane is currently known to be alive.
    pub pane_alive: bool,
    /// Consecutive ticks where the agent was Working but pane content didn't change.
    pub stall_ticks: u32,
    /// How many times this agent has been auto-restarted this session.
    pub restart_count: u8,
    /// Last pane content snapshot (for stall detection).
    pub last_content: String,
}

impl Default for WorkerHealth {
    fn default() -> Self {
        Self {
            pane_alive: true,
            stall_ticks: 0,
            restart_count: 0,
            last_content: String::new(),
        }
    }
}

impl WorkerHealth {
    /// Derive the observable health status from current fields.
    pub fn status(&self) -> HealthStatus {
        if self.restart_count >= 3 {
            HealthStatus::Dead
        } else if !self.pane_alive {
            HealthStatus::Restarting
        } else if self.stall_ticks >= 3 {
            HealthStatus::Stalled
        } else {
            HealthStatus::Healthy
        }
    }
}

/// The type of agent runtime.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentType {
    Claude,
    Codex,
    Droid,
    Gemini,
    Pi,
}

/// All supported agent types, in display order.
pub const ALL_AGENT_TYPES: &[AgentType] = &[
    AgentType::Claude,
    AgentType::Codex,
    AgentType::Droid,
    AgentType::Gemini,
    AgentType::Pi,
];

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentType::Claude => write!(f, "Claude"),
            AgentType::Codex => write!(f, "Codex"),
            AgentType::Droid => write!(f, "Droid"),
            AgentType::Gemini => write!(f, "Gemini"),
            AgentType::Pi => write!(f, "Pi"),
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
            AgentType::Pi => "pi",
        }
    }

    /// Tmux session prefix (e.g., "claude-myrepo")
    pub fn session_prefix(&self) -> &str {
        match self {
            AgentType::Claude => "claude",
            AgentType::Codex => "codex",
            AgentType::Droid => "droid",
            AgentType::Gemini => "gemini",
            AgentType::Pi => "pi",
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
            "pi" => Some(AgentType::Pi),
            _ => None,
        }
    }

    /// The shell command to launch this agent with autonomous permissions.
    #[cfg(test)]
    pub fn launch_cmd(&self) -> &str {
        match self {
            AgentType::Claude => "claude code --dangerously-skip-permissions .",
            AgentType::Codex => "codex",
            AgentType::Droid => "droid",
            AgentType::Gemini => "gemini --sandbox=false",
            // Pi has no permission prompts, so there is no bypass flag to pass.
            AgentType::Pi => "pi",
        }
    }

    /// Returns the exit command for this agent type.
    /// `(key, is_named)` — if `is_named` is true, send as a tmux named key (e.g. "C-c");
    /// otherwise send as literal text followed by Enter.
    pub fn exit_cmd(&self) -> (&str, bool) {
        match self {
            AgentType::Claude | AgentType::Gemini => ("q", false),
            // Pi workers are a shell loop, not a TUI: interrupt the loop.
            AgentType::Codex | AgentType::Droid | AgentType::Pi => ("C-c", true),
        }
    }

    /// The command to start the worker fix-loop (sent once on first launch).
    pub fn worker_loop_cmd(&self) -> &str {
        match self {
            AgentType::Claude => "/autocoder:fix-loop",
            AgentType::Codex => "/goal Work the issue queue: pull the next available GitHub issue, fix it, open a PR, and repeat until the queue is empty or you are paused.",
            AgentType::Droid => "/fix-loop",
            AgentType::Gemini => "/fix-loop",
            // Pi runs the loop in the shell (pi-fix-loop.sh), so nothing is
            // typed into a session -- the same shape as Droid's wrapper.
            AgentType::Pi => "",
        }
    }

    /// The command to dispatch work to an already-running idle worker (ongoing cycles).
    #[allow(dead_code)]
    pub fn worker_cmd(&self) -> &str {
        match self {
            AgentType::Claude => "/autocoder:fix",
            AgentType::Gemini => "/fix",
            AgentType::Codex | AgentType::Droid | AgentType::Pi => "",
        }
    }

    /// The command to send to a manager to start (or restart) the monitor loop.
    pub fn manager_cmd(&self) -> &str {
        match self {
            AgentType::Claude => "/autocoder:monitor-workers",
            AgentType::Gemini => "/monitor-workers",
            AgentType::Codex => "/goal Monitor and coordinate workers: check worker status, unblock stuck agents, merge completed PRs, triage new issues, and repeat indefinitely.",
            AgentType::Droid | AgentType::Pi => "",
        }
    }

    /// The tmux send-keys key sequence to gracefully exit this agent (returns to shell).
    /// Returns ("key", literal) where literal=true means use -l flag (send as text).
    pub fn exit_key(&self) -> (&str, bool) {
        match self {
            AgentType::Claude | AgentType::Gemini => ("q", true),
            AgentType::Codex | AgentType::Droid | AgentType::Pi => ("C-c", false),
        }
    }

    /// Status file directory within a worktree
    pub fn status_dir(&self) -> &str {
        match self {
            AgentType::Claude | AgentType::Codex | AgentType::Gemini | AgentType::Pi => {
                ".codex/loops"
            }
            AgentType::Droid => ".factory/loops",
        }
    }

    /// Whether this runtime is supervised by an outer shell loop wrapper.
    pub fn uses_loop_wrapper(&self) -> bool {
        matches!(self, AgentType::Droid | AgentType::Gemini | AgentType::Pi)
    }

    pub fn from_name(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "claude" => Some(AgentType::Claude),
            "codex" => Some(AgentType::Codex),
            "droid" => Some(AgentType::Droid),
            "gemini" => Some(AgentType::Gemini),
            "pi" => Some(AgentType::Pi),
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
    /// Number of issues completed (dispatched → cleared) in this session.
    pub completed_issue_count: u32,
    /// Health tracking for auto-recovery
    pub health: WorkerHealth,
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
    if tail_text.contains("What should Claude do instead?")
        || tail_text.contains("What should Gemini do instead?")
        || tail_text.contains("What should the agent do instead?")
    {
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

impl AgentInfo {
    /// Check if this agent appears to need human attention based on pane content.
    #[allow(dead_code)]
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
                || lower.contains("what should gemini do")
                || lower.contains("what should the agent do")
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
    /// Cached GitHub issues
    pub issue_cache: IssueCache,
    /// Set to true when the swarm was intentionally stopped via the TUI.
    /// Prevents automatic respawning by heal_workers and revive_agents.
    pub stopped: bool,
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
            self.workers
                .iter_mut()
                .find(|w| w.role == role || w.id == role)
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
    use crate::model::issue::{GitHubIssue, IssueCache, IssuePriority, IssueState, IssueType};
    use crate::model::status::{AgentState, AgentStatus};

    fn make_agent(id: &str, state: AgentState, pane_content: &str) -> AgentInfo {
        AgentInfo {
            id: id.to_string(),
            role: id.to_string(),
            worktree_path: PathBuf::from("/tmp/test"),
            tmux_target: "test:0.0".to_string(),
            status: AgentStatus {
                timestamp: None,
                state,
            },
            is_manager: id == "manager",
            pane_content: pane_content.to_string(),
            dispatched_issue: None,
            current_issue: None,
            current_issue_title: None,
            waiting_for_input: false,
            resurrection_attempts: 0,
            completed_issue_count: 0,
            health: WorkerHealth::default(),
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
            issue_cache: IssueCache::default(),
            stopped: false,
        }
    }

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
            completed_issue_count: 0,
            health: WorkerHealth::default(),
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
            stopped: false,
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
            updated_at: None,
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
            updated_at: None,
        }
    }

    #[test]
    fn attention_count_returns_blocked_issue_count() {
        let swarm = make_swarm_with_issues(vec![blocked_issue(1), open_issue(2), blocked_issue(3)]);
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
        assert_eq!(
            AgentType::Codex.worker_loop_cmd(),
            "/goal Work the issue queue: pull the next available GitHub issue, fix it, open a PR, and repeat until the queue is empty or you are paused."
        );
        assert_eq!(AgentType::Droid.worker_loop_cmd(), "/fix-loop");
    }

    #[test]
    fn status_directories_match_runtime_storage() {
        assert_eq!(AgentType::Codex.status_dir(), ".codex/loops");
        assert_eq!(AgentType::Claude.status_dir(), ".codex/loops");
        assert_eq!(AgentType::Droid.status_dir(), ".factory/loops");
    }

    #[test]
    fn loop_wrappers_match_runtime_requirements() {
        assert!(!AgentType::Codex.uses_loop_wrapper());
        assert!(AgentType::Droid.uses_loop_wrapper());
        assert!(!AgentType::Claude.uses_loop_wrapper());
        assert!(AgentType::Gemini.uses_loop_wrapper());
    }

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
    fn workflow_display() {
        assert_eq!(Workflow::Autocoder.to_string(), "Autocoder");
        assert_eq!(Workflow::Modernize.to_string(), "Modernize");
    }

    #[test]
    fn needs_attention_idle_agent() {
        let agent = make_agent("w-0", AgentState::Idle, "some output");
        assert!(agent.needs_attention());
    }

    #[test]
    fn needs_attention_working_agent() {
        let agent = make_agent(
            "w-0",
            AgentState::Working { issue: Some(42) },
            "doing stuff",
        );
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

    #[test]
    fn swarm_all_agent_count() {
        let swarm = make_swarm(vec![
            make_agent("w-0", AgentState::Idle, ""),
            make_agent("w-1", AgentState::Working { issue: None }, ""),
        ]);
        assert_eq!(swarm.all_agents().len(), 3); // manager + 2 workers
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

    // --- detect_waiting_for_input ---

    #[test]
    fn waiting_detected_for_bypass_permissions_prompt() {
        assert!(detect_waiting_for_input("bypass permissions\nsome output"));
    }

    #[test]
    fn waiting_detected_for_allow_prompt() {
        assert!(detect_waiting_for_input("Running tool...\nAllow?"));
    }

    #[test]
    fn waiting_detected_for_allow_this_action() {
        assert!(detect_waiting_for_input("allow this action (y/n)"));
    }

    #[test]
    fn waiting_detected_for_yn_brackets() {
        assert!(detect_waiting_for_input("Continue? [Y/n]"));
        assert!(detect_waiting_for_input("Continue? [y/N]"));
    }

    #[test]
    fn waiting_detected_for_interrupted_state() {
        assert!(detect_waiting_for_input(
            "Some output\nWhat should Claude do instead?\nmore"
        ));
    }

    #[test]
    fn waiting_detected_for_interrupted_with_prompt_indicator() {
        assert!(detect_waiting_for_input("Interrupted\nSome context\n❯"));
    }

    #[test]
    fn waiting_detected_for_shift_tab_bypass_line() {
        assert!(detect_waiting_for_input(
            "bypass permissions on /some/file (shift+tab to allow)"
        ));
    }

    #[test]
    fn waiting_not_detected_for_normal_output() {
        assert!(!detect_waiting_for_input(
            "Running cargo build...\nCompiling foo v1.0\nFinished"
        ));
    }

    #[test]
    fn waiting_not_detected_for_empty_string() {
        assert!(!detect_waiting_for_input(""));
    }

    #[test]
    fn worker_health_default_is_healthy() {
        let h = WorkerHealth::default();
        assert_eq!(h.status(), HealthStatus::Healthy);
    }

    #[test]
    fn worker_health_stall_ticks_triggers_stalled() {
        let mut h = WorkerHealth::default();
        h.stall_ticks = 3;
        assert_eq!(h.status(), HealthStatus::Stalled);
    }

    #[test]
    fn worker_health_pane_dead_triggers_restarting() {
        let mut h = WorkerHealth::default();
        h.pane_alive = false;
        assert_eq!(h.status(), HealthStatus::Restarting);
    }

    #[test]
    fn worker_health_restart_cap_triggers_dead() {
        let mut h = WorkerHealth::default();
        h.restart_count = 3;
        assert_eq!(h.status(), HealthStatus::Dead);
    }

    #[test]
    fn worker_health_dead_takes_priority_over_stalled() {
        let mut h = WorkerHealth::default();
        h.restart_count = 3;
        h.stall_ticks = 10;
        h.pane_alive = false;
        assert_eq!(h.status(), HealthStatus::Dead);
    }

    #[test]
    fn worker_health_restarting_takes_priority_over_stalled() {
        let mut h = WorkerHealth::default();
        h.pane_alive = false;
        h.stall_ticks = 5;
        assert_eq!(h.status(), HealthStatus::Restarting);
    }

    fn make_agent_simple(id: &str, role: &str, is_manager: bool) -> AgentInfo {
        AgentInfo {
            id: id.to_string(),
            role: role.to_string(),
            worktree_path: PathBuf::from("/tmp/test"),
            tmux_target: format!("test:0.{}", if is_manager { 0 } else { 1 }),
            status: AgentStatus::default(),
            is_manager,
            pane_content: String::new(),
            dispatched_issue: None,
            current_issue: None,
            current_issue_title: None,
            waiting_for_input: false,
            resurrection_attempts: 0,
            completed_issue_count: 0,
            health: WorkerHealth::default(),
        }
    }

    fn make_swarm_with_agents(manager: AgentInfo, workers: Vec<AgentInfo>) -> Swarm {
        Swarm {
            repo_path: PathBuf::from("/tmp/test"),
            project_name: "test".to_string(),
            agent_type: AgentType::Claude,
            workflow: None,
            tmux_session: "claude-test".to_string(),
            manager,
            workers,
            issue_cache: IssueCache::default(),
            stopped: false,
        }
    }

    #[test]
    fn detect_waiting_recognizes_input_prompt() {
        assert!(detect_waiting_for_input("some output\nAllow? (y/n)"));
        assert!(detect_waiting_for_input("text\nWhat should Claude do instead?"));
        assert!(detect_waiting_for_input("bypass permissions on this file with shift+tab"));
    }

    #[test]
    fn detect_waiting_returns_false_for_normal_output() {
        assert!(!detect_waiting_for_input("Running tests...\nAll tests passed."));
        assert!(!detect_waiting_for_input(""));
        assert!(!detect_waiting_for_input("Working on issue #42"));
    }

    #[test]
    fn idle_count_counts_idle_agents() {
        let manager = make_agent_simple("mgr", "manager", true);
        let mut w1 = make_agent_simple("w1", "worker1", false);
        let mut w2 = make_agent_simple("w2", "worker2", false);
        let mut w3 = make_agent_simple("w3", "worker3", false);
        w1.status.state = super::super::status::AgentState::Idle;
        w2.status.state = super::super::status::AgentState::Working { issue: Some(1) };
        w3.status.state = super::super::status::AgentState::Idle;
        let mut swarm = make_swarm_with_agents(manager, vec![w1, w2, w3]);
        assert_eq!(swarm.idle_count(), 2);
        swarm.workers[1].status.state = super::super::status::AgentState::Idle;
        assert_eq!(swarm.idle_count(), 3);
    }

    #[test]
    fn waiting_count_counts_waiting_agents() {
        let mut manager = make_agent_simple("mgr", "manager", true);
        let mut w1 = make_agent_simple("w1", "worker1", false);
        let w2 = make_agent_simple("w2", "worker2", false);
        manager.waiting_for_input = true;
        w1.waiting_for_input = true;
        let swarm = make_swarm_with_agents(manager, vec![w1, w2]);
        assert_eq!(swarm.waiting_count(), 2);
    }

    #[test]
    fn waiting_count_zero_when_none_waiting() {
        let manager = make_agent_simple("mgr", "manager", true);
        let w1 = make_agent_simple("w1", "worker1", false);
        let swarm = make_swarm_with_agents(manager, vec![w1]);
        assert_eq!(swarm.waiting_count(), 0);
    }

    #[test]
    fn all_agents_includes_manager_and_workers() {
        let manager = make_agent_simple("mgr", "manager", true);
        let w1 = make_agent_simple("w1", "worker1", false);
        let w2 = make_agent_simple("w2", "worker2", false);
        let swarm = make_swarm_with_agents(manager, vec![w1, w2]);
        let all = swarm.all_agents();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].id, "mgr");
        assert_eq!(all[1].id, "w1");
        assert_eq!(all[2].id, "w2");
    }

    #[test]
    fn next_waiting_agent_finds_first_when_none_specified() {
        let manager = make_agent_simple("mgr", "manager", true);
        let mut w1 = make_agent_simple("w1", "worker1", false);
        w1.waiting_for_input = true;
        let swarm = make_swarm_with_agents(manager, vec![w1]);
        let found = swarm.next_waiting_agent(None);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "w1");
    }

    #[test]
    fn next_waiting_agent_cycles_past_current_and_wraps() {
        let manager = make_agent_simple("mgr", "manager", true);
        let mut w1 = make_agent_simple("w1", "worker1", false);
        let mut w2 = make_agent_simple("w2", "worker2", false);
        w1.waiting_for_input = true;
        w2.waiting_for_input = true;
        let swarm = make_swarm_with_agents(manager, vec![w1, w2]);
        // After w1, should get w2
        let next = swarm.next_waiting_agent(Some("w1"));
        assert_eq!(next.unwrap().id, "w2");
        // After w2 (last), should wrap to w1
        let wrapped = swarm.next_waiting_agent(Some("w2"));
        assert_eq!(wrapped.unwrap().id, "w1");
    }
}
