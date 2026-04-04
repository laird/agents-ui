use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use std::collections::HashMap;

use crate::adapter::claude::ClaudeAdapter;
use crate::adapter::traits::{AgentRuntime, SwarmConfig};
use crate::event::{Event, EventHandler};
use crate::model::issue::{GitHubIssue, IssueCache, IssueFilter};
use crate::model::swarm::{ALL_AGENT_TYPES, AgentType, Swarm};
use crate::scripts::launcher;
use crate::tmux::proxy;
use crate::transport::ServerTransport;
use crate::tui::Tui;
use crate::ui::agent_view::AgentView;
use crate::ui::issue_detail::IssueDetailView;
use crate::ui::issue_list::IssueListView;
use crate::ui::repo_view::RepoView;
use crate::ui::repos_list::ReposListView;
use crate::ui::swarm_view::{SwarmPanel, SwarmView};
use crate::ui::text_input::TextInput;

#[derive(Debug, Clone, PartialEq, Eq)]
enum ParsedGhCommand {
    NotGh,
    Parsed(Vec<String>),
    Malformed(String),
}

fn parse_direct_gh_command(command: &str) -> ParsedGhCommand {
    if command.split_whitespace().next() != Some("gh") {
        return ParsedGhCommand::NotGh;
    }

    let argv = match shell_words::split(command) {
        Ok(argv) => argv,
        Err(err) => return ParsedGhCommand::Malformed(err.to_string()),
    };

    match argv.split_first() {
        Some((program, args)) if program == "gh" => ParsedGhCommand::Parsed(args.to_vec()),
        _ => ParsedGhCommand::NotGh,
    }
}

fn gh_issue_command_mutates(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("issue"))
        && matches!(
            args.get(1).map(String::as_str),
            Some(
                "close"
                    | "comment"
                    | "create"
                    | "delete"
                    | "edit"
                    | "lock"
                    | "pin"
                    | "reopen"
                    | "transfer"
                    | "unlock"
                    | "unpin"
            )
        )
}

const AGENTS_UI_GH_REPO: &str = "laird/agents-ui";
const REPORT_LOG_ERRORS_COMMAND: &str = "gh agents-ui report-errors";
const REPORT_LOG_ERRORS_COMMAND_ALIAS: &str = "/report-errors";

#[derive(Debug, Clone, PartialEq, Eq)]
struct LogErrorCandidate {
    fingerprint: String,
    summary: String,
    occurrences: usize,
    samples: Vec<String>,
}

fn is_report_log_errors_command(command: &str) -> bool {
    let trimmed = command.trim();
    trimmed == REPORT_LOG_ERRORS_COMMAND || trimmed == REPORT_LOG_ERRORS_COMMAND_ALIAS
}

fn normalize_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_ws = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !prev_ws {
                out.push(' ');
            }
            prev_ws = true;
        } else {
            out.push(ch);
            prev_ws = false;
        }
    }
    out.trim().to_string()
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    let mut chars = input.chars();
    let collected: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{collected}…")
    } else {
        collected
    }
}

fn extract_error_message(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if let Some(idx) = trimmed.find(" ERROR ") {
        return Some(trimmed[(idx + " ERROR ".len())..].trim().to_string());
    }
    trimmed.strip_prefix("ERROR ").map(|s| s.trim().to_string())
}

fn log_error_fingerprint(summary: &str) -> String {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for byte in summary.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

fn collect_recent_log_error_candidates(
    log_contents: &str,
    max_errors: usize,
    max_samples: usize,
) -> Vec<LogErrorCandidate> {
    let mut by_fingerprint: HashMap<String, LogErrorCandidate> = HashMap::new();
    let mut order = Vec::new();

    for line in log_contents.lines().rev() {
        let Some(raw_message) = extract_error_message(line) else {
            continue;
        };
        let normalized = normalize_whitespace(&raw_message);
        if normalized.is_empty() {
            continue;
        }
        let summary = truncate_chars(&normalized, 140);
        let fingerprint = log_error_fingerprint(&summary.to_lowercase());

        if let Some(existing) = by_fingerprint.get_mut(&fingerprint) {
            existing.occurrences += 1;
            if existing.samples.len() < max_samples {
                existing.samples.push(truncate_chars(line.trim(), 240));
            }
            continue;
        }

        if order.len() >= max_errors {
            continue;
        }

        order.push(fingerprint.clone());
        by_fingerprint.insert(
            fingerprint.clone(),
            LogErrorCandidate {
                fingerprint,
                summary,
                occurrences: 1,
                samples: vec![truncate_chars(line.trim(), 240)],
            },
        );
    }

    order
        .into_iter()
        .filter_map(|fingerprint| by_fingerprint.remove(&fingerprint))
        .collect()
}

fn agents_tui_log_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("agents-ui")
        .join("agents-ui.log")
}

fn format_log_samples(samples: &[String]) -> String {
    samples
        .iter()
        .map(|line| format!("- `{}`", line.replace('`', "'")))
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_log_issue_body(candidate: &LogErrorCandidate) -> String {
    format!(
        "Automated report from `agents-tui` log analysis.\n\nFingerprint: `[log-fingerprint:{}]`\nOccurrences in scan: {}\n\nSummary:\n{}\n\nRecent log lines:\n{}\n",
        candidate.fingerprint,
        candidate.occurrences,
        candidate.summary,
        format_log_samples(&candidate.samples)
    )
}

fn build_log_issue_comment(candidate: &LogErrorCandidate) -> String {
    format!(
        "Additional `agents-tui` log evidence at {}.\n\nOccurrences in latest scan: {}\n\nRecent log lines:\n{}\n",
        chrono::Utc::now().to_rfc3339(),
        candidate.occurrences,
        format_log_samples(&candidate.samples)
    )
}

/// Which screen we're on.
#[derive(Debug, Clone)]
pub enum Screen {
    RuntimeSelect,
    InstallScopeSelect,
    ReposList,
    /// Prompt for repo path to launch a new swarm.
    NewSwarm {
        field: NewSwarmField,
    },
    RepoView {
        swarm_idx: usize,
    },
    AgentView {
        swarm_idx: usize,
        agent_id: String,
    },
    IssueView {
        swarm_idx: usize,
        issue_number: u32,
    },
    /// View full issue details (body, labels, state).
    IssueDetail {
        swarm_idx: usize,
    },
    /// Full issue list for a swarm (press 'L' from RepoView).
    IssueList {
        swarm_idx: usize,
    },
    /// Overlay to pick a new agent runtime to switch the running swarm to.
    SwitchAgent {
        swarm_idx: usize,
        selected: usize,
    },
}

#[derive(Debug, Clone)]
pub enum NewSwarmField {
    RepoPath,
    AgentRuntime,
    RuntimeSelection,
    NumWorkers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallScope {
    User,
    Repo,
}

/// Which field is focused in the create-issue dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateIssueField {
    Title,
    Body,
    Priority,
    IssueType,
    Labels,
}

/// Priority level for a new issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssuePriority {
    P0,
    P1,
    P2,
    P3,
}

impl IssuePriority {
    pub fn label(self) -> &'static str {
        match self {
            Self::P0 => "P0",
            Self::P1 => "P1",
            Self::P2 => "P2",
            Self::P3 => "P3",
        }
    }
    pub fn desc(self) -> &'static str {
        match self {
            Self::P0 => "Critical",
            Self::P1 => "High",
            Self::P2 => "Medium",
            Self::P3 => "Low",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::P0 => Self::P1,
            Self::P1 => Self::P2,
            Self::P2 => Self::P3,
            Self::P3 => Self::P0,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Self::P0 => Self::P3,
            Self::P1 => Self::P0,
            Self::P2 => Self::P1,
            Self::P3 => Self::P2,
        }
    }
}

/// Issue type: bug or enhancement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueType {
    Bug,
    Enhancement,
}

impl IssueType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bug => "bug",
            Self::Enhancement => "enhancement",
        }
    }
    pub fn toggle(self) -> Self {
        match self {
            Self::Bug => Self::Enhancement,
            Self::Enhancement => Self::Bug,
        }
    }
}

/// Blocking labels that can be toggled on/off.
pub const BLOCKING_LABELS: &[&str] = &[
    "needs-design",
    "needs-clarification",
    "needs-approval",
    "too-complex",
    "future",
    "proposal",
];

/// Form state for the create-issue dialog.
#[derive(Debug, Clone)]
pub struct CreateIssueForm {
    pub title: String,
    pub body: String,
    pub field: CreateIssueField,
    pub priority: IssuePriority,
    pub issue_type: IssueType,
    /// Which blocking labels are selected (indexed into BLOCKING_LABELS).
    pub label_toggles: [bool; 6],
    /// Which blocking label is highlighted (for arrow navigation).
    pub label_cursor: usize,
}

impl CreateIssueForm {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            body: String::new(),
            field: CreateIssueField::Title,
            priority: IssuePriority::P2,
            issue_type: IssueType::Bug,
            label_toggles: [false; 6],
            label_cursor: 0,
        }
    }

    /// Build the comma-separated labels string for gh issue create.
    pub fn labels_string(&self) -> String {
        let mut labels = vec![
            self.priority.label().to_string(),
            self.issue_type.label().to_string(),
        ];
        for (i, &on) in self.label_toggles.iter().enumerate() {
            if on {
                labels.push(BLOCKING_LABELS[i].to_string());
            }
        }
        labels.join(",")
    }
}

#[derive(Debug, Clone)]
struct PendingLaunch {
    repo_path: PathBuf,
    num_workers: u32,
    agent_type: AgentType,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeOption {
    pub(crate) agent_type: AgentType,
    pub(crate) available: bool,
    pub(crate) detail: String,
}

pub struct App {
    pub running: bool,
    pub screen: Screen,
    pub swarms: Vec<Swarm>,
    pub repos_list: ReposListView,
    pub repo_view: RepoView,
    pub agent_view: AgentView,
    pub issue_view: Option<crate::ui::issue_view::IssueView>,
    pub events: EventHandler,
    pub adapter: ClaudeAdapter,
    transport: ServerTransport,
    pub agents_dir: std::path::PathBuf,
    /// Active pane watcher handles (so we can cancel them).
    pane_watchers: Vec<tokio::task::JoinHandle<()>>,
    issue_watchers: Vec<tokio::task::JoinHandle<()>>,
    /// Input buffer for new swarm dialog.
    pub dialog_input: TextInput,
    pub manager_input: TextInput,
    /// Stored repo path during new swarm flow.
    pub new_swarm_repo: String,
    /// Selected agent type during new swarm flow.
    pub new_swarm_agent_type: AgentType,
    /// Status message shown at bottom of repos list.
    pub status_message: Option<String>,
    /// Available repos (git directories found nearby) that don't have active swarms.
    pub available_repos: Vec<PathBuf>,
    /// Swarm view state (new three-panel layout).
    pub swarm_view: SwarmView,
    /// Which panel is focused in swarm view.
    pub swarm_focus: SwarmPanel,
    /// Cached GitHub issues per project.
    pub issue_caches: HashMap<String, IssueCache>,
    /// Blink state for attention indicators.
    pub blink: bool,
    pub blink_counter: u32,
    /// Create-issue dialog form state (None = closed).
    pub create_issue_form: Option<CreateIssueForm>,
    /// Pending swarm teardown confirmation (swarm index).
    pub confirm_teardown: Option<usize>,
    /// Pending swarm stop-all confirmation (swarm index).
    pub confirm_stop: Option<usize>,
    /// Default runtime for launched/discovered swarms.
    pub default_agent_type: AgentType,
    /// True when runtime was explicitly pinned via CLI flag.
    pub runtime_locked_from_cli: bool,
    /// Repo where startup runtime preference should be persisted.
    pub runtime_pref_repo_root: Option<PathBuf>,
    /// Current selection in install scope dialog.
    pub install_scope: InstallScope,
    /// Deferred launch context when waiting on install scope selection.
    pending_launch: Option<PendingLaunch>,
    /// Last time worker healing was run.
    last_heal: Instant,
    /// Counter for periodic issue refresh (every ~60 ticks = 15 seconds at 250ms tick).
    issue_refresh_counter: u32,
    /// Configurable keyboard shortcuts.
    pub shortcuts: crate::config::shortcuts::ShortcutsConfig,
    /// Whether the shortcuts viewer overlay is visible.
    pub show_shortcuts_viewer: bool,
    /// Last time we auto-dispatched /monitor-workers (for debounce).
    auto_dispatch_last: Option<std::time::Instant>,
    /// Issue detail view state.
    pub issue_detail_view: Option<IssueDetailView>,
    /// Issue list view state (full issue list screen).
    pub issue_list_view: IssueListView,
    /// App-level keybinding configuration.
    pub keybindings: crate::config::keybindings::KeyBindings,
    /// Whether the ? help overlay is visible.
    pub show_help: bool,
    /// Rich feedback dialog state (None = closed).
    pub feedback_state: Option<crate::ui::feedback_dialog::FeedbackState>,
    /// Projects intentionally stopped by the user — excluded from auto-heal and revival.
    intentionally_stopped: std::collections::HashSet<String>,
    /// Tick counter for auto-clearing transient status messages (~3s at 20Hz = 60 ticks).
    status_message_age: u32,
    /// Runtime choices for the current switch-runtime overlay.
    switch_agent_options: Vec<RuntimeOption>,
    /// Deduplicated runtime-limit alerts seen in pane output.
    runtime_alerts: HashMap<String, String>,
}

impl App {
    pub async fn new(
        initial_agent_type: Option<AgentType>,
        runtime_locked_from_cli: bool,
        runtime_pref_repo_root: Option<PathBuf>,
        remote_server: Option<String>,
        startup_warning: Option<String>,
    ) -> Result<Self> {
        let agents_dir = launcher::resolve_agents_dir();
        let transport = ServerTransport::new(remote_server);
        let default_agent_type = initial_agent_type.clone().unwrap_or(AgentType::Claude);
        let adapter = ClaudeAdapter::new(default_agent_type.clone(), transport.clone());
        let events = EventHandler::new();

        let mut swarms = Vec::new();
        if initial_agent_type.is_some() {
            // Discover existing swarms on startup when runtime is known
            swarms = match adapter.discover(&agents_dir).await {
                Ok(s) => {
                    tracing::info!("Discovered {} existing swarm(s)", s.len());
                    s
                }
                Err(e) => {
                    tracing::warn!("Failed to discover existing swarms: {e}");
                    vec![]
                }
            };
        }

        let mut app = Self {
            running: true,
            screen: if initial_agent_type.is_some() {
                Screen::ReposList
            } else {
                Screen::RuntimeSelect
            },
            swarms,
            repos_list: ReposListView::new(),
            repo_view: RepoView::new(),
            agent_view: AgentView::new(),
            issue_view: None,
            events,
            adapter,
            transport,
            agents_dir,
            pane_watchers: Vec::new(),
            issue_watchers: Vec::new(),
            dialog_input: TextInput::new(),
            manager_input: TextInput::new(),
            new_swarm_repo: String::new(),
            new_swarm_agent_type: AgentType::Claude,
            status_message: startup_warning,
            available_repos: Vec::new(),
            swarm_view: SwarmView::new(),
            swarm_focus: SwarmPanel::Manager,
            issue_caches: HashMap::new(),
            blink: false,
            blink_counter: 0,
            create_issue_form: None,
            confirm_teardown: None,
            confirm_stop: None,
            default_agent_type,
            runtime_locked_from_cli,
            runtime_pref_repo_root,
            install_scope: InstallScope::User,
            pending_launch: None,
            last_heal: Instant::now(),
            issue_refresh_counter: 0,
            shortcuts: {
                crate::config::shortcuts::ShortcutsConfig::ensure_defaults_written();
                crate::config::shortcuts::ShortcutsConfig::load()
            },
            show_shortcuts_viewer: false,
            auto_dispatch_last: None,
            issue_detail_view: None,
            issue_list_view: IssueListView::new(),
            keybindings: crate::config::keybindings::KeyBindings::load(),
            show_help: false,
            feedback_state: None,
            intentionally_stopped: std::collections::HashSet::new(),
            status_message_age: 0,
            switch_agent_options: Vec::new(),
            runtime_alerts: HashMap::new(),
        };

        // Scan for available repos (git directories in cwd or children)
        app.scan_available_repos();

        if initial_agent_type.is_some() {
            app.refresh_statuses();
            // Start pane watchers and issue fetchers for discovered swarms
            app.start_all_pane_watchers();
            app.start_all_issue_watchers();
            app.auto_select_current_repo_swarm().await;
        }

        // Trigger immediate issue refresh for all discovered swarms
        for idx in 0..app.swarms.len() {
            app.start_issue_refresh(idx);
        }

        Ok(app)
    }

    pub async fn run(&mut self, terminal: &mut Tui) -> Result<()> {
        while self.running {
            // Render
            terminal.draw(|f| {
                let area = f.area();
                match &self.screen {
                    Screen::RuntimeSelect => {
                        crate::ui::new_swarm::render_runtime_dialog(
                            f,
                            area,
                            self.default_agent_type.clone(),
                        );
                    }
                    Screen::InstallScopeSelect => {
                        let repo_path = self
                            .pending_launch
                            .as_ref()
                            .map(|p| p.repo_path.to_string_lossy().to_string())
                            .unwrap_or_default();
                        crate::ui::new_swarm::render_install_scope_dialog(
                            f,
                            area,
                            self.install_scope,
                            self.pending_launch
                                .as_ref()
                                .map(|p| p.agent_type.clone())
                                .unwrap_or(AgentType::Droid),
                            repo_path,
                        );
                    }
                    Screen::ReposList => {
                        self.repos_list.render(
                            f,
                            area,
                            &self.swarms,
                            &self.available_repos,
                            self.status_message.as_deref(),
                            &self.issue_caches,
                        );
                    }
                    Screen::NewSwarm { field } => {
                        let field = field.clone();
                        let repo = self.new_swarm_repo.clone();
                        let agent_type = self.new_swarm_agent_type.clone();
                        crate::ui::new_swarm::render_new_swarm_dialog(
                            f, area, &field, &self.dialog_input, &repo, &agent_type,
                        );
                    }
                    Screen::RepoView { swarm_idx } => {
                        if let Some(swarm) = self.swarms.get(*swarm_idx) {
                            let swarm = swarm.clone();
                            let cache = self.issue_caches.get(&swarm.project_name);
                            let issues = cache.map(|c| c.issues.clone()).unwrap_or_default();
                            let issues_loading = cache.map(|c| c.is_loading).unwrap_or(false);
                            let last_fetched = cache.and_then(|c| c.last_fetched);
                            let focus = self.swarm_focus;
                            let blink = self.blink;
                            self.swarm_view.render(
                                f,
                                area,
                                &swarm,
                                &issues,
                                focus,
                                blink,
                                issues_loading,
                                last_fetched,
                                &self.manager_input,
                            );
                        } else {
                            tracing::warn!("RepoView swarm_idx {} out of bounds (have {} swarms), falling back to ReposList", swarm_idx, self.swarms.len());
                        }
                        if let Some(_swarm) = self.swarms.get(*swarm_idx) {
                            if let Some(ref form) = self.create_issue_form {
                                crate::ui::new_swarm::render_create_issue_dialog(
                                    f, area, form,
                                );
                            }
                        }
                    }
                    Screen::AgentView {
                        swarm_idx,
                        agent_id,
                    } => {
                        if let Some(swarm) = self.swarms.get(*swarm_idx) {
                            if let Some(agent) = swarm.agent(agent_id) {
                                let agent = agent.clone();
                                self.agent_view.render(f, area, &agent);
                            }
                        }
                    }
                    Screen::IssueView {
                        swarm_idx,
                        issue_number,
                    } => {
                        let issue = self.swarms.get(*swarm_idx).and_then(|s| {
                            self.issue_caches.get(&s.project_name).and_then(|c| {
                                c.issues.iter().find(|i| i.number == *issue_number).cloned()
                            })
                        });
                        if let Some(ref mut view) = self.issue_view {
                            view.render(f, area, issue.as_ref());
                        }
                    }
                    Screen::IssueDetail { .. } => {
                        if let Some(ref view) = self.issue_detail_view {
                            view.render(f, area);
                        }
                    }
                    Screen::IssueList { swarm_idx } => {
                        if let Some(swarm) = self.swarms.get(*swarm_idx) {
                            let swarm = swarm.clone();
                            let issues = self.issue_caches
                                .get(&swarm.project_name)
                                .map(|c| c.issues.clone())
                                .unwrap_or_default();
                            self.issue_list_view.render(
                                f,
                                area,
                                &swarm,
                                &issues,
                                self.status_message.as_deref(),
                            );
                        }
                    }
                    Screen::SwitchAgent { swarm_idx, selected } => {
                        if let Some(swarm) = self.swarms.get(*swarm_idx) {
                            crate::ui::new_swarm::render_switch_agent_dialog(
                                f,
                                area,
                                &swarm.project_name.clone(),
                                &swarm.agent_type.clone(),
                                &self.switch_agent_options,
                                *selected,
                            );
                        }
                    }
                }

                // Shortcuts viewer overlay
                if self.show_shortcuts_viewer {
                    let panel = match &self.screen {
                        Screen::RepoView { .. } => match self.swarm_focus {
                            SwarmPanel::Workers => "workers",
                            SwarmPanel::Issues => "issues",
                            SwarmPanel::Manager => "manager",
                        },
                        Screen::IssueView { .. } | Screen::IssueDetail { .. } => "issues",
                        _ => "global",
                    };
                    crate::ui::shortcuts_viewer::render_shortcuts_viewer(
                        f, area, &self.shortcuts, panel,
                    );
                }

                // Help overlay (rendered on top of everything)
                if self.show_help {
                    let context = if matches!(self.screen, Screen::RepoView { .. })
                        && self.swarm_focus == SwarmPanel::Issues
                    {
                        Some(crate::ui::help_overlay::ISSUES_PANEL_CONTEXT)
                    } else {
                        None
                    };
                    crate::ui::help_overlay::render_help_overlay(f, f.area(), &self.keybindings, context);
                }
                // Feedback dialog (rendered on top of everything)
                if let Some(ref state) = self.feedback_state {
                    crate::ui::feedback_dialog::render_feedback_dialog(f, f.area(), state);
                }
            })?;

            // Fix invalid screen state (swarm removed while viewing)
            match &self.screen {
                Screen::RepoView { swarm_idx } if *swarm_idx >= self.swarms.len() => {
                    tracing::warn!(
                        "Screen points to invalid swarm_idx {}, falling back",
                        swarm_idx
                    );
                    self.screen = Screen::ReposList;
                }
                Screen::AgentView { swarm_idx, .. } if *swarm_idx >= self.swarms.len() => {
                    tracing::warn!(
                        "AgentView points to invalid swarm_idx {}, falling back",
                        swarm_idx
                    );
                    self.screen = Screen::ReposList;
                }
                Screen::IssueView { swarm_idx, .. } if *swarm_idx >= self.swarms.len() => {
                    tracing::warn!(
                        "IssueView points to invalid swarm_idx {}, falling back",
                        swarm_idx
                    );
                    self.screen = Screen::ReposList;
                }
                Screen::IssueList { swarm_idx } if *swarm_idx >= self.swarms.len() => {
                    self.screen = Screen::ReposList;
                }
                Screen::SwitchAgent { swarm_idx, .. } if *swarm_idx >= self.swarms.len() => {
                    self.screen = Screen::ReposList;
                }
                Screen::RuntimeSelect | Screen::InstallScopeSelect => {}
                _ => {}
            }

            // Handle events
            if let Some(event) = self.events.next().await {
                self.handle_event(event).await?;
            }
        }

        Ok(())
    }

