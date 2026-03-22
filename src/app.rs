use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use std::path::PathBuf;
use std::time::Duration;

use crate::adapter::claude::ClaudeAdapter;
use crate::adapter::traits::{AgentRuntime, SwarmConfig};
use crate::config::keybindings::{Action, KeyBindings};
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
    pub keybindings: KeyBindings,
    pub show_help: bool,
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
        let keybindings = KeyBindings::load();

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
            keybindings,
            show_help: false,
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
            let show_help = self.show_help;
            let keybindings = self.keybindings.clone();
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
                // Render help overlay on top if active
                if show_help {
                    crate::ui::help_overlay::render_help_overlay(f, area, &keybindings);
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
                // Update the agent's pane content
                for swarm in &mut self.swarms {
                    if let Some(agent) = swarm.agent_mut(&agent_id) {
                        agent.pane_content = content;
                        break;
                    }
                }

                // Auto-scroll views that are following the bottom
                // AgentView: if viewing this agent and at the bottom, stay there
                if let Screen::AgentView { agent_id: ref viewing_id, .. } = self.screen {
                    if *viewing_id == agent_id && self.agent_view.is_at_bottom() {
                        self.agent_view.scroll_to_bottom();
                    }
                }
                // RepoView: if manager pane updated and manager scroll is at bottom
                if let Screen::RepoView { .. } = self.screen {
                    if agent_id == "manager" && self.repo_view.is_manager_at_bottom() {
                        self.repo_view.scroll_manager_to_bottom();
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
        // Global: Force quit (Ctrl+C) always quits
        if self.keybindings.matches(Action::ForceQuit, &key) {
            self.running = false;
            return Ok(());
        }

        // Help overlay toggle
        if self.show_help {
            if self.keybindings.matches(Action::ShowHelp, &key)
                || self.keybindings.matches(Action::Back, &key)
            {
                self.show_help = false;
            }
            return Ok(());
        }

        // Show help from any non-input screen
        if self.keybindings.matches(Action::ShowHelp, &key) {
            match &self.screen {
                Screen::NewSwarm { .. } => {} // Don't intercept in input mode
                Screen::AgentView { .. } => {} // Don't intercept in input mode
                Screen::RepoView { .. } => {
                    // Only show help when not in manager input mode
                    if self.repo_view.input.is_empty() {
                        self.show_help = true;
                        return Ok(());
                    }
                }
                _ => {
                    self.show_help = true;
                    return Ok(());
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
        let kb = &self.keybindings;
        if kb.matches(Action::Quit, &key) {
            self.running = false;
        } else if kb.matches(Action::MoveDown, &key) {
            self.repos_list.next(self.swarms.len());
        } else if kb.matches(Action::MoveUp, &key) {
            self.repos_list.previous(self.swarms.len());
        } else if kb.matches(Action::Select, &key) {
            if let Some(idx) = self.repos_list.selected() {
                if idx < self.swarms.len() {
                    self.repo_view = RepoView::new();
                    self.screen = Screen::RepoView { swarm_idx: idx };
                }
            }
        } else if kb.matches(Action::NewSwarm, &key) {
            // New swarm dialog — pre-fill with current directory
            self.dialog_input = std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            self.new_swarm_repo = String::new();
            self.status_message = None;
            self.screen = Screen::NewSwarm {
                field: NewSwarmField::RepoPath,
            };
        } else if kb.matches(Action::Refresh, &key) {
            // Refresh: re-discover swarms
            self.status_message = Some("Refreshing...".to_string());
            if let Ok(swarms) = self.adapter.discover(&self.agents_dir).await {
                self.swarms = swarms;
                self.start_all_pane_watchers();
                self.status_message = Some(format!("Found {} swarm(s)", self.swarms.len()));
            }
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
        let kb = &self.keybindings;

        match &self.repo_view.focus {
            RepoViewFocus::Manager => {
                // Manager session focused — typing goes to input, scrolling works
                if kb.matches(Action::Back, &key) {
                    if !self.repo_view.input.is_empty() {
                        self.repo_view.input.clear();
                    } else {
                        self.screen = Screen::ReposList;
                    }
                } else if kb.matches(Action::MoveDown, &key) || key.code == KeyCode::Tab {
                    self.repo_view.focus_workers();
                } else if kb.matches(Action::Select, &key) {
                    if !self.repo_view.input.is_empty() {
                        let input = self.repo_view.input.drain(..).collect::<String>();
                        if let Some(swarm) = self.swarms.get(swarm_idx) {
                            let target = swarm.manager.tmux_target.clone();
                            self.adapter.send_input(&target, &input).await?;
                        }
                        self.repo_view.scroll_manager_to_bottom();
                    }
                } else if kb.matches(Action::Fullscreen, &key) {
                    self.agent_view = AgentView::new();
                    self.agent_view.scroll_to_bottom();
                    self.screen = Screen::AgentView {
                        swarm_idx,
                        agent_id: "manager".to_string(),
                    };
                } else if kb.matches(Action::ScrollUp, &key) {
                    self.repo_view.scroll_manager_up(10);
                } else if kb.matches(Action::ScrollDown, &key) {
                    self.repo_view.scroll_manager_down(10);
                } else if kb.matches(Action::Quit, &key) {
                    self.running = false;
                } else {
                    match key.code {
                        KeyCode::Char(c) => {
                            self.repo_view.input.push(c);
                        }
                        KeyCode::Backspace => {
                            self.repo_view.input.pop();
                        }
                        _ => {}
                    }
                }
            }
            RepoViewFocus::Workers => {
                if kb.matches(Action::Quit, &key) {
                    self.running = false;
                } else if kb.matches(Action::Back, &key) {
                    self.screen = Screen::ReposList;
                } else if kb.matches(Action::MoveDown, &key) {
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        self.repo_view.next_worker(swarm.workers.len());
                    }
                } else if kb.matches(Action::MoveUp, &key) {
                    if let Some(swarm) = self.swarms.get(swarm_idx) {
                        let at_top = self.repo_view.previous_worker(swarm.workers.len());
                        if at_top {
                            self.repo_view.focus_manager();
                        }
                    }
                } else if kb.matches(Action::Select, &key) {
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
                } else if kb.matches(Action::FocusManager, &key) {
                    self.repo_view.focus_manager();
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
            KeyCode::PageUp => {
                self.agent_view.scroll_up(10);
            }
            KeyCode::PageDown => {
                self.agent_view.scroll_down(10);
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
