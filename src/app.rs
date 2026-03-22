use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::time::Duration;

use crate::adapter::claude::ClaudeAdapter;
use crate::adapter::traits::{AgentRuntime, SwarmConfig};
use crate::event::{Event, EventHandler};
use crate::model::swarm::{AgentType, Swarm};
use crate::scripts::launcher;
use crate::tmux::proxy;
use crate::tui::Tui;
use crate::ui::agent_view::AgentView;
use crate::ui::repo_view::RepoView;
use crate::ui::repos_list::ReposListView;

/// Which screen we're on.
#[derive(Debug, Clone)]
pub enum Screen {
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
    Launching,
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
    pub agents_dir: std::path::PathBuf,
    /// Active pane watcher handles (so we can cancel them).
    pane_watchers: Vec<tokio::task::JoinHandle<()>>,
    /// Input buffer for new swarm dialog.
    pub dialog_input: String,
    /// Stored repo path during new swarm flow.
    pub new_swarm_repo: String,
    /// Status message shown at bottom of repos list.
    pub status_message: Option<String>,
    /// Counter for periodic issue refresh (every ~60 ticks = 15 seconds at 250ms tick).
    issue_refresh_counter: u32,
}

impl App {
    pub async fn new() -> Result<Self> {
        let agents_dir = launcher::resolve_agents_dir();
        let adapter = ClaudeAdapter::new();
        let events = EventHandler::new();

        // Discover existing swarms on startup
        let swarms = match adapter.discover(&agents_dir).await {
            Ok(s) => {
                tracing::info!("Discovered {} existing swarm(s)", s.len());
                s
            }
            Err(e) => {
                tracing::warn!("Failed to discover existing swarms: {e}");
                vec![]
            }
        };

        let mut app = Self {
            running: true,
            screen: Screen::ReposList,
            swarms,
            repos_list: ReposListView::new(),
            repo_view: RepoView::new(),
            agent_view: AgentView::new(),
            events,
            adapter,
            agents_dir,
            pane_watchers: Vec::new(),
            dialog_input: String::new(),
            new_swarm_repo: String::new(),
            status_message: None,
            issue_refresh_counter: 0,
        };

        // Start pane watchers for discovered swarms
        app.start_all_pane_watchers();

        Ok(app)
    }