    fn set_status(&mut self, msg: String) {
        self.status_message = Some(msg);
        self.status_message_age = 0;
    }

    async fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => self.handle_key(key).await?,
            Event::Tick => {
                self.refresh_statuses();
                self.repo_view.tick_banners();
                // Blink toggle for attention indicators
                self.blink_counter += 1;
                if self.blink_counter % 5 == 0 {
                    self.blink = !self.blink;
                }
                // Auto-dispatch idle workers every ~5 seconds (20 ticks)
                if self.blink_counter % 20 == 0 {
                    self.dispatch_idle_workers().await;
                }
                // Manage manager sessions every ~30 seconds (120 ticks)
                if self.blink_counter % 120 == 0 {
                    self.manage_manager_sessions().await;
                }
                // Revive agents that dropped to shell (e.g. after self-update) every ~60 seconds
                if self.blink_counter % 240 == 0 {
                    self.revive_dropped_agents().await;
                }
                // Prune swarms whose tmux sessions were externally killed (every ~45 seconds)
                if self.blink_counter % 180 == 0 {
                    self.prune_dead_swarms().await;
                }
                // Periodically heal worker infrastructure (every 30 seconds)
                if self.last_heal.elapsed() >= Duration::from_secs(30) {
                    self.last_heal = Instant::now();
                    self.heal_all_workers().await;
                }
                // Periodic issue refresh (~every 60 seconds when viewing a repo)
                self.issue_refresh_counter += 1;
                if self.issue_refresh_counter >= 240 {
                    self.issue_refresh_counter = 0;
                    if let Screen::RepoView { swarm_idx } = &self.screen {
                        self.start_issue_refresh(*swarm_idx);
                    }
                }
                // Auto-clear transient status messages after ~3 seconds (60 ticks at 20Hz).
                // Confirmation prompts (containing "? (y to confirm") are never auto-cleared.
                match &self.status_message {
                    None => {
                        self.status_message_age = 0;
                    }
                    Some(msg) => {
                        let is_confirmation =
                            msg.contains("? (y to confirm") || msg.contains("? (y/n)");
                        if is_confirmation {
                            self.status_message_age = 0;
                        } else {
                            self.status_message_age += 1;
                            if self.status_message_age >= 60 {
                                self.status_message = None;
                                self.status_message_age = 0;
                            }
                        }
                    }
                }
                // Auto-dispatch: send /monitor-workers to manager when workers are idle
                self.check_auto_dispatch().await;
            }
            Event::PaneOutput { agent_id, content } => {
                // agent_id is globally unique (e.g., "nextgen-CDD/manager")
                let is_manager = agent_id.ends_with("/manager");
                let runtime_alert = detect_runtime_alert(&content);
                for swarm in &mut self.swarms {
                    if let Some(agent) = swarm.agent_by_id_mut(&agent_id) {
                        agent.waiting_for_input =
                            crate::model::swarm::detect_waiting_for_input(&content);
                        agent.pane_content = content;
                        break;
                    }
                }
                // Auto-scroll manager panel to bottom when new content arrives
                if is_manager {
                    self.swarm_view.scroll_manager_to_bottom();
                }
                match runtime_alert {
                    Some(alert) => {
                        let changed = self
                            .runtime_alerts
                            .get(&agent_id)
                            .map(|existing| existing != &alert)
                            .unwrap_or(true);
                        self.runtime_alerts.insert(agent_id.clone(), alert.clone());
                        if changed {
                            self.set_status(format!(
                                "{agent_id}: {alert}. Press Shift+S to switch runtime."
                            ));
                        }
                    }
                    None => {
                        self.runtime_alerts.remove(&agent_id);
                    }
                }
            }
            Event::LaunchProgress {
                project_name,
                message,
            } => {
                // Append progress to the placeholder swarm's manager pane_content
                for swarm in &mut self.swarms {
                    if swarm.project_name == project_name {
                        swarm.manager.pane_content.push_str(&message);
                        break;
                    }
                }
            }
            Event::IssuesUpdated {
                project_name,
                issues,
            } => {
                let cache = self
                    .issue_caches
                    .entry(project_name)
                    .or_insert_with(IssueCache::default);
                let mut issues = issues;
                // Cross-reference: mark issues being worked by specific workers
                for swarm in &self.swarms {
                    for worker in &swarm.workers {
                        if let crate::model::status::AgentState::Working { issue: Some(n) } =
                            &worker.status.state
                        {
                            if let Some(issue) = issues.iter_mut().find(|i| i.number == *n) {
                                issue.assigned_worker = Some(worker.role.clone());
                            }
                        }
                    }
                }
                cache.issues = issues;
                cache.last_fetched = Some(std::time::Instant::now());
            }
            Event::GhWarning {
                project_name,
                message,
            } => {
                tracing::warn!("GitHub warning for {project_name}: {message}");
                self.set_status(message);
            }
            Event::IssueFetched { issue_number, body } => {
                if let Some(ref mut view) = self.issue_view {
                    if view.issue_number == issue_number {
                        view.body = body;
                    }
                }
            }
            Event::SwarmDiscovered => {
                // Remember which project we're viewing
                let current_project = match &self.screen {
                    Screen::RepoView { swarm_idx } => {
                        self.swarms.get(*swarm_idx).map(|s| s.project_name.clone())
                    }
                    Screen::AgentView { swarm_idx, .. } | Screen::IssueView { swarm_idx, .. } => {
                        self.swarms.get(*swarm_idx).map(|s| s.project_name.clone())
                    }
                    _ => None,
                };
                let close_startup_session = match &self.screen {
                    Screen::AgentView {
                        swarm_idx,
                        agent_id,
                    } => self
                        .swarms
                        .get(*swarm_idx)
                        .map(|swarm| {
                            agent_id == &swarm.manager.id && swarm.manager.tmux_target == "startup"
                        })
                        .unwrap_or(false),
                    _ => false,
                };

                if let Ok(swarms) = self.adapter.discover(&self.agents_dir).await {
                    self.swarms = swarms;
                    self.refresh_statuses();
                    self.start_all_pane_watchers();
                    self.start_all_issue_watchers();
                    self.scan_available_repos();

                    // Re-point the screen to the same project after re-discovery
                    if let Some(project) = current_project {
                        if let Some(new_idx) =
                            self.swarms.iter().position(|s| s.project_name == project)
                        {
                            match &self.screen {
                                Screen::RepoView { .. } => {
                                    self.screen = Screen::RepoView { swarm_idx: new_idx };
                                }
                                Screen::AgentView { agent_id, .. } => {
                                    if close_startup_session {
                                        self.screen = Screen::RepoView { swarm_idx: new_idx };
                                    } else {
                                        let aid = agent_id.clone();
                                        self.screen = Screen::AgentView {
                                            swarm_idx: new_idx,
                                            agent_id: aid,
                                        };
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            Event::TerminalResize { width, height } => {
                // Resize all tmux sessions to match the new terminal size
                let tmux_width = width.saturating_sub(2);
                for swarm in &self.swarms {
                    let session = swarm.tmux_session.clone();
                    let w = tmux_width;
                    let h = height;
                    tokio::spawn(async move {
                        if let Err(e) = crate::tmux::session::resize_session(&session, w, h).await {
                            tracing::warn!("Failed to resize session {session}: {e}");
                        }
                    });
                }
            }
            Event::IssuesRefreshed { swarm_idx, issues } => {
                if let Some(swarm) = self.swarms.get_mut(swarm_idx) {
                    swarm.issue_cache.issues = issues;
                    swarm.issue_cache.is_loading = false;
                }
            }
            Event::Error(msg) => {
                tracing::error!("Background error: {msg}");
            }
        }
        Ok(())
    }

    /// Returns true if we're in a passthrough mode where keystrokes go directly to a session.
    fn is_passthrough_mode(&self) -> bool {
        matches!(&self.screen, Screen::AgentView { .. })
            || matches!(&self.screen, Screen::RepoView { .. })
                && self.swarm_focus == SwarmPanel::Manager
    }

    async fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Global: Ctrl+C quits (but not in passthrough mode — there it goes to tmux)
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && key.code == KeyCode::Char('c')
            && !self.is_passthrough_mode()
        {
            self.running = false;
            return Ok(());
        }

        // Help overlay: Esc or ? closes it; all other keys are consumed
        if self.show_help {
            if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') {
                self.show_help = false;
            }
            return Ok(());
        }

        // ? key: toggle help overlay only on non-session screens
        if key.code == KeyCode::Char('?')
            && key.modifiers == KeyModifiers::NONE
            && matches!(
                self.screen,
                Screen::ReposList
                    | Screen::NewSwarm { .. }
                    | Screen::RuntimeSelect
                    | Screen::InstallScopeSelect
            )
        {
            self.show_help = true;
            return Ok(());
        }

        // Shortcuts viewer: ? toggles, any other key dismisses
        if self.show_shortcuts_viewer {
            if key.code == KeyCode::Char('?') {
                self.show_shortcuts_viewer = false;
            } else if key.code == KeyCode::Char('e') {
                // Open config file in editor
                let path = crate::config::shortcuts::ShortcutsConfig::config_path();
                self.show_shortcuts_viewer = false;
                self.set_status(format!("Edit: {}", path.display()));
            } else {
                self.show_shortcuts_viewer = false;
            }
            return Ok(());
        }

        // ? key opens shortcuts viewer (not in passthrough mode, not on non-session screens)
        if key.code == KeyCode::Char('?')
            && !self.is_passthrough_mode()
            && !matches!(
                self.screen,
                Screen::ReposList
                    | Screen::NewSwarm { .. }
                    | Screen::RuntimeSelect
                    | Screen::InstallScopeSelect
                    | Screen::AgentView { .. }
            )
        {
            self.show_shortcuts_viewer = true;
            return Ok(());
        }

        // Global: Alt+Left goes back one level
        if key.modifiers.contains(KeyModifiers::ALT) && key.code == KeyCode::Left {
            match &self.screen {
                Screen::AgentView { swarm_idx, .. }
                | Screen::IssueView { swarm_idx, .. }
                | Screen::IssueDetail { swarm_idx, .. }
                | Screen::IssueList { swarm_idx }
                | Screen::SwitchAgent { swarm_idx, .. } => {
                    let idx = *swarm_idx;
                    self.enter_repo_view(idx).await;
                }
                Screen::RepoView { .. } | Screen::NewSwarm { .. } => {
                    self.screen = Screen::ReposList;
                }
                Screen::InstallScopeSelect => {
                    self.pending_launch = None;
                    self.screen = Screen::NewSwarm {
                        field: NewSwarmField::NumWorkers,
                    };
                }
                Screen::RuntimeSelect => {}
                Screen::ReposList => {} // Already at top
            }
            return Ok(());
        }

        // Global: Alt+0 jumps to Repo View (swarm view), Alt+1-9 jumps to worker
        if key.modifiers.contains(KeyModifiers::ALT) {
            if let KeyCode::Char(c @ '0'..='9') = key.code {
                // Find the current swarm index (use 0 if on repos list)
                let swarm_idx = match &self.screen {
                    Screen::RepoView { swarm_idx } => *swarm_idx,
                    Screen::AgentView { swarm_idx, .. }
                    | Screen::IssueView { swarm_idx, .. }
                    | Screen::IssueList { swarm_idx }
                    | Screen::SwitchAgent { swarm_idx, .. } => *swarm_idx,
                    _ => {
                        // From repos list, use the selected swarm or first one
                        self.repos_list.selected().unwrap_or(0)
                    }
                };

                if c == '0' {
                    // Alt+0: go back one level
                    match &self.screen {
                        Screen::AgentView { swarm_idx, .. }
                        | Screen::IssueView { swarm_idx, .. }
                        | Screen::IssueDetail { swarm_idx, .. }
                        | Screen::IssueList { swarm_idx }
                        | Screen::SwitchAgent { swarm_idx, .. } => {
                            let idx = *swarm_idx;
                            self.enter_repo_view(idx).await;
                        }
                        Screen::RepoView { .. } | Screen::NewSwarm { .. } => {
                            self.screen = Screen::ReposList;
                        }
                        Screen::InstallScopeSelect => {
                            self.pending_launch = None;
                            self.screen = Screen::NewSwarm {
                                field: NewSwarmField::NumWorkers,
                            };
                        }
                        Screen::RuntimeSelect => {}
                        Screen::ReposList => {} // Already at top
                    }
                    return Ok(());
                }

                if swarm_idx < self.swarms.len() {
                    if c == '0' {
                        // Alt+0: go to Repo View with manager focused
                        self.enter_repo_view(swarm_idx).await;
                        self.repo_view.focus = crate::ui::repo_view::RepoViewFocus::ManagerInput;
                        return Ok(());
                    } else {
                        // Alt+1-9: jump to worker agent view
                        let worker_idx = (c as usize) - ('1' as usize);
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            if let Some(worker) = swarm.workers.get(worker_idx) {
                                let aid = worker.id.clone();
                                self.enter_agent_view(swarm_idx, aid).await;
                                return Ok(());
                            }
                        }
                    }
                }
                // Alt+a: jump to next agent needing attention (global shortcut)
                if c == 'a' {
                    let swarm_idx = match &self.screen {
                        Screen::RepoView { swarm_idx } => Some(*swarm_idx),
                        Screen::AgentView { swarm_idx, .. } => Some(*swarm_idx),
                        Screen::ReposList if self.swarms.len() == 1 => Some(0),
                        _ => None,
                    };
                    if let Some(idx) = swarm_idx {
                        self.jump_to_next_waiting(idx).await;
                        return Ok(());
                    }
                }

                // Alt+f: file feedback as GitHub issue
                if c == 'f' {
                    let (repo_path, repo_name) = match &self.screen {
                        Screen::RepoView { swarm_idx } | Screen::AgentView { swarm_idx, .. } => {
                            let idx = *swarm_idx;
                            self.swarms
                                .get(idx)
                                .map(|s| (s.repo_path.clone(), s.project_name.clone()))
                                .unwrap_or_else(|| {
                                    (std::path::PathBuf::from("."), "agents-ui".to_string())
                                })
                        }
                        _ => (std::path::PathBuf::from("."), "agents-ui".to_string()),
                    };
                    self.feedback_state = Some(crate::ui::feedback_dialog::FeedbackState::new(
                        repo_name, repo_path,
                    ));
                    return Ok(());
                }
            }
        }

        // Esc closes feedback overlay first
        if key.code == KeyCode::Esc {
            if self.feedback_state.is_some() {
                self.feedback_state = None;
                return Ok(());
            }
            // existing Esc handling continues below
        }

        // Feedback dialog captures all keys when open
        if self.feedback_state.is_some() {
            self.handle_feedback_key(key).await?;
            return Ok(());
        }

        match &self.screen.clone() {
            Screen::RuntimeSelect => self.handle_runtime_select_key(key).await?,
            Screen::InstallScopeSelect => self.handle_install_scope_key(key).await?,
            Screen::ReposList => self.handle_repos_list_key(key).await?,
            Screen::NewSwarm { field } => self.handle_new_swarm_key(key, field.clone()).await?,
            Screen::RepoView { swarm_idx } => self.handle_repo_view_key(key, *swarm_idx).await?,
            Screen::AgentView {
                swarm_idx,
                agent_id,
            } => {
                self.handle_agent_view_key(key, *swarm_idx, agent_id.clone())
                    .await?
            }
            Screen::IssueView {
                swarm_idx,
                issue_number,
            } => {
                self.handle_issue_view_key(key, *swarm_idx, *issue_number)
                    .await?
            }
            Screen::IssueDetail { swarm_idx } => {
                self.handle_issue_detail_key(key, *swarm_idx).await?
            }
            Screen::IssueList { swarm_idx } => self.handle_issue_list_key(key, *swarm_idx).await?,
            Screen::SwitchAgent {
                swarm_idx,
                selected,
            } => {
                self.handle_switch_agent_key(key, *swarm_idx, *selected)
                    .await?
            }
        }

        Ok(())
    }

    async fn handle_switch_agent_key(
        &mut self,
        key: KeyEvent,
        swarm_idx: usize,
        selected: usize,
    ) -> Result<()> {
        let n = self.switch_agent_options.len();
        if n == 0 {
            self.enter_repo_view(swarm_idx).await;
            self.set_status("No alternate runtimes are available for this repo".to_string());
            return Ok(());
        }
        match key.code {
            KeyCode::Esc | KeyCode::Backspace => {
                self.enter_repo_view(swarm_idx).await;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let new_selected = if selected == 0 { n - 1 } else { selected - 1 };
                self.screen = Screen::SwitchAgent {
                    swarm_idx,
                    selected: new_selected,
                };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.screen = Screen::SwitchAgent {
                    swarm_idx,
                    selected: (selected + 1) % n,
                };
            }
            KeyCode::Enter => {
                let Some(option) = self.switch_agent_options.get(selected).cloned() else {
                    return Ok(());
                };
                if !option.available {
                    self.set_status(format!(
                        "{} unavailable for this repo: {}",
                        option.agent_type, option.detail
                    ));
                    return Ok(());
                }

                let new_runtime = self
                    .switch_agent_options
                    .get(selected)
                    .map(|option| option.agent_type.clone())
                    .unwrap_or(crate::model::swarm::AgentType::Claude);
                self.status_message = Some(format!("Switching to {}...", new_runtime));
                self.enter_repo_view(swarm_idx).await;
                if swarm_idx < self.swarms.len() {
                    match self
                        .adapter
                        .switch_agent(&mut self.swarms[swarm_idx], new_runtime.clone())
                        .await
                    {
                        Err(e) => {
                            self.status_message = Some(format!("Switch failed: {e}"));
                        }
                        Ok(()) => {
                            let repo_path = self.swarms[swarm_idx].repo_path.clone();
                            if let Err(e) = crate::config::persistence::save_repo_agent_type(
                                &repo_path,
                                &new_runtime,
                            ) {
                                tracing::warn!(
                                    "Failed to persist switched runtime for {}: {}",
                                    repo_path.display(),
                                    e
                                );
                            }
                            self.status_message = Some(format!("Switched to {}", new_runtime));
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_runtime_select_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.default_agent_type = prev_runtime(&self.default_agent_type);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.default_agent_type = next_runtime(&self.default_agent_type);
            }
            KeyCode::Char('c') => {
                self.apply_runtime_selection(AgentType::Claude).await?;
            }
            KeyCode::Char('x') => {
                self.apply_runtime_selection(AgentType::Codex).await?;
            }
            KeyCode::Char('d') => {
                self.apply_runtime_selection(AgentType::Droid).await?;
            }
            KeyCode::Char('g') => {
                self.apply_runtime_selection(AgentType::Gemini).await?;
            }
            KeyCode::Enter => {
                self.apply_runtime_selection(self.default_agent_type.clone())
                    .await?;
            }
            KeyCode::Char('q') => {
                self.running = false;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_install_scope_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.install_scope = InstallScope::User;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.install_scope = InstallScope::Repo;
            }
            KeyCode::Char('u') => {
                self.install_scope = InstallScope::User;
            }
            KeyCode::Char('r') => {
                self.install_scope = InstallScope::Repo;
            }
            KeyCode::Esc => {
                self.pending_launch = None;
                self.screen = Screen::NewSwarm {
                    field: NewSwarmField::NumWorkers,
                };
            }
            KeyCode::Enter => {
                let pending = match self.pending_launch.clone() {
                    Some(p) => p,
                    None => {
                        self.screen = Screen::ReposList;
                        return Ok(());
                    }
                };

                self.pending_launch = None;
                self.start_swarm_bootstrap(
                    pending.repo_path,
                    pending.num_workers,
                    pending.agent_type,
                    Some(self.install_scope),
                );
            }
            _ => {}
        }
        Ok(())
    }

    async fn apply_runtime_selection(&mut self, agent_type: AgentType) -> Result<()> {
        self.default_agent_type = agent_type.clone();
        self.ensure_runtime_prerequisites(&agent_type).await?;
        self.adapter = ClaudeAdapter::new(agent_type.clone(), self.transport.clone());

        if !self.runtime_locked_from_cli && !self.transport.is_remote() {
            if let Some(root) = &self.runtime_pref_repo_root {
                if let Err(e) = crate::config::persistence::save_repo_agent_type(root, &agent_type)
                {
                    tracing::warn!(
                        "Failed to save runtime preference to {}: {e}",
                        root.display()
                    );
                }
            }
        }

        self.swarms = match self.adapter.discover(&self.agents_dir).await {
            Ok(s) => {
                tracing::info!("Discovered {} existing swarm(s)", s.len());
                s
            }
            Err(e) => {
                tracing::warn!("Failed to discover existing swarms: {e}");
                vec![]
            }
        };

        self.refresh_statuses();
        self.start_all_pane_watchers();
        self.start_all_issue_watchers();
        self.scan_available_repos();
        self.screen = Screen::ReposList;
        self.auto_select_current_repo_swarm().await;
        self.set_status(format!("Using {} runtime", self.default_agent_type));

        Ok(())
    }

    async fn auto_select_current_repo_swarm(&mut self) {
        if let Ok(cwd) = std::env::current_dir() {
            let cwd_name = cwd
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if let Some(idx) = self.swarms.iter().position(|s| s.project_name == cwd_name) {
                self.repos_list.table_state.select(Some(idx));
                self.enter_repo_view(idx).await;
            }
        }
    }

    fn preferred_agent_type_for_repo(&self, repo_path: &std::path::Path) -> AgentType {
        if self.runtime_locked_from_cli || self.transport.is_remote() {
            return self.default_agent_type.clone();
        }

        if let Some(root) = crate::config::persistence::find_repo_root(repo_path) {
            if let Ok(Some(saved)) = crate::config::persistence::load_repo_agent_type(&root) {
                return saved;
            }
        }

        self.default_agent_type.clone()
    }

    fn persist_agent_type_for_repo(&self, repo_path: &std::path::Path, agent_type: &AgentType) {
        if self.runtime_locked_from_cli || self.transport.is_remote() {
            return;
        }

        if let Some(root) = crate::config::persistence::find_repo_root(repo_path) {
            if let Err(e) = crate::config::persistence::save_repo_agent_type(&root, agent_type) {
                tracing::warn!(
                    "Failed to persist runtime preference to {}: {e}",
                    root.display()
                );
            }
        }
    }

    fn selected_agent_type_for_new_swarm(&self) -> AgentType {
        if self.runtime_locked_from_cli || self.transport.is_remote() {
            self.default_agent_type.clone()
        } else {
            self.new_swarm_agent_type.clone()
        }
    }

    async fn droid_plugin_installed(
        &self,
        repo_path: &std::path::Path,
        scope: &str,
    ) -> Result<bool> {
        let output = self
            .transport
            .output(
                "droid",
                &[
                    "plugin".to_string(),
                    "list".to_string(),
                    "--scope".to_string(),
                    scope.to_string(),
                ],
                Some(repo_path),
            )
            .await
            .context("Failed to check Droid plugins")?;

        if !output.status.success() {
            return Ok(false);
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
        Ok(stdout.contains("autocoder"))
    }

    async fn droid_repo_assets_present(&self, repo_path: &std::path::Path) -> bool {
        self.transport
            .path_exists(
                &repo_path
                    .join(".factory")
                    .join("skills")
                    .join("autocoder")
                    .join("SKILL.md"),
            )
            .await
    }

    async fn droid_agents_installed(&self, repo_path: &std::path::Path) -> Result<bool> {
        let user_installed = self.droid_plugin_installed(repo_path, "user").await?;
        let project_installed = self.droid_plugin_installed(repo_path, "project").await?;
        Ok(user_installed || project_installed || self.droid_repo_assets_present(repo_path).await)
    }

    async fn codex_agents_installed(&self, repo_path: &std::path::Path) -> bool {
        codex_repo_assets_present(&self.transport, repo_path).await
            || codex_user_assets_present(&self.transport).await
    }

    async fn gemini_agents_installed(&self, repo_path: &std::path::Path) -> bool {
        gemini_repo_assets_present(&self.transport, repo_path).await
    }

    async fn runtime_options_for_repo(
        &self,
        repo_path: &std::path::Path,
        current: &AgentType,
    ) -> Vec<RuntimeOption> {
        let mut options = Vec::new();

        for agent_type in ALL_AGENT_TYPES {
            let (binary, install_detail) = match agent_type {
                AgentType::Claude => ("claude", None),
                AgentType::Codex => ("codex", Some("needs Codex repo/user assets")),
                AgentType::Droid => ("droid", Some("needs Droid plugin/assets")),
                AgentType::Gemini => ("gemini", Some("needs laird/agents skills/scripts/agents")),
            };

            if agent_type != current && !self.transport.command_exists(binary).await {
                options.push(RuntimeOption {
                    agent_type: agent_type.clone(),
                    available: false,
                    detail: format!("{binary} not installed"),
                });
                continue;
            }

            let ready = match agent_type {
                AgentType::Codex => self.codex_agents_installed(repo_path).await,
                AgentType::Droid => self
                    .droid_agents_installed(repo_path)
                    .await
                    .unwrap_or(false),
                AgentType::Gemini => self.gemini_agents_installed(repo_path).await,
                _ => true,
            };

            options.push(RuntimeOption {
                agent_type: agent_type.clone(),
                available: ready || agent_type == current,
                detail: if agent_type == current {
                    "current".to_string()
                } else if ready {
                    "ready".to_string()
                } else {
                    install_detail.unwrap_or("unavailable").to_string()
                },
            });
        }

        options
    }

    fn start_swarm_bootstrap(
        &mut self,
        repo_path: PathBuf,
        num_workers: u32,
        agent_type: AgentType,
        install_scope: Option<InstallScope>,
    ) {
        let project_name = repo_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let manager_id = format!("{project_name}/manager");

        let placeholder = Swarm {
            repo_path: repo_path.clone(),
            project_name: project_name.clone(),
            agent_type: agent_type.clone(),
            workflow: None,
            tmux_session: format!("{}-{project_name}", agent_type.session_prefix()),
            manager: crate::model::swarm::AgentInfo {
                id: manager_id.clone(),
                role: "manager".to_string(),
                worktree_path: repo_path.clone(),
                tmux_target: "startup".to_string(),
                status: crate::model::status::AgentStatus::default(),
                is_manager: true,
                pane_content: format!(
                    "🚀 Starting swarm bootstrap for {project_name}...\n\n\
                     Workers: {num_workers}\n\
                     Runtime: {}\n\n\
                     ⏳ Opening startup session...\n",
                    agent_type,
                ),
                dispatched_issue: None,
                current_issue: None,
                current_issue_title: None,
                waiting_for_input: false,
            },
            workers: Vec::new(),
            issue_cache: crate::model::issue::IssueCache::default(),
            stopped: false,
        };

        self.swarms.push(placeholder);
        let swarm_idx = self.swarms.len() - 1;
        self.swarm_view = SwarmView::new();
        self.swarm_focus = SwarmPanel::Manager;
        self.agent_view = AgentView::new();
        self.agent_view.scroll_to_bottom();
        self.screen = Screen::AgentView {
            swarm_idx,
            agent_id: manager_id,
        };

        let config = SwarmConfig {
            repo_path,
            agent_type: agent_type.clone(),
            num_workers,
            agents_dir: self.agents_dir.clone(),
        };
        let tx = self.events.tx();
        let pname = project_name.clone();
        let adapter = ClaudeAdapter::new(agent_type, self.transport.clone());
        let transport = self.transport.clone();
        let agents_dir = self.agents_dir.clone();

        tokio::spawn(async move {
            let tx2 = tx.clone();
            let pname2 = pname.clone();
            let progress = move |msg: &str| {
                if let Err(e) = tx2.send(Event::LaunchProgress {
                    project_name: pname2.clone(),
                    message: msg.to_string(),
                }) {
                    tracing::debug!("Failed to send launch progress event: {e}");
                }
            };

            tracing::info!(
                "Background launch starting for {}",
                config.repo_path.display()
            );
            if let Some(scope) = install_scope {
                match install_agents_for_scope_with_progress(
                    &transport,
                    &agents_dir,
                    &config.repo_path,
                    config.agent_type.clone(),
                    scope,
                    &progress,
                )
                .await
                {
                    Ok(()) => {}
                    Err(e) => {
                        tracing::error!("Background install failed: {e}");
                        progress(&format!(
                            "\n❌ Startup failed during install: {e}\n\nPress Esc to go back.\n"
                        ));
                        return;
                    }
                }
            }

            match adapter.launch_with_progress(&config, &progress).await {
                Ok(swarm) => {
                    tracing::info!(
                        "Background launch succeeded: session={}",
                        swarm.tmux_session
                    );
                    progress("✅ Triggering swarm discovery...\n");
                    if let Err(e) = tx.send(Event::SwarmDiscovered) {
                        tracing::warn!("Failed to send swarm discovered event: {e}");
                    }
                }
                Err(e) => {
                    tracing::error!("Background launch failed: {e}");
                    progress(&format!(
                        "\n❌ Launch failed: {e}\n\nPress Esc to go back.\n"
                    ));
                }
            }
        });
    }

    /// Total rows in the repos list (active swarms + available repos).
    fn repos_list_len(&self) -> usize {
        self.swarms.len() + self.available_repos.len()
    }

    /// Enter repo view for a swarm, resizing the manager's tmux pane to fill the terminal.
    async fn enter_repo_view(&mut self, swarm_idx: usize) {
        if let Some(swarm) = self.swarms.get(swarm_idx) {
            let target = swarm.manager.tmux_target.clone();
            if let Ok((width, height)) = crossterm::terminal::size() {
                if let Err(e) =
                    proxy::resize_pane(&self.transport, &target, width.saturating_sub(2), height)
                        .await
                {
                    tracing::warn!("Failed to resize manager pane {target}: {e}");
                }
            }
        }
        self.repo_view = RepoView::new();
        self.screen = Screen::RepoView { swarm_idx };
    }

    /// Enter agent view for a specific agent, resizing its tmux pane to full terminal width.
    async fn enter_agent_view(&mut self, swarm_idx: usize, agent_id: String) {
        if let Some(swarm) = self.swarms.get(swarm_idx) {
            if let Some(agent) = swarm.agent(&agent_id) {
                let target = agent.tmux_target.clone();
                if let Ok((width, height)) = crossterm::terminal::size() {
                    if let Err(e) = proxy::resize_pane(
                        &self.transport,
                        &target,
                        width.saturating_sub(2),
                        height,
                    )
                    .await
                    {
                        tracing::warn!("Failed to resize agent pane {target}: {e}");
                    }
                }
            }
        }
        self.agent_view = AgentView::new();
        self.agent_view.scroll_to_bottom();
        self.screen = Screen::AgentView {
            swarm_idx,
            agent_id,
        };
    }

    /// Handle selecting a row in the repos list.
    /// If it's an active swarm, jump to repo view.
    /// If it's an available repo, open the new swarm dialog pre-filled.
    async fn select_repo_row(&mut self, idx: usize) -> Result<()> {
        if idx < self.swarms.len() {
            // Active swarm — jump to repo view
            self.enter_repo_view(idx).await;
        } else {
            // Available repo — open new swarm dialog pre-filled
            let avail_idx = idx - self.swarms.len();
            if let Some(repo_path) = self.available_repos.get(avail_idx) {
                self.new_swarm_repo = repo_path.to_string_lossy().to_string();
                self.new_swarm_agent_type = self.preferred_agent_type_for_repo(repo_path);
                self.dialog_input = TextInput::with_text("2".to_string());
                self.status_message = None;
                self.screen = if self.runtime_locked_from_cli || self.transport.is_remote() {
                    Screen::NewSwarm {
                        field: NewSwarmField::NumWorkers,
                    }
                } else {
                    Screen::NewSwarm {
                        field: NewSwarmField::AgentRuntime,
                    }
                };
            }
        }
        Ok(())
    }

    async fn handle_repos_list_key(&mut self, key: KeyEvent) -> Result<()> {
        // Handle teardown confirmation
        if let Some(idx) = self.confirm_teardown {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    if idx < self.swarms.len() {
                        let swarm = self.swarms[idx].clone();
                        let project = swarm.project_name.clone();
                        self.intentionally_stopped.insert(project.clone());
                        self.set_status(format!("Tearing down {project}..."));
                        if let Err(e) = self.adapter.teardown(&swarm).await {
                            self.set_status(format!("Teardown error: {e}"));
                        } else {
                            self.swarms.remove(idx);
                            self.start_all_pane_watchers();
                            self.scan_available_repos();
                            self.set_status(format!("Torn down {project}"));
                        }
                    }
                    self.confirm_teardown = None;
                }
                _ => {
                    // Any other key cancels
                    self.confirm_teardown = None;
                    self.set_status("Teardown cancelled".to_string());
                }
            }
            return Ok(());
        }

        let total = self.repos_list_len();
        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Down | KeyCode::Char('j') => {
                self.repos_list.next(total);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.repos_list.previous(total);
            }
            KeyCode::Enter => {
                if let Some(idx) = self.repos_list.selected() {
                    self.select_repo_row(idx).await?;
                }
            }
            KeyCode::Char('n') => {
                // New swarm dialog — pre-fill with current directory
                self.dialog_input.set_text(
                    std::env::current_dir()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default(),
                );
                self.new_swarm_repo = String::new();
                self.status_message = None;
                self.screen = Screen::NewSwarm {
                    field: NewSwarmField::RepoPath,
                };
            }
            KeyCode::Char('r') => {
                self.refresh_repo_list().await;
            }
            KeyCode::Char('d') => {
                // Tear down the selected swarm (with confirmation)
                if let Some(idx) = self.repos_list.selected() {
                    if idx < self.swarms.len() {
                        let project = self.swarms[idx].project_name.clone();
                        self.confirm_teardown = Some(idx);
                        self.set_status(format!(
                            "Tear down {project}? (y to confirm, any other key to cancel)"
                        ));
                    }
                }
            }
            KeyCode::Char(c @ '1'..='9') => {
                // Jump to repo N (1-indexed, across active + available)
                let idx = (c as usize) - ('1' as usize);
                if idx < self.repos_list_len() {
                    self.repos_list.table_state.select(Some(idx));
                    self.select_repo_row(idx).await?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_new_swarm_key(&mut self, key: KeyEvent, field: NewSwarmField) -> Result<()> {
        match field {
            NewSwarmField::RepoPath => match key.code {
                KeyCode::Esc => {
                    self.screen = Screen::ReposList;
                }
                KeyCode::Enter => {
                    let path = if self.dialog_input.is_empty() {
                        // Default to current directory
                        std::env::current_dir()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    } else {
                        self.dialog_input.text().to_string()
                    };

                    // Expand ~ to home dir
                    let path = if path.starts_with('~') {
                        if let Some(home) = dirs::home_dir() {
                            path.replacen('~', &home.to_string_lossy(), 1)
                        } else {
                            path
                        }
                    } else {
                        path
                    };

                    let repo_path = PathBuf::from(&path);
                    if !self.repo_path_exists(&repo_path).await {
                        self.set_status(format!("Path not found: {path}"));
                        self.screen = Screen::ReposList;
                        return Ok(());
                    }

                    self.new_swarm_repo = path;
                    self.new_swarm_agent_type = self.preferred_agent_type_for_repo(&repo_path);
                    self.screen = Screen::NewSwarm {
                        field: NewSwarmField::AgentRuntime,
                    };
                }
                KeyCode::Left => {
                    self.dialog_input.move_left();
                }
                KeyCode::Right => {
                    self.dialog_input.move_right();
                }
                KeyCode::Home => {
                    self.dialog_input.move_home();
                }
                KeyCode::End => {
                    self.dialog_input.move_end();
                }
                KeyCode::Delete => {
                    self.dialog_input.delete();
                }
                KeyCode::Char(c) => {
                    self.dialog_input.insert_char(c);
                }
                KeyCode::Backspace => {
                    self.dialog_input.backspace();
                }
                KeyCode::Tab => {
                    // Simple tab completion: try to complete the path
                    if let Some(completed) = tab_complete_path(self.dialog_input.text()) {
                        self.dialog_input.set_text(completed);
                    }
                }
                _ => {}
            },
            NewSwarmField::AgentRuntime => match key.code {
                KeyCode::Esc => {
                    self.screen = Screen::NewSwarm {
                        field: NewSwarmField::RepoPath,
                    };
                    self.dialog_input.set_text(self.new_swarm_repo.clone());
                }
                KeyCode::Enter => {
                    self.dialog_input.set_text("2".to_string()); // Default 2 workers
                    self.screen = Screen::NewSwarm {
                        field: NewSwarmField::RuntimeSelection,
                    };
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let idx = ALL_AGENT_TYPES
                        .iter()
                        .position(|t| *t == self.new_swarm_agent_type)
                        .unwrap_or(0);
                    let new_idx = if idx == 0 {
                        ALL_AGENT_TYPES.len() - 1
                    } else {
                        idx - 1
                    };
                    self.new_swarm_agent_type = ALL_AGENT_TYPES[new_idx].clone();
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let idx = ALL_AGENT_TYPES
                        .iter()
                        .position(|t| *t == self.new_swarm_agent_type)
                        .unwrap_or(0);
                    let new_idx = (idx + 1) % ALL_AGENT_TYPES.len();
                    self.new_swarm_agent_type = ALL_AGENT_TYPES[new_idx].clone();
                }
                _ => {}
            },
            NewSwarmField::RuntimeSelection => match key.code {
                KeyCode::Esc => {
                    self.screen = Screen::NewSwarm {
                        field: NewSwarmField::AgentRuntime,
                    };
                }
                KeyCode::Enter => {
                    self.dialog_input.set_text("2".to_string()); // Default 2 workers
                    self.screen = Screen::NewSwarm {
                        field: NewSwarmField::NumWorkers,
                    };
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    self.new_swarm_agent_type = match self.new_swarm_agent_type {
                        AgentType::Claude => AgentType::Gemini,
                        AgentType::Codex => AgentType::Claude,
                        AgentType::Droid => AgentType::Codex,
                        AgentType::Gemini => AgentType::Droid,
                    };
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    self.new_swarm_agent_type = match self.new_swarm_agent_type {
                        AgentType::Claude => AgentType::Codex,
                        AgentType::Codex => AgentType::Droid,
                        AgentType::Droid => AgentType::Gemini,
                        AgentType::Gemini => AgentType::Claude,
                    };
                }
                KeyCode::Char('c') => self.new_swarm_agent_type = AgentType::Claude,
                KeyCode::Char('x') => self.new_swarm_agent_type = AgentType::Codex,
                KeyCode::Char('d') => self.new_swarm_agent_type = AgentType::Droid,
                KeyCode::Char('g') => self.new_swarm_agent_type = AgentType::Gemini,
                _ => {}
            },
            NewSwarmField::NumWorkers => match key.code {
                KeyCode::Esc => {
                    self.screen = Screen::NewSwarm {
                        field: NewSwarmField::RuntimeSelection,
                    };
                }
                KeyCode::Enter => {
                    let num_workers: u32 = self.dialog_input.text().parse().unwrap_or(2);
                    let repo_path = PathBuf::from(&self.new_swarm_repo);
                    self.dialog_input = TextInput::new();
                    let agent_type = self.selected_agent_type_for_new_swarm();
                    self.persist_agent_type_for_repo(&repo_path, &agent_type);

                    if agent_type == AgentType::Droid {
                        match self.droid_agents_installed(&repo_path).await {
                            Ok(true) => {
                                self.start_swarm_bootstrap(
                                    repo_path,
                                    num_workers,
                                    agent_type,
                                    None,
                                );
                            }
                            Ok(false) => {
                                self.pending_launch = Some(PendingLaunch {
                                    repo_path,
                                    num_workers,
                                    agent_type,
                                });
                                self.install_scope = InstallScope::User;
                                self.screen = Screen::InstallScopeSelect;
                            }
                            Err(e) => {
                                self.set_status(format!("Failed to check Droid install: {e}"));
                                self.screen = Screen::ReposList;
                            }
                        }
                    } else if agent_type == AgentType::Gemini {
                        if self.gemini_agents_installed(&repo_path).await {
                            self.start_swarm_bootstrap(repo_path, num_workers, agent_type, None);
                        } else {
                            self.pending_launch = Some(PendingLaunch {
                                repo_path,
                                num_workers,
                                agent_type,
                            });
                            self.install_scope = InstallScope::User;
                            self.screen = Screen::InstallScopeSelect;
                        }
                    } else if agent_type == AgentType::Codex {
                        if self.codex_agents_installed(&repo_path).await {
                            self.start_swarm_bootstrap(repo_path, num_workers, agent_type, None);
                        } else {
                            self.start_swarm_bootstrap(
                                repo_path,
                                num_workers,
                                agent_type,
                                Some(InstallScope::Repo),
                            );
                        }
                    } else {
                        self.start_swarm_bootstrap(repo_path, num_workers, agent_type, None);
                    }
                }
                KeyCode::Up => {
                    let n: u32 = self.dialog_input.text().parse().unwrap_or(1);
                    self.dialog_input.set_text((n + 1).to_string());
                }
                KeyCode::Down => {
                    let n: u32 = self.dialog_input.text().parse().unwrap_or(2);
                    self.dialog_input
                        .set_text(n.max(2).saturating_sub(1).to_string());
                }
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    self.dialog_input.insert_char(c);
                }
                KeyCode::Backspace => {
                    self.dialog_input.backspace();
                }
                _ => {}
            },
        }
        Ok(())
    }

    async fn handle_repo_view_key(&mut self, key: KeyEvent, swarm_idx: usize) -> Result<()> {
        // Handle teardown confirmation from Repo View
        if let Some(idx) = self.confirm_teardown {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    if idx < self.swarms.len() {
                        let swarm = self.swarms[idx].clone();
                        let project = swarm.project_name.clone();
                        self.intentionally_stopped.insert(project.clone());
                        self.set_status(format!("Tearing down {project}..."));
                        if let Err(e) = self.adapter.teardown(&swarm).await {
                            self.set_status(format!("Teardown error: {e}"));
                        } else {
                            self.swarms.remove(idx);
                            self.start_all_pane_watchers();
                            self.scan_available_repos();
                            self.set_status(format!("Torn down {project}"));
                            self.screen = Screen::ReposList;
                        }
                    }
                    self.confirm_teardown = None;
                }
                _ => {
                    self.confirm_teardown = None;
                    self.set_status("Teardown cancelled".to_string());
                }
            }
            return Ok(());
        }

        // Handle stop-all confirmation from Repo View
        if let Some(idx) = self.confirm_stop {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    if idx < self.swarms.len() {
                        let swarm = self.swarms[idx].clone();
                        let project = swarm.project_name.clone();
                        // Write tombstones before stopping so heal/revive won't respawn.
                        crate::config::persistence::mark_swarm_stopped(&project);
                        for worker in &swarm.workers {
                            crate::adapter::claude::mark_agent_stopped(&worker.worktree_path);
                        }
                        if let Err(e) = self.adapter.stop(&swarm).await {
                            self.set_status(format!("Stop error: {e}"));
                        } else {
                            self.swarms[idx].stopped = true;
                            self.set_status(format!("Stopped all workers in {project}"));
                        }
                    }
                    self.confirm_stop = None;
                }
                _ => {
                    self.confirm_stop = None;
                    self.set_status("Stop cancelled".to_string());
                }
            }
            return Ok(());
        }

        // Handle create-issue dialog input
        let mut submit_issue: Option<(String, String)> = None;
        if let Some(ref mut form) = self.create_issue_form {
            match key.code {
                KeyCode::Esc => {
                    self.create_issue_form = None;
                }
                KeyCode::Tab | KeyCode::Down if form.field != CreateIssueField::Labels => {
                    form.field = match form.field {
                        CreateIssueField::Title => CreateIssueField::Body,
                        CreateIssueField::Body => CreateIssueField::Priority,
                        CreateIssueField::Priority => CreateIssueField::IssueType,
                        CreateIssueField::IssueType => CreateIssueField::Labels,
                        CreateIssueField::Labels => CreateIssueField::Labels,
                    };
                }
                KeyCode::BackTab | KeyCode::Up if form.field != CreateIssueField::Title => {
                    form.field = match form.field {
                        CreateIssueField::Title => CreateIssueField::Title,
                        CreateIssueField::Body => CreateIssueField::Title,
                        CreateIssueField::Priority => CreateIssueField::Body,
                        CreateIssueField::IssueType => CreateIssueField::Priority,
                        CreateIssueField::Labels => CreateIssueField::IssueType,
                    };
                }
                KeyCode::Enter => {
                    if !form.title.is_empty() {
                        submit_issue = Some((form.title.clone(), form.labels_string()));
                    }
                    self.create_issue_form = None;
                }
                KeyCode::Left => match form.field {
                    CreateIssueField::Priority => form.priority = form.priority.prev(),
                    CreateIssueField::IssueType => form.issue_type = form.issue_type.toggle(),
                    CreateIssueField::Labels => {
                        if form.label_cursor > 0 {
                            form.label_cursor -= 1;
                        }
                    }
                    _ => {}
                },
                KeyCode::Right => match form.field {
                    CreateIssueField::Priority => form.priority = form.priority.next(),
                    CreateIssueField::IssueType => form.issue_type = form.issue_type.toggle(),
                    CreateIssueField::Labels => {
                        if form.label_cursor < BLOCKING_LABELS.len() - 1 {
                            form.label_cursor += 1;
                        }
                    }
                    _ => {}
                },
                KeyCode::Char(' ') if form.field == CreateIssueField::Labels => {
                    let idx = form.label_cursor;
                    form.label_toggles[idx] = !form.label_toggles[idx];
                }
                KeyCode::Char(' ') if form.field == CreateIssueField::Priority => {
                    form.priority = form.priority.next();
                }
                KeyCode::Char(' ') if form.field == CreateIssueField::IssueType => {
                    form.issue_type = form.issue_type.toggle();
                }
                KeyCode::Char(c) if form.field == CreateIssueField::Title => {
                    form.title.push(c);
                }
                KeyCode::Backspace if form.field == CreateIssueField::Title => {
                    form.title.pop();
                }
                KeyCode::Char(c) if form.field == CreateIssueField::Body => {
                    form.body.push(c);
                }
                KeyCode::Backspace if form.field == CreateIssueField::Body => {
                    form.body.pop();
                }
                _ => {}
            }
            if let Some((title, labels)) = submit_issue {
                self.create_issue(swarm_idx, title, labels).await;
            }
            return Ok(());
        }

        // Esc goes back one level:
        // Sessions/Issues → Manager focus, Manager → Repos List
        if key.code == KeyCode::Esc {
            match self.swarm_focus {
                SwarmPanel::Workers | SwarmPanel::Issues => {
                    self.swarm_focus = SwarmPanel::Manager;
                }
                SwarmPanel::Manager => {
                    self.screen = Screen::ReposList;
                }
            }
            return Ok(());
        }

        // Tab cycles focus: Manager → Workers → Issues → Manager
        if key.code == KeyCode::Tab {
            self.swarm_view.search_query = None;
            self.swarm_focus = self.swarm_focus.next();
            return Ok(());
        }

        match self.swarm_focus {
            SwarmPanel::Manager => {
                // Manager pane: passthrough all keys to tmux
                let target = self
                    .swarms
                    .get(swarm_idx)
                    .map(|s| s.manager.tmux_target.clone());
                let target = match target {
                    Some(t) => t,
                    None => return Ok(()),
                };

                // PgUp/PgDn scroll manager view
                match key.code {
                    KeyCode::PageUp => {
                        self.swarm_view.scroll_manager_up(10);
                        return Ok(());
                    }
                    KeyCode::PageDown => {
                        self.swarm_view.scroll_manager_down(10);
                        return Ok(());
                    }
                    _ => {}
                }

                // Alt+d: send deploy command to manager
                if key.modifiers.contains(KeyModifiers::ALT) && key.code == KeyCode::Char('d') {
                    let cmd = "deploy";
                    tracing::info!("Sending '{cmd}' to manager at {target}");
                    proxy::send_keys(&self.transport, &target, cmd).await?;
                    self.swarm_view.scroll_manager_to_bottom();
                    self.set_status("Sent deploy to manager".to_string());
                    return Ok(());
                }

                // Forward everything else to the manager tmux pane
                match key.code {
                    KeyCode::Enter => {
                        self.manager_input.drain();
                    }
                    KeyCode::Left => self.manager_input.move_left(),
                    KeyCode::Right => self.manager_input.move_right(),
                    KeyCode::Home => self.manager_input.move_home(),
                    KeyCode::End => self.manager_input.move_end(),
                    KeyCode::Delete => self.manager_input.delete(),
                    KeyCode::Backspace => self.manager_input.backspace(),
                    KeyCode::Char(c)
                        if !key.modifiers.contains(KeyModifiers::CONTROL)
                            && !key.modifiers.contains(KeyModifiers::ALT) =>
                    {
                        self.manager_input.insert_char(c);
                    }
                    _ => {}
                }
                let tmux_key = key_event_to_tmux(key);
                if let Some(tmux_key) = tmux_key {
                    send_raw_key(&self.transport, &target, &tmux_key).await?;
                    self.swarm_view.scroll_manager_to_bottom();
                }
            }
            SwarmPanel::Workers => {
                let worker_count = self
                    .swarms
                    .get(swarm_idx)
                    .map(|s| s.workers.len())
                    .unwrap_or(0);
                match key.code {
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.swarm_view.next_worker(worker_count);
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if self.swarm_view.prev_worker(worker_count) {
                            self.swarm_focus = SwarmPanel::Manager;
                        }
                    }
                    KeyCode::Enter => {
                        // Drill into selected worker's agent view
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            if let Some(worker_idx) = self.swarm_view.selected_worker() {
                                if let Some(worker) = swarm.workers.get(worker_idx) {
                                    let aid = worker.role.clone();
                                    self.enter_agent_view(swarm_idx, aid).await;
                                }
                            }
                        }
                    }
                    KeyCode::Char('a') => {
                        // Add a new worker
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            let swarm_clone = swarm.clone();
                            self.set_status("Adding worker...".to_string());
                            match self.adapter.add_worker(&swarm_clone).await {
                                Ok(worker) => {
                                    let id = worker.role.clone();
                                    if let Some(swarm) = self.swarms.get_mut(swarm_idx) {
                                        swarm.workers.push(worker);
                                    }
                                    self.start_all_pane_watchers();
                                    self.set_status(format!("Added {id}"));
                                }
                                Err(e) => {
                                    self.set_status(format!("Failed: {e}"));
                                }
                            }
                        }
                    }
                    KeyCode::Char('f') => {
                        // Start the selected worker loop (only if idle)
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            if let Some(worker_idx) = self.swarm_view.selected_worker() {
                                if let Some(worker) = swarm.workers.get(worker_idx) {
                                    if !matches!(
                                        worker.status.state,
                                        crate::model::status::AgentState::Idle
                                    ) {
                                        self.set_status(format!(
                                            "Worker {} is busy — wait until idle",
                                            worker.role
                                        ));
                                    } else {
                                        let target = worker.tmux_target.clone();
                                        let id = worker.role.clone();
                                        if let Err(e) =
                                            self.adapter.start_worker_loop(&target).await
                                        {
                                            self.set_status(format!("Failed: {e}"));
                                        } else {
                                            self.set_status(format!(
                                                "Started worker loop for {id}"
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char('F') => {
                        // Send /fix-loop to ALL idle workers at once
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            let idle_workers: Vec<(String, String)> = swarm
                                .workers
                                .iter()
                                .filter(|w| {
                                    matches!(w.status.state, crate::model::status::AgentState::Idle)
                                })
                                .map(|w| (w.tmux_target.clone(), w.role.clone()))
                                .collect();
                            if idle_workers.is_empty() {
                                self.set_status("No idle workers to start".to_string());
                            } else {
                                let count = idle_workers.len();
                                for (target, id) in idle_workers {
                                    if let Err(e) = self.adapter.start_worker_loop(&target).await {
                                        tracing::error!("Failed to send /fix-loop to {id}: {e}");
                                    }
                                }
                                self.set_status(format!(
                                    "Sent /fix-loop to {count} idle worker{}",
                                    if count == 1 { "" } else { "s" }
                                ));
                            }
                        }
                    }
                    KeyCode::Char('b') => {
                        self.jump_to_next_blocked(swarm_idx);
                    }
                    KeyCode::Char('r') => {
                        self.refresh_statuses();
                        self.set_status("Refreshed worker list".to_string());
                    }
                    KeyCode::Char('d') => {
                        // Shut down selected worker
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            if let Some(worker_idx) = self.swarm_view.selected_worker() {
                                if let Some(worker) = swarm.workers.get(worker_idx) {
                                    let target = worker.tmux_target.clone();
                                    let id = worker.role.clone();
                                    // Write tombstone before killing so heal/revive won't respawn.
                                    crate::adapter::claude::mark_agent_stopped(
                                        &worker.worktree_path,
                                    );
                                    if let Err(e) = proxy::kill_pane(&self.transport, &target).await
                                    {
                                        tracing::warn!("Failed to shut down {id} at {target}: {e}");
                                        self.set_status(format!("Failed shutting down {id}: {e}"));
                                    } else {
                                        self.set_status(format!("Shutting down {id}..."));
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char('S') => {
                        // Open switch-agent dialog
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            self.switch_agent_options = self
                                .runtime_options_for_repo(&swarm.repo_path, &swarm.agent_type)
                                .await;
                            let selected = self
                                .switch_agent_options
                                .iter()
                                .position(|option| option.agent_type == swarm.agent_type)
                                .or_else(|| {
                                    self.switch_agent_options
                                        .iter()
                                        .position(|option| option.available)
                                })
                                .unwrap_or(0);
                            self.screen = Screen::SwitchAgent {
                                swarm_idx,
                                selected,
                            };
                        }
                        return Ok(());
                    }
                    KeyCode::Char('W') => {
                        // Stop all workers (with confirmation)
                        if swarm_idx < self.swarms.len() {
                            let project = self.swarms[swarm_idx].project_name.clone();
                            self.confirm_stop = Some(swarm_idx);
                            self.set_status(format!("Stop all workers in {project}? (y to confirm, any other key to cancel)"));
                        }
                    }
                    KeyCode::Char('T') => {
                        // Teardown entire swarm (with confirmation)
                        if swarm_idx < self.swarms.len() {
                            let project = self.swarms[swarm_idx].project_name.clone();
                            self.confirm_teardown = Some(swarm_idx);
                            self.set_status(format!(
                                "Teardown swarm {project}? (y to confirm, any other key to cancel)"
                            ));
                        }
                    }
                    KeyCode::Char('L') => {
                        // Open full issue list
                        self.issue_list_view = IssueListView::new();
                        self.screen = Screen::IssueList { swarm_idx };
                    }
                    KeyCode::Char('g') => {
                        // Open selected worker's current issue in browser
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            if let Some(worker_idx) = self.swarm_view.selected_worker() {
                                if let Some(worker) = swarm.workers.get(worker_idx) {
                                    let issue_num =
                                        worker.current_issue.or(worker.dispatched_issue);
                                    if let Some(num) = issue_num {
                                        let repo_path = swarm.repo_path.clone();
                                        let transport = self.transport.clone();
                                        tokio::spawn(async move {
                                            match crate::github::gh_repo_output(
                                                &transport,
                                                &repo_path,
                                                &[
                                                    "issue".to_string(),
                                                    "view".to_string(),
                                                    num.to_string(),
                                                    "--web".to_string(),
                                                ],
                                            )
                                            .await
                                            {
                                                Ok(out) if out.status.success() => {}
                                                Ok(out) => {
                                                    let err = String::from_utf8_lossy(&out.stderr)
                                                        .trim()
                                                        .to_string();
                                                    tracing::warn!(
                                                        "Failed opening issue #{} in browser for {}: {}",
                                                        num,
                                                        repo_path.display(),
                                                        err
                                                    );
                                                }
                                                Err(e) => {
                                                    tracing::warn!(
                                                        "Error opening issue #{} in browser for {}: {}",
                                                        num,
                                                        repo_path.display(),
                                                        e
                                                    );
                                                }
                                            }
                                        });
                                        self.set_status(format!("Opening issue #{num} in browser"));
                                    } else {
                                        self.set_status(format!(
                                            "No issue assigned to {}",
                                            worker.role
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char(c @ '1'..='9') => {
                        let worker_idx = (c as usize) - ('1' as usize);
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            if let Some(worker) = swarm.workers.get(worker_idx) {
                                let aid = worker.role.clone();
                                self.enter_agent_view(swarm_idx, aid).await;
                            }
                        }
                    }
                    _ => {
                        if let KeyCode::Char(c) = key.code {
                            self.try_shortcut("workers", &c.to_string(), swarm_idx, None)
                                .await?;
                        }
                    }
                }
            }
            SwarmPanel::Issues => {
                // Search mode intercepts all input
                if self.swarm_view.search_query.is_some() {
                    match key.code {
                        KeyCode::Esc => {
                            self.swarm_view.search_query = None;
                            self.swarm_view.issues_table.select(Some(0));
                        }
                        KeyCode::Enter => {
                            self.swarm_view.search_query = None;
                        }
                        KeyCode::Backspace => {
                            if let Some(q) = self.swarm_view.search_query.as_mut() {
                                q.pop();
                            }
                            self.swarm_view.issues_table.select(Some(0));
                        }
                        KeyCode::Char(c) => {
                            if let Some(q) = self.swarm_view.search_query.as_mut() {
                                q.push(c);
                            }
                            self.swarm_view.issues_table.select(Some(0));
                        }
                        _ => {}
                    }
                    return Ok(());
                }

                let issue_count = self
                    .swarms
                    .get(swarm_idx)
                    .and_then(|s| self.issue_caches.get(&s.project_name))
                    .map(|c| self.swarm_view.apply_filters(&c.issues).len())
                    .unwrap_or(0);
                match key.code {
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.swarm_view.next_issue(issue_count);
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.swarm_view.prev_issue(issue_count);
                    }
                    KeyCode::Char('f') => {
                        // Cycle issue filter
                        self.swarm_view.issue_filter = self.swarm_view.issue_filter.next();
                        self.swarm_view.issues_table.select(Some(0));
                    }
                    KeyCode::Char('t') => {
                        // Cycle issue type filter (Bug → Enhancement → Proposal → All)
                        self.swarm_view.cycle_issue_type_filter();
                    }
                    KeyCode::Char('P') => {
                        // Cycle priority filter (None → P0 → P1 → P2 → P3 → None)
                        self.swarm_view.cycle_priority_filter();
                    }
                    KeyCode::Char('a') => {
                        // Add new issue: open create-issue dialog
                        self.create_issue_form = Some(CreateIssueForm::new());
                    }
                    KeyCode::Char('p') => {
                        // Approve: send "approve <issue_number>" to manager pane
                        self.send_issue_command_to_manager(swarm_idx, "approve")
                            .await?;
                    }
                    KeyCode::Char('b') => {
                        // Jump to next blocked issue (cycle through blocked issues)
                        self.jump_to_next_blocked(swarm_idx);
                    }
                    KeyCode::Char('r') => {
                        self.start_issue_refresh(swarm_idx);
                        self.status_message = Some("Refreshing issues...".to_string());
                    }
                    KeyCode::Char('R') => {
                        // Review-blocked in selected runtime (only if manager is idle)
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            let manager_idle = matches!(
                                swarm.manager.status.state,
                                crate::model::status::AgentState::Idle
                            );
                            let agent_type = swarm.agent_type.clone();
                            let target = swarm.manager.tmux_target.clone();
                            if !manager_idle {
                                self.set_status("Manager is busy — wait until idle".to_string());
                            } else if let Some(cmd) = self.review_blocked_cmd(&agent_type) {
                                tracing::info!("Sending '{cmd}' to manager at {target}");
                                proxy::send_keys(&self.transport, &target, &cmd).await?;
                                self.set_status(format!("Sent: {cmd}"));
                            } else {
                                self.set_status(format!(
                                    "No review-blocked command configured for {}",
                                    agent_type
                                ));
                            }
                        }
                    }
                    KeyCode::Enter => {
                        // Drill into issue detail view
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            let issues: Vec<&GitHubIssue> = self
                                .issue_caches
                                .get(&swarm.project_name)
                                .map(|c| self.swarm_view.apply_filters(&c.issues))
                                .unwrap_or_default();
                            if let Some(issue) = self
                                .swarm_view
                                .selected_issue()
                                .and_then(|idx| issues.get(idx))
                            {
                                let num = issue.number;
                                let repo_path = swarm.repo_path.clone();
                                let transport = self.transport.clone();
                                let tx = self.events.tx();
                                self.issue_view = Some(crate::ui::issue_view::IssueView::new(num));
                                self.screen = Screen::IssueView {
                                    swarm_idx,
                                    issue_number: num,
                                };
                                // Fetch issue body in background
                                tokio::spawn(async move {
                                    let result = crate::github::gh_repo_output(
                                        &transport,
                                        &repo_path,
                                        &["issue".to_string(), "view".to_string(), num.to_string()],
                                    )
                                    .await;
                                    match result {
                                        Ok(output) if output.status.success() => {
                                            let body =
                                                String::from_utf8_lossy(&output.stdout).to_string();
                                            if let Err(e) =
                                                tx.send(crate::event::Event::IssueFetched {
                                                    issue_number: num,
                                                    body,
                                                })
                                            {
                                                tracing::debug!(
                                                    "Failed sending issue body update for #{}: {}",
                                                    num,
                                                    e
                                                );
                                            }
                                        }
                                        Ok(output) => {
                                            let err = String::from_utf8_lossy(&output.stderr)
                                                .trim()
                                                .to_string();
                                            tracing::warn!(
                                                "Failed to fetch issue #{} body for {}: {}",
                                                num,
                                                repo_path.display(),
                                                err
                                            );
                                        }
                                        Err(e) => {
                                            tracing::warn!(
                                                "Error fetching issue #{} body for {}: {}",
                                                num,
                                                repo_path.display(),
                                                e
                                            );
                                        }
                                    }
                                });
                            }
                        }
                    }
                    KeyCode::Char('d') | KeyCode::Char(' ') => {
                        self.dispatch_selected_issue(swarm_idx).await;
                    }
                    KeyCode::Char('g') => {
                        // Open selected issue in browser via gh issue view --web
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            let issues: Vec<&GitHubIssue> = self
                                .issue_caches
                                .get(&swarm.project_name)
                                .map(|c| self.swarm_view.apply_filters(&c.issues))
                                .unwrap_or_default();
                            if let Some(issue) = self
                                .swarm_view
                                .selected_issue()
                                .and_then(|idx| issues.get(idx))
                            {
                                let num = issue.number;
                                let repo_path = swarm.repo_path.clone();
                                let transport = self.transport.clone();
                                tokio::spawn(async move {
                                    match crate::github::gh_repo_output(
                                        &transport,
                                        &repo_path,
                                        &[
                                            "issue".to_string(),
                                            "view".to_string(),
                                            num.to_string(),
                                            "--web".to_string(),
                                        ],
                                    )
                                    .await
                                    {
                                        Ok(out) if out.status.success() => {}
                                        Ok(out) => {
                                            let err = String::from_utf8_lossy(&out.stderr)
                                                .trim()
                                                .to_string();
                                            tracing::warn!(
                                                "Failed opening selected issue #{} in browser for {}: {}",
                                                num,
                                                repo_path.display(),
                                                err
                                            );
                                        }
                                        Err(e) => {
                                            tracing::warn!(
                                                "Error opening selected issue #{} in browser for {}: {}",
                                                num,
                                                repo_path.display(),
                                                e
                                            );
                                        }
                                    }
                                });
                                self.set_status(format!(
                                    "Opening issue #{} in browser",
                                    issue.number
                                ));
                            }
                        }
                    }
                    KeyCode::Char('u') => {
                        // Release stuck issue: remove "working" label
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            let project_name = swarm.project_name.clone();
                            let issues: Vec<&GitHubIssue> = self
                                .issue_caches
                                .get(&project_name)
                                .map(|c| self.swarm_view.apply_filters(&c.issues))
                                .unwrap_or_default();
                            if let Some(issue) = self
                                .swarm_view
                                .selected_issue()
                                .and_then(|idx| issues.get(idx))
                            {
                                if issue.is_working {
                                    let num = issue.number;
                                    let repo_path = swarm.repo_path.clone();
                                    let transport = self.transport.clone();
                                    // Optimistically update local cache
                                    if let Some(cache) = self.issue_caches.get_mut(&project_name) {
                                        if let Some(i) =
                                            cache.issues.iter_mut().find(|i| i.number == num)
                                        {
                                            i.is_working = false;
                                            i.assigned_worker = None;
                                            i.labels.retain(|l| l != "working");
                                        }
                                    }
                                    tokio::spawn(async move {
                                        match crate::github::gh_repo_output(
                                            &transport,
                                            &repo_path,
                                            &[
                                                "issue".to_string(),
                                                "edit".to_string(),
                                                num.to_string(),
                                                "--remove-label".to_string(),
                                                "working".to_string(),
                                            ],
                                        )
                                        .await
                                        {
                                            Ok(out) if out.status.success() => {}
                                            Ok(out) => {
                                                let err = String::from_utf8_lossy(&out.stderr)
                                                    .trim()
                                                    .to_string();
                                                tracing::warn!(
                                                    "Failed removing `working` label for issue #{} in {}: {}",
                                                    num,
                                                    repo_path.display(),
                                                    err
                                                );
                                            }
                                            Err(e) => {
                                                tracing::warn!(
                                                    "Error removing `working` label for issue #{} in {}: {}",
                                                    num,
                                                    repo_path.display(),
                                                    e
                                                );
                                            }
                                        }
                                    });
                                    self.set_status(format!(
                                        "Released issue #{num} (removed working label)"
                                    ));
                                } else {
                                    self.set_status(format!(
                                        "Issue #{} is not marked as working",
                                        issue.number
                                    ));
                                }
                            }
                        }
                    }
                    KeyCode::Char('/') => {
                        self.swarm_view.search_query = Some(String::new());
                        self.swarm_view.issues_table.select(Some(0));
                    }
                    KeyCode::Char('c') => {
                        self.swarm_view.issue_filter = IssueFilter::All;
                        self.swarm_view.issue_type_filter = None;
                        self.swarm_view.priority_filter = None;
                        self.swarm_view.search_query = None;
                        self.swarm_view.issues_table.select(Some(0));
                        self.set_status("Filters cleared".to_string());
                    }
                    _ => {
                        if let KeyCode::Char(c) = key.code {
                            let issue_num = {
                                let selected = self.swarm_view.selected_issue();
                                self.swarms
                                    .get(swarm_idx)
                                    .and_then(|s| self.issue_caches.get(&s.project_name))
                                    .and_then(|cache| {
                                        let issues = self.swarm_view.apply_filters(&cache.issues);
                                        selected.and_then(|idx| issues.get(idx)).map(|i| i.number)
                                    })
                            };
                            self.try_shortcut("issues", &c.to_string(), swarm_idx, issue_num)
                                .await?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Send a command with the selected issue number to the manager pane.
    /// Only sends if the manager is idle — never interrupts a working session.
    async fn send_issue_command_to_manager(&mut self, swarm_idx: usize, cmd: &str) -> Result<()> {
        let (target, issue_num, manager_idle) = {
            let swarm = match self.swarms.get(swarm_idx) {
                Some(s) => s,
                None => return Ok(()),
            };
            let target = swarm.manager.tmux_target.clone();
            let idle = matches!(
                swarm.manager.status.state,
                crate::model::status::AgentState::Idle
            );
            let issues: Vec<&GitHubIssue> = self
                .issue_caches
                .get(&swarm.project_name)
                .map(|c| self.swarm_view.apply_filters(&c.issues))
                .unwrap_or_default();
            let issue_num = self
                .swarm_view
                .selected_issue()
                .and_then(|idx| issues.get(idx))
                .map(|i| i.number);
            (target, issue_num, idle)
        };

        if !manager_idle {
            self.set_status("Manager is busy — wait until idle".to_string());
            return Ok(());
        }

        if let Some(num) = issue_num {
            let full_cmd = format!("{cmd} {num}");
            tracing::info!("Sending '{full_cmd}' to manager at {target}");
            proxy::send_keys(&self.transport, &target, &full_cmd).await?;
            self.set_status(format!("Sent: {full_cmd}"));
        }
        Ok(())
    }

    async fn create_issue(&mut self, swarm_idx: usize, title: String, labels: String) {
        let repo_path = match self.swarms.get(swarm_idx) {
            Some(swarm) => swarm.repo_path.clone(),
            None => return,
        };

        let args = vec![
            "issue".to_string(),
            "create".to_string(),
            "--title".to_string(),
            title.clone(),
            "--label".to_string(),
            labels,
        ];

        match crate::github::gh_repo_output(&self.transport, &repo_path, &args).await {
            Ok(output) if output.status.success() => {
                self.set_status(format!("Created issue: {title}"));
                self.start_issue_refresh(swarm_idx);
            }
            Ok(output) => {
                let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
                self.set_status(format!("Create issue failed: {err}"));
            }
            Err(e) => {
                self.set_status(format!("Create issue failed: {e}"));
            }
        }
    }

    async fn run_direct_gh_shortcut(
        &mut self,
        swarm_idx: usize,
        label: &str,
        command: &str,
        args: Vec<String>,
    ) -> Result<()> {
        let repo_path = match self.swarms.get(swarm_idx) {
            Some(swarm) => swarm.repo_path.clone(),
            None => return Ok(()),
        };

        match crate::github::gh_repo_output(&self.transport, &repo_path, &args).await {
            Ok(output) => {
                if output.status.success() {
                    self.set_status(format!("[{label}] {command}"));
                    if gh_issue_command_mutates(&args) {
                        self.start_issue_refresh(swarm_idx);
                    }
                } else {
                    let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    tracing::warn!(
                        "Direct gh command failed in {}: `{}` -> {}",
                        repo_path.display(),
                        command,
                        err
                    );
                    self.set_status(format!("[{label}] failed: {err}"));
                }
            }
            Err(e) => {
                tracing::error!(
                    "Direct gh command errored in {}: `{}` -> {}",
                    repo_path.display(),
                    command,
                    e
                );
                self.set_status(format!("[{label}] failed: {e}"));
            }
        }
        Ok(())
    }

    async fn run_direct_gh_command(&mut self, swarm_idx: usize, command: &str) -> Result<bool> {
        let args = match parse_direct_gh_command(command) {
            ParsedGhCommand::Parsed(args) => args,
            ParsedGhCommand::Malformed(err) => {
                tracing::warn!("Malformed direct gh command `{command}`: {err}");
                self.set_status(format!("Invalid gh command: {err}"));
                return Ok(true);
            }
            ParsedGhCommand::NotGh => return Ok(false),
        };

        self.run_direct_gh_shortcut(swarm_idx, "gh", command, args)
            .await?;
        Ok(true)
    }

    async fn gh_agents_ui_output(&self, mut args: Vec<String>) -> Result<std::process::Output> {
        let mut full_args = vec!["-R".to_string(), AGENTS_UI_GH_REPO.to_string()];
        full_args.append(&mut args);
        self.transport
            .output("gh", &full_args, None)
            .await
            .context("Failed to execute gh command for laird/agents-ui")
    }

    async fn find_existing_log_issue(&self, fingerprint: &str) -> Result<Option<u32>> {
        let search = format!("\"[log-fingerprint:{fingerprint}]\" in:body");
        let output = self
            .gh_agents_ui_output(vec![
                "issue".to_string(),
                "list".to_string(),
                "--state".to_string(),
                "all".to_string(),
                "--search".to_string(),
                search,
                "--json".to_string(),
                "number".to_string(),
                "--limit".to_string(),
                "1".to_string(),
            ])
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(anyhow::anyhow!("gh issue list failed: {stderr}"));
        }

        let parsed: serde_json::Value =
            serde_json::from_slice(&output.stdout).context("Failed to parse gh issue list JSON")?;
        Ok(parsed
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|value| value.get("number"))
            .and_then(|n| n.as_u64())
            .map(|n| n as u32))
    }

    async fn create_log_issue(&self, candidate: &LogErrorCandidate) -> Result<()> {
        let title = format!("TUI log error: {}", truncate_chars(&candidate.summary, 90));
        let body = build_log_issue_body(candidate);
        let output = self
            .gh_agents_ui_output(vec![
                "issue".to_string(),
                "create".to_string(),
                "--title".to_string(),
                title.clone(),
                "--body".to_string(),
                body,
            ])
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(anyhow::anyhow!(
                "gh issue create failed for `{title}`: {stderr}"
            ));
        }

        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        tracing::info!(
            "Created log issue for fingerprint {}: {}",
            candidate.fingerprint,
            url
        );
        Ok(())
    }

    async fn append_log_issue_comment(
        &self,
        issue_number: u32,
        candidate: &LogErrorCandidate,
    ) -> Result<()> {
        let body = build_log_issue_comment(candidate);
        let output = self
            .gh_agents_ui_output(vec![
                "issue".to_string(),
                "comment".to_string(),
                issue_number.to_string(),
                "--body".to_string(),
                body,
            ])
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(anyhow::anyhow!(
                "gh issue comment failed for #{}: {}",
                issue_number,
                stderr
            ));
        }

        tracing::info!(
            "Appended log evidence to issue #{} for fingerprint {}",
            issue_number,
            candidate.fingerprint
        );
        Ok(())
    }

    async fn sync_log_errors_to_github(&mut self) -> Result<()> {
        let log_path = agents_tui_log_path();
        let log_contents = std::fs::read_to_string(&log_path)
            .with_context(|| format!("Failed to read {}", log_path.display()))?;
        let candidates = collect_recent_log_error_candidates(&log_contents, 10, 3);

        if candidates.is_empty() {
            self.set_status("No recent ERROR entries found in agents-ui log".to_string());
            return Ok(());
        }

        let mut created = 0;
        let mut commented = 0;
        let mut failed = 0;

        for candidate in candidates {
            match self.find_existing_log_issue(&candidate.fingerprint).await {
                Ok(Some(issue_number)) => match self
                    .append_log_issue_comment(issue_number, &candidate)
                    .await
                {
                    Ok(()) => commented += 1,
                    Err(e) => {
                        failed += 1;
                        tracing::error!(
                            "Failed to append log evidence to #{} for fingerprint {}: {:#}",
                            issue_number,
                            candidate.fingerprint,
                            e
                        );
                    }
                },
                Ok(None) => match self.create_log_issue(&candidate).await {
                    Ok(()) => created += 1,
                    Err(e) => {
                        failed += 1;
                        tracing::error!(
                            "Failed to create log issue for fingerprint {}: {:#}",
                            candidate.fingerprint,
                            e
                        );
                    }
                },
                Err(e) => {
                    failed += 1;
                    tracing::error!(
                        "Failed to look up issue for fingerprint {}: {:#}",
                        candidate.fingerprint,
                        e
                    );
                }
            }
        }

        self.set_status(format!(
            "Log sync to {}: {} created, {} updated, {} failed",
            AGENTS_UI_GH_REPO, created, commented, failed
        ));
        Ok(())
    }

    async fn run_report_log_errors_command(&mut self, command: &str) -> Result<bool> {
        if !is_report_log_errors_command(command) {
            return Ok(false);
        }

        tracing::info!("Running log error report command: {command}");
        self.set_status(format!("Syncing log errors to {}...", AGENTS_UI_GH_REPO));
        if let Err(e) = self.sync_log_errors_to_github().await {
            tracing::error!("Log error sync command failed: {:#}", e);
            self.set_status(format!("Log sync failed: {e}"));
        }
        Ok(true)
    }

    fn worker_dispatch_cmd(&self, agent_type: &AgentType, issue_number: u32) -> Option<String> {
        match agent_type {
            AgentType::Claude => Some(format!("/autocoder:fix {issue_number}")),
            AgentType::Gemini => Some(format!("/fix {issue_number}")),
            AgentType::Codex => Some(format!(
                "Use the autocoder skill to fix issue #{issue_number} in this repository."
            )),
            AgentType::Droid => Some(format!("/fix {issue_number}")),
        }
    }

    fn review_blocked_cmd(&self, agent_type: &AgentType) -> Option<String> {
        match agent_type {
            AgentType::Claude => Some("/autocoder:review-blocked".to_string()),
            AgentType::Gemini => Some("/review-blocked".to_string()),
            AgentType::Codex => Some(
                "Use the autocoder skill to review blocked issues in this repository and recommend the next human actions.".to_string()
            ),
            AgentType::Droid => Some("/review-blocked".to_string()),
        }
    }

    fn monitor_workers_cmd(&self, agent_type: &AgentType) -> Option<String> {
        match agent_type {
            AgentType::Claude => Some("/autocoder:monitor-workers".to_string()),
            AgentType::Gemini => Some("/monitor-workers".to_string()),
            AgentType::Codex => Some(
                "Use the autocoder skill to monitor workers for this repository in this manager session.".to_string()
            ),
            AgentType::Droid => Some("/monitor-workers".to_string()),
        }
    }

    async fn handle_issue_view_key(
        &mut self,
        key: KeyEvent,
        swarm_idx: usize,
        issue_number: u32,
    ) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.issue_view = None;
                self.screen = Screen::RepoView { swarm_idx };
            }
            KeyCode::PageUp => {
                if let Some(ref mut view) = self.issue_view {
                    view.scroll_up(10);
                }
            }
            KeyCode::PageDown => {
                if let Some(ref mut view) = self.issue_view {
                    view.scroll_down(10);
                }
            }
            KeyCode::Char('g') => {
                // Open in browser
                if let Some(swarm) = self.swarms.get(swarm_idx) {
                    let repo_path = swarm.repo_path.clone();
                    let transport = self.transport.clone();
                    tokio::spawn(async move {
                        match transport
                            .output(
                                "gh",
                                &[
                                    "issue".to_string(),
                                    "view".to_string(),
                                    issue_number.to_string(),
                                    "--web".to_string(),
                                ],
                                Some(&repo_path),
                            )
                            .await
                        {
                            Ok(out) if out.status.success() => {}
                            Ok(out) => {
                                let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                                tracing::warn!(
                                    "Failed opening issue #{} in browser for {}: {}",
                                    issue_number,
                                    repo_path.display(),
                                    err
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Error opening issue #{} in browser for {}: {}",
                                    issue_number,
                                    repo_path.display(),
                                    e
                                );
                            }
                        }
                    });
                    self.set_status(format!("Opening issue #{issue_number} in browser"));
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_agent_view_key(
        &mut self,
        key: KeyEvent,
        swarm_idx: usize,
        agent_id: String,
    ) -> Result<()> {
        // Ctrl+] escapes back to repo view (like ssh escape)
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char(']') {
            self.screen = Screen::RepoView { swarm_idx };
            return Ok(());
        }

        let target = self
            .swarms
            .get(swarm_idx)
            .and_then(|s| s.agent(&agent_id))
            .map(|a| a.tmux_target.clone());

        let target = match target {
            Some(t) => t,
            None => {
                self.enter_repo_view(swarm_idx).await;
                return Ok(());
            }
        };

        // Esc is forwarded to the pane (use Alt+Left to go back)
        if key.code == KeyCode::Esc {
            send_raw_key(&self.transport, &target, "Escape").await?;
            return Ok(());
        }

        // Alt+0 → repo view, Alt+m → manager, Alt+1-9 → worker
        if key.modifiers.contains(KeyModifiers::ALT) {
            match key.code {
                KeyCode::Char('0') => {
                    self.enter_repo_view(swarm_idx).await;
                    return Ok(());
                }
                KeyCode::Char('m') => {
                    self.enter_agent_view(swarm_idx, "manager".to_string())
                        .await;
                    return Ok(());
                }
                KeyCode::Char('a') => {
                    // Add a new worker to this swarm
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        let swarm_clone = swarm.clone();
                        self.set_status("Adding worker...".to_string());
                        match self.adapter.add_worker(&swarm_clone).await {
                            Ok(worker) => {
                                let id = worker.role.clone();
                                if let Some(swarm) = self.swarms.get_mut(swarm_idx) {
                                    swarm.workers.push(worker);
                                }
                                self.start_all_pane_watchers();
                                self.status_message =
                                    Some(format!("Added {id} (starting worker loop)"));
                            }
                            Err(e) => {
                                self.status_message = Some(format!("Failed to add worker: {e}"));
                            }
                        }
                    }
                }
                KeyCode::Char('f') => {
                    // Start the selected worker loop (only if idle)
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        if let Some(worker_idx) = self.repo_view.selected_worker() {
                            if let Some(worker) = swarm.workers.get(worker_idx) {
                                if !matches!(
                                    worker.status.state,
                                    crate::model::status::AgentState::Idle
                                ) {
                                    self.status_message = Some(format!(
                                        "Worker {} is busy — wait until idle",
                                        worker.role
                                    ));
                                } else {
                                    let target = worker.tmux_target.clone();
                                    let id = worker.role.clone();
                                    tracing::info!("Starting worker loop for {id} at {target}");
                                    if let Err(e) = self.adapter.start_worker_loop(&target).await {
                                        tracing::error!(
                                            "Failed to start worker loop for {id}: {e}"
                                        );
                                        self.status_message =
                                            Some(format!("Failed to start {id}: {e}"));
                                    } else {
                                        self.status_message =
                                            Some(format!("Started worker loop for {id}"));
                                    }
                                }
                            }
                        }
                    }
                }
                KeyCode::Char('i') => {
                    // View the issue that the selected worker is working on
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        if let Some(worker_idx) = self.repo_view.selected_worker() {
                            if let Some(worker) = swarm.workers.get(worker_idx) {
                                if let crate::model::status::AgentState::Working {
                                    issue: Some(n),
                                } = &worker.status.state
                                {
                                    self.open_issue_detail(*n, swarm_idx).await;
                                } else {
                                    self.status_message =
                                        Some("Selected worker has no active issue".to_string());
                                }
                            }
                        }
                    }
                }
                KeyCode::Char('d') => {
                    // Shut down the selected worker's session
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        if let Some(worker_idx) = self.repo_view.selected_worker() {
                            if let Some(worker) = swarm.workers.get(worker_idx) {
                                let target = worker.tmux_target.clone();
                                let id = worker.role.clone();
                                tracing::info!("Shutting down worker {id} at {target}");
                                if let Err(e) = proxy::kill_pane(&self.transport, &target).await {
                                    tracing::error!("Failed to kill pane for {id}: {e}");
                                }
                                self.status_message = Some(format!("Shutting down {id}..."));
                            }
                        }
                    }
                }
                KeyCode::Char(c @ '1'..='9') => {
                    let worker_idx = (c as usize) - ('1' as usize);
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        if let Some(worker) = swarm.workers.get(worker_idx) {
                            let aid = worker.id.clone();
                            self.enter_agent_view(swarm_idx, aid).await;
                            return Ok(());
                        }
                    }
                }
                KeyCode::Char('R') => {
                    // Restart all idle workers by starting their worker loops
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        let idle_workers: Vec<(String, String)> = swarm
                            .workers
                            .iter()
                            .filter(|w| {
                                matches!(
                                    w.status.state,
                                    crate::model::status::AgentState::Idle
                                        | crate::model::status::AgentState::Unknown(_)
                                )
                            })
                            .map(|w| (w.id.clone(), w.tmux_target.clone()))
                            .collect();

                        let count = idle_workers.len();
                        for (id, target) in idle_workers {
                            tracing::info!("Restarting idle worker {id}");
                            if let Err(e) = self.adapter.start_worker_loop(&target).await {
                                tracing::error!("Failed to restart {id}: {e}");
                            }
                        }
                        self.status_message = Some(format!("Restarted {count} idle worker(s)"));
                    }
                }
                KeyCode::Char('B') => {
                    // Send deploy command to manager session
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        let target = swarm.manager.tmux_target.clone();
                        tracing::info!("Sending deploy command to manager at {target}");
                        if let Err(e) = self.adapter.send_input(&target, "deploy").await {
                            tracing::error!("Failed to send deploy: {e}");
                            self.set_status(format!("Deploy failed: {e}"));
                        } else {
                            self.set_status("Sent deploy to manager".to_string());
                        }
                    }
                }
                KeyCode::Char('D') => {
                    // Stop all workers (send stop to each)
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        let transport = self.transport.clone();
                        let workers: Vec<(String, String)> = swarm
                            .workers
                            .iter()
                            .map(|w| (w.id.clone(), w.tmux_target.clone()))
                            .collect();

                        let count = workers.len();
                        for (id, target) in workers {
                            tracing::info!("Stopping worker {id}");
                            if let Err(e) = proxy::kill_pane(&transport, &target).await {
                                tracing::error!("Failed to stop {id}: {e}");
                            }
                        }
                        self.status_message = Some(format!("Stopping all {count} worker(s)..."));
                    }
                }
                _ => {}
            }
        }

        // PageUp/PageDown/Home/End scroll the view without sending to pane
        match key.code {
            KeyCode::PageUp => {
                self.agent_view.page_up();
                return Ok(());
            }
            KeyCode::PageDown => {
                self.agent_view.page_down();
                return Ok(());
            }
            KeyCode::Home => {
                self.agent_view.scroll_to_top();
                return Ok(());
            }
            KeyCode::End => {
                self.agent_view.scroll_to_bottom();
                return Ok(());
            }
            KeyCode::Enter => {
                if !self.agent_view.input.is_empty() {
                    let input = self.agent_view.input.drain();
                    if !self.run_report_log_errors_command(&input).await?
                        && !self.run_direct_gh_command(swarm_idx, &input).await?
                    {
                        proxy::send_keys(&self.transport, &target, &input).await?;
                        self.agent_view.scroll_to_bottom();
                    }
                }
                return Ok(());
            }
            KeyCode::Delete => {
                self.agent_view.input.delete();
                return Ok(());
            }
            KeyCode::Backspace => {
                self.agent_view.input.backspace();
                return Ok(());
            }
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.agent_view.input.insert_char(c);
                return Ok(());
            }
            _ => {}
        }

        // Everything else is forwarded directly to the tmux pane.
        let tmux_key = key_event_to_tmux(key);
        if let Some(tmux_key) = tmux_key {
            send_raw_key(&self.transport, &target, &tmux_key).await?;
            self.agent_view.scroll_to_bottom();
        }

        Ok(())
    }

    /// Start pane watchers for all agents in all swarms.
    /// Jump to the next agent session that is waiting for user input.
    fn jump_to_next_blocked(&mut self, swarm_idx: usize) {
        let blocked_count = self
            .swarms
            .get(swarm_idx)
            .and_then(|s| self.issue_caches.get(&s.project_name))
            .map(|c| c.issues.iter().filter(|i| i.is_blocked()).count())
            .unwrap_or(0);

        if blocked_count == 0 {
            self.set_status("No blocked issues".to_string());
            return;
        }

        // If already viewing blocked issues, cycle to next; otherwise go to first
        let next_idx = if self.swarm_focus == SwarmPanel::Issues
            && self.swarm_view.issue_filter == IssueFilter::Blocked
        {
            let current = self.swarm_view.issues_table.selected().unwrap_or(0);
            (current + 1) % blocked_count
        } else {
            0
        };

        self.swarm_view.issue_filter = IssueFilter::Blocked;
        self.swarm_view.issues_table.select(Some(next_idx));
        self.swarm_focus = SwarmPanel::Issues;
    }

    async fn jump_to_next_waiting(&mut self, swarm_idx: usize) {
        // Get the current agent ID if we're in an agent view
        let current_id = match &self.screen {
            Screen::AgentView { agent_id, .. } => Some(agent_id.clone()),
            _ => None,
        };

        if let Some(swarm) = self.swarms.get(swarm_idx) {
            if let Some(agent) = swarm.next_waiting_agent(current_id.as_deref()) {
                let agent_id = agent.id.clone();
                self.enter_agent_view(swarm_idx, agent_id).await;
            } else {
                self.set_status("No sessions waiting for input".to_string());
            }
        }
    }

    /// Try to execute a configured shortcut for the given panel and key.
    async fn try_shortcut(
        &mut self,
        panel: &str,
        key: &str,
        swarm_idx: usize,
        issue: Option<u32>,
    ) -> Result<()> {
        use crate::config::shortcuts::ShortcutsConfig;

        let shortcut = self.shortcuts.for_panel(panel).get(key).cloned();
        if let Some(shortcut) = shortcut {
            let project = self.swarms.get(swarm_idx).map(|s| s.project_name.as_str());
            let worker_target = self
                .repo_view
                .selected_worker()
                .and_then(|idx| self.swarms.get(swarm_idx).and_then(|s| s.workers.get(idx)))
                .map(|w| w.tmux_target.clone());

            let cmd = ShortcutsConfig::expand_command(
                &shortcut.command,
                issue,
                worker_target.as_deref(),
                project,
            );

            if !shortcut.raw && self.run_report_log_errors_command(&cmd).await? {
                return Ok(());
            }

            if let Some(swarm) = self.swarms.get(swarm_idx) {
                if !shortcut.raw {
                    match parse_direct_gh_command(&cmd) {
                        ParsedGhCommand::Parsed(args) => {
                            self.run_direct_gh_shortcut(swarm_idx, &shortcut.label, &cmd, args)
                                .await?;
                            return Ok(());
                        }
                        ParsedGhCommand::Malformed(err) => {
                            tracing::warn!(
                                "Malformed gh shortcut `{}` command `{}`: {}",
                                shortcut.label,
                                cmd,
                                err
                            );
                            self.set_status(format!(
                                "[{}] invalid gh command: {err}",
                                shortcut.label
                            ));
                            return Ok(());
                        }
                        ParsedGhCommand::NotGh => {}
                    }
                }

                let target = if shortcut.target == "worker" {
                    worker_target
                        .clone()
                        .unwrap_or_else(|| swarm.manager.tmux_target.clone())
                } else {
                    swarm.manager.tmux_target.clone()
                };

                if shortcut.raw {
                    self.adapter.send_raw_key(&target, &cmd, false).await?;
                } else {
                    self.adapter.send_input(&target, &cmd).await?;
                }
                self.set_status(format!("[{}] {}", shortcut.label, cmd));
            }
        }
        Ok(())
    }

    async fn handle_issue_detail_key(&mut self, key: KeyEvent, swarm_idx: usize) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Backspace => {
                self.issue_detail_view = None;
                self.screen = Screen::RepoView { swarm_idx };
            }
            KeyCode::Char('q') => {
                self.running = false;
            }
            KeyCode::Char('g') => {
                // Open issue in browser
                if let Some(ref view) = self.issue_detail_view {
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        match crate::github::gh_repo_output(
                            &self.transport,
                            &swarm.repo_path,
                            &[
                                "issue".to_string(),
                                "view".to_string(),
                                "--web".to_string(),
                                view.issue_number.to_string(),
                            ],
                        )
                        .await
                        {
                            Ok(out) if out.status.success() => {
                                self.set_status(format!(
                                    "Opening issue #{} in browser",
                                    view.issue_number
                                ));
                            }
                            Ok(out) => {
                                let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                                tracing::warn!(
                                    "Failed opening issue #{} in browser for {}: {}",
                                    view.issue_number,
                                    swarm.repo_path.display(),
                                    err
                                );
                                self.set_status(format!("Open issue failed: {err}"));
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Error opening issue #{} in browser for {}: {}",
                                    view.issue_number,
                                    swarm.repo_path.display(),
                                    e
                                );
                                self.set_status(format!("Open issue failed: {e}"));
                            }
                        }
                    }
                }
            }
            KeyCode::Char('d') => {
                // Dispatch the currently-viewed issue to an idle worker
                if let Some(issue_num) = self.issue_detail_view.as_ref().map(|v| v.issue_number) {
                    self.dispatch_issue_by_number(swarm_idx, issue_num).await;
                }
            }
            KeyCode::PageUp => {
                if let Some(ref mut view) = self.issue_detail_view {
                    view.scroll_up(10);
                }
            }
            KeyCode::PageDown => {
                if let Some(ref mut view) = self.issue_detail_view {
                    view.scroll_down(10);
                }
            }
            KeyCode::Up => {
                if let Some(ref mut view) = self.issue_detail_view {
                    view.scroll_up(1);
                }
            }
            KeyCode::Down => {
                if let Some(ref mut view) = self.issue_detail_view {
                    view.scroll_down(1);
                }
            }
            KeyCode::Char('n') | KeyCode::Char('p') => {
                // Navigate to next/previous issue in the sorted list
                let current_num = self.issue_detail_view.as_ref().map(|v| v.issue_number);
                if let (Some(current_num), Some(swarm)) = (current_num, self.swarms.get(swarm_idx))
                {
                    let project_name = swarm.project_name.clone();
                    if let Some(cache) = self.issue_caches.get(&project_name) {
                        let sorted_nums: Vec<u32> = IssueListView::sorted_open(&cache.issues)
                            .into_iter()
                            .map(|i| i.number)
                            .collect();
                        if let Some(pos) = sorted_nums.iter().position(|&n| n == current_num) {
                            let next_pos = if key.code == KeyCode::Char('n') {
                                (pos + 1) % sorted_nums.len()
                            } else {
                                if pos == 0 {
                                    sorted_nums.len() - 1
                                } else {
                                    pos - 1
                                }
                            };
                            if let Some(&next_num) = sorted_nums.get(next_pos) {
                                self.issue_list_view.table_state.select(Some(next_pos));
                                self.open_issue_detail(next_num, swarm_idx).await;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_issue_list_key(&mut self, key: KeyEvent, swarm_idx: usize) -> Result<()> {
        let project_name = self.swarms.get(swarm_idx).map(|s| s.project_name.clone());
        let issue_count = project_name
            .as_deref()
            .and_then(|p| self.issue_caches.get(p))
            .map(|c| IssueListView::sorted_open(&c.issues).len())
            .unwrap_or(0);

        match key.code {
            KeyCode::Esc | KeyCode::Backspace => {
                self.enter_repo_view(swarm_idx).await;
            }
            KeyCode::Char('q') => {
                self.running = false;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.issue_list_view.next(issue_count);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.issue_list_view.previous(issue_count);
            }
            KeyCode::Enter => {
                // Open issue detail view for selected issue
                if let Some(pname) = project_name {
                    if let Some(cache) = self.issue_caches.get(&pname) {
                        let sorted = IssueListView::sorted_open(&cache.issues);
                        if let Some(issue) =
                            self.issue_list_view.selected().and_then(|i| sorted.get(i))
                        {
                            let num = issue.number;
                            self.open_issue_detail(num, swarm_idx).await;
                        }
                    }
                }
            }
            KeyCode::Char(' ') | KeyCode::Char('d') => {
                // Dispatch selected issue to an idle worker
                self.dispatch_issue_from_list(swarm_idx).await;
            }
            KeyCode::Char('r') => {
                // Refresh issues
                self.start_issue_refresh(swarm_idx);
                self.status_message = Some("Refreshing issues...".to_string());
            }
            _ => {
                // Fall through to config-driven shortcuts (e.g. x=fix, b=brainstorm)
                if let KeyCode::Char(c) = key.code {
                    let issue_num = self.swarms.get(swarm_idx)
                        .and_then(|s| self.issue_caches.get(&s.project_name))
                        .and_then(|cache| {
                            let sorted = IssueListView::sorted_open(&cache.issues);
                            self.issue_list_view.selected().and_then(|i| sorted.get(i)).map(|iss| iss.number)
                        });
                    self.try_shortcut("issues", &c.to_string(), swarm_idx, issue_num).await?;
                }
            }
        }
        Ok(())
    }

    async fn refresh_repo_list(&mut self) {
        self.set_status("Refreshing...".to_string());
        if let Ok(swarms) = self.adapter.discover(&self.agents_dir).await {
            self.swarms = swarms;
            self.refresh_statuses();
            self.start_all_pane_watchers();
            self.set_status(format!("Found {} swarm(s)", self.swarms.len()));
        }
    }

    async fn dispatch_issue_from_list(&mut self, swarm_idx: usize) {
        let Some(swarm) = self.swarms.get(swarm_idx) else {
            return;
        };
        let project_name = swarm.project_name.clone();
        let agent_type = swarm.agent_type.clone();

        let issues = self
            .issue_caches
            .get(&project_name)
            .map(|c| {
                IssueListView::sorted_open(&c.issues)
                    .into_iter()
                    .map(|i| i.number)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let Some(issue_num) = self
            .issue_list_view
            .selected()
            .and_then(|idx| issues.get(idx).copied())
        else {
            self.status_message = Some("No issue selected".to_string());
            return;
        };

        let idle_worker = self.swarms[swarm_idx]
            .workers
            .iter()
            .enumerate()
            .find(|(_, w)| {
                !w.is_manager
                    && w.dispatched_issue.is_none()
                    && matches!(w.status.state, crate::model::status::AgentState::Idle)
            })
            .map(|(idx, w)| (idx, w.tmux_target.clone(), w.role.clone()));

        let Some((worker_idx, target, role)) = idle_worker else {
            self.status_message = Some("No idle workers available".to_string());
            return;
        };

        let Some(cmd) = self.worker_dispatch_cmd(&agent_type, issue_num) else {
            self.status_message = Some(format!("No dispatch command configured for {agent_type}"));
            return;
        };

        tracing::info!("IssueList dispatch: #{issue_num} \u{2192} {role} via {target}");
        if let Ok(()) = crate::tmux::proxy::send_keys_no_enter(&self.transport, &target, &cmd).await
        {
            tokio::time::sleep(Duration::from_millis(200)).await;
            if let Err(e) =
                crate::tmux::proxy::send_keys_no_enter(&self.transport, &target, "Enter").await
            {
                tracing::warn!(
                    "Failed sending Enter after dispatching issue #{} to {}: {}",
                    issue_num,
                    role,
                    e
                );
            }
            self.swarms[swarm_idx].workers[worker_idx].dispatched_issue = Some(issue_num);
            self.swarms[swarm_idx].workers[worker_idx].status.state =
                crate::model::status::AgentState::Working {
                    issue: Some(issue_num),
                };
            self.status_message = Some(format!("Dispatched #{issue_num} \u{2192} {role}"));
        } else {
            self.status_message = Some(format!("Failed to dispatch #{issue_num}"));
        }
    }

    async fn handle_feedback_key(&mut self, key: crossterm::event::KeyEvent) -> anyhow::Result<()> {
        use crate::ui::feedback_dialog::{FeedbackField, FeedbackType};
        use crossterm::event::KeyCode;

        let state = match self.feedback_state.as_mut() {
            Some(s) => s,
            None => return Ok(()),
        };

        match &state.field {
            FeedbackField::FeedbackType => match key.code {
                KeyCode::Left | KeyCode::Char('h') => {
                    if state.type_index > 0 {
                        state.type_index -= 1;
                    }
                    let types = FeedbackType::all();
                    state.feedback_type = types[state.type_index];
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    let types = FeedbackType::all();
                    if state.type_index + 1 < types.len() {
                        state.type_index += 1;
                        state.feedback_type = types[state.type_index];
                    }
                }
                KeyCode::Enter => {
                    state.field = FeedbackField::Title;
                }
                KeyCode::Esc => {
                    self.feedback_state = None;
                }
                _ => {}
            },
            FeedbackField::Title => match key.code {
                KeyCode::Char(c) => {
                    state.title.push(c);
                }
                KeyCode::Backspace => {
                    state.title.pop();
                }
                KeyCode::Enter => {
                    if !state.title.is_empty() {
                        state.field = FeedbackField::Body;
                    }
                }
                KeyCode::Esc => {
                    state.field = FeedbackField::FeedbackType;
                }
                _ => {}
            },
            FeedbackField::Body => match key.code {
                KeyCode::Char(c)
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL)
                        && c == 'j' =>
                {
                    // Ctrl+Enter (represented as Ctrl+j in some terminals) = submit
                    state.field = FeedbackField::Submitting;
                }
                KeyCode::Enter
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    state.field = FeedbackField::Submitting;
                }
                KeyCode::Char(c) => {
                    state.body.push(c);
                }
                KeyCode::Backspace => {
                    state.body.pop();
                }
                KeyCode::Esc => {
                    state.field = FeedbackField::Title;
                }
                _ => {}
            },
            FeedbackField::Submitting => {
                // Transition to submitting — fire the gh issue create command
            }
            FeedbackField::Done(_) => match key.code {
                KeyCode::Enter | KeyCode::Esc => {
                    self.feedback_state = None;
                }
                _ => {}
            },
        }

        // If we just transitioned to Submitting, fire the request
        if matches!(
            self.feedback_state.as_ref().map(|s| &s.field),
            Some(FeedbackField::Submitting)
        ) {
            let state = self.feedback_state.as_ref().unwrap();
            let title = state.title.clone();
            let body = state.body.clone();
            let label = state.feedback_type.github_label().to_string();
            let repo_path = state.repo_path.clone();

            let result = crate::github::gh_repo_output(
                &self.transport,
                &repo_path,
                &[
                    "issue".to_string(),
                    "create".to_string(),
                    "--title".to_string(),
                    title.clone(),
                    "--body".to_string(),
                    body.clone(),
                    "--label".to_string(),
                    label,
                ],
            )
            .await;

            if let Some(state) = self.feedback_state.as_mut() {
                state.field = match result {
                    Ok(out) if out.status.success() => {
                        let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        FeedbackField::Done(format!("Filed! {url}"))
                    }
                    Ok(out) => {
                        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                        FeedbackField::Done(format!("Error: {err}"))
                    }
                    Err(e) => FeedbackField::Done(format!("Error: {e}")),
                };
            }
        }

        Ok(())
    }

    fn start_issue_refresh(&self, swarm_idx: usize) {
        if let Some(swarm) = self.swarms.get(swarm_idx) {
            let repo_path = swarm.repo_path.clone();
            let transport = self.transport.clone();
            let tx = self.events.tx();
            tokio::spawn(async move {
                match crate::github::fetch_issues(&transport, &repo_path).await {
                    Ok(issues) => {
                        if let Err(e) = tx.send(Event::IssuesRefreshed { swarm_idx, issues }) {
                            tracing::debug!(
                                "Failed to send issue refresh event for {}: {}",
                                repo_path.display(),
                                e
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch issues: {e}");
                    }
                }
            });
        }
    }

    /// Open an issue detail view by fetching issue data from GitHub.
    async fn open_issue_detail(&mut self, issue_number: u32, swarm_idx: usize) {
        let output = if let Some(swarm) = self.swarms.get(swarm_idx) {
            crate::github::gh_repo_output(
                &self.transport,
                &swarm.repo_path,
                &[
                    "issue".to_string(),
                    "view".to_string(),
                    issue_number.to_string(),
                    "--json".to_string(),
                    "number,title,body,labels,state,comments,assignees,createdAt,updatedAt"
                        .to_string(),
                ],
            )
            .await
        } else {
            return;
        };

        match output {
            Ok(out) if out.status.success() => {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                    let title = json["title"].as_str().unwrap_or("").to_string();
                    let body = json["body"].as_str().unwrap_or("").to_string();
                    let state = json["state"].as_str().unwrap_or("OPEN").to_string();
                    let labels: Vec<String> = json["labels"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|l| l["name"].as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();
                    let comments: Vec<(String, String)> = json["comments"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .rev()
                                .take(10)
                                .rev()
                                .filter_map(|c| {
                                    let author = c["author"]["login"].as_str()?;
                                    let body = c["body"].as_str()?;
                                    Some((author.to_string(), body.to_string()))
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    let assignees: Vec<String> = json["assignees"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|a| a["login"].as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();
                    let age_from_rfc3339 = |field: &str| -> String {
                        json[field]
                            .as_str()
                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                            .map(|dt| {
                                let secs =
                                    (chrono::Utc::now() - dt.to_utc()).num_seconds().max(0) as u64;
                                if secs < 3600 {
                                    format!("{}m ", secs / 60)
                                } else if secs < 86400 {
                                    format!("{}h ", secs / 3600)
                                } else if secs < 7 * 86400 {
                                    format!("{}d ", secs / 86400)
                                } else {
                                    format!("{}w ", secs / (7 * 86400))
                                }
                            })
                            .unwrap_or_default()
                    };
                    let created_at_age = age_from_rfc3339("createdAt");
                    let updated_at_age = age_from_rfc3339("updatedAt");

                    self.issue_detail_view = Some(IssueDetailView::new(
                        issue_number,
                        title,
                        body,
                        labels,
                        state,
                        comments,
                        assignees,
                        created_at_age,
                        updated_at_age,
                    ));
                    self.screen = Screen::IssueDetail { swarm_idx };
                }
            }
            _ => {
                self.status_message = Some(format!("Failed to fetch issue #{issue_number}"));
            }
        }
    }

    fn start_all_pane_watchers(&mut self) {
        // Cancel existing watchers
        for handle in self.pane_watchers.drain(..) {
            handle.abort();
        }

        let tx = self.events.tx();

        for swarm in &self.swarms {
            // Agent IDs are globally unique (e.g., "nextgen-CDD/manager")
            let handle = proxy::spawn_pane_watcher(
                self.transport.clone(),
                swarm.manager.tmux_target.clone(),
                swarm.manager.id.clone(),
                tx.clone(),
                Duration::from_millis(500),
            );
            self.pane_watchers.push(handle);

            for worker in &swarm.workers {
                let handle = proxy::spawn_pane_watcher(
                    self.transport.clone(),
                    worker.tmux_target.clone(),
                    worker.id.clone(),
                    tx.clone(),
                    Duration::from_millis(500),
                );
                self.pane_watchers.push(handle);
            }
        }
    }

    /// Dispatch the currently selected issue (in the Issues panel) to an idle worker.
    async fn dispatch_selected_issue(&mut self, swarm_idx: usize) {
        let Some(swarm) = self.swarms.get(swarm_idx) else {
            return;
        };
        let project_name = swarm.project_name.clone();
        let agent_type = swarm.agent_type.clone();

        // Get the selected issue number
        let issues: Vec<u32> = self
            .issue_caches
            .get(&project_name)
            .map(|c| {
                self.swarm_view
                    .apply_filters(&c.issues)
                    .into_iter()
                    .map(|i| i.number)
                    .collect()
            })
            .unwrap_or_default();
        let Some(issue_num) = self
            .swarm_view
            .selected_issue()
            .and_then(|idx| issues.get(idx).copied())
        else {
            self.set_status("No issue selected".to_string());
            return;
        };

        // Find an idle worker
        let idle_worker = self.swarms[swarm_idx]
            .workers
            .iter()
            .enumerate()
            .find(|(_, w)| {
                !w.is_manager
                    && w.dispatched_issue.is_none()
                    && matches!(w.status.state, crate::model::status::AgentState::Idle)
            })
            .map(|(idx, w)| (idx, w.tmux_target.clone(), w.role.clone()));

        let Some((worker_idx, target, role)) = idle_worker else {
            self.set_status("No idle workers available".to_string());
            return;
        };

        let Some(cmd) = self.worker_dispatch_cmd(&agent_type, issue_num) else {
            self.set_status(format!("No dispatch command configured for {agent_type}"));
            return;
        };

        tracing::info!("Manual dispatch: #{issue_num} → {role} via {target}");
        if let Ok(()) = crate::tmux::proxy::send_keys_no_enter(&self.transport, &target, &cmd).await
        {
            tokio::time::sleep(Duration::from_millis(200)).await;
            if let Err(e) =
                crate::tmux::proxy::send_keys_no_enter(&self.transport, &target, "Enter").await
            {
                tracing::warn!(
                    "Failed sending Enter after dispatching issue #{} to {}: {}",
                    issue_num,
                    role,
                    e
                );
            }
            self.swarms[swarm_idx].workers[worker_idx].dispatched_issue = Some(issue_num);
            self.swarms[swarm_idx].workers[worker_idx].status.state =
                crate::model::status::AgentState::Working {
                    issue: Some(issue_num),
                };
            self.set_status(format!("Dispatched #{issue_num} → {role}"));
        } else {
            self.set_status(format!("Failed to dispatch #{issue_num}"));
        }
    }

    /// Dispatch a specific issue number to an idle worker (used from Issue Detail view).
    async fn dispatch_issue_by_number(&mut self, swarm_idx: usize, issue_num: u32) {
        let Some(swarm) = self.swarms.get(swarm_idx) else {
            return;
        };
        let agent_type = swarm.agent_type.clone();

        let idle_worker = self.swarms[swarm_idx]
            .workers
            .iter()
            .enumerate()
            .find(|(_, w)| {
                !w.is_manager
                    && w.dispatched_issue.is_none()
                    && matches!(w.status.state, crate::model::status::AgentState::Idle)
            })
            .map(|(idx, w)| (idx, w.tmux_target.clone(), w.role.clone()));

        let Some((worker_idx, target, role)) = idle_worker else {
            self.set_status("No idle workers available".to_string());
            return;
        };

        let Some(cmd) = self.worker_dispatch_cmd(&agent_type, issue_num) else {
            self.set_status(format!("No dispatch command configured for {agent_type}"));
            return;
        };

        if let Ok(()) = crate::tmux::proxy::send_keys_no_enter(&self.transport, &target, &cmd).await
        {
            tokio::time::sleep(Duration::from_millis(200)).await;
            if let Err(e) =
                crate::tmux::proxy::send_keys_no_enter(&self.transport, &target, "Enter").await
            {
                tracing::warn!(
                    "Failed sending Enter after dispatching issue #{} to {}: {}",
                    issue_num,
                    role,
                    e
                );
            }
            self.swarms[swarm_idx].workers[worker_idx].dispatched_issue = Some(issue_num);
            self.swarms[swarm_idx].workers[worker_idx].status.state =
                crate::model::status::AgentState::Working {
                    issue: Some(issue_num),
                };
            self.set_status(format!("Dispatched #{issue_num} → {role}"));
        } else {
            self.set_status(format!("Failed to dispatch #{issue_num}"));
        }
    }

    /// Start issue fetchers for all swarms.
    /// Check for idle workers and dispatch the next available issue to them.
    async fn dispatch_idle_workers(&mut self) {
        for si in 0..self.swarms.len() {
            if self.swarms[si].stopped {
                continue;
            }
            let project_name = self.swarms[si].project_name.clone();
            let agent_type = self.swarms[si].agent_type.clone();

            // Get dispatched issue numbers to avoid double-dispatch
            let already_dispatched: Vec<u32> = self.swarms[si]
                .workers
                .iter()
                .filter_map(|w| w.dispatched_issue)
                .collect();

            // Get issues being worked (from GitHub labels)
            let being_worked: Vec<u32> = self
                .issue_caches
                .get(&project_name)
                .map(|c| {
                    c.issues
                        .iter()
                        .filter(|i| i.is_being_worked())
                        .map(|i| i.number)
                        .collect()
                })
                .unwrap_or_default();

            // Find next dispatchable issue: open, not blocked, not being worked, not already dispatched
            let next_issue = self
                .issue_caches
                .get(&project_name)
                .and_then(|c| {
                    c.issues
                        .iter()
                        .filter(|i| {
                            i.state == crate::model::issue::IssueState::Open
                                && !i.is_blocked()
                                && !i.is_being_worked()
                                && !already_dispatched.contains(&i.number)
                                && !being_worked.contains(&i.number)
                        })
                        // Sort by priority (P0 first)
                        .min_by_key(|i| i.priority_num().unwrap_or(99))
                })
                .map(|i| i.number);

            if let Some(issue_num) = next_issue {
                // Find an idle worker to dispatch to
                let idle_worker = self.swarms[si]
                    .workers
                    .iter()
                    .enumerate()
                    .find(|(_, w)| {
                        !w.is_manager
                            && w.dispatched_issue.is_none()
                            && matches!(w.status.state, crate::model::status::AgentState::Idle)
                    })
                    .map(|(idx, w)| (idx, w.tmux_target.clone()));

                if let Some((worker_idx, target)) = idle_worker {
                    let Some(cmd) = self.worker_dispatch_cmd(&agent_type, issue_num) else {
                        self.set_status(format!(
                            "No worker dispatch command configured for {}",
                            agent_type
                        ));
                        continue;
                    };
                    tracing::info!(
                        "Dispatching #{issue_num} to {} via {target}",
                        self.swarms[si].workers[worker_idx].role
                    );

                    // Send command, then Enter separately
                    if let Ok(()) =
                        crate::tmux::proxy::send_keys_no_enter(&self.transport, &target, &cmd).await
                    {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        if let Err(e) = crate::tmux::proxy::send_keys_no_enter(
                            &self.transport,
                            &target,
                            "Enter",
                        )
                        .await
                        {
                            tracing::warn!(
                                "Failed sending Enter after auto-dispatching issue #{} to {}: {}",
                                issue_num,
                                self.swarms[si].workers[worker_idx].role,
                                e
                            );
                        }

                        // Track the dispatch
                        self.swarms[si].workers[worker_idx].dispatched_issue = Some(issue_num);
                        self.swarms[si].workers[worker_idx].status.state =
                            crate::model::status::AgentState::Working {
                                issue: Some(issue_num),
                            };
                        self.set_status(format!(
                            "Dispatched #{issue_num} → {}",
                            self.swarms[si].workers[worker_idx].role
                        ));
                    }
                }
            }
        }
    }

    /// Refresh agent statuses from status files and generate banners on state changes.
    /// Validate and heal all worker infrastructure across all swarms.
    async fn heal_all_workers(&mut self) {
        let mut any_repairs = false;
        let mut all_repairs = Vec::new();

        for i in 0..self.swarms.len() {
            if self.swarms[i].stopped {
                continue;
            }
            if self
                .intentionally_stopped
                .contains(&self.swarms[i].project_name)
            {
                continue;
            }
            match self.adapter.heal_workers(&mut self.swarms[i]).await {
                Ok(repairs) => {
                    if !repairs.is_empty() {
                        any_repairs = true;
                        all_repairs.extend(repairs);
                    }
                }
                Err(e) => {
                    tracing::warn!("Worker healing failed: {e}");
                }
            }
        }

        if any_repairs {
            let msg = all_repairs.join("; ");
            tracing::info!("Healed workers: {msg}");
            self.set_status(format!("Healed: {msg}"));
            self.start_all_pane_watchers();
        }
    }

    /// Remove swarms whose tmux sessions have been externally killed.
    /// This prevents heal_workers from respawning intentionally-killed sessions.
    async fn prune_dead_swarms(&mut self) {
        let mut to_remove = Vec::new();
        for (i, swarm) in self.swarms.iter().enumerate() {
            if swarm.tmux_session.is_empty() {
                continue; // Placeholder swarm still launching
            }
            let alive = tokio::process::Command::new("tmux")
                .args(["has-session", "-t", &swarm.tmux_session])
                .output()
                .await
                .map(|o| o.status.success())
                .unwrap_or(false);
            if !alive {
                tracing::info!("Pruning dead swarm: {} (session gone)", swarm.project_name);
                to_remove.push(i);
            }
        }
        let pruned = !to_remove.is_empty();
        for i in to_remove.into_iter().rev() {
            self.swarms.remove(i);
        }
        if pruned {
            self.scan_available_repos();
            if let Screen::RepoView { swarm_idx } = self.screen {
                if swarm_idx >= self.swarms.len() {
                    self.screen = Screen::ReposList;
                }
            }
        }
    }

    /// Re-launch any agents that have dropped back to a shell prompt (e.g. after a self-update).
    async fn revive_dropped_agents(&mut self) {
        let intentionally_stopped = self.intentionally_stopped.clone();
        let swarms: Vec<_> = self
            .swarms
            .iter()
            .filter(|s| {
                !s.stopped
                    && !s.manager.tmux_target.is_empty()
                    && !intentionally_stopped.contains(&s.project_name)
            })
            .cloned()
            .collect();
        for swarm in swarms {
            if let Err(e) = self.adapter.revive_agents(&swarm).await {
                tracing::warn!("revive_agents failed for {}: {e}", swarm.project_name);
            }
        }
    }

    /// Send contextual commands to the manager session when appropriate.
    async fn manage_manager_sessions(&mut self) {
        use crate::ui::swarm_view::agent_needs_input;

        for si in 0..self.swarms.len() {
            let swarm = &self.swarms[si];
            if swarm.stopped {
                continue;
            }
            if matches!(swarm.agent_type, AgentType::Codex) {
                continue;
            }
            let manager_target = swarm.manager.tmux_target.clone();
            if manager_target.is_empty() {
                continue; // Placeholder swarm, not ready yet
            }

            // Don't send commands if manager is busy or waiting for input
            let manager_idle = matches!(
                swarm.manager.status.state,
                crate::model::status::AgentState::Idle
            );
            let manager_waiting = agent_needs_input(&swarm.manager.pane_content);
            if !manager_idle || manager_waiting {
                continue;
            }

            let project = &swarm.project_name;
            let cache = self.issue_caches.get(project);

            // Count idle workers
            let idle_workers = swarm
                .workers
                .iter()
                .filter(|w| {
                    w.dispatched_issue.is_none()
                        && matches!(w.status.state, crate::model::status::AgentState::Idle)
                })
                .count();

            // Count available (unblocked, not being worked) issues
            let available_issues = cache
                .map(|c| {
                    c.issues
                        .iter()
                        .filter(|i| {
                            i.state == crate::model::issue::IssueState::Open
                                && !i.is_blocked()
                                && !i.is_being_worked()
                        })
                        .count()
                })
                .unwrap_or(0);

            // Count blocked issues
            let blocked_issues = cache
                .map(|c| c.issues.iter().filter(|i| i.is_blocked()).count())
                .unwrap_or(0);

            if idle_workers > 0 && available_issues > 0 {
                // Workers are idle and there's work — run monitor-workers to dispatch
                if let Some(monitor_cmd) = self.monitor_workers_cmd(&swarm.agent_type) {
                    tracing::info!(
                        "Manager idle with {idle_workers} idle workers and {available_issues} available issues — sending {monitor_cmd}"
                    );
                    match crate::tmux::proxy::send_keys_no_enter(
                        &self.transport,
                        &manager_target,
                        &monitor_cmd,
                    )
                    .await
                    {
                        Ok(()) => {
                            tokio::time::sleep(Duration::from_millis(200)).await;
                            match crate::tmux::proxy::send_keys_no_enter(
                                &self.transport,
                                &manager_target,
                                "Enter",
                            )
                            .await
                            {
                                Ok(()) => {
                                    self.set_status(format!("Sent {monitor_cmd} to manager"));
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "Failed sending Enter after monitor command to {}: {}",
                                        manager_target,
                                        e
                                    );
                                    self.set_status(format!("Failed sending {monitor_cmd}: {e}"));
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed sending monitor command to {}: {}",
                                manager_target,
                                e
                            );
                            self.set_status(format!("Failed sending {monitor_cmd}: {e}"));
                        }
                    }
                }
            } else if available_issues == 0 && blocked_issues > 0 && idle_workers > 0 {
                // No available work but blocked issues exist — review them
                if let Some(review_cmd) = self.review_blocked_cmd(&swarm.agent_type) {
                    tracing::info!(
                        "No available issues, {blocked_issues} blocked — sending {review_cmd}"
                    );
                    match crate::tmux::proxy::send_keys_no_enter(
                        &self.transport,
                        &manager_target,
                        &review_cmd,
                    )
                    .await
                    {
                        Ok(()) => {
                            tokio::time::sleep(Duration::from_millis(200)).await;
                            match crate::tmux::proxy::send_keys_no_enter(
                                &self.transport,
                                &manager_target,
                                "Enter",
                            )
                            .await
                            {
                                Ok(()) => {
                                    self.set_status(format!("Sent {review_cmd} to manager"));
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "Failed sending Enter after review-blocked command to {}: {}",
                                        manager_target,
                                        e
                                    );
                                    self.set_status(format!("Failed sending {review_cmd}: {e}"));
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed sending review-blocked command to {}: {}",
                                manager_target,
                                e
                            );
                            self.set_status(format!("Failed sending {review_cmd}: {e}"));
                        }
                    }
                }
            }
        }
    }
    fn start_all_issue_watchers(&mut self) {
        for handle in self.issue_watchers.drain(..) {
            handle.abort();
        }
        let tx = self.events.tx();
        for swarm in &self.swarms {
            let handle = crate::github::spawn_issue_fetcher(
                self.transport.clone(),
                swarm.repo_path.clone(),
                swarm.project_name.clone(),
                tx.clone(),
                Duration::from_secs(60),
            );
            self.issue_watchers.push(handle);
        }
    }

    /// Refresh agent statuses from status files and pane content.
    /// Check if any worker is idle and auto-dispatch /monitor-workers to the manager.
    /// Debounced to at most once every 3 minutes.
    async fn check_auto_dispatch(&mut self) {
        use crate::model::status::AgentState;

        const DEBOUNCE_SECS: u64 = 180; // 3 minutes

        // Check debounce
        if let Some(last) = self.auto_dispatch_last {
            if last.elapsed() < std::time::Duration::from_secs(DEBOUNCE_SECS) {
                return;
            }
        }

        for swarm in &self.swarms {
            // Check if any worker is idle
            let has_idle_worker = swarm
                .workers
                .iter()
                .any(|w| matches!(w.status.state, AgentState::Idle));

            if !has_idle_worker {
                continue;
            }

            // Check that the manager is not already busy with /monitor-workers
            let manager_busy = swarm
                .manager
                .pane_content
                .to_lowercase()
                .contains("monitor-workers");

            if manager_busy {
                continue;
            }

            let Some(monitor_cmd) = self.monitor_workers_cmd(&swarm.agent_type) else {
                continue;
            };

            // Send the runtime-specific monitor command to the manager pane
            let target = &swarm.manager.tmux_target;
            tracing::info!(
                "Auto-dispatching manager monitor command to {} (idle worker detected in {})",
                target,
                swarm.project_name
            );
            if let Err(e) =
                crate::tmux::proxy::send_keys(&self.transport, target, &monitor_cmd).await
            {
                tracing::warn!("Failed to auto-dispatch manager monitor command: {e}");
            }
            self.auto_dispatch_last = Some(std::time::Instant::now());
            self.status_message = Some(format!(
                "Auto-dispatched manager monitor command: {monitor_cmd}"
            ));
            return; // Only dispatch once per tick
        }
    }

    fn refresh_statuses(&mut self) {
        // Read JSON status files from /tmp/agents-ui/
        let json_statuses = crate::model::status::read_json_status_files();

        for swarm in &mut self.swarms {
            // Update workers with JSON status info
            for worker in &mut swarm.workers {
                if let Some(json_status) = json_statuses.get(&worker.tmux_target) {
                    worker.current_issue = json_status.issue;
                    worker.current_issue_title = json_status.title.clone();
                    // Update AgentState if the JSON reports working with an issue
                    if json_status.status == "working" {
                        if let Some(issue_num) = json_status.issue {
                            worker.status.state = crate::model::status::AgentState::Working {
                                issue: Some(issue_num),
                            };
                        }
                    } else if json_status.status == "idle" {
                        worker.status.state = crate::model::status::AgentState::Idle;
                    }
                }
            }
            // Also check the manager
            if let Some(json_status) = json_statuses.get(&swarm.manager.tmux_target) {
                swarm.manager.current_issue = json_status.issue;
                swarm.manager.current_issue_title = json_status.title.clone();
            }
            // Refresh all agents (manager + workers)
            let agents = std::iter::once(&mut swarm.manager).chain(swarm.workers.iter_mut());

            for agent in agents {
                // Try status file first
                let status_file = agent
                    .worktree_path
                    .join(swarm.agent_type.status_dir())
                    .join("fix-loop.status");
                if !self.transport.is_remote() && status_file.exists() {
                    agent.status = crate::model::status::read_status_file(&status_file);
                } else if !agent.pane_content.is_empty() {
                    // Infer status from pane content
                    agent.status = infer_status_from_pane(&agent.pane_content);
                }

                // Re-infer from pane content, persisting "Working #N" status
                // until we see an explicit change
                if !agent.pane_content.is_empty() {
                    let new_status = infer_status_from_pane(&agent.pane_content);
                    // Persist "Working #N" status until we see an explicit change
                    // (Idle, Stopped, or a different issue number)
                    match (&agent.status.state, &new_status.state) {
                        (
                            crate::model::status::AgentState::Working { issue: Some(_) },
                            crate::model::status::AgentState::Working { issue: None },
                        ) => {
                            // Keep the old status with issue number — new inference
                            // just lost track of it because the text scrolled
                        }
                        _ => {
                            // Clear dispatch tracking when worker goes idle
                            if matches!(
                                new_status.state,
                                crate::model::status::AgentState::Idle
                                    | crate::model::status::AgentState::Stopped
                            ) {
                                agent.dispatched_issue = None;
                            }
                            agent.status = new_status;
                        }
                    }
                }

                // If we detected "Working" but have no issue number, try the git branch
                if let crate::model::status::AgentState::Working { issue: None } =
                    &agent.status.state
                {
                    if let Some(n) = issue_from_git_branch(&agent.worktree_path) {
                        agent.status.state =
                            crate::model::status::AgentState::Working { issue: Some(n) };
                    }
                }

                if !agent.is_manager {
                    if let Some(dead_status) = crate::model::status::dead_worker_status(
                        &swarm.agent_type,
                        &agent.worktree_path,
                        &agent.pane_content,
                    ) {
                        agent.dispatched_issue = None;
                        agent.status = dead_status;
                    }
                }
            }

            // Parse manager pane for /monitor-workers output to supplement worker statuses
            let manager_content = swarm.manager.pane_content.clone();
            if !manager_content.is_empty() {
                let updates = parse_monitor_workers_output(&manager_content);
                for (wt_suffix, issue_num, status_text) in &updates {
                    // Match worker by worktree path suffix (e.g., "wt-2")
                    for worker in &mut swarm.workers {
                        let wt_name = worker
                            .worktree_path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        if wt_name.ends_with(&format!("-{wt_suffix}")) || wt_name == *wt_suffix {
                            // Only update if the worker doesn't already have issue info
                            let can_supplement = matches!(
                                &worker.status.state,
                                crate::model::status::AgentState::Working { issue: None }
                                    | crate::model::status::AgentState::Idle
                            ) || matches!(
                                &worker.status.state,
                                crate::model::status::AgentState::Unknown(detail)
                                    if !detail.to_lowercase().starts_with("dead:")
                            );
                            if can_supplement {
                                let lower = status_text.to_lowercase();
                                if lower.contains("dispatched")
                                    || lower.contains("working")
                                    || lower.contains("active")
                                {
                                    worker.status.state =
                                        crate::model::status::AgentState::Working {
                                            issue: *issue_num,
                                        };
                                } else if lower.contains("idle") {
                                    worker.status.state = crate::model::status::AgentState::Idle;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Scan for git repos in cwd and child directories.
    fn scan_available_repos(&mut self) {
        if self.transport.is_remote() {
            self.available_repos.clear();
            return;
        }

        let mut repos = Vec::new();

        if let Ok(cwd) = std::env::current_dir() {
            // Check if cwd itself is a git repo
            if cwd.join(".git").exists() {
                let active_names: Vec<&str> = self
                    .swarms
                    .iter()
                    .map(|s| s.project_name.as_str())
                    .collect();
                let name = cwd
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if !active_names.contains(&name.as_str()) {
                    repos.push(cwd.clone());
                }
            }

            // Check child directories
            if let Ok(entries) = std::fs::read_dir(&cwd) {
                let active_names: Vec<String> =
                    self.swarms.iter().map(|s| s.project_name.clone()).collect();
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() && path.join(".git").exists() {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        // Skip if already has an active swarm, or is a worktree (contains "-wt-")
                        if !active_names.contains(&name) && !name.contains("-wt-") {
                            repos.push(path);
                        }
                    }
                }
            }
        }

        repos.sort();
        self.available_repos = repos;
    }

    async fn repo_path_exists(&self, repo_path: &std::path::Path) -> bool {
        if self.transport.is_remote() {
            self.transport.dir_exists(&repo_path.join(".git")).await
        } else {
            repo_path.join(".git").exists()
        }
    }

    async fn ensure_runtime_prerequisites(&self, agent_type: &AgentType) -> Result<()> {
        let outcome = crate::runtime::validate_environment(
            &self.transport,
            Some(agent_type),
            self.runtime_pref_repo_root.as_deref(),
        )
        .await?;
        if let Some(gh_warning) = outcome.gh_warning {
            tracing::warn!("{gh_warning}");
        }

        Ok(())
    }
}

fn next_runtime(agent_type: &AgentType) -> AgentType {
    match agent_type {
        AgentType::Claude => AgentType::Codex,
        AgentType::Codex => AgentType::Droid,
        AgentType::Droid => AgentType::Gemini,
        AgentType::Gemini => AgentType::Claude,
    }
}

fn prev_runtime(agent_type: &AgentType) -> AgentType {
    match agent_type {
        AgentType::Claude => AgentType::Gemini,
        AgentType::Codex => AgentType::Claude,
        AgentType::Droid => AgentType::Codex,
        AgentType::Gemini => AgentType::Droid,
    }
}

fn installer_script_candidates(
    agents_dir: &std::path::Path,
    repo_path: &std::path::Path,
    script_name: &str,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(parent) = repo_path.parent() {
        candidates.push(parent.join("agents/scripts").join(script_name));
    }

    candidates.push(PathBuf::from("../agents/scripts").join(script_name));
    candidates.push(agents_dir.join("scripts").join(script_name));

    if agents_dir.ends_with("plugins/autocoder") {
        if let Some(root) = agents_dir.parent().and_then(|p| p.parent()) {
            candidates.push(root.join("scripts").join(script_name));
        }
    }

    candidates
}

fn agents_repo_root_candidates(
    agents_dir: Option<&std::path::Path>,
    repo_path: &std::path::Path,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(parent) = repo_path.parent() {
        candidates.push(parent.join("agents"));
    }

    if let Some(agents_dir) = agents_dir {
        let root = if agents_dir.ends_with("plugins/autocoder") {
            agents_dir
                .parent()
                .and_then(|p| p.parent())
                .map(PathBuf::from)
                .unwrap_or_else(|| agents_dir.to_path_buf())
        } else {
            agents_dir.to_path_buf()
        };

        if !candidates.contains(&root) {
            candidates.push(root);
        }
    }

    candidates
}

async fn codex_repo_assets_present(
    transport: &ServerTransport,
    repo_path: &std::path::Path,
) -> bool {
    transport
        .path_exists(&repo_path.join("scripts").join("codex-fix-loop.sh"))
        .await
        && transport
            .path_exists(&repo_path.join("scripts").join("codex-autocoder.sh"))
            .await
}

async fn codex_user_assets_present(transport: &ServerTransport) -> bool {
    if transport.is_remote() {
        transport
            .output(
                "sh",
                &[
                    "-lc".to_string(),
                    "test -e \"$HOME/.codex/skills/autocoder/SKILL.md\" && test -e \"$HOME/.local/bin/codex-start-parallel\"".to_string(),
                ],
                None,
            )
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    } else {
        let Some(home) = dirs::home_dir() else {
            return false;
        };
        transport
            .path_exists(&home.join(".codex/skills/autocoder/SKILL.md"))
            .await
            && transport
                .path_exists(&home.join(".local/bin/codex-start-parallel"))
                .await
    }
}

async fn gemini_repo_assets_present(
    transport: &ServerTransport,
    repo_path: &std::path::Path,
) -> bool {
    transport
        .path_exists(
            &repo_path
                .join(".agent")
                .join("scripts")
                .join("start-parallel-agents.sh"),
        )
        .await
        && transport
            .path_exists(
                &repo_path
                    .join(".agent")
                    .join("workflows")
                    .join("fix-loop.md"),
            )
            .await
        && transport
            .path_exists(
                &repo_path
                    .join(".agent")
                    .join("workflows")
                    .join("monitor-workers.md"),
            )
            .await
}

async fn gemini_support_root(
    transport: &ServerTransport,
    agents_dir: Option<&std::path::Path>,
    repo_path: &std::path::Path,
) -> Option<PathBuf> {
    for root in agents_repo_root_candidates(agents_dir, repo_path) {
        if transport
            .path_exists(
                &root
                    .join(".agent")
                    .join("scripts")
                    .join("start-parallel-agents.sh"),
            )
            .await
            && transport
                .path_exists(&root.join(".agent").join("workflows").join("fix-loop.md"))
                .await
            && transport
                .path_exists(
                    &root
                        .join(".agent")
                        .join("workflows")
                        .join("monitor-workers.md"),
                )
                .await
        {
            return Some(root);
        }
    }

    None
}

#[cfg(test)]
async fn gemini_agents_assets_present(
    transport: &ServerTransport,
    agents_dir: Option<&std::path::Path>,
    repo_path: &std::path::Path,
) -> bool {
    gemini_repo_assets_present(transport, repo_path).await
        || gemini_support_root(transport, agents_dir, repo_path)
            .await
            .is_some()
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst).with_context(|| format!("Failed to create {}", dst.display()))?;

    for entry in
        std::fs::read_dir(src).with_context(|| format!("Failed to read {}", src.display()))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if file_type.is_symlink() {
            let target = std::fs::read_link(&src_path)
                .with_context(|| format!("Failed to read link {}", src_path.display()))?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(&target, &dst_path).with_context(|| {
                format!(
                    "Failed to create symlink {} -> {}",
                    dst_path.display(),
                    target.display()
                )
            })?;
            #[cfg(windows)]
            {
                let target_path = src_path
                    .canonicalize()
                    .with_context(|| format!("Failed to resolve {}", src_path.display()))?;
                if target_path.is_dir() {
                    std::os::windows::fs::symlink_dir(&target, &dst_path)
                } else {
                    std::os::windows::fs::symlink_file(&target, &dst_path)
                }
                .with_context(|| {
                    format!(
                        "Failed to create symlink {} -> {}",
                        dst_path.display(),
                        target.display()
                    )
                })?;
            }
        } else {
            std::fs::copy(&src_path, &dst_path).with_context(|| {
                format!(
                    "Failed to copy {} to {}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn format_command_output(output: &std::process::Output) -> String {
    let mut text = String::new();

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        text.push_str("stdout:\n");
        text.push_str(&stdout);
        text.push('\n');
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        text.push_str("stderr:\n");
        text.push_str(&stderr);
        text.push('\n');
    }

    if text.is_empty() {
        text.push_str("(no output)\n");
    }

    text
}

fn detect_runtime_alert(content: &str) -> Option<String> {
    crate::runtime::detect_runtime_alert(content)
}

async fn install_agents_for_scope_with_progress<F: Fn(&str)>(
    transport: &ServerTransport,
    agents_dir: &std::path::Path,
    repo_path: &std::path::Path,
    agent_type: AgentType,
    scope: InstallScope,
    progress: &F,
) -> Result<()> {
    if agent_type == AgentType::Gemini {
        if gemini_repo_assets_present(transport, repo_path).await {
            progress("✅ Gemini runtime assets already installed\n");
            return Ok(());
        }

        progress("⏳ Checking Gemini CLI...\n");
        let version = transport
            .output("gemini", &["--help".to_string()], Some(repo_path))
            .await
            .context("Failed to run gemini. Is gemini CLI installed?")?;
        progress(&format!(
            "$ gemini --help\n{}",
            format_command_output(&version)
        ));
        if !version.status.success() {
            anyhow::bail!("gemini CLI is not available");
        }

        let support_root = gemini_support_root(transport, Some(agents_dir), repo_path)
            .await
            .context("Could not find laird/agents support files in ../agents")?;
        let source_agent_dir = support_root.join(".agent");
        let target_agent_dir = repo_path.join(".agent");

        if target_agent_dir.exists() {
            anyhow::bail!(
                "Gemini install target already exists at {}",
                target_agent_dir.display()
            );
        }

        match scope {
            InstallScope::User => {
                progress(&format!(
                    "⏳ Symlinking shared Gemini assets from {} into {}\n",
                    source_agent_dir.display(),
                    target_agent_dir.display()
                ));
                #[cfg(unix)]
                std::os::unix::fs::symlink(&source_agent_dir, &target_agent_dir).with_context(
                    || {
                        format!(
                            "Failed to create symlink {} -> {}",
                            target_agent_dir.display(),
                            source_agent_dir.display()
                        )
                    },
                )?;
                #[cfg(windows)]
                std::os::windows::fs::symlink_dir(&source_agent_dir, &target_agent_dir)
                    .with_context(|| {
                        format!(
                            "Failed to create symlink {} -> {}",
                            target_agent_dir.display(),
                            source_agent_dir.display()
                        )
                    })?;
            }
            InstallScope::Repo => {
                progress(&format!(
                    "⏳ Copying Gemini assets from {} into {}\n",
                    source_agent_dir.display(),
                    target_agent_dir.display()
                ));
                copy_dir_recursive(&source_agent_dir, &target_agent_dir)?;
            }
        }

        progress("✅ Gemini runtime assets installed\n");
        return Ok(());
    }

    if agent_type == AgentType::Codex {
        if codex_repo_assets_present(transport, repo_path).await
            || codex_user_assets_present(transport).await
        {
            progress("✅ Codex runtime assets already installed\n");
            return Ok(());
        }

        progress("⏳ Checking Codex CLI...\n");
        let version = transport
            .output("codex", &["--version".to_string()], Some(repo_path))
            .await
            .context("Failed to run codex. Is codex CLI installed?")?;
        progress(&format!(
            "$ codex --version\n{}",
            format_command_output(&version)
        ));
        if !version.status.success() {
            anyhow::bail!("codex CLI is not available");
        }

        if transport.is_remote() {
            anyhow::bail!(
                "Codex support is not installed on the remote server. Install it on the server and restart atui"
            );
        }

        let installer = installer_script_candidates(agents_dir, repo_path, "install-codex.sh")
            .into_iter()
            .find(|path| path.exists())
            .map(|path| std::fs::canonicalize(&path).unwrap_or(path))
            .context("Could not find install-codex.sh")?;
        progress(&format!(
            "⏳ Running Codex installer: {}\n",
            installer.display()
        ));
        let output = transport
            .output(
                "bash",
                &[
                    installer.to_string_lossy().to_string(),
                    repo_path.to_string_lossy().to_string(),
                ],
                Some(repo_path),
            )
            .await
            .with_context(|| format!("Failed running Codex installer: {}", installer.display()))?;
        progress(&format!(
            "$ bash {} {}\n{}",
            installer.display(),
            repo_path.display(),
            format_command_output(&output),
        ));
        if !output.status.success() {
            anyhow::bail!(
                "Codex installer failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        progress("✅ Codex runtime assets installed\n");
        return Ok(());
    }

    if agent_type != AgentType::Droid {
        return Ok(());
    }

    let user_installed = transport
        .output(
            "droid",
            &[
                "plugin".to_string(),
                "list".to_string(),
                "--scope".to_string(),
                "user".to_string(),
            ],
            Some(repo_path),
        )
        .await
        .context("Failed to check Droid user plugins")
        .map(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .to_lowercase()
                    .contains("autocoder")
        })?;
    let project_installed = transport
        .output(
            "droid",
            &[
                "plugin".to_string(),
                "list".to_string(),
                "--scope".to_string(),
                "project".to_string(),
            ],
            Some(repo_path),
        )
        .await
        .context("Failed to check Droid project plugins")
        .map(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .to_lowercase()
                    .contains("autocoder")
        })?;
    if user_installed
        || project_installed
        || transport
            .path_exists(
                &repo_path
                    .join(".factory")
                    .join("skills")
                    .join("autocoder")
                    .join("SKILL.md"),
            )
            .await
    {
        progress("✅ Droid runtime assets already installed\n");
        return Ok(());
    }

    progress("⏳ Checking Droid CLI...\n");
    let version = transport
        .output("droid", &["--version".to_string()], Some(repo_path))
        .await
        .context("Failed to run droid. Is droid CLI installed?")?;
    progress(&format!(
        "$ droid --version\n{}",
        format_command_output(&version)
    ));
    if !version.status.success() {
        anyhow::bail!("droid CLI is not available");
    }

    let scope_flag = match scope {
        InstallScope::User => "user",
        InstallScope::Repo => "project",
    };

    progress("⏳ Configuring Droid marketplace...\n");
    let add_output = transport
        .output(
            "droid",
            &[
                "plugin".to_string(),
                "marketplace".to_string(),
                "add".to_string(),
                "https://github.com/laird/agents".to_string(),
            ],
            Some(repo_path),
        )
        .await
        .context("Failed to configure Droid marketplace")?;
    progress(&format!(
        "$ droid plugin marketplace add https://github.com/laird/agents\n{}",
        format_command_output(&add_output),
    ));

    let add_failed = !add_output.status.success()
        && !String::from_utf8_lossy(&add_output.stderr)
            .to_lowercase()
            .contains("already");
    if !add_failed {
        progress(&format!(
            "⏳ Installing Droid autocoder plugin ({scope_flag})...\n"
        ));
        let install_output = transport
            .output(
                "droid",
                &[
                    "plugin".to_string(),
                    "install".to_string(),
                    "autocoder@plugin-marketplace".to_string(),
                    "--scope".to_string(),
                    scope_flag.to_string(),
                ],
                Some(repo_path),
            )
            .await
            .context("Failed to install Droid autocoder plugin")?;
        progress(&format!(
            "$ droid plugin install autocoder@plugin-marketplace --scope {scope_flag}\n{}",
            format_command_output(&install_output),
        ));
        if install_output.status.success()
            || String::from_utf8_lossy(&install_output.stderr)
                .to_lowercase()
                .contains("already")
        {
            progress("✅ Droid runtime assets installed\n");
            return Ok(());
        }
    }

    if scope == InstallScope::Repo {
        if transport.is_remote() {
            anyhow::bail!(
                "Droid support is not installed on the remote server. Install it on the server and restart atui"
            );
        }
        let installer = installer_script_candidates(agents_dir, repo_path, "install-droid.sh")
            .into_iter()
            .find(|path| path.exists())
            .map(|path| std::fs::canonicalize(&path).unwrap_or(path))
            .context("Could not find install-droid.sh for repo fallback")?;
        progress(&format!(
            "⏳ Running fallback Droid installer: {}\n",
            installer.display()
        ));
        let output = transport
            .output(
                "bash",
                &[
                    installer.to_string_lossy().to_string(),
                    repo_path.to_string_lossy().to_string(),
                ],
                Some(repo_path),
            )
            .await
            .with_context(|| {
                format!(
                    "Failed running fallback Droid installer: {}",
                    installer.display()
                )
            })?;
        progress(&format!(
            "$ bash {} {}\n{}",
            installer.display(),
            repo_path.display(),
            format_command_output(&output),
        ));
        if !output.status.success() {
            anyhow::bail!(
                "Fallback Droid installer failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        progress("✅ Droid runtime assets installed\n");
        return Ok(());
    }

    anyhow::bail!("Droid plugin install failed")
}

/// Simple tab completion for file paths.
fn tab_complete_path(input: &str) -> Option<String> {
    let expanded = if input.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            input.replacen('~', &home.to_string_lossy(), 1)
        } else {
            input.to_string()
        }
    } else {
        input.to_string()
    };

    let path = PathBuf::from(&expanded);
    let (dir, prefix) = if path.is_dir() {
        (path, "")
    } else {
        let parent = path.parent()?;
        let file_name = path.file_name()?.to_str()?;
        (parent.to_path_buf(), file_name)
    };

    let entries: Vec<_> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with(prefix))
                .unwrap_or(false)
        })
        .collect();

    if entries.len() == 1 {
        let entry = &entries[0];
        let mut completed = dir.join(entry.file_name()).to_string_lossy().to_string();
        if entry.file_type().ok()?.is_dir() {
            completed.push('/');
        }
        // Restore ~ prefix if original had it
        if input.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                let home_str = home.to_string_lossy().to_string();
                if completed.starts_with(&home_str) {
                    completed = completed.replacen(&home_str, "~", 1);
                }
            }
        }
        Some(completed)
    } else {
        // Find longest common prefix among matches
        if entries.is_empty() {
            return None;
        }
        let names: Vec<String> = entries
            .iter()
            .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
            .collect();
        let common = longest_common_prefix(&names);
        if common.len() > prefix.len() {
            let mut completed = dir.join(&common).to_string_lossy().to_string();
            if input.starts_with('~') {
                if let Some(home) = dirs::home_dir() {
                    let home_str = home.to_string_lossy().to_string();
                    if completed.starts_with(&home_str) {
                        completed = completed.replacen(&home_str, "~", 1);
                    }
                }
            }
            Some(completed)
        } else {
            None
        }
    }
}

/// Infer agent status from the last lines of tmux pane content.
fn infer_status_from_pane(content: &str) -> crate::model::status::AgentStatus {
    use crate::model::status::{AgentState, AgentStatus};

    // Strip ANSI escape codes for matching
    let stripped: String = content
        .chars()
        .fold((String::new(), false), |(mut s, in_esc), c| {
            if c == '\x1b' {
                (s, true)
            } else if in_esc {
                (s, c != 'm' && !c.is_ascii_uppercase())
            } else {
                s.push(c);
                (s, false)
            }
        })
        .0;

    let last_lines: Vec<&str> = stripped.lines().rev().take(15).collect();

    let tail = last_lines.join(" ").to_lowercase();

    let state = if tail.contains("droid_fix_loop_idle")
        || tail.contains("idle_no_work_available")
        || tail.contains("fix_loop_idle")
    {
        AgentState::Idle
    } else if tail.contains("waiting for input")
        || tail.contains("> ")
        || tail.contains("what would you like")
        || tail.contains("how can i help")
    {
        AgentState::Idle
    } else if tail.contains("working")
        || tail.contains("reading")
        || tail.contains("writing")
        || tail.contains("editing")
        || tail.contains("searching")
        || tail.contains("running")
        || tail.contains("executing")
        || tail.contains("thinking")
        || tail.contains("analyzing")
    {
        // Try to extract issue number
        let issue = extract_issue_from_text(&tail);
        AgentState::Working { issue }
    } else if tail.contains("$") || tail.contains("%") {
        // Shell prompt — claude hasn't started or has exited
        AgentState::Unknown("Shell".into())
    } else if !content.trim().is_empty() {
        AgentState::Working { issue: None }
    } else {
        AgentState::Unknown("No output".into())
    };

    AgentStatus {
        timestamp: None,
        state,
    }
}

/// Parse /monitor-workers output from manager pane content.
/// Returns Vec of (worktree_suffix, optional_issue_num, status_text).
fn parse_monitor_workers_output(content: &str) -> Vec<(String, Option<u32>, String)> {
    let mut results = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        // Match table rows like: | wt-2 | 2.0 | **dispatched** | #100 (P2 issue detail) |
        if !trimmed.starts_with('|')
            || trimmed.starts_with("| Worker")
            || trimmed.starts_with("|--")
        {
            continue;
        }

        let cells: Vec<&str> = trimmed
            .split('|')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if cells.len() >= 3 {
            let worker_cell = cells[0].trim();
            let status_cell = cells.get(2).unwrap_or(&"").trim();
            let issue_cell = cells.get(3).unwrap_or(&"").trim();

            // Extract worktree suffix (e.g., "wt-2" from the worker column)
            let wt_suffix = worker_cell.to_string();

            // Extract issue number from issue cell (e.g., "#100" from "#100 (P2 issue detail)")
            let issue_num = issue_cell
                .split_whitespace()
                .find(|w| w.starts_with('#'))
                .and_then(|w| {
                    w.trim_start_matches('#')
                        .trim_end_matches(|c: char| !c.is_ascii_digit())
                        .parse::<u32>()
                        .ok()
                });

            // Clean status text (remove ** markdown bold markers)
            let status = status_cell.replace("**", "");

            if !wt_suffix.is_empty() {
                results.push((wt_suffix, issue_num, status));
            }
        }
    }

    results
}

/// Try to extract an issue number from the current git branch name.
/// Matches patterns like "fix/issue-42-auto" or "enhancement/issue-42-auto".
fn issue_from_git_branch(worktree_path: &std::path::Path) -> Option<u32> {
    // Read .git/HEAD or the worktree's gitdir HEAD to get branch name
    let head_path = worktree_path.join(".git");
    let head_content = if head_path.is_file() {
        // Worktree: .git is a file pointing to the gitdir
        let gitdir_ref = std::fs::read_to_string(&head_path).ok()?;
        let gitdir = gitdir_ref.trim().strip_prefix("gitdir: ")?;
        std::fs::read_to_string(std::path::Path::new(gitdir).join("HEAD")).ok()?
    } else {
        std::fs::read_to_string(head_path.join("HEAD")).ok()?
    };

    let branch = head_content.trim().strip_prefix("ref: refs/heads/")?;
    // Match "fix/issue-N-auto" or "enhancement/issue-N-auto"
    let after_issue = branch
        .strip_prefix("fix/issue-")
        .or_else(|| branch.strip_prefix("enhancement/issue-"))?;
    let num_str = after_issue.split('-').next()?;
    num_str.parse::<u32>().ok()
}

/// Try to extract an issue number from text (e.g., "#42", "issue 42").
fn extract_issue_from_text(text: &str) -> Option<u32> {
    for word in text.split_whitespace() {
        if let Some(stripped) = word.strip_prefix('#') {
            if let Ok(n) = stripped
                .trim_end_matches(|c: char| !c.is_ascii_digit())
                .parse::<u32>()
            {
                if n > 0 && n < 100000 {
                    return Some(n);
                }
            }
        }
    }
    None
}

/// Convert a crossterm KeyEvent to a tmux send-keys compatible string.
fn key_event_to_tmux(key: KeyEvent) -> Option<String> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Char(c) => {
            if ctrl {
                // tmux notation: C-a, C-c, etc.
                Some(format!("C-{c}"))
            } else {
                // Literal character — needs to be sent without "Enter"
                Some(c.to_string())
            }
        }
        KeyCode::Enter => Some("Enter".to_string()),
        KeyCode::Tab => Some("Tab".to_string()),
        KeyCode::Backspace => Some("BSpace".to_string()),
        // Esc is handled in handle_agent_view_key before this function is reached.
        KeyCode::Esc => None,
        KeyCode::Up => Some("Up".to_string()),
        KeyCode::Down => Some("Down".to_string()),
        KeyCode::Left => Some("Left".to_string()),
        KeyCode::Right => Some("Right".to_string()),
        KeyCode::Home => Some("Home".to_string()),
        KeyCode::End => Some("End".to_string()),
        KeyCode::Delete => Some("DC".to_string()),
        KeyCode::F(n) => Some(format!("F{n}")),
        _ => None,
    }
}

/// Send a raw key to a tmux pane (without appending Enter).
async fn send_raw_key(transport: &ServerTransport, target: &str, tmux_key: &str) -> Result<()> {
    let output = transport
        .output(
            "tmux",
            &[
                "send-keys".to_string(),
                "-t".to_string(),
                target.to_string(),
                tmux_key.to_string(),
            ],
            None,
        )
        .await
        .context("Failed to send key to tmux")?;

    if !output.status.success() {
        tracing::warn!(
            "tmux send-keys failed for {tmux_key}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn longest_common_prefix(strings: &[String]) -> String {
    if strings.is_empty() {
        return String::new();
    }
    let first = &strings[0];
    let mut len = first.len();
    for s in &strings[1..] {
        len = len.min(s.len());
        for (i, (a, b)) in first.chars().zip(s.chars()).enumerate() {
            if a != b {
                len = len.min(i);
                break;
            }
        }
    }
    first[..len].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::status::AgentState;
    use crate::model::swarm::AgentType;
    use crate::transport::ServerTransport;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("agents-ui-{name}-{}-{nanos}", std::process::id()))
    }

    #[test]
    fn runtime_navigation_cycles_through_supported_runtimes() {
        assert_eq!(next_runtime(&AgentType::Claude), AgentType::Codex);
        assert_eq!(next_runtime(&AgentType::Codex), AgentType::Droid);
        assert_eq!(next_runtime(&AgentType::Droid), AgentType::Gemini);
        assert_eq!(next_runtime(&AgentType::Gemini), AgentType::Claude);

        assert_eq!(prev_runtime(&AgentType::Claude), AgentType::Gemini);
        assert_eq!(prev_runtime(&AgentType::Gemini), AgentType::Droid);
        assert_eq!(prev_runtime(&AgentType::Droid), AgentType::Codex);
        assert_eq!(prev_runtime(&AgentType::Codex), AgentType::Claude);
    }

    #[tokio::test]
    async fn codex_repo_assets_require_both_core_wrappers() {
        let root = temp_path("codex-repo-assets");
        let scripts = root.join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        let transport = ServerTransport::default();

        assert!(!codex_repo_assets_present(&transport, &root).await);

        std::fs::write(scripts.join("codex-fix-loop.sh"), "#!/bin/bash\n").unwrap();
        assert!(!codex_repo_assets_present(&transport, &root).await);

        std::fs::write(scripts.join("codex-autocoder.sh"), "#!/bin/bash\n").unwrap();
        assert!(codex_repo_assets_present(&transport, &root).await);

        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn codex_user_assets_require_skill_and_binary() {
        let home = temp_path("codex-user-assets");
        let skills = home.join(".codex/skills/autocoder");
        let bin = home.join(".local/bin");
        std::fs::create_dir_all(&skills).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        let original_home = std::env::var_os("HOME");
        unsafe { std::env::set_var("HOME", &home) };
        let transport = ServerTransport::default();

        assert!(!codex_user_assets_present(&transport).await);

        std::fs::write(skills.join("SKILL.md"), "name: autocoder\n").unwrap();
        assert!(!codex_user_assets_present(&transport).await);

        std::fs::write(bin.join("codex-start-parallel"), "#!/bin/bash\n").unwrap();
        assert!(codex_user_assets_present(&transport).await);

        if let Some(value) = original_home {
            unsafe { std::env::set_var("HOME", value) };
        } else {
            unsafe { std::env::remove_var("HOME") };
        }
        std::fs::remove_dir_all(home).ok();
    }

    #[tokio::test]
    async fn gemini_assets_require_repo_workflows_or_shared_source() {
        let root = temp_path("gemini-assets");
        let repo_parent = root.join("workspace");
        let repo = repo_parent.join("repo");
        let agents = repo_parent.join("agents");
        let transport = ServerTransport::default();

        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&agents).unwrap();

        assert!(!gemini_agents_assets_present(&transport, None, &repo).await);

        let repo_scripts = repo.join(".agent/scripts");
        std::fs::create_dir_all(&repo_scripts).unwrap();
        std::fs::write(
            repo_scripts.join("start-parallel-agents.sh"),
            "#!/bin/bash\n",
        )
        .unwrap();
        assert!(!gemini_agents_assets_present(&transport, None, &repo).await);

        let repo_workflows = repo.join(".agent/workflows");
        std::fs::create_dir_all(&repo_workflows).unwrap();
        std::fs::write(repo_workflows.join("fix-loop.md"), "# fix-loop\n").unwrap();
        assert!(!gemini_agents_assets_present(&transport, None, &repo).await);

        std::fs::write(
            repo_workflows.join("monitor-workers.md"),
            "# monitor-workers\n",
        )
        .unwrap();
        assert!(gemini_agents_assets_present(&transport, None, &repo).await);

        std::fs::remove_dir_all(repo.join(".agent")).ok();
        assert!(!gemini_agents_assets_present(&transport, None, &repo).await);

        let shared_scripts = agents.join(".agent/scripts");
        std::fs::create_dir_all(&shared_scripts).unwrap();
        std::fs::write(
            shared_scripts.join("start-parallel-agents.sh"),
            "#!/bin/bash\n",
        )
        .unwrap();
        let shared_workflows = agents.join(".agent/workflows");
        std::fs::create_dir_all(&shared_workflows).unwrap();
        std::fs::write(shared_workflows.join("fix-loop.md"), "# fix-loop\n").unwrap();
        std::fs::write(
            shared_workflows.join("monitor-workers.md"),
            "# monitor-workers\n",
        )
        .unwrap();
        assert!(gemini_agents_assets_present(&transport, None, &repo).await);

        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn selected_agent_type_for_new_swarm_uses_dialog_choice_for_all_runtimes() {
        for runtime in [
            AgentType::Claude,
            AgentType::Codex,
            AgentType::Droid,
            AgentType::Gemini,
        ] {
            let mut app = App::new(None, false, None, None, None).await.unwrap();
            app.default_agent_type = AgentType::Claude;
            app.new_swarm_agent_type = runtime.clone();

            assert_eq!(app.selected_agent_type_for_new_swarm(), runtime);
        }
    }

    #[tokio::test]
    async fn selected_agent_type_for_new_swarm_respects_locked_runtime() {
        let mut app = App::new(Some(AgentType::Claude), true, None, None, None)
            .await
            .unwrap();
        app.new_swarm_agent_type = AgentType::Codex;

        assert_eq!(app.selected_agent_type_for_new_swarm(), AgentType::Claude);
    }

    #[tokio::test]
    async fn selecting_available_repo_shows_runtime_picker_only_when_unlocked() {
        let repo_path = temp_path("new-swarm-repo-flow");
        std::fs::create_dir_all(&repo_path).unwrap();

        let mut unlocked = App::new(None, false, None, None, None).await.unwrap();
        unlocked.available_repos = vec![repo_path.clone()];
        unlocked.select_repo_row(0).await.unwrap();
        assert!(matches!(
            unlocked.screen,
            Screen::NewSwarm {
                field: NewSwarmField::AgentRuntime
            }
        ));

        let mut locked = App::new(Some(AgentType::Gemini), true, None, None, None)
            .await
            .unwrap();
        locked.available_repos = vec![repo_path.clone()];
        locked.select_repo_row(0).await.unwrap();
        assert!(matches!(
            locked.screen,
            Screen::NewSwarm {
                field: NewSwarmField::NumWorkers
            }
        ));

        std::fs::remove_dir_all(repo_path).ok();
    }

    #[tokio::test]
    async fn preferred_agent_type_for_repo_defaults_to_current_runtime_when_unsaved() {
        let app = App::new(Some(AgentType::Droid), false, None, None, None)
            .await
            .unwrap();
        let repo_path = temp_path("preferred-runtime-unsaved");
        std::fs::create_dir_all(&repo_path).unwrap();

        assert_eq!(
            app.preferred_agent_type_for_repo(&repo_path),
            AgentType::Droid
        );

        std::fs::remove_dir_all(repo_path).ok();
    }

    #[test]
    fn installer_candidates_cover_repo_relative_and_agents_dir_locations() {
        let root = temp_path("installer-candidates");
        let repo_parent = root.join("workspace");
        let repo = repo_parent.join("repo");
        let agents_root = root.join("agents");
        let plugin_dir = agents_root.join("plugins/autocoder");

        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&plugin_dir).unwrap();

        let candidates = installer_script_candidates(&plugin_dir, &repo, "install-codex.sh");

        assert!(
            candidates
                .iter()
                .any(|p| p.ends_with("workspace/agents/scripts/install-codex.sh"))
        );
        assert!(
            candidates
                .iter()
                .any(|p| p.ends_with("plugins/autocoder/scripts/install-codex.sh"))
        );
        assert!(
            candidates
                .iter()
                .any(|p| p.ends_with("agents/scripts/install-codex.sh"))
        );

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn extract_issue_from_text_handles_punctuation_and_bounds() {
        assert_eq!(
            extract_issue_from_text("working issue #123, updating tests"),
            Some(123)
        );
        assert_eq!(extract_issue_from_text("done with #100000"), None);
        assert_eq!(extract_issue_from_text("nothing assigned"), None);
    }

    #[test]
    fn infer_status_from_pane_detects_issue_and_idle_prompt() {
        let working = infer_status_from_pane("Reading files\nworking on issue #77 now");
        assert!(matches!(
            working.state,
            AgentState::Working { issue: Some(77) }
        ));

        let idle = infer_status_from_pane("What would you like me to do next?");
        assert!(matches!(idle.state, AgentState::Idle));
    }

    #[test]
    fn infer_status_from_pane_detects_fix_loop_idle_markers() {
        let idle = infer_status_from_pane("IDLE_NO_WORK_AVAILABLE\nDROID_FIX_LOOP_IDLE");
        assert!(matches!(idle.state, AgentState::Idle));
    }

    #[test]
    fn key_event_to_tmux_maps_control_and_special_keys() {
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(key_event_to_tmux(ctrl_c).as_deref(), Some("C-c"));

        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(key_event_to_tmux(enter).as_deref(), Some("Enter"));
    }

    #[test]
    fn lcp_empty_input() {
        assert_eq!(longest_common_prefix(&[]), "");
    }

    #[test]
    fn lcp_single_string() {
        assert_eq!(longest_common_prefix(&["hello".to_string()]), "hello");
    }

    #[test]
    fn lcp_common_prefix() {
        assert_eq!(
            longest_common_prefix(&[
                "foobar".to_string(),
                "foobaz".to_string(),
                "fooqux".to_string(),
            ]),
            "foo"
        );
    }

    #[test]
    fn lcp_identical_strings() {
        assert_eq!(
            longest_common_prefix(&["abc".to_string(), "abc".to_string()]),
            "abc"
        );
    }

    #[test]
    fn lcp_no_common_prefix() {
        assert_eq!(
            longest_common_prefix(&["abc".to_string(), "xyz".to_string()]),
            ""
        );
    }

    #[test]
    fn lcp_one_empty_string() {
        assert_eq!(
            longest_common_prefix(&["".to_string(), "abc".to_string()]),
            ""
        );
    }

    #[test]
    fn lcp_different_lengths() {
        assert_eq!(
            longest_common_prefix(&["ab".to_string(), "abcdef".to_string()]),
            "ab"
        );
    }

    #[test]
    fn parse_direct_gh_command_extracts_repo_scoped_args() {
        assert_eq!(
            parse_direct_gh_command("gh issue edit 123 --remove-label proposal"),
            ParsedGhCommand::Parsed(vec![
                "issue".to_string(),
                "edit".to_string(),
                "123".to_string(),
                "--remove-label".to_string(),
                "proposal".to_string(),
            ])
        );
    }

    #[test]
    fn parse_direct_gh_command_preserves_quoted_arguments() {
        assert_eq!(
            parse_direct_gh_command("gh issue create --title \"Fix flaky test\""),
            ParsedGhCommand::Parsed(vec![
                "issue".to_string(),
                "create".to_string(),
                "--title".to_string(),
                "Fix flaky test".to_string(),
            ])
        );
    }

    #[test]
    fn parse_direct_gh_command_ignores_non_gh_commands() {
        assert_eq!(
            parse_direct_gh_command("/autocoder:fix 42"),
            ParsedGhCommand::NotGh
        );
    }

    #[test]
    fn parse_direct_gh_command_handles_unclosed_quotes_without_crashing() {
        let parsed = parse_direct_gh_command("gh issue create --title \"unterminated");
        match parsed {
            ParsedGhCommand::Malformed(msg) => {
                assert!(msg.contains("missing closing quote"));
            }
            other => panic!("expected malformed gh command, got {other:?}"),
        }
    }

    #[test]
    fn report_log_errors_command_matches_primary_and_alias() {
        assert!(is_report_log_errors_command("gh agents-ui report-errors"));
        assert!(is_report_log_errors_command("/report-errors"));
        assert!(!is_report_log_errors_command("gh issue list"));
    }

    #[test]
    fn collect_recent_log_error_candidates_dedupes_and_counts() {
        let log = "\
2026-04-01T10:00:00Z  INFO agents_tui::app: startup
2026-04-01T10:01:00Z ERROR agents_tui::app: Failed to parse gh command
2026-04-01T10:02:00Z ERROR agents_tui::app: Failed to parse gh command
2026-04-01T10:03:00Z ERROR agents_tui::tmux: tmux send-keys failed
";
        let candidates = collect_recent_log_error_candidates(log, 10, 3);
        assert_eq!(candidates.len(), 2);
        assert_eq!(
            candidates[0].summary,
            "agents_tui::tmux: tmux send-keys failed"
        );
        assert_eq!(
            candidates[1].summary,
            "agents_tui::app: Failed to parse gh command"
        );
        assert_eq!(candidates[1].occurrences, 2);
    }

    #[test]
    fn gh_issue_command_mutates_only_for_write_actions() {
        assert!(gh_issue_command_mutates(&[
            "issue".to_string(),
            "edit".to_string(),
            "42".to_string(),
        ]));
        assert!(!gh_issue_command_mutates(&[
            "issue".to_string(),
            "list".to_string(),
        ]));
        assert!(!gh_issue_command_mutates(&[
            "repo".to_string(),
            "view".to_string(),
        ]));
    }
}
