use anyhow::{Context, Result};
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
    RuntimeSelection,
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
    /// Selected agent type during new swarm flow.
    pub new_swarm_agent_type: AgentType,
    /// Status message shown at bottom of repos list.
    pub status_message: Option<String>,
    /// Timestamp of last Esc press (for double-Esc detection).
    last_esc: Option<std::time::Instant>,
    /// Available repos (git directories found nearby) that don't have active swarms.
    pub available_repos: Vec<PathBuf>,
    /// Last time we auto-dispatched /monitor-workers (for debounce).
    auto_dispatch_last: Option<std::time::Instant>,
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
            new_swarm_agent_type: AgentType::Claude,
            status_message: None,
            last_esc: None,
            available_repos: Vec::new(),
            auto_dispatch_last: None,
        };

        // Scan for available repos (git directories in cwd or children)
        app.scan_available_repos();

        // Start pane watchers for discovered swarms
        app.start_all_pane_watchers();

        // Auto-select: if launched inside a repo that has a running swarm, jump straight into it
        if let Ok(cwd) = std::env::current_dir() {
            let cwd_name = cwd
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if let Some(idx) = app
                .swarms
                .iter()
                .position(|s| s.project_name == cwd_name)
            {
                app.repos_list.table_state.select(Some(idx));
                app.repo_view = RepoView::new();
                app.screen = Screen::RepoView { swarm_idx: idx };
            }
        }

        Ok(app)
    }

    pub async fn run(&mut self, terminal: &mut Tui) -> Result<()> {
        while self.running {
            // Render
            terminal.draw(|f| {
                let area = f.area();
                match &self.screen {
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
                        let agent_type = self.new_swarm_agent_type.clone();
                        crate::ui::new_swarm::render_new_swarm_dialog(
                            f, area, &field, &input, &repo, &agent_type,
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
                // Periodic refresh — status files could be re-read here
                self.refresh_statuses();
                // Auto-dispatch: send /monitor-workers to manager when workers are idle
                self.check_auto_dispatch().await;
            }
            Event::PaneOutput { agent_id, content } => {
                // Update the agent's pane content
                for swarm in &mut self.swarms {
                    if let Some(agent) = swarm.agent_mut(&agent_id) {
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
            Event::Error(msg) => {
                tracing::error!("Background error: {msg}");
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

                if swarm_idx < self.swarms.len() {
                    if c == '0' {
                        // Alt+0: go to Repo View
                        self.repo_view = RepoView::new();
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
                    if !repo_path.exists() {
                        self.status_message = Some(format!("Path not found: {path}"));
                        self.screen = Screen::ReposList;
                        return Ok(());
                    }

                    self.new_swarm_repo = path;
                    self.new_swarm_agent_type = AgentType::Claude;
                    self.screen = Screen::NewSwarm {
                        field: NewSwarmField::RuntimeSelection,
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
            NewSwarmField::RuntimeSelection => match key.code {
                KeyCode::Esc => {
                    self.screen = Screen::NewSwarm {
                        field: NewSwarmField::RepoPath,
                    };
                    self.dialog_input = self.new_swarm_repo.clone();
                }
                KeyCode::Enter => {
                    self.dialog_input = "2".to_string(); // Default 2 workers
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
                    let num_workers: u32 = self.dialog_input.parse().unwrap_or(2);
                    let repo_path = PathBuf::from(&self.new_swarm_repo);

                    self.screen = Screen::NewSwarm {
                        field: NewSwarmField::Launching,
                    };
                    self.dialog_input = String::new();

                    // Launch the swarm
                    let config = SwarmConfig {
                        repo_path: repo_path.clone(),
                        agent_type: self.new_swarm_agent_type.clone(),
                        num_workers,
                        agents_dir: self.agents_dir.clone(),
                    };

                    match self.adapter.launch(&config).await {
                        Ok(swarm) => {
                            let project = swarm.project_name.clone();
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
        if self.repo_view.focus_manager {
            // In manager chat mode
            match key.code {
                KeyCode::Esc => {
                    self.repo_view.focus_manager = false;
                }
                KeyCode::Enter => {
                    if !self.repo_view.input.is_empty() {
                        let input = self.repo_view.input.drain(..).collect::<String>();
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            let target = swarm.manager.tmux_target.clone();
                            self.adapter.send_input(&target, &input).await?;
                        }
                    }
                }
                KeyCode::Char(c) => {
                    self.repo_view.input.push(c);
                }
                KeyCode::Backspace => {
                    self.repo_view.input.pop();
                }
                _ => {}
            }
        } else {
            // Worker table focused
            match key.code {
                KeyCode::Char('q') => self.running = false,
                KeyCode::Esc => {
                    self.screen = Screen::ReposList;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        self.repo_view.next_worker(swarm.workers.len());
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        self.repo_view.previous_worker(swarm.workers.len());
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
                KeyCode::Char('a') => {
                    // Add a new worker to this swarm
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        let swarm_clone = swarm.clone();
                        self.status_message = Some("Adding worker...".to_string());
                        match self.adapter.add_worker(&swarm_clone).await {
                            Ok(worker) => {
                                let id = worker.id.clone();
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
                                let id = worker.id.clone();
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
                                let id = worker.id.clone();
                                tracing::info!("Shutting down worker {id} at {target}");
                                if let Err(e) = proxy::kill_pane(&target).await {
                                    tracing::error!("Failed to kill pane for {id}: {e}");
                                }
                                self.status_message =
                                    Some(format!("Shutting down {id}..."));
                            }
                        }
                    }
                }
                KeyCode::Char(c @ '1'..='9') => {
                    // Jump directly to worker by number (1=worker-0, 2=worker-1, etc.)
                    let worker_idx = (c as usize) - ('1' as usize);
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
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
                _ => {
                    // Alt+m → manager, Alt+1-9 → worker N
                    if key.modifiers.contains(KeyModifiers::ALT) {
                        match key.code {
                            KeyCode::Char('m') => {
                                self.agent_view = AgentView::new();
                                self.agent_view.scroll_to_bottom();
                                self.screen = Screen::AgentView {
                                    swarm_idx,
                                    agent_id: "manager".to_string(),
                                };
                            }
                            KeyCode::Char(c @ '1'..='9') => {
                                let worker_idx = (c as usize) - ('1' as usize);
                                if let Some(swarm) = self.swarms.get(swarm_idx) {
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
                            _ => {}
                        }
                    }
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

        // Double-Esc to go back (Esc is NEVER forwarded to worker panes — it
        // interrupts active Claude work and can break sessions)
        if key.code == KeyCode::Esc {
            let now = std::time::Instant::now();
            if let Some(last) = self.last_esc {
                if now.duration_since(last) < Duration::from_millis(500) {
                    self.last_esc = None;
                    self.screen = Screen::RepoView { swarm_idx };
                    return Ok(());
                }
            }
            self.last_esc = Some(now);
            return Ok(());
        }
        self.last_esc = None;

        // Ctrl+] goes back
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char(']') {
            self.screen = Screen::RepoView { swarm_idx };
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
                KeyCode::Char(c @ '1'..='9') => {
                    let worker_idx = (c as usize) - ('1' as usize);
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
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
                _ => {}
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
            send_raw_key(&target, &tmux_key).await?;
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

            // Send /monitor-workers to the manager pane
            let target = &swarm.manager.tmux_target;
            tracing::info!(
                "Auto-dispatching /monitor-workers to manager (idle worker detected in {})",
                swarm.project_name
            );
            if let Err(e) = crate::tmux::proxy::send_keys(target, "/monitor-workers").await {
                tracing::warn!("Failed to auto-dispatch /monitor-workers: {e}");
            }
            self.auto_dispatch_last = Some(std::time::Instant::now());
            self.status_message =
                Some("Auto-dispatched /monitor-workers (idle worker detected)".to_string());
            return; // Only dispatch once per tick
        }
    }

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
                if status_file.exists() {
                    agent.status = crate::model::status::read_status_file(&status_file);
                    continue;
                }

                // Infer status from pane content
                if !agent.pane_content.is_empty() {
                    agent.status = infer_status_from_pane(&agent.pane_content);
                }
            }
        }
    }

    /// Scan for git repos in cwd and child directories.
    fn scan_available_repos(&mut self) {
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
        // Never forward Escape to tmux panes — it interrupts Claude sessions.
        // Esc is handled separately as double-Esc for back navigation.
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
async fn send_raw_key(target: &str, tmux_key: &str) -> Result<()> {
    let output = tokio::process::Command::new("tmux")
        .args(["send-keys", "-t", target, tmux_key])
        .output()
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
