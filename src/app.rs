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
                // Periodic refresh — status files could be re-read here
                self.refresh_statuses();
            }
            Event::PaneOutput { agent_id, content } => {
                // Update the agent's pane content and infer status from it
                for swarm in &mut self.swarms {
                    if let Some(agent) = swarm.agent_mut(&agent_id) {
                        agent.pane_content = content.clone();
                        // Infer status from pane content for faster updates
                        if let Some(inferred) = infer_status_from_pane(&content) {
                            agent.status.state = inferred;
                        }
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

        // Global: F1 jumps to next agent needing attention
        if key.code == KeyCode::F(1) {
            if let Some((swarm_idx, agent_id)) = self.find_next_attention_agent() {
                self.enter_agent_view(swarm_idx, agent_id).await;
            }
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
                        self.switch_gh_auth_for_swarm(idx);
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
            KeyCode::Char('d') => {
                // Stop (teardown) selected swarm
                if let Some(idx) = self.repos_list.selected() {
                    if let Some(swarm) = self.swarms.get(idx) {
                        let project = swarm.project_name.clone();
                        let session = swarm.tmux_session.clone();
                        tracing::info!("Stopping swarm {project} (session {session})");

                        // Kill the tmux session
                        if let Err(e) = proxy::kill_session(&session).await {
                            tracing::warn!("Failed to kill session {session}: {e}");
                        }

                        self.swarms.remove(idx);
                        self.start_all_pane_watchers();
                        self.status_message = Some(format!("Stopped swarm for {project}"));

                        // Adjust selection
                        if !self.swarms.is_empty() {
                            let new_idx = idx.min(self.swarms.len() - 1);
                            self.repos_list.table_state.select(Some(new_idx));
                        }
                    }
                }
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
                            self.swarms.push(swarm);
                            self.start_all_pane_watchers();
                            let idx = self.swarms.len() - 1;
                            self.switch_gh_auth_for_swarm(idx);
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
        if self.repo_view.focus_workers {
            // Worker table focused (toggled via Tab)
            match key.code {
                KeyCode::Tab | KeyCode::Esc => {
                    self.repo_view.focus_workers = false;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        if self.repo_view.next_worker(swarm.workers.len()) {
                            self.repo_view.focus_workers = false;
                        }
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        if self.repo_view.previous_worker(swarm.workers.len()) {
                            self.repo_view.focus_workers = false;
                        }
                    }
                }
                KeyCode::Enter => {
                    // Drill into selected worker
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        if let Some(worker_idx) = self.repo_view.selected_worker() {
                            if let Some(worker) = swarm.workers.get(worker_idx) {
                                let id = worker.id.clone();
                                self.enter_agent_view(swarm_idx, id).await;
                            }
                        }
                    }
                }
                KeyCode::Char('d') => {
                    // Stop selected worker
                    let worker_info = self.repo_view.selected_worker().and_then(|worker_idx| {
                        self.swarms.get(swarm_idx).and_then(|swarm| {
                            swarm.workers.get(worker_idx).map(|w| {
                                (worker_idx, w.tmux_target.clone(), w.id.clone())
                            })
                        })
                    });

                    if let Some((worker_idx, target, id)) = worker_info {
                        tracing::info!("Stopping worker {id} (pane {target})");

                        if let Err(e) = proxy::kill_pane(&target).await {
                            tracing::warn!("Failed to kill pane for {id}: {e}");
                        }

                        if let Some(swarm) = self.swarms.get_mut(swarm_idx) {
                            swarm.workers.remove(worker_idx);
                            let remaining = swarm.workers.len();
                            self.start_all_pane_watchers();

                            if remaining > 0 {
                                let new_idx = worker_idx.min(remaining - 1);
                                self.repo_view.worker_table_state.select(Some(new_idx));
                            }
                        }
                    }
                }
                _ => {}
            }
        } else {
            // Manager session focused (default) — typing goes to input
            // Esc → back to repos list
            if key.code == KeyCode::Esc {
                self.screen = Screen::ReposList;
                return Ok(());
            }
            match key.code {
                KeyCode::Enter => {
                    if self.repo_view.input.is_empty() {
                        // Empty input → fullscreen manager view
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            let id = swarm.manager.id.clone();
                            self.enter_agent_view(swarm_idx, id).await;
                        }
                    } else {
                        let input = self.repo_view.input.drain(..).collect::<String>();
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            let target = swarm.manager.tmux_target.clone();
                            self.adapter.send_input(&target, &input).await?;
                        }
                        self.repo_view.scroll_manager_to_bottom();
                    }
                }
                KeyCode::Tab => {
                    self.repo_view.focus_workers = true;
                }
                KeyCode::PageUp => {
                    self.repo_view.scroll_manager_up(10);
                }
                KeyCode::PageDown => {
                    self.repo_view.scroll_manager_down(10);
                }
                KeyCode::F(5) => {
                    // Refresh: re-discover swarms and restart pane watchers
                    if let Ok(swarms) = self.adapter.discover(&self.agents_dir).await {
                        self.swarms = swarms;
                        self.start_all_pane_watchers();
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
        }
        Ok(())
    }

    async fn handle_agent_view_key(
        &mut self,
        key: KeyEvent,
        swarm_idx: usize,
        agent_id: String,
    ) -> Result<()> {
        // Esc → back to repo view
        if key.code == KeyCode::Esc {
            self.screen = Screen::RepoView { swarm_idx };
            return Ok(());
        }
        match key.code {
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
            KeyCode::PageUp => {
                self.agent_view.scroll_up(10);
            }
            KeyCode::PageDown => {
                self.agent_view.scroll_down(10);
            }
            KeyCode::F(5) => {
                // Refresh: re-discover swarms and restart pane watchers
                if let Ok(swarms) = self.adapter.discover(&self.agents_dir).await {
                    self.swarms = swarms;
                    self.start_all_pane_watchers();
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Switch gh CLI auth to match the repo owner for the given swarm.
    /// Runs in background to avoid blocking the UI.
    fn switch_gh_auth_for_swarm(&self, swarm_idx: usize) {
        if let Some(swarm) = self.swarms.get(swarm_idx) {
            let repo_path = swarm.repo_path.clone();
            tokio::spawn(async move {
                // Get repo owner
                let owner_output = tokio::process::Command::new("gh")
                    .args(["repo", "view", "--json", "owner", "--jq", ".owner.login"])
                    .current_dir(&repo_path)
                    .output()
                    .await;

                let owner = match owner_output {
                    Ok(out) if out.status.success() => {
                        String::from_utf8_lossy(&out.stdout).trim().to_string()
                    }
                    _ => return,
                };

                if owner.is_empty() {
                    return;
                }

                // Check current active user
                let status_output = tokio::process::Command::new("gh")
                    .args(["api", "user", "--jq", ".login"])
                    .output()
                    .await;

                let current_user = match status_output {
                    Ok(out) if out.status.success() => {
                        String::from_utf8_lossy(&out.stdout).trim().to_string()
                    }
                    _ => return,
                };

                if current_user != owner {
                    tracing::info!("Switching gh auth from {current_user} to {owner}");
                    let _ = tokio::process::Command::new("gh")
                        .args(["auth", "switch", "--user", &owner])
                        .output()
                        .await;
                }
            });
        }
    }

    /// Enter agent view for a given agent, resizing its tmux pane to fill the terminal.
    async fn enter_agent_view(&mut self, swarm_idx: usize, agent_id: String) {
        if let Some(swarm) = self.swarms.get(swarm_idx) {
            if let Some(agent) = swarm.agent(&agent_id) {
                let target = agent.tmux_target.clone();
                // Resize pane to current terminal size so captured content fills the view
                if let Ok((width, height)) = crossterm::terminal::size() {
                    if let Err(e) = proxy::resize_pane(&target, width, height).await {
                        tracing::warn!("Failed to resize pane {target}: {e}");
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

    /// Find the next agent needing attention across all swarms.
    /// Returns (swarm_idx, agent_id) if found.
    fn find_next_attention_agent(&self) -> Option<(usize, String)> {
        for (swarm_idx, swarm) in self.swarms.iter().enumerate() {
            // Check manager first
            if swarm.manager.needs_attention() {
                return Some((swarm_idx, swarm.manager.id.clone()));
            }
            // Then workers
            for worker in &swarm.workers {
                if worker.needs_attention() {
                    return Some((swarm_idx, worker.id.clone()));
                }
            }
        }
        None
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

    /// Refresh agent statuses from status files.
    fn refresh_statuses(&mut self) {
        for swarm in &mut self.swarms {
            // Refresh manager status
            let manager_status_file = swarm
                .manager
                .worktree_path
                .join(swarm.agent_type.status_dir())
                .join("fix-loop.status");
            if manager_status_file.exists() {
                swarm.manager.status =
                    crate::model::status::read_status_file(&manager_status_file);
            }

            // Refresh worker statuses
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

/// Infer agent status from pane content by scanning recent lines.
/// Returns Some(state) if a clear status indicator is found, None otherwise.
fn infer_status_from_pane(content: &str) -> Option<crate::model::status::AgentState> {
    use crate::model::status::AgentState;

    // Scan last 15 lines of pane content for status indicators
    for line in content.lines().rev().take(15) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_lowercase();

        if lower.contains("idle_no_work_available") {
            return Some(AgentState::Idle);
        }
        // Detect the Claude Code prompt (agent is idle, waiting for input)
        if trimmed.ends_with("> ") || trimmed == ">" {
            return Some(AgentState::Idle);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longest_common_prefix_single() {
        let input = vec!["hello".to_string()];
        assert_eq!(longest_common_prefix(&input), "hello");
    }

    #[test]
    fn longest_common_prefix_common() {
        let input = vec![
            "foobar".to_string(),
            "foobaz".to_string(),
            "foobot".to_string(),
        ];
        assert_eq!(longest_common_prefix(&input), "foob");
    }

    #[test]
    fn longest_common_prefix_no_common() {
        let input = vec!["abc".to_string(), "xyz".to_string()];
        assert_eq!(longest_common_prefix(&input), "");
    }

    #[test]
    fn longest_common_prefix_empty_list() {
        let input: Vec<String> = vec![];
        assert_eq!(longest_common_prefix(&input), "");
    }

    #[test]
    fn longest_common_prefix_identical() {
        let input = vec!["same".to_string(), "same".to_string()];
        assert_eq!(longest_common_prefix(&input), "same");
    }

    #[test]
    fn longest_common_prefix_one_empty() {
        let input = vec!["abc".to_string(), "".to_string()];
        assert_eq!(longest_common_prefix(&input), "");
    }

    #[test]
    fn infer_status_idle_no_work() {
        let content = "some output\nIDLE_NO_WORK_AVAILABLE\n";
        assert_eq!(
            infer_status_from_pane(content),
            Some(crate::model::status::AgentState::Idle)
        );
    }

    #[test]
    fn infer_status_prompt() {
        let content = "Done with task\n> ";
        assert_eq!(
            infer_status_from_pane(content),
            Some(crate::model::status::AgentState::Idle)
        );
    }

    #[test]
    fn infer_status_no_indicator() {
        let content = "Working on something\nReading files...";
        assert_eq!(infer_status_from_pane(content), None);
    }
}
