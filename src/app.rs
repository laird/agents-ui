use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::adapter::claude::ClaudeAdapter;
use crate::adapter::traits::{AgentRuntime, SwarmConfig};
use crate::event::{Event, EventHandler};
use crate::model::swarm::{AgentType, ALL_AGENT_TYPES, Swarm};
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
    AgentRuntime,
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
    /// Last time worker healing was run.
    last_heal: Instant,
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
            last_heal: Instant::now(),
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

                // Periodically heal worker infrastructure (every 30 seconds)
                if self.last_heal.elapsed() >= Duration::from_secs(30) {
                    self.last_heal = Instant::now();
                    self.heal_all_workers().await;
                }
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
            Event::TerminalResize { width, height } => {
                // Resize all tmux sessions to match the new terminal size
                for swarm in &self.swarms {
                    let session = swarm.tmux_session.clone();
                    let w = width;
                    let h = height;
                    tokio::spawn(async move {
                        if let Err(e) =
                            crate::tmux::session::resize_session(&session, w, h).await
                        {
                            tracing::warn!("Failed to resize session {session}: {e}");
                        }
                    });
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
                    self.new_swarm_agent_type = AgentType::Claude;
                    self.screen = Screen::NewSwarm {
                        field: NewSwarmField::AgentRuntime,
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
            NewSwarmField::AgentRuntime => match key.code {
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
            NewSwarmField::NumWorkers => match key.code {
                KeyCode::Esc => {
                    self.screen = Screen::NewSwarm {
                        field: NewSwarmField::AgentRuntime,
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
            // In manager interactive mode
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
                        self.repo_view.manager_scroll_to_bottom();
                    }
                }
                KeyCode::Up => {
                    self.repo_view.manager_scroll_up(1);
                }
                KeyCode::Down => {
                    self.repo_view.manager_scroll_down(1);
                }
                KeyCode::PageUp => {
                    self.repo_view.manager_page_up();
                }
                KeyCode::PageDown => {
                    self.repo_view.manager_page_down();
                }
                KeyCode::Home => {
                    self.repo_view.manager_scroll_to_top();
                }
                KeyCode::End => {
                    self.repo_view.manager_scroll_to_bottom();
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
                KeyCode::Char('m') => {
                    // Focus the manager panel (interactive mode with scrolling)
                    self.repo_view.focus_manager = true;
                    self.repo_view.manager_scroll_to_bottom();
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
                KeyCode::Char('R') => {
                    // Restart all idle workers (send fix-loop to each)
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
                        let loop_cmd = swarm.agent_type.worker_loop_cmd().to_string();
                        for (id, target) in idle_workers {
                            tracing::info!("Restarting idle worker {id}");
                            if let Err(e) = proxy::send_keys(&target, &loop_cmd).await {
                                tracing::error!("Failed to restart {id}: {e}");
                            }
                        }
                        self.status_message =
                            Some(format!("Restarted {count} idle worker(s)"));
                    }
                }
                KeyCode::Char('D') => {
                    // Stop all workers (send stop to each)
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        let workers: Vec<(String, String)> = swarm
                            .workers
                            .iter()
                            .map(|w| (w.id.clone(), w.tmux_target.clone()))
                            .collect();

                        let count = workers.len();
                        for (id, target) in workers {
                            tracing::info!("Stopping worker {id}");
                            if let Err(e) = proxy::kill_pane(&target).await {
                                tracing::error!("Failed to stop {id}: {e}");
                            }
                        }
                        self.status_message =
                            Some(format!("Stopping all {count} worker(s)..."));
                    }
                }
                _ => {}
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
        match key.code {
            KeyCode::Esc => {
                self.screen = Screen::RepoView { swarm_idx };
            }
            KeyCode::Enter => {
                if !self.agent_view.input.is_empty() {
                    let input = self.agent_view.input.drain(..).collect::<String>();
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        if let Some(agent) = swarm.agent(&agent_id) {
                            let target = agent.tmux_target.clone();
                            self.adapter.send_input(&target, &input).await?;
                        }
                    }
                    self.agent_view.scroll_to_bottom();
                }
            }
            KeyCode::Char(c) => {
                self.agent_view.input.push(c);
            }
            KeyCode::Backspace => {
                self.agent_view.input.pop();
            }
            KeyCode::Up => {
                self.agent_view.scroll_up(1);
            }
            KeyCode::Down => {
                self.agent_view.scroll_down(1);
            }
            KeyCode::PageUp => {
                self.agent_view.page_up();
            }
            KeyCode::PageDown => {
                self.agent_view.page_down();
            }
            KeyCode::Home => {
                self.agent_view.scroll_to_top();
            }
            KeyCode::End => {
                self.agent_view.scroll_to_bottom();
            }
            _ => {}
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

    /// Validate and heal all worker infrastructure across all swarms.
    async fn heal_all_workers(&mut self) {
        let mut any_repairs = false;
        let mut all_repairs = Vec::new();

        for i in 0..self.swarms.len() {
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
            self.status_message = Some(format!("Healed: {msg}"));
            self.start_all_pane_watchers();
        }
    }

    /// Refresh agent statuses from status files.
    fn refresh_statuses(&mut self) {
        for swarm in &mut self.swarms {
            for worker in &mut swarm.workers {
                let status_file = worker
                    .worktree_path
                    .join(swarm.agent_type.status_dir())
                    .join("fix-loop.status");
                if status_file.exists() {
                    worker.status = crate::model::status::read_status_file(&status_file);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lcp_empty_input() {
        assert_eq!(longest_common_prefix(&[]), "");
    }

    #[test]
    fn lcp_single_string() {
        assert_eq!(
            longest_common_prefix(&["hello".to_string()]),
            "hello"
        );
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
}
