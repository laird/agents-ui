use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::time::Duration;

use std::collections::HashMap;

use crate::adapter::claude::ClaudeAdapter;
use crate::adapter::traits::{AgentRuntime, SwarmConfig};
use crate::event::{Event, EventHandler};
use crate::model::issue::{GitHubIssue, IssueCache};
use crate::model::swarm::{AgentType, Swarm};
use crate::scripts::launcher;
use crate::transport::ServerTransport;
use crate::tmux::proxy;
use crate::tui::Tui;
use crate::ui::agent_view::AgentView;
use crate::ui::repo_view::RepoView;
use crate::ui::swarm_view::{SwarmView, SwarmPanel};
use crate::ui::repos_list::ReposListView;

/// Which screen we're on.
#[derive(Debug, Clone)]
pub enum Screen {
    RuntimeSelect,
    InstallScopeSelect,
    ReposList,
    /// Prompt for repo path to launch a new swarm.
    NewSwarm { field: NewSwarmField },
    RepoView { swarm_idx: usize },
    AgentView { swarm_idx: usize, agent_id: String },
}

#[derive(Debug, Clone)]
pub enum NewSwarmField {
    RepoPath,
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
            field: CreateIssueField::Title,
            priority: IssuePriority::P2,
            issue_type: IssueType::Bug,
            label_toggles: [false; 6],
            label_cursor: 0,
        }
    }

    /// Build the comma-separated labels string for gh issue create.
    pub fn labels_string(&self) -> String {
        let mut labels = vec![self.priority.label().to_string(), self.issue_type.label().to_string()];
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

pub struct App {
    pub running: bool,
    pub screen: Screen,
    pub swarms: Vec<Swarm>,
    pub repos_list: ReposListView,
    pub repo_view: RepoView,
    pub agent_view: AgentView,
    pub events: EventHandler,
    pub adapter: ClaudeAdapter,
    transport: ServerTransport,
    pub agents_dir: std::path::PathBuf,
    /// Active pane watcher handles (so we can cancel them).
    pane_watchers: Vec<tokio::task::JoinHandle<()>>,
    issue_watchers: Vec<tokio::task::JoinHandle<()>>,
    /// Input buffer for new swarm dialog.
    pub dialog_input: String,
    /// Stored repo path during new swarm flow.
    pub new_swarm_repo: String,
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
            events,
            adapter,
            transport,
            agents_dir,
            pane_watchers: Vec::new(),
            issue_watchers: Vec::new(),
            dialog_input: String::new(),
            new_swarm_repo: String::new(),
            status_message: startup_warning,
            available_repos: Vec::new(),
            swarm_view: SwarmView::new(),
            swarm_focus: SwarmPanel::Manager,
            issue_caches: HashMap::new(),
            blink: false,
            blink_counter: 0,
            create_issue_form: None,
            default_agent_type,
            runtime_locked_from_cli,
            runtime_pref_repo_root,
            install_scope: InstallScope::User,
            pending_launch: None,
        };

        // Scan for available repos (git directories in cwd or children)
        app.scan_available_repos();

        if initial_agent_type.is_some() {
            // Start pane watchers and issue fetchers for discovered swarms
            app.start_all_pane_watchers();
            app.start_all_issue_watchers();
            app.auto_select_current_repo_swarm();
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
                        );
                    }
                    Screen::NewSwarm { field } => {
                        let field = field.clone();
                        let input = self.dialog_input.clone();
                        let repo = self.new_swarm_repo.clone();
                        crate::ui::new_swarm::render_new_swarm_dialog(
                            f, area, &field, &input, &repo,
                        );
                    }
                    Screen::RepoView { swarm_idx } => {
                        if let Some(swarm) = self.swarms.get(*swarm_idx) {
                            let swarm = swarm.clone();
                            let issues = self.issue_caches
                                .get(&swarm.project_name)
                                .map(|c| c.issues.clone())
                                .unwrap_or_default();
                            let focus = self.swarm_focus;
                            let blink = self.blink;
                            self.swarm_view.render(
                                f, area, &swarm, &issues, focus, blink,
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
                }
            })?;

            // Fix invalid screen state (swarm removed while viewing)
            match &self.screen {
                Screen::RepoView { swarm_idx } if *swarm_idx >= self.swarms.len() => {
                    tracing::warn!("Screen points to invalid swarm_idx {}, falling back", swarm_idx);
                    self.screen = Screen::ReposList;
                }
                Screen::AgentView { swarm_idx, .. } if *swarm_idx >= self.swarms.len() => {
                    tracing::warn!("AgentView points to invalid swarm_idx {}, falling back", swarm_idx);
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

    async fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => self.handle_key(key).await?,
            Event::Tick => {
                // Periodic refresh — status files could be re-read here
                self.refresh_statuses();
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
            }
            Event::PaneOutput { agent_id, content } => {
                // agent_id is globally unique (e.g., "nextgen-CDD/manager")
                let is_manager = agent_id.ends_with("/manager");
                for swarm in &mut self.swarms {
                    if let Some(agent) = swarm.agent_by_id_mut(&agent_id) {
                        agent.pane_content = content;
                        break;
                    }
                }
                // Auto-scroll manager panel to bottom when new content arrives
                if is_manager {
                    self.swarm_view.scroll_manager_to_bottom();
                }
            }
            Event::LaunchProgress { project_name, message } => {
                // Append progress to the placeholder swarm's manager pane_content
                for swarm in &mut self.swarms {
                    if swarm.project_name == project_name {
                        swarm.manager.pane_content.push_str(&message);
                        break;
                    }
                }
            }
            Event::IssuesUpdated { project_name, issues } => {
                let cache = self.issue_caches.entry(project_name).or_insert_with(IssueCache::default);
                let mut issues = issues;
                // Cross-reference: mark issues being worked by specific workers
                for swarm in &self.swarms {
                    for worker in &swarm.workers {
                        if let crate::model::status::AgentState::Working { issue: Some(n) } = &worker.status.state {
                            if let Some(issue) = issues.iter_mut().find(|i| i.number == *n) {
                                issue.assigned_worker = Some(worker.role.clone());
                            }
                        }
                    }
                }
                cache.issues = issues;
                cache.last_fetched = Some(std::time::Instant::now());
            }
            Event::GhWarning { project_name, message } => {
                tracing::warn!("GitHub warning for {project_name}: {message}");
                self.status_message = Some(message);
            }
            Event::SwarmDiscovered => {
                // Remember which project we're viewing
                let current_project = match &self.screen {
                    Screen::RepoView { swarm_idx } => {
                        self.swarms.get(*swarm_idx).map(|s| s.project_name.clone())
                    }
                    Screen::AgentView { swarm_idx, .. } => {
                        self.swarms.get(*swarm_idx).map(|s| s.project_name.clone())
                    }
                    _ => None,
                };

                if let Ok(swarms) = self.adapter.discover(&self.agents_dir).await {
                    self.swarms = swarms;
                    self.start_all_pane_watchers();
                    self.start_all_issue_watchers();
                    self.scan_available_repos();

                    // Re-point the screen to the same project after re-discovery
                    if let Some(project) = current_project {
                        if let Some(new_idx) = self.swarms.iter().position(|s| s.project_name == project) {
                            match &self.screen {
                                Screen::RepoView { .. } => {
                                    self.screen = Screen::RepoView { swarm_idx: new_idx };
                                }
                                Screen::AgentView { agent_id, .. } => {
                                    let aid = agent_id.clone();
                                    self.screen = Screen::AgentView { swarm_idx: new_idx, agent_id: aid };
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Global: Ctrl+C always quits
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.running = false;
            return Ok(());
        }

        // Global: Alt+Left goes back one level
        if key.modifiers.contains(KeyModifiers::ALT) && key.code == KeyCode::Left {
            match &self.screen {
                Screen::AgentView { swarm_idx, .. } => {
                    let idx = *swarm_idx;
                    self.screen = Screen::RepoView { swarm_idx: idx };
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
                    Screen::AgentView { swarm_idx, .. } => *swarm_idx,
                    _ => {
                        // From repos list, use the selected swarm or first one
                        self.repos_list.selected().unwrap_or(0)
                    }
                };

                if c == '0' {
                    // Alt+0: go back one level
                    match &self.screen {
                        Screen::AgentView { swarm_idx, .. } => {
                            let idx = *swarm_idx;
                            self.screen = Screen::RepoView { swarm_idx: idx };
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
                        self.repo_view = RepoView::new();
                        self.repo_view.focus_manager = true;
                        self.repo_view.manager_scroll = u16::MAX;
                        self.screen = Screen::RepoView { swarm_idx };
                        return Ok(());
                    } else {
                        // Alt+1-9: jump to worker agent view
                        let worker_idx = (c as usize) - ('1' as usize);
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            if let Some(worker) = swarm.workers.get(worker_idx) {
                                self.agent_view = AgentView::new();
                                self.agent_view.scroll_to_bottom();
                                self.screen = Screen::AgentView {
                                    swarm_idx,
                                    agent_id: worker.role.clone(),
                                };
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }

        match &self.screen.clone() {
            Screen::RuntimeSelect => self.handle_runtime_select_key(key).await?,
            Screen::InstallScopeSelect => self.handle_install_scope_key(key).await?,
            Screen::ReposList => self.handle_repos_list_key(key).await?,
            Screen::NewSwarm { field } => {
                self.handle_new_swarm_key(key, field.clone()).await?
            }
            Screen::RepoView { swarm_idx } => {
                self.handle_repo_view_key(key, *swarm_idx).await?
            }
            Screen::AgentView {
                swarm_idx,
                agent_id,
            } => {
                self.handle_agent_view_key(key, *swarm_idx, agent_id.clone())
                    .await?
            }
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
            KeyCode::Enter => {
                self.apply_runtime_selection(self.default_agent_type.clone()).await?;
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

                self.status_message = Some("Installing agents runtime...".to_string());
                match self
                    .install_agents_for_scope(
                        &pending.repo_path,
                        pending.agent_type.clone(),
                        self.install_scope,
                    )
                    .await
                {
                    Ok(()) => {
                        self.pending_launch = None;
                        self.launch_new_swarm(
                            pending.repo_path,
                            pending.num_workers,
                            pending.agent_type,
                        );
                    }
                    Err(e) => {
                        self.pending_launch = None;
                        self.screen = Screen::NewSwarm {
                            field: NewSwarmField::NumWorkers,
                        };
                        self.new_swarm_repo = pending.repo_path.to_string_lossy().to_string();
                        self.dialog_input = pending.num_workers.to_string();
                        self.status_message = Some(format!("Install failed: {e}"));
                    }
                }
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
                if let Err(e) = crate::config::persistence::save_repo_agent_type(root, &agent_type) {
                    tracing::warn!("Failed to save runtime preference to {}: {e}", root.display());
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

        self.start_all_pane_watchers();
        self.start_all_issue_watchers();
        self.scan_available_repos();
        self.screen = Screen::ReposList;
        self.auto_select_current_repo_swarm();
        self.status_message = Some(format!("Using {} runtime", self.default_agent_type));

        Ok(())
    }

    fn auto_select_current_repo_swarm(&mut self) {
        if let Ok(cwd) = std::env::current_dir() {
            let cwd_name = cwd
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if let Some(idx) = self
                .swarms
                .iter()
                .position(|s| s.project_name == cwd_name)
            {
                self.repos_list.table_state.select(Some(idx));
                self.repo_view = RepoView::new();
                self.screen = Screen::RepoView { swarm_idx: idx };
            }
        }
    }

    fn resolve_agent_type_for_repo(&self, repo_path: &std::path::Path) -> AgentType {
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
                tracing::warn!("Failed to persist runtime preference to {}: {e}", root.display());
            }
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

    fn find_droid_installer_script(&self, repo_path: &std::path::Path) -> Option<PathBuf> {
        installer_script_candidates(&self.agents_dir, repo_path, "install-droid.sh")
            .into_iter()
            .find(|path| path.exists())
            .map(|path| std::fs::canonicalize(&path).unwrap_or(path))
    }

    fn find_codex_installer_script(&self, repo_path: &std::path::Path) -> Option<PathBuf> {
        installer_script_candidates(&self.agents_dir, repo_path, "install-codex.sh")
            .into_iter()
            .find(|path| path.exists())
            .map(|path| std::fs::canonicalize(&path).unwrap_or(path))
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

    async fn install_agents_for_scope(
        &self,
        repo_path: &std::path::Path,
        agent_type: AgentType,
        scope: InstallScope,
    ) -> Result<()> {
        if agent_type == AgentType::Codex {
            if self.codex_agents_installed(repo_path).await {
                return Ok(());
            }

            let version = self
                .transport
                .output("codex", &["--version".to_string()], Some(repo_path))
                .await
                .context("Failed to run codex. Is codex CLI installed?")?;
            if !version.status.success() {
                anyhow::bail!("codex CLI is not available");
            }

            if self.transport.is_remote() {
                anyhow::bail!("Codex support is not installed on the remote server. Install it on the server and restart atui");
            }

            let installer = self
                .find_codex_installer_script(repo_path)
                .context("Could not find install-codex.sh")?;
            let output = self
                .transport
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
                        "Failed running Codex installer: {}",
                        installer.display()
                    )
                })?;
            if !output.status.success() {
                anyhow::bail!(
                    "Codex installer failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            return Ok(());
        }

        if agent_type != AgentType::Droid {
            return Ok(());
        }

        if self.droid_agents_installed(repo_path).await? {
            return Ok(());
        }

        let version = self
            .transport
            .output("droid", &["--version".to_string()], Some(repo_path))
            .await
            .context("Failed to run droid. Is droid CLI installed?")?;
        if !version.status.success() {
            anyhow::bail!("droid CLI is not available");
        }

        let scope_flag = match scope {
            InstallScope::User => "user",
            InstallScope::Repo => "project",
        };

        let mut native_install_failed = None::<String>;

        let add_output = self
            .transport
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
        if !add_output.status.success() {
            let stderr = String::from_utf8_lossy(&add_output.stderr).trim().to_string();
            if !stderr.to_lowercase().contains("already") {
                native_install_failed = Some(format!("Marketplace add failed: {stderr}"));
            }
        }

        if native_install_failed.is_none() {
            let install_output = self
                .transport
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
            if !install_output.status.success() {
                let stderr = String::from_utf8_lossy(&install_output.stderr).trim().to_string();
                if !stderr.to_lowercase().contains("already") {
                    native_install_failed = Some(format!("Plugin install failed: {stderr}"));
                }
            }
        }

        if let Some(err) = native_install_failed {
            if scope == InstallScope::Repo {
                tracing::warn!("Native Droid plugin install failed, falling back to repo installer: {err}");
                if self.transport.is_remote() {
                    anyhow::bail!("Droid support is not installed on the remote server. Install it on the server and restart atui");
                }
                let installer = self
                    .find_droid_installer_script(repo_path)
                    .context("Could not find install-droid.sh for repo fallback")?;
                let output = self
                    .transport
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
                if !output.status.success() {
                    anyhow::bail!(
                        "Fallback Droid installer failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
            } else {
                anyhow::bail!("Droid plugin install failed: {err}");
            }
        }

        Ok(())
    }

    fn launch_new_swarm(&mut self, repo_path: PathBuf, num_workers: u32, agent_type: AgentType) {
        let project_name = repo_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let placeholder = Swarm {
            repo_path: repo_path.clone(),
            project_name: project_name.clone(),
            agent_type: agent_type.clone(),
            workflow: None,
            tmux_session: format!("{}-{project_name}", agent_type.session_prefix()),
            manager: crate::model::swarm::AgentInfo {
                id: format!("{project_name}/manager"),
                role: "manager".to_string(),
                worktree_path: repo_path.clone(),
                tmux_target: String::new(),
                status: crate::model::status::AgentStatus::default(),
                is_manager: true,
                pane_content: format!(
                    "🚀 Launching swarm for {project_name}...\n\n\
                     Workers: {num_workers}\n\
                     Runtime: {}\n\n\
                     ⏳ Preparing runtime...\n",
                    agent_type,
                ),
                dispatched_issue: None,
            },
            workers: Vec::new(),
        };

        self.swarms.push(placeholder);
        let swarm_idx = self.swarms.len() - 1;
        self.swarm_view = SwarmView::new();
        self.swarm_focus = SwarmPanel::Manager;
        self.screen = Screen::RepoView { swarm_idx };

        let config = SwarmConfig {
            repo_path,
            agent_type: agent_type.clone(),
            num_workers,
            agents_dir: self.agents_dir.clone(),
        };
        let tx = self.events.tx();
        let pname = project_name.clone();
        let adapter = ClaudeAdapter::new(agent_type, self.transport.clone());

        tokio::spawn(async move {
            let tx2 = tx.clone();
            let pname2 = pname.clone();
            let progress = move |msg: &str| {
                tx2.send(Event::LaunchProgress {
                    project_name: pname2.clone(),
                    message: msg.to_string(),
                })
                .ok();
            };

            tracing::info!("Background launch starting for {}", config.repo_path.display());
            match adapter.launch_with_progress(&config, &progress).await {
                Ok(swarm) => {
                    tracing::info!("Background launch succeeded: session={}", swarm.tmux_session);
                    progress("✅ Triggering swarm discovery...\n");
                    tx.send(Event::SwarmDiscovered)
                    .ok();
                }
                Err(e) => {
                    tracing::error!("Background launch failed: {e}");
                    progress(&format!("\n❌ Launch failed: {e}\n\nPress Esc to go back.\n"));
                }
            }
        });
    }

    /// Total rows in the repos list (active swarms + available repos).
    fn repos_list_len(&self) -> usize {
        self.swarms.len() + self.available_repos.len()
    }

    /// Handle selecting a row in the repos list.
    /// If it's an active swarm, jump to repo view.
    /// If it's an available repo, open the new swarm dialog pre-filled.
    async fn select_repo_row(&mut self, idx: usize) -> Result<()> {
        if idx < self.swarms.len() {
            // Active swarm — jump to repo view
            self.repo_view = RepoView::new();
            self.screen = Screen::RepoView { swarm_idx: idx };
        } else {
            // Available repo — open new swarm dialog pre-filled
            let avail_idx = idx - self.swarms.len();
            if let Some(repo_path) = self.available_repos.get(avail_idx) {
                self.new_swarm_repo = repo_path.to_string_lossy().to_string();
                self.dialog_input = "2".to_string();
                self.status_message = None;
                self.screen = Screen::NewSwarm {
                    field: NewSwarmField::NumWorkers,
                };
            }
        }
        Ok(())
    }

    async fn handle_repos_list_key(&mut self, key: KeyEvent) -> Result<()> {
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
                self.dialog_input = std::env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                self.new_swarm_repo = String::new();
                self.status_message = None;
                self.screen = Screen::NewSwarm {
                    field: NewSwarmField::RepoPath,
                };
            }
            KeyCode::Char('r') => {
                // Refresh: re-discover swarms
                self.status_message = Some("Refreshing...".to_string());
                if let Ok(swarms) = self.adapter.discover(&self.agents_dir).await {
                    self.swarms = swarms;
                    self.start_all_pane_watchers();
                    self.status_message = Some(format!("Found {} swarm(s)", self.swarms.len()));
                }
            }
            KeyCode::Char('d') => {
                // Shut down the selected swarm
                if let Some(idx) = self.repos_list.selected() {
                    if idx < self.swarms.len() {
                        let swarm = self.swarms[idx].clone();
                        let project = swarm.project_name.clone();
                        self.status_message = Some(format!("Shutting down {project}..."));
                        if let Err(e) = self.adapter.teardown(&swarm).await {
                            self.status_message = Some(format!("Teardown error: {e}"));
                        } else {
                            self.swarms.remove(idx);
                            self.start_all_pane_watchers();
                            self.scan_available_repos();
                            self.status_message = Some(format!("Shut down {project}"));
                        }
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
                        self.dialog_input.clone()
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
                        self.status_message = Some(format!("Path not found: {path}"));
                        self.screen = Screen::ReposList;
                        return Ok(());
                    }

                    self.new_swarm_repo = path;
                    self.dialog_input = "2".to_string(); // Default 2 workers
                    self.screen = Screen::NewSwarm {
                        field: NewSwarmField::NumWorkers,
                    };
                }
                KeyCode::Char(c) => {
                    self.dialog_input.push(c);
                }
                KeyCode::Backspace => {
                    self.dialog_input.pop();
                }
                KeyCode::Tab => {
                    // Simple tab completion: try to complete the path
                    if !self.transport.is_remote() {
                        if let Some(completed) = tab_complete_path(&self.dialog_input) {
                            self.dialog_input = completed;
                        }
                    }
                }
                _ => {}
            },
            NewSwarmField::NumWorkers => match key.code {
                KeyCode::Esc => {
                    self.screen = Screen::NewSwarm {
                        field: NewSwarmField::RepoPath,
                    };
                    self.dialog_input = self.new_swarm_repo.clone();
                }
                KeyCode::Enter => {
                    let num_workers: u32 = self.dialog_input.parse().unwrap_or(2);
                    let repo_path = PathBuf::from(&self.new_swarm_repo);
                    self.dialog_input = String::new();
                    let agent_type = self.resolve_agent_type_for_repo(&repo_path);
                    self.persist_agent_type_for_repo(&repo_path, &agent_type);

                    if agent_type == AgentType::Droid {
                        match self.droid_agents_installed(&repo_path).await {
                            Ok(true) => {
                                self.launch_new_swarm(repo_path, num_workers, agent_type);
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
                                self.status_message = Some(format!("Failed to check Droid install: {e}"));
                                self.screen = Screen::ReposList;
                            }
                        }
                    } else if agent_type == AgentType::Codex {
                        if self.codex_agents_installed(&repo_path).await {
                            self.launch_new_swarm(repo_path, num_workers, agent_type);
                        } else {
                            self.status_message = Some("Installing Codex runtime...".to_string());
                            match self
                                .install_agents_for_scope(&repo_path, agent_type.clone(), InstallScope::Repo)
                                .await
                            {
                                Ok(()) => {
                                    self.launch_new_swarm(repo_path, num_workers, agent_type);
                                }
                                Err(e) => {
                                    self.new_swarm_repo = repo_path.to_string_lossy().to_string();
                                    self.dialog_input = num_workers.to_string();
                                    self.status_message = Some(format!("Codex install failed: {e}"));
                                    self.screen = Screen::NewSwarm {
                                        field: NewSwarmField::NumWorkers,
                                    };
                                }
                            }
                        }
                    } else {
                        self.launch_new_swarm(repo_path, num_workers, agent_type);
                    }
                }
                KeyCode::Up => {
                    let n: u32 = self.dialog_input.parse().unwrap_or(1);
                    self.dialog_input = (n + 1).to_string();
                }
                KeyCode::Down => {
                    let n: u32 = self.dialog_input.parse().unwrap_or(2);
                    self.dialog_input = n.max(2).saturating_sub(1).to_string();
                }
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    self.dialog_input.push(c);
                }
                KeyCode::Backspace => {
                    self.dialog_input.pop();
                }
                _ => {}
            },
        }
        Ok(())
    }

    async fn handle_repo_view_key(&mut self, key: KeyEvent, swarm_idx: usize) -> Result<()> {
        // Handle create-issue dialog input
        if let Some(ref mut form) = self.create_issue_form {
            match key.code {
                KeyCode::Esc => {
                    self.create_issue_form = None;
                }
                KeyCode::Tab | KeyCode::Down if form.field != CreateIssueField::Labels => {
                    form.field = match form.field {
                        CreateIssueField::Title => CreateIssueField::Priority,
                        CreateIssueField::Priority => CreateIssueField::IssueType,
                        CreateIssueField::IssueType => CreateIssueField::Labels,
                        CreateIssueField::Labels => CreateIssueField::Labels,
                    };
                }
                KeyCode::BackTab | KeyCode::Up if form.field != CreateIssueField::Title => {
                    form.field = match form.field {
                        CreateIssueField::Title => CreateIssueField::Title,
                        CreateIssueField::Priority => CreateIssueField::Title,
                        CreateIssueField::IssueType => CreateIssueField::Priority,
                        CreateIssueField::Labels => CreateIssueField::IssueType,
                    };
                }
                KeyCode::Enter => {
                    if !form.title.is_empty() {
                        let title = form.title.clone();
                        let labels = form.labels_string();
                        let target = self.swarms.get(swarm_idx)
                            .map(|s| s.manager.tmux_target.clone());
                        if let Some(target) = target {
                            let cmd = format!("create gh issue --label \"{labels}\" \"{title}\"");
                            tracing::info!("Sending '{cmd}' to manager at {target}");
                            proxy::send_keys(&self.transport, &target, &cmd).await?;
                            self.status_message = Some(format!("Created issue: {title}"));
                        }
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
                _ => {}
            }
            return Ok(());
        }

        // Esc goes back to repos list (except when Manager is focused — that's passthrough)
        if key.code == KeyCode::Esc && self.swarm_focus != SwarmPanel::Manager {
            self.screen = Screen::ReposList;
            return Ok(());
        }

        // Tab cycles focus: Manager → Workers → Issues → Manager
        if key.code == KeyCode::Tab {
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

                // Forward everything else to the manager tmux pane
                let tmux_key = key_event_to_tmux(key);
                if let Some(tmux_key) = tmux_key {
                    send_raw_key(&self.transport, &target, &tmux_key).await?;
                    self.swarm_view.scroll_manager_to_bottom();
                }
            }
            SwarmPanel::Workers => {
                let worker_count = self.swarms.get(swarm_idx)
                    .map(|s| s.workers.len()).unwrap_or(0);
                match key.code {
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.swarm_view.next_worker(worker_count);
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.swarm_view.prev_worker(worker_count);
                    }
                    KeyCode::Enter => {
                        // Drill into selected worker's agent view
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            if let Some(worker_idx) = self.swarm_view.selected_worker() {
                                if let Some(worker) = swarm.workers.get(worker_idx) {
                                    self.agent_view = AgentView::new();
                                    self.agent_view.scroll_to_bottom();
                                    self.screen = Screen::AgentView {
                                        swarm_idx,
                                        agent_id: worker.role.clone(),
                                    };
                                }
                            }
                        }
                    }
                    KeyCode::Char('a') => {
                        // Add a new worker
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            let swarm_clone = swarm.clone();
                            self.status_message = Some("Adding worker...".to_string());
                            match self.adapter.add_worker(&swarm_clone).await {
                                Ok(worker) => {
                                    let id = worker.role.clone();
                                    if let Some(swarm) = self.swarms.get_mut(swarm_idx) {
                                        swarm.workers.push(worker);
                                    }
                                    self.start_all_pane_watchers();
                                    self.status_message = Some(format!("Added {id}"));
                                }
                                Err(e) => {
                                    self.status_message = Some(format!("Failed: {e}"));
                                }
                            }
                        }
                    }
                    KeyCode::Char('f') => {
                        // Send /fix-loop to selected worker
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            if let Some(worker_idx) = self.swarm_view.selected_worker() {
                                if let Some(worker) = swarm.workers.get(worker_idx) {
                                    let target = worker.tmux_target.clone();
                                    let id = worker.role.clone();
                                    if let Err(e) = self.adapter.start_worker_loop(&target).await {
                                        self.status_message = Some(format!("Failed: {e}"));
                                    } else {
                                        self.status_message = Some(format!("Sent /fix-loop to {id}"));
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char('d') => {
                        // Shut down selected worker
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            if let Some(worker_idx) = self.swarm_view.selected_worker() {
                                if let Some(worker) = swarm.workers.get(worker_idx) {
                                    let target = worker.tmux_target.clone();
                                    let id = worker.role.clone();
                                    let _ = proxy::kill_pane(&self.transport, &target).await;
                                    self.status_message = Some(format!("Shutting down {id}..."));
                                }
                            }
                        }
                    }
                    KeyCode::Char(c @ '1'..='9') => {
                        let worker_idx = (c as usize) - ('1' as usize);
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            if let Some(worker) = swarm.workers.get(worker_idx) {
                                self.agent_view = AgentView::new();
                                self.agent_view.scroll_to_bottom();
                                self.screen = Screen::AgentView {
                                    swarm_idx,
                                    agent_id: worker.role.clone(),
                                };
                            }
                        }
                    }
                    _ => {}
                }
            }
            SwarmPanel::Issues => {
                let issue_count = self.swarms.get(swarm_idx)
                    .and_then(|s| self.issue_caches.get(&s.project_name))
                    .map(|c| c.issues.iter().filter(|i| i.matches_filter(self.swarm_view.issue_filter)).count())
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
                    KeyCode::Char('a') => {
                        // Add new issue: open create-issue dialog
                        self.create_issue_form = Some(CreateIssueForm::new());
                    }
                    KeyCode::Char('p') => {
                        // Approve: send "approve <issue_number>" to manager pane
                        self.send_issue_command_to_manager(swarm_idx, "approve").await?;
                    }
                    KeyCode::Char('b') => {
                        // Brainstorm: send "brainstorm <issue_number>" to manager pane
                        self.send_issue_command_to_manager(swarm_idx, "brainstorm").await?;
                    }
                    KeyCode::Char('r') => {
                        // Review-blocked in selected runtime
                        if let Some(agent_type) = self.swarms.get(swarm_idx).map(|s| s.agent_type.clone()) {
                            if let Some(cmd) = self.review_blocked_cmd(&agent_type) {
                                if let Some(target) = self.swarms.get(swarm_idx).map(|s| s.manager.tmux_target.clone()) {
                                    tracing::info!("Sending '{cmd}' to manager at {target}");
                                    proxy::send_keys(&self.transport, &target, &cmd).await?;
                                    self.status_message = Some(format!("Sent: {cmd}"));
                                }
                            } else {
                                self.status_message = Some(format!("No review-blocked command configured for {}", agent_type));
                            }
                        }
                    }
                    KeyCode::Char('g') => {
                        // Open selected issue in browser via gh issue view --web
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            let issues: Vec<&GitHubIssue> = self.issue_caches
                                .get(&swarm.project_name)
                                .map(|c| c.issues.iter().filter(|i| i.matches_filter(self.swarm_view.issue_filter)).collect())
                                .unwrap_or_default();
                            if let Some(issue) = self.swarm_view.selected_issue()
                                .and_then(|idx| issues.get(idx))
                            {
                                let num = issue.number;
                                let repo_path = swarm.repo_path.clone();
                                let transport = self.transport.clone();
                                tokio::spawn(async move {
                                    let _ = transport
                                        .output(
                                            "gh",
                                            &[
                                                "issue".to_string(),
                                                "view".to_string(),
                                                num.to_string(),
                                                "--web".to_string(),
                                            ],
                                            Some(&repo_path),
                                        )
                                        .await;
                                });
                                self.status_message = Some(format!("Opening issue #{} in browser", issue.number));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Send a command with the selected issue number to the manager pane.
    async fn send_issue_command_to_manager(&mut self, swarm_idx: usize, cmd: &str) -> Result<()> {
        let (target, issue_num) = {
            let swarm = match self.swarms.get(swarm_idx) {
                Some(s) => s,
                None => return Ok(()),
            };
            let target = swarm.manager.tmux_target.clone();
            let issues: Vec<&GitHubIssue> = self.issue_caches
                .get(&swarm.project_name)
                .map(|c| c.issues.iter().filter(|i| i.matches_filter(self.swarm_view.issue_filter)).collect())
                .unwrap_or_default();
            let issue_num = self.swarm_view.selected_issue()
                .and_then(|idx| issues.get(idx))
                .map(|i| i.number);
            (target, issue_num)
        };

        if let Some(num) = issue_num {
            let full_cmd = format!("{cmd} {num}");
            tracing::info!("Sending '{full_cmd}' to manager at {target}");
            proxy::send_keys(&self.transport, &target, &full_cmd).await?;
            self.status_message = Some(format!("Sent: {full_cmd}"));
        }
        Ok(())
    }

    fn worker_dispatch_cmd(&self, agent_type: &AgentType, issue_number: u32) -> Option<String> {
        match agent_type {
            AgentType::Claude => Some(format!("/autocoder:fix {issue_number}")),
            AgentType::Gemini => Some(format!("/fix {issue_number}")),
            AgentType::Codex => Some(format!(
                "Use the repository's Codex autocoder workflow to work issue #{issue_number} specifically. Start by reading AGENTS.md, skills/autocoder/SKILL.md, skills/autocoder/references/workflow-map.md, and skills/autocoder/references/command-mapping.md. Translate the legacy /fix behavior into direct Codex actions. Do one issue-focused pass, run relevant tests, and summarize the outcome."
            )),
            AgentType::Droid => Some(format!("/fix {issue_number}")),
        }
    }

    fn review_blocked_cmd(&self, agent_type: &AgentType) -> Option<String> {
        match agent_type {
            AgentType::Claude => Some("/autocoder:review-blocked".to_string()),
            AgentType::Gemini => Some("/review-blocked".to_string()),
            AgentType::Codex => Some(
                "Review the repository's blocked autocoder issues. Start by reading AGENTS.md, skills/autocoder/SKILL.md, and skills/autocoder/references/command-mapping.md. Summarize blocked issues by priority and recommend the next human review actions.".to_string()
            ),
            AgentType::Droid => Some("/review-blocked".to_string()),
        }
    }

    fn monitor_workers_cmd(&self, agent_type: &AgentType) -> Option<String> {
        match agent_type {
            AgentType::Claude => Some("/autocoder:monitor-workers".to_string()),
            AgentType::Gemini => Some("/monitor-workers".to_string()),
            AgentType::Codex => Some("/monitor-workers".to_string()),
            AgentType::Droid => Some("/monitor-workers".to_string()),
        }
    }

    async fn handle_agent_view_key(
        &mut self,
        key: KeyEvent,
        swarm_idx: usize,
        agent_id: String,
    ) -> Result<()> {
        let target = self
            .swarms
            .get(swarm_idx)
            .and_then(|s| s.agent(&agent_id))
            .map(|a| a.tmux_target.clone());

        let target = match target {
            Some(t) => t,
            None => {
                self.screen = Screen::RepoView { swarm_idx };
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
                    self.screen = Screen::RepoView { swarm_idx };
                    return Ok(());
                }
                KeyCode::Char('m') => {
                    self.agent_view = AgentView::new();
                    self.agent_view.scroll_to_bottom();
                    self.screen = Screen::AgentView {
                        swarm_idx,
                        agent_id: "manager".to_string(),
                    };
                    return Ok(());
                }
                KeyCode::Char('a') => {
                    // Add a new worker to this swarm
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        let swarm_clone = swarm.clone();
                        self.status_message = Some("Adding worker...".to_string());
                        match self.adapter.add_worker(&swarm_clone).await {
                            Ok(worker) => {
                                let id = worker.role.clone();
                                if let Some(swarm) = self.swarms.get_mut(swarm_idx) {
                                    swarm.workers.push(worker);
                                }
                                self.start_all_pane_watchers();
                                self.status_message =
                                    Some(format!("Added {id} (running /fix-loop)"));
                            }
                            Err(e) => {
                                self.status_message =
                                    Some(format!("Failed to add worker: {e}"));
                            }
                        }
                    }
                }
                KeyCode::Char('f') => {
                    // Send /fix-loop to the selected worker
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        if let Some(worker_idx) = self.repo_view.selected_worker() {
                            if let Some(worker) = swarm.workers.get(worker_idx) {
                                let target = worker.tmux_target.clone();
                                let id = worker.role.clone();
                                tracing::info!("Sending /fix-loop to {id} at {target}");
                                if let Err(e) = self.adapter.start_worker_loop(&target).await {
                                    tracing::error!("Failed to send /fix-loop to {id}: {e}");
                                    self.status_message =
                                        Some(format!("Failed to start {id}: {e}"));
                                } else {
                                    self.status_message =
                                        Some(format!("Sent /fix-loop to {id}"));
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
                                self.status_message =
                                    Some(format!("Shutting down {id}..."));
                            }
                        }
                    }
                }
                KeyCode::Char(c @ '1'..='9') => {
                    let worker_idx = (c as usize) - ('1' as usize);
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        if let Some(worker) = swarm.workers.get(worker_idx) {
                            self.agent_view = AgentView::new();
                            self.agent_view.scroll_to_bottom();
                            self.screen = Screen::AgentView {
                                swarm_idx,
                                agent_id: worker.role.clone(),
                            };
                            return Ok(());
                        }
                    }
                }
                _ => {
                    // Alt+z: tear down the entire swarm (only from repo view)
                    if key.modifiers.contains(KeyModifiers::ALT)
                        && key.code == KeyCode::Char('z')
                    {
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            let project = swarm.project_name.clone();
                            tracing::info!("Tearing down swarm for {project}");
                            if let Err(e) = self.adapter.teardown(swarm).await {
                                tracing::error!("Teardown failed: {e}");
                                self.status_message =
                                    Some(format!("Teardown failed: {e}"));
                            } else {
                                self.swarms.remove(swarm_idx);
                                self.status_message =
                                    Some(format!("Swarm {project} shut down"));
                                self.screen = Screen::ReposList;
                            }
                        }
                    }
                }
            }
        }

        // PageUp/PageDown scroll the view without sending to pane
        match key.code {
            KeyCode::PageUp => {
                self.agent_view.scroll_up(10);
                return Ok(());
            }
            KeyCode::PageDown => {
                self.agent_view.scroll_down(10);
                return Ok(());
            }
            _ => {}
        }

        // Everything else is forwarded directly to the tmux pane.
        // This gives us Claude's native tab completion, slash commands, etc.
        let tmux_key = key_event_to_tmux(key);
        if let Some(tmux_key) = tmux_key {
            send_raw_key(&self.transport, &target, &tmux_key).await?;
            self.agent_view.scroll_to_bottom();
        }

        Ok(())
    }

    /// Start pane watchers for all agents in all swarms.
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

    /// Start issue fetchers for all swarms.
    /// Check for idle workers and dispatch the next available issue to them.
    async fn dispatch_idle_workers(&mut self) {
        for si in 0..self.swarms.len() {
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
                        .min_by_key(|i| i.priority().unwrap_or(99))
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
                            && matches!(
                                w.status.state,
                                crate::model::status::AgentState::Idle
                            )
                    })
                    .map(|(idx, w)| (idx, w.tmux_target.clone()));

                if let Some((worker_idx, target)) = idle_worker {
                    let Some(cmd) = self.worker_dispatch_cmd(&agent_type, issue_num) else {
                        self.status_message = Some(format!(
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
                    if let Ok(()) = crate::tmux::proxy::send_keys_no_enter(&self.transport, &target, &cmd).await {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        crate::tmux::proxy::send_keys_no_enter(&self.transport, &target, "Enter").await.ok();

                        // Track the dispatch
                        self.swarms[si].workers[worker_idx].dispatched_issue = Some(issue_num);
                        self.swarms[si].workers[worker_idx].status.state =
                            crate::model::status::AgentState::Working {
                                issue: Some(issue_num),
                            };
                        self.status_message = Some(format!(
                            "Dispatched #{issue_num} → {}",
                            self.swarms[si].workers[worker_idx].role
                        ));
                    }
                }
            }
        }
    }

    /// Send contextual commands to the manager session when appropriate.
    async fn manage_manager_sessions(&mut self) {
        use crate::ui::swarm_view::agent_needs_input;

        for si in 0..self.swarms.len() {
            let swarm = &self.swarms[si];
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
                    tracing::info!("Manager idle with {idle_workers} idle workers and {available_issues} available issues — sending {monitor_cmd}");
                    crate::tmux::proxy::send_keys_no_enter(&self.transport, &manager_target, &monitor_cmd).await.ok();
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    crate::tmux::proxy::send_keys_no_enter(&self.transport, &manager_target, "Enter").await.ok();
                    self.status_message = Some(format!("Sent {monitor_cmd} to manager"));
                }
            } else if available_issues == 0 && blocked_issues > 0 && idle_workers > 0 {
                // No available work but blocked issues exist — review them
                if let Some(review_cmd) = self.review_blocked_cmd(&swarm.agent_type) {
                    tracing::info!("No available issues, {blocked_issues} blocked — sending {review_cmd}");
                    crate::tmux::proxy::send_keys_no_enter(&self.transport, &manager_target, &review_cmd).await.ok();
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    crate::tmux::proxy::send_keys_no_enter(&self.transport, &manager_target, "Enter").await.ok();
                    self.status_message = Some(format!("Sent {review_cmd} to manager"));
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
    fn refresh_statuses(&mut self) {
        for swarm in &mut self.swarms {
            // Refresh all agents (manager + workers)
            let agents = std::iter::once(&mut swarm.manager)
                .chain(swarm.workers.iter_mut());

            for agent in agents {
                // Try status file first
                let status_file = agent
                    .worktree_path
                    .join(swarm.agent_type.status_dir())
                    .join("fix-loop.status");
                if !self.transport.is_remote() && status_file.exists() {
                    agent.status = crate::model::status::read_status_file(&status_file);
                    continue;
                }

                // Infer status from pane content
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
                            if matches!(new_status.state,
                                crate::model::status::AgentState::Idle |
                                crate::model::status::AgentState::Stopped
                            ) {
                                agent.dispatched_issue = None;
                            }
                            agent.status = new_status;
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
                let active_names: Vec<&str> =
                    self.swarms.iter().map(|s| s.project_name.as_str()).collect();
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
        let location = self.transport.server().unwrap_or("this machine");
        let tmux_hint = if cfg!(target_os = "macos") {
            "brew install tmux"
        } else {
            "sudo apt install tmux"
        };
        if !self.transport.command_exists("tmux").await {
            anyhow::bail!("tmux is not installed on {location}. Install with: {tmux_hint}");
        }

        let (binary, hint) = match agent_type {
            AgentType::Claude => ("claude", "See https://docs.anthropic.com/en/docs/claude-code"),
            AgentType::Codex => ("codex", "npm install -g @openai/codex"),
            AgentType::Droid => ("droid", "See https://droid.dev"),
            AgentType::Gemini => ("gemini", "See https://ai.google.dev"),
        };

        if !self.transport.command_exists(binary).await {
            anyhow::bail!("{binary} is not installed on {location}. {hint}");
        }

        // Non-fatal: check gh auth status
        if let Some(gh_err) = crate::github::check_gh_auth(&self.transport).await {
            tracing::warn!("{gh_err}");
        }

        Ok(())
    }
}

fn next_runtime(agent_type: &AgentType) -> AgentType {
    match agent_type {
        AgentType::Claude => AgentType::Codex,
        AgentType::Codex => AgentType::Droid,
        AgentType::Droid => AgentType::Claude,
        AgentType::Gemini => AgentType::Claude,
    }
}

fn prev_runtime(agent_type: &AgentType) -> AgentType {
    match agent_type {
        AgentType::Claude => AgentType::Droid,
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

    let last_lines: Vec<&str> = stripped
        .lines()
        .rev()
        .take(15)
        .collect();

    let tail = last_lines.join(" ").to_lowercase();

    let state = if tail.contains("waiting for input")
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

/// Try to extract an issue number from text (e.g., "#42", "issue 42").
fn extract_issue_from_text(text: &str) -> Option<u32> {
    for word in text.split_whitespace() {
        if let Some(stripped) = word.strip_prefix('#') {
            if let Ok(n) = stripped.trim_end_matches(|c: char| !c.is_ascii_digit()).parse::<u32>() {
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
        KeyCode::Esc => Some("Escape".to_string()),
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
async fn send_raw_key(
    transport: &ServerTransport,
    target: &str,
    tmux_key: &str,
) -> Result<()> {
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
    use super::{
        codex_repo_assets_present, codex_user_assets_present, extract_issue_from_text,
        infer_status_from_pane, installer_script_candidates, key_event_to_tmux, next_runtime,
        prev_runtime,
    };
    use crate::model::status::AgentState;
    use crate::model::swarm::AgentType;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use crate::transport::ServerTransport;
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
        assert_eq!(next_runtime(&AgentType::Droid), AgentType::Claude);

        assert_eq!(prev_runtime(&AgentType::Claude), AgentType::Droid);
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

        assert!(candidates.iter().any(|p| p.ends_with("workspace/agents/scripts/install-codex.sh")));
        assert!(candidates.iter().any(|p| p.ends_with("plugins/autocoder/scripts/install-codex.sh")));
        assert!(candidates.iter().any(|p| p.ends_with("agents/scripts/install-codex.sh")));

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
    fn key_event_to_tmux_maps_control_and_special_keys() {
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(key_event_to_tmux(ctrl_c).as_deref(), Some("C-c"));

        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(key_event_to_tmux(enter).as_deref(), Some("Enter"));
    }
}