    pub async fn run(&mut self, terminal: &mut Tui) -> Result<()> {
        while self.running {
            // Render
            terminal.draw(|f| {
                let area = f.area();
                match &self.screen {
                    Screen::ReposList => {
                        self.repos_list
                            .render(f, area, &self.swarms, self.status_message.as_deref());
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
                            self.repo_view.render(f, area, &swarm);
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
                self.refresh_statuses();
                self.repo_view.tick_banners();
                // Periodic issue refresh (~every 60 seconds when viewing a repo)
                self.issue_refresh_counter += 1;
                if self.issue_refresh_counter >= 240 {
                    // 240 ticks * 250ms = 60 seconds
                    self.issue_refresh_counter = 0;
                    if let Screen::RepoView { swarm_idx } = &self.screen {
                        self.start_issue_refresh(*swarm_idx);
                    }
                }
            }
            Event::PaneOutput { agent_id, content } => {
                // Update the agent's pane content and detect waiting state
                for swarm in &mut self.swarms {
                    if let Some(agent) = swarm.agent_mut(&agent_id) {
                        agent.waiting_for_input =
                            crate::model::swarm::detect_waiting_for_input(&content);
                        agent.pane_content = content;
                        break;
                    }
                }
            }
            Event::StatusChange { agent_id, status } => {
                for swarm in &mut self.swarms {
                    if let Some(agent) = swarm.agent_mut(&agent_id) {
                        agent.status = status;
                        break;
                    }
                }
            }
            Event::SwarmDiscovered { .. } => {
                // Re-discover swarms
                if let Ok(swarms) = self.adapter.discover(&self.agents_dir).await {
                    self.swarms = swarms;
                    self.start_all_pane_watchers();
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

    /// Convert a crossterm KeyEvent to a tmux send-keys argument.
    /// Returns (key_string, literal) where literal means use -l flag.
    fn key_to_tmux(key: &KeyEvent) -> Option<(String, bool)> {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if let KeyCode::Char(c) = key.code {
                return Some((format!("C-{c}"), false));
            }
        }
        if key.modifiers.contains(KeyModifiers::ALT) {
            if let KeyCode::Char(c) = key.code {
                return Some((format!("M-{c}"), false));
            }
        }
        match key.code {
            KeyCode::Char(c) => Some((c.to_string(), true)),
            KeyCode::Enter => Some(("Enter".to_string(), false)),
            KeyCode::Backspace => Some(("BSpace".to_string(), false)),
            KeyCode::Tab => Some(("Tab".to_string(), false)),
            KeyCode::Esc => Some(("Escape".to_string(), false)),
            KeyCode::Up => Some(("Up".to_string(), false)),
            KeyCode::Down => Some(("Down".to_string(), false)),
            KeyCode::Left => Some(("Left".to_string(), false)),
            KeyCode::Right => Some(("Right".to_string(), false)),
            KeyCode::Home => Some(("Home".to_string(), false)),
            KeyCode::End => Some(("End".to_string(), false)),
            KeyCode::Delete => Some(("DC".to_string(), false)),
            _ => None,
        }
    }

    /// Returns true if we're in a passthrough mode where keystrokes go directly to tmux.
    fn is_passthrough_mode(&self) -> bool {
        matches!(&self.screen, Screen::AgentView { .. })
            || matches!(
                (&self.screen, &self.repo_view.focus),
                (Screen::RepoView { .. }, crate::ui::repo_view::RepoViewFocus::ManagerInput)
            )
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

        // Global: Alt+0 navigates to Repo View, Alt+1..9 to worker/manager agent views
        if key.modifiers.contains(KeyModifiers::ALT) {
            if let KeyCode::Char(c) = key.code {
                if let Some(digit) = c.to_digit(10) {
                    // Determine the current swarm index (use 0 if on repos list with one swarm)
                    let swarm_idx = match &self.screen {
                        Screen::RepoView { swarm_idx } => Some(*swarm_idx),
                        Screen::AgentView { swarm_idx, .. } => Some(*swarm_idx),
                        Screen::ReposList if self.swarms.len() == 1 => Some(0),
                        _ => None,
                    };

                    if let Some(swarm_idx) = swarm_idx {
                        if digit == 0 {
                            // Alt+0: go to Repo View (overview with workers list)
                            self.screen = Screen::RepoView { swarm_idx };
                            return Ok(());
                        } else if let Some(swarm) = self.swarms.get(swarm_idx) {
                            // Alt+1..N: go to worker N-1 agent view
                            let worker_idx = (digit as usize) - 1;
                            if let Some(worker) = swarm.workers.get(worker_idx) {
                                self.agent_view = AgentView::new();
                                self.agent_view.scroll_to_bottom();
                                self.screen = Screen::AgentView {
                                    swarm_idx,
                                    agent_id: worker.id.clone(),
                                };
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }

        match &self.screen.clone() {
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

    async fn handle_repos_list_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Down | KeyCode::Char('j') => {
                self.repos_list.next(self.swarms.len());
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.repos_list.previous(self.swarms.len());
            }
            KeyCode::Enter => {
                if let Some(idx) = self.repos_list.selected() {
                    if idx < self.swarms.len() {
                        self.repo_view = RepoView::new();
                        self.screen = Screen::RepoView { swarm_idx: idx };
                        self.start_issue_refresh(idx);
                    }
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
                    if !repo_path.exists() {
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
                    if let Some(completed) = tab_complete_path(&self.dialog_input) {
                        self.dialog_input = completed;
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

                    self.screen = Screen::NewSwarm {
                        field: NewSwarmField::Launching,
                    };
                    self.dialog_input = String::new();

                    // Launch the swarm
                    let config = SwarmConfig {
                        repo_path: repo_path.clone(),
                        agent_type: AgentType::Claude,
                        num_workers,
                        agents_dir: self.agents_dir.clone(),
                    };

                    match self.adapter.launch(&config).await {
                        Ok(swarm) => {
                            let project = swarm.project_name.clone();

                            // Send post-launch commands after a delay
                            // so Claude sessions have time to initialize
                            self.send_post_launch_commands(&swarm);

                            self.swarms.push(swarm);
                            self.start_all_pane_watchers();
                            let idx = self.swarms.len() - 1;
                            self.repo_view = RepoView::new();
                            self.screen = Screen::RepoView { swarm_idx: idx };
                            self.status_message =
                                Some(format!("Launched swarm for {project}"));
                        }
                        Err(e) => {
                            self.status_message =
                                Some(format!("Failed to launch: {e}"));
                            self.screen = Screen::ReposList;
                        }
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
            NewSwarmField::Launching => {
                // Ignore input while launching
                if key.code == KeyCode::Esc {
                    self.screen = Screen::ReposList;
                }
            }
        }
        Ok(())
    }

    async fn handle_repo_view_key(&mut self, key: KeyEvent, swarm_idx: usize) -> Result<()> {
        use crate::ui::repo_view::RepoViewFocus;
        use crate::model::issue::IssuePriority;

        match self.repo_view.focus.clone() {
            RepoViewFocus::ManagerInput => {
                // Ctrl+] escapes back to Workers focus (like ssh escape)
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && key.code == KeyCode::Char(']')
                {
                    self.repo_view.focus = RepoViewFocus::Workers;
                    return Ok(());
                }

                // Everything else passes through directly to the manager tmux pane
                if let Some((tmux_key, literal)) = Self::key_to_tmux(&key) {
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        let target = swarm.manager.tmux_target.clone();
                        self.adapter.send_raw_key(&target, &tmux_key, literal).await?;
                    }
                }
            }
            RepoViewFocus::Workers => {
                // Dismiss peek popup on any non-Space key
                if key.code != KeyCode::Char(' ') {
                    self.repo_view.peek_worker = None;
                }

                match key.code {
                    KeyCode::Char('q') => self.running = false,
                    KeyCode::Esc => {
                        self.screen = Screen::ReposList;
                    }
                    KeyCode::Tab => {
                        self.repo_view.focus = RepoViewFocus::Issues;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            self.repo_view.next_worker(swarm.workers.len());
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            if self.repo_view.previous_worker(swarm.workers.len()) {
                                self.repo_view.focus = RepoViewFocus::ManagerInput;
                            }
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            if let Some(worker_idx) = self.repo_view.selected_worker() {
                                if let Some(worker) = swarm.workers.get(worker_idx) {
                                    self.agent_view = AgentView::new();
                                    self.agent_view.scroll_to_bottom();
                                    self.screen = Screen::AgentView {
                                        swarm_idx,
                                        agent_id: worker.id.clone(),
                                    };
                                }
                            }
                        }
                    }
                    KeyCode::Char('m') => {
                        self.agent_view = AgentView::new();
                        self.agent_view.scroll_to_bottom();
                        self.screen = Screen::AgentView {
                            swarm_idx,
                            agent_id: "manager".to_string(),
                        };
                    }
                    KeyCode::Char(' ') => {
                        // Quick peek toggle
                        if self.repo_view.peek_worker.is_some() {
                            self.repo_view.peek_worker = None;
                        } else if let Some(idx) = self.repo_view.selected_worker() {
                            self.repo_view.peek_worker = Some(idx);
                        }
                    }
                    KeyCode::Char('a') => {
                        // TODO: add worker
                    }
                    KeyCode::Char('n') | KeyCode::Char('!') => {
                        // Jump to next session waiting for input
                        self.jump_to_next_waiting(swarm_idx);
                    }
                    KeyCode::Char('i') => {
                        use crate::ui::repo_view::CreateIssueState;
                        self.repo_view.focus = RepoViewFocus::CreateIssue(CreateIssueState::SelectType);
                    }
                    _ => {}
                }
            }
            RepoViewFocus::Issues => {
                match key.code {
                    KeyCode::Char('q') => self.running = false,
                    KeyCode::Esc => {
                        self.screen = Screen::ReposList;
                    }
                    KeyCode::Tab => {
                        self.repo_view.focus = RepoViewFocus::Workers;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            let filtered_len = swarm
                                .issue_cache
                                .filtered(self.repo_view.priority_filter.as_ref())
                                .len();
                            self.repo_view.next_issue(filtered_len);
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if self.repo_view.previous_issue() {
                            self.repo_view.focus = RepoViewFocus::ManagerInput;
                        }
                    }
                    KeyCode::Enter => {
                        // Dispatch selected issue to manager
                        if let Some(issue_idx) = self.repo_view.selected_issue_idx() {
                            if let Some(swarm) = self.swarms.get(swarm_idx) {
                                let filtered = swarm
                                    .issue_cache
                                    .filtered(self.repo_view.priority_filter.as_ref());
                                if let Some(issue) = filtered.get(issue_idx) {
                                    let cmd = if issue.labels.iter().any(|l| {
                                        l == "needs-approval"
                                            || l == "needs-design"
                                            || l == "needs-clarification"
                                            || l == "too-complex"
                                    }) {
                                        format!("/review-blocked {}", issue.number)
                                    } else if issue.labels.iter().any(|l| l == "proposal") {
                                        format!("/refine-proposal {}", issue.number)
                                    } else {
                                        format!(
                                            "Please assign issue #{} ({}) to an idle worker",
                                            issue.number, issue.title
                                        )
                                    };
                                    let target = swarm.manager.tmux_target.clone();
                                    self.adapter.send_input(&target, &cmd).await?;
                                    self.status_message =
                                        Some(format!("Sent to manager: {}", cmd));
                                }
                            }
                        }
                    }
                    KeyCode::Char('0') => {
                        self.repo_view.priority_filter = None;
                        self.repo_view.issue_list_state.select(Some(0));
                    }
                    KeyCode::Char('1') => {
                        self.repo_view.priority_filter = Some(IssuePriority::P0);
                        self.repo_view.issue_list_state.select(Some(0));
                    }
                    KeyCode::Char('2') => {
                        self.repo_view.priority_filter = Some(IssuePriority::P1);
                        self.repo_view.issue_list_state.select(Some(0));
                    }
                    KeyCode::Char('3') => {
                        self.repo_view.priority_filter = Some(IssuePriority::P2);
                        self.repo_view.issue_list_state.select(Some(0));
                    }
                    KeyCode::Char('4') => {
                        self.repo_view.priority_filter = Some(IssuePriority::P3);
                        self.repo_view.issue_list_state.select(Some(0));
                    }
                    KeyCode::Char('r') => {
                        // Manual refresh
                        self.start_issue_refresh(swarm_idx);
                    }
                    KeyCode::Char('n') | KeyCode::Char('!') => {
                        // Jump to next session waiting for input
                        self.jump_to_next_waiting(swarm_idx);
                    }
                    KeyCode::Char('i') => {
                        use crate::ui::repo_view::CreateIssueState;
                        self.repo_view.focus = RepoViewFocus::CreateIssue(CreateIssueState::SelectType);
                    }
                    _ => {}
                }
            }
            RepoViewFocus::CreateIssue(state) => {
                use crate::ui::repo_view::{CreateIssueState, NewIssueType};
                match state {
                    CreateIssueState::SelectType => match key.code {
                        KeyCode::Esc => {
                            self.repo_view.focus = RepoViewFocus::Workers;
                        }
                        KeyCode::Char('b') => {
                            self.repo_view.focus = RepoViewFocus::CreateIssue(
                                CreateIssueState::EnterTitle { issue_type: NewIssueType::Bug, title: String::new() },
                            );
                        }
                        KeyCode::Char('e') => {
                            self.repo_view.focus = RepoViewFocus::CreateIssue(
                                CreateIssueState::EnterTitle { issue_type: NewIssueType::Enhancement, title: String::new() },
                            );
                        }
                        KeyCode::Char('i') => {
                            self.repo_view.focus = RepoViewFocus::CreateIssue(
                                CreateIssueState::EnterTitle { issue_type: NewIssueType::Plain, title: String::new() },
                            );
                        }
                        _ => {}
                    },
                    CreateIssueState::EnterTitle { issue_type, title } => match key.code {
                        KeyCode::Esc => {
                            self.repo_view.focus = RepoViewFocus::CreateIssue(CreateIssueState::SelectType);
                        }
                        KeyCode::Enter => {
                            let trimmed = title.trim();
                            if !trimmed.is_empty() {
                                let escaped = trimmed.replace('"', r#"\""#);
                                let label_flag = match issue_type {
                                    NewIssueType::Bug => " --label \"bug\"",
                                    NewIssueType::Enhancement => " --label \"enhancement\"",
                                    NewIssueType::Plain => "",
                                };
                                let cmd = format!(
                                    "gh issue create{} --title \"{}\" --body \"Created from agents-ui\"",
                                    label_flag, escaped
                                );
                                if let Some(swarm) = self.swarms.get(swarm_idx) {
                                    let target = swarm.manager.tmux_target.clone();
                                    if let Err(e) = self.adapter.send_input(&target, &cmd).await {
                                        tracing::warn!("Failed to send issue creation: {e}");
                                    }
                                    let type_label = match issue_type {
                                        NewIssueType::Bug => "bug",
                                        NewIssueType::Enhancement => "enhancement",
                                        NewIssueType::Plain => "issue",
                                    };
                                    self.status_message = Some(format!("Creating {}: {}", type_label, trimmed));
                                    // Trigger issue refresh after a delay
                                    self.issue_refresh_counter = 232;
                                }
                                self.repo_view.focus = RepoViewFocus::Issues;
                            }
                        }
                        KeyCode::Char(c) => {
                            let mut t = title.clone();
                            t.push(c);
                            self.repo_view.focus = RepoViewFocus::CreateIssue(
                                CreateIssueState::EnterTitle { issue_type: issue_type.clone(), title: t },
                            );
                        }
                        KeyCode::Backspace => {
                            let mut t = title.clone();
                            t.pop();
                            self.repo_view.focus = RepoViewFocus::CreateIssue(
                                CreateIssueState::EnterTitle { issue_type: issue_type.clone(), title: t },
                            );
                        }
                        _ => {}
                    },
                }
            }
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
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && key.code == KeyCode::Char(']')
        {
            self.screen = Screen::RepoView { swarm_idx };
            return Ok(());
        }

        // PageUp/PageDown for scrolling (not passed to tmux)
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

        // Everything else passes through directly to the tmux pane
        if let Some((tmux_key, literal)) = Self::key_to_tmux(&key) {
            if let Some(swarm) = self.swarms.get(swarm_idx) {
                if let Some(agent) = swarm.agent(&agent_id) {
                    let target = agent.tmux_target.clone();
                    self.adapter.send_raw_key(&target, &tmux_key, literal).await?;
                }
            }
            self.agent_view.scroll_to_bottom();
        }
        Ok(())
    }

    /// Start pane watchers for all agents in all swarms.
    /// Send post-launch commands to manager and worker sessions.
    /// Spawns a background task that waits for sessions to initialize,
    /// Spawn a background task to fetch GitHub issues for a swarm.
    /// Jump to the next agent session that is waiting for user input.
    fn jump_to_next_waiting(&mut self, swarm_idx: usize) {
        // Get the current agent ID if we're in an agent view
        let current_id = match &self.screen {
            Screen::AgentView { agent_id, .. } => Some(agent_id.clone()),
            _ => None,
        };

        if let Some(swarm) = self.swarms.get(swarm_idx) {
            if let Some(agent) = swarm.next_waiting_agent(current_id.as_deref()) {
                let agent_id = agent.id.clone();
                self.agent_view = AgentView::new();
                self.agent_view.scroll_to_bottom();
                self.screen = Screen::AgentView {
                    swarm_idx,
                    agent_id,
                };
            } else {
                self.status_message = Some("No sessions waiting for input".to_string());
            }
        }
    }

    fn start_issue_refresh(&self, swarm_idx: usize) {
        if let Some(swarm) = self.swarms.get(swarm_idx) {
            let repo_path = swarm.repo_path.clone();
            let tx = self.events.tx();
            tokio::spawn(async move {
                match crate::model::issue::fetch_issues(&repo_path).await {
                    Ok(issues) => {
                        let _ = tx.send(Event::IssuesRefreshed { swarm_idx, issues });
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch issues: {e}");
                    }
                }
            });
        }
    }

    /// then sends `/autocoder:monitor-loop` to the manager and
    /// `/autocoder:fix-loop` to each worker.
    fn send_post_launch_commands(&self, swarm: &Swarm) {
        let manager_target = swarm.manager.tmux_target.clone();
        let worker_targets: Vec<String> = swarm.workers.iter().map(|w| w.tmux_target.clone()).collect();
        let plugin_installed = self.agents_dir.exists()
            && self.agents_dir.join("scripts/start-parallel-agents.sh").exists();

        tokio::spawn(async move {
            if !plugin_installed {
                tracing::warn!("Autocoder plugin not found; skipping post-launch commands");
                return;
            }

            // Wait for Claude sessions to initialize before sending commands
            tokio::time::sleep(Duration::from_secs(5)).await;

            // Send /autocoder:monitor-loop to manager
            if let Err(e) = proxy::send_keys(&manager_target, "/autocoder:monitor-loop").await {
                tracing::warn!("Failed to send /autocoder:monitor-loop to manager: {e}");
            } else {
                tracing::info!("Sent /autocoder:monitor-loop to manager at {manager_target}");
            }

            // Send /autocoder:fix-loop to each worker
            for target in &worker_targets {
                if let Err(e) = proxy::send_keys(target, "/autocoder:fix-loop").await {
                    tracing::warn!("Failed to send /autocoder:fix-loop to worker at {target}: {e}");
                } else {
                    tracing::info!("Sent /autocoder:fix-loop to worker at {target}");
                }
            }
        });
    }

    fn start_all_pane_watchers(&mut self) {
        // Cancel existing watchers
        for handle in self.pane_watchers.drain(..) {
            handle.abort();
        }

        let tx = self.events.tx();

        for swarm in &self.swarms {
            // Watch manager pane
            let handle = proxy::spawn_pane_watcher(
                swarm.manager.tmux_target.clone(),
                swarm.manager.id.clone(),
                tx.clone(),
                Duration::from_millis(500),
            );
            self.pane_watchers.push(handle);

            // Watch worker panes
            for worker in &swarm.workers {
                let handle = proxy::spawn_pane_watcher(
                    worker.tmux_target.clone(),
                    worker.id.clone(),
                    tx.clone(),
                    Duration::from_millis(500),
                );
                self.pane_watchers.push(handle);
            }
        }
    }

    /// Refresh agent statuses from status files and generate banners on state changes.
    fn refresh_statuses(&mut self) {
        use crate::model::status::AgentState;

        for swarm in &mut self.swarms {
            for worker in &mut swarm.workers {
                let status_file = worker
                    .worktree_path
                    .join(swarm.agent_type.status_dir())
                    .join("fix-loop.status");
                if status_file.exists() {
                    let old_state = worker.status.state.clone();
                    worker.status = crate::model::status::read_status_file(&status_file);
                    let new_state = &worker.status.state;

                    // Generate banners on state transitions
                    if old_state != *new_state {
                        let (msg, style) = match new_state {
                            AgentState::Working { issue: Some(n) } => (
                                format!("{} started working on #{n}", worker.id),
                                ratatui::style::Style::default()
                                    .fg(ratatui::style::Color::Green),
                            ),
                            AgentState::Idle => (
                                format!("{} is now idle", worker.id),
                                ratatui::style::Style::default()
                                    .fg(ratatui::style::Color::Yellow),
                            ),
                            AgentState::Completed { detail } => (
                                format!("{} completed: {detail}", worker.id),
                                ratatui::style::Style::default()
                                    .fg(ratatui::style::Color::Green)
                                    .add_modifier(ratatui::style::Modifier::BOLD),
                            ),
                            AgentState::Stopped => (
                                format!("{} stopped", worker.id),
                                ratatui::style::Style::default()
                                    .fg(ratatui::style::Color::Red),
                            ),
                            _ => continue,
                        };
                        self.repo_view.add_banner(msg, style);
                    }
                }
            }
        }
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
