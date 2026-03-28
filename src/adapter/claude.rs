use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::model::status::{self, AgentStatus};
use crate::model::swarm::{AgentInfo, AgentType, Swarm};
use crate::tmux::{proxy, session};
use crate::transport::ServerTransport;

use super::traits::{AgentRuntime, SwarmConfig};

#[derive(Clone)]
pub struct ClaudeAdapter {
    runtime: AgentType,
    transport: ServerTransport,
}

impl ClaudeAdapter {
    pub fn new(runtime: AgentType, transport: ServerTransport) -> Self {
        Self { runtime, transport }
    }

    async fn output(
        &self,
        program: &str,
        args: &[String],
        current_dir: Option<&Path>,
    ) -> Result<std::process::Output> {
        self.transport.output(program, args, current_dir).await
    }

    /// Launch a swarm with progress reporting via a callback.
    pub async fn launch_with_progress<F: Fn(&str)>(
        &self,
        config: &SwarmConfig,
        progress: F,
    ) -> Result<Swarm> {
        let runtime = &config.agent_type;
        let project_name = Self::project_name(&config.repo_path);
        let session_name = Self::session_name(runtime, &project_name);

        // Clear any stopped tombstone — the user is explicitly launching this swarm.
        crate::config::persistence::clear_swarm_stopped(&project_name);

        if session::has_session(&self.transport, &session_name).await {
            progress("♻️  Found existing session, reconnecting...\n");
            let swarm = self
                .build_swarm_from_session(&session_name, config.repo_path.clone(), runtime.clone())
                .await?;
            self.ensure_swarm_agents_running(&swarm).await?;
            return Ok(swarm);
        }

        if matches!(runtime, AgentType::Claude) {
            progress("⏳ Checking autocoder plugin...\n");
            self.ensure_plugin_installed().await?;
            progress("✅ Plugin ready\n");
        }

        progress(&format!(
            "⏳ Creating {} git worktrees...\n",
            config.num_workers
        ));
        let worktree_paths = self
            .create_worktrees(&config.repo_path, &project_name, config.num_workers)
            .await?;
        progress(&format!("✅ Created {} worktrees\n", worktree_paths.len()));

        progress("⏳ Creating tmux session...\n");
        self.create_tmux_session(&session_name, &config.repo_path, &worktree_paths)
            .await?;
        progress(&format!("✅ tmux session: {session_name}\n"));

        for (i, _wt) in worktree_paths.iter().enumerate() {
            let n = i + 1; // 1-indexed to match worktree naming (wt-1, wt-2, ...)
            progress(&format!("⏳ Starting worker-{n}...\n"));
            let target = format!("{session_name}:worker-{n}.0");
            if let Err(e) = self
                .launch_agent_in_pane(&target, &session_name, runtime)
                .await
            {
                progress(&format!("⚠️  worker-{n} failed: {e}\n"));
            } else {
                progress(&format!("✅ worker-{n} started\n"));
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        progress("⏳ Starting manager...\n");
        let manager_target = format!("{session_name}:review.0");
        if let Err(e) = self
            .launch_agent_in_pane(&manager_target, &session_name, runtime)
            .await
        {
            progress(&format!("⚠️  Manager failed: {e}\n"));
        } else {
            progress("✅ Manager started\n");
            if let Some(cmd) = manager_bootstrap_cmd(runtime) {
                progress("⏳ Waiting for manager to be ready...\n");
                if Self::wait_for_claude_ready(&self.transport, &manager_target).await {
                    progress(&format!("⏳ Sending {cmd} to manager...\n"));
                    proxy::send_keys(&self.transport, &manager_target, &cmd)
                        .await
                        .ok();
                    progress("✅ Manager running manage-loop\n");
                } else {
                    progress("⚠️  Manager not ready in time, manage-loop not started\n");
                }
            }
        }

        progress("\n🎉 Swarm launched!\n");

        let swarm = self
            .build_swarm_from_session(&session_name, config.repo_path.clone(), runtime.clone())
            .await?;

        // Start worker fix-loops after agents have had time to initialize
        let loop_cmd = runtime.worker_loop_cmd().to_string();
        if !loop_cmd.is_empty() {
            let transport = self.transport.clone();
            let targets: Vec<String> = swarm.workers.iter().map(|w| w.tmux_target.clone()).collect();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                for target in &targets {
                    if let Err(e) = proxy::send_keys(&transport, target, &loop_cmd).await {
                        tracing::warn!("Failed to send {loop_cmd} to {target}: {e}");
                    } else {
                        tracing::info!("Sent {loop_cmd} to worker at {target}");
                    }
                }
            });
        }

        progress("✅ Workers will start fix-loop after initialization\n");

        Ok(swarm)
    }

    /// Derive the project name from a repo path (last directory component).
    fn project_name(repo_path: &Path) -> String {
        repo_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// Expected tmux session name for a given project.
    fn session_name(agent_type: &AgentType, project: &str) -> String {
        format!("{}-{project}", agent_type.session_prefix())
    }

    /// Create git worktrees for workers.
    async fn create_worktrees(
        &self,
        repo_path: &Path,
        project_name: &str,
        num_workers: u32,
    ) -> Result<Vec<PathBuf>> {
        let parent = repo_path.parent().unwrap_or(repo_path);
        let mut worktree_paths = Vec::new();

        for i in 1..=num_workers {
            let wt_path = parent.join(format!("{project_name}-wt-{i}"));
            let branch_name = format!("worker-{i}");

            if wt_path.exists() {
                tracing::info!("Worktree already exists: {}", wt_path.display());
                worktree_paths.push(wt_path);
                continue;
            }

            // Create a branch for this worker
            let output = self
                .output(
                    "git",
                    &["branch".to_string(), branch_name.clone()],
                    Some(repo_path),
                )
                .await
                .context("Failed to create branch")?;
            // Ignore errors (branch may already exist)
            if !output.status.success() {
                tracing::info!("Branch {branch_name} may already exist, continuing");
            }

            // Create the worktree
            let output = self
                .output(
                    "git",
                    &[
                        "worktree".to_string(),
                        "add".to_string(),
                        wt_path.to_string_lossy().to_string(),
                        branch_name.clone(),
                    ],
                    Some(repo_path),
                )
                .await
                .context("Failed to create worktree")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // If worktree already exists for this branch, try without branch
                if stderr.contains("already checked out") || stderr.contains("already exists") {
                    tracing::warn!("Worktree issue for {branch_name}: {stderr}");
                    // Try with a detached head
                    let output2 = self
                        .output(
                            "git",
                            &[
                                "worktree".to_string(),
                                "add".to_string(),
                                "--detach".to_string(),
                                wt_path.to_string_lossy().to_string(),
                            ],
                            Some(repo_path),
                        )
                        .await?;
                    if !output2.status.success() {
                        anyhow::bail!(
                            "Failed to create worktree at {}: {}",
                            wt_path.display(),
                            String::from_utf8_lossy(&output2.stderr)
                        );
                    }
                } else {
                    anyhow::bail!("Failed to create worktree: {stderr}");
                }
            }

            worktree_paths.push(wt_path);
        }

        Ok(worktree_paths)
    }

    /// Create a tmux session with each agent in its own full-width window.
    /// Layout: window 0 = "review" (manager), windows 1..N = "worker-N" (one per worker)
    async fn create_tmux_session(
        &self,
        session_name: &str,
        repo_path: &Path,
        worktree_paths: &[PathBuf],
    ) -> Result<()> {
        // Create session with first window "review" (manager) in the base repo
        let output = self
            .output(
                "tmux",
                &[
                    "new-session".to_string(),
                    "-d".to_string(),
                    "-s".to_string(),
                    session_name.to_string(),
                    "-n".to_string(),
                    "review".to_string(),
                    "-c".to_string(),
                    repo_path.to_string_lossy().to_string(),
                ],
                None,
            )
            .await
            .context("Failed to create tmux session")?;

        if !output.status.success() {
            anyhow::bail!(
                "tmux new-session failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Set environment variables to force tmux mode
        self.output(
            "tmux",
            &[
                "set-environment".to_string(),
                "-t".to_string(),
                session_name.to_string(),
                "AGENTS_MUX".to_string(),
                "tmux".to_string(),
            ],
            None,
        )
        .await
        .ok();
        self.output(
            "tmux",
            &[
                "set-environment".to_string(),
                "-t".to_string(),
                session_name.to_string(),
                "MUX".to_string(),
                "tmux".to_string(),
            ],
            None,
        )
        .await
        .ok();

        // Create one window per worker — each gets full terminal width (1-indexed)
        for (i, wt_path) in worktree_paths.iter().enumerate() {
            let window_name = format!("worker-{}", i + 1);
            let output = self
                .output(
                    "tmux",
                    &[
                        "new-window".to_string(),
                        "-t".to_string(),
                        session_name.to_string(),
                        "-n".to_string(),
                        window_name.clone(),
                        "-c".to_string(),
                        wt_path.to_string_lossy().to_string(),
                    ],
                    None,
                )
                .await?;
            if !output.status.success() {
                tracing::warn!(
                    "Failed to create window for worker {i}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        // Select the review window as default
        self.output(
            "tmux",
            &[
                "select-window".to_string(),
                "-t".to_string(),
                format!("{session_name}:review"),
            ],
            None,
        )
        .await
        .ok();

        Ok(())
    }

    /// Launch runtime agent in a specific tmux pane.
    async fn launch_agent_in_pane(
        &self,
        target: &str,
        session_name: &str,
        runtime: &AgentType,
    ) -> Result<()> {
        let cmd = match runtime {
            AgentType::Claude => format!(
                "claude --dangerously-skip-permissions --append-system-prompt 'This session is managed by agents-ui via tmux. \
                IMPORTANT: Always use tmux commands (tmux capture-pane, tmux send-keys, etc.) \
                for reading worker screens and dispatching work. Do NOT use cmux. \
                The tmux session is named {session_name}.'"
            ),
            _ => runtime.launch_cmd().to_string(),
        };
        proxy::send_keys(&self.transport, target, &cmd).await
    }

    async fn ensure_swarm_agents_running(&self, swarm: &Swarm) -> Result<()> {
        if swarm.stopped || crate::config::persistence::is_swarm_stopped(&swarm.project_name) {
            tracing::info!("Skipping ensure_swarm_agents_running for stopped swarm {}", swarm.project_name);
            return Ok(());
        }

        self.ensure_agent_running(&swarm.manager, &swarm.tmux_session, &swarm.agent_type)
            .await?;

        for worker in &swarm.workers {
            self.ensure_agent_running(worker, &swarm.tmux_session, &swarm.agent_type)
                .await?;
        }

        Ok(())
    }

    async fn ensure_agent_running(
        &self,
        agent: &AgentInfo,
        session_name: &str,
        runtime: &AgentType,
    ) -> Result<()> {
        let content = proxy::capture_pane(&self.transport, &agent.tmux_target, 80)
            .await
            .unwrap_or_default();

        match classify_pane_state(&content) {
            PaneState::NeedsLaunch => {
                tracing::info!("Launching {} in existing pane {}", runtime, agent.tmux_target);
                self.launch_agent_in_pane(&agent.tmux_target, session_name, runtime)
                    .await?;
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                if let Some(cmd) = self.bootstrap_command(runtime, agent).await {
                    tracing::info!("Sending bootstrap command to {}: {}", agent.id, cmd);
                    proxy::send_keys(&self.transport, &agent.tmux_target, &cmd).await?;
                }
            }
            PaneState::AgentIdle => {
                if let Some(cmd) = self.bootstrap_command(runtime, agent).await {
                    tracing::info!("Agent {} is idle, sending bootstrap: {}", agent.id, cmd);
                    proxy::send_keys(&self.transport, &agent.tmux_target, &cmd).await?;
                }
            }
            PaneState::AgentBusy => {
                tracing::info!("Agent {} already active in {}", agent.id, agent.tmux_target);
            }
            PaneState::Unknown => {
                // Can't tell from pane content — probe by sending Enter and re-reading
                tracing::info!("Probing pane {} for {}", agent.tmux_target, agent.id);
                proxy::send_keys_no_enter(&self.transport, &agent.tmux_target, "").await.ok();
                // Send a bare Enter to elicit a prompt
                self.transport.output(
                    "tmux",
                    &[
                        "send-keys".to_string(),
                        "-t".to_string(),
                        agent.tmux_target.clone(),
                        "Enter".to_string(),
                    ],
                    None,
                ).await.ok();
                tokio::time::sleep(std::time::Duration::from_millis(800)).await;

                let probed = proxy::capture_pane(&self.transport, &agent.tmux_target, 80)
                    .await
                    .unwrap_or_default();

                match classify_pane_state(&probed) {
                    PaneState::NeedsLaunch => {
                        tracing::info!("Probe: shell detected in {}, launching {}", agent.tmux_target, runtime);
                        self.launch_agent_in_pane(&agent.tmux_target, session_name, runtime)
                            .await?;
                        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                        if let Some(cmd) = self.bootstrap_command(runtime, agent).await {
                            tracing::info!("Sending bootstrap command to {}: {}", agent.id, cmd);
                            proxy::send_keys(&self.transport, &agent.tmux_target, &cmd).await?;
                        }
                    }
                    PaneState::AgentIdle => {
                        if let Some(cmd) = self.bootstrap_command(runtime, agent).await {
                            tracing::info!("Probe: agent {} is idle, sending bootstrap: {}", agent.id, cmd);
                            proxy::send_keys(&self.transport, &agent.tmux_target, &cmd).await?;
                        }
                    }
                    _ => {
                        tracing::info!("Probe: agent {} state unclear, leaving alone", agent.id);
                    }
                }
            }
        }
        Ok(())
    }

    /// Poll a tmux pane until Claude's prompt indicator appears, or timeout.
    /// Returns true if the prompt was detected, false on timeout.
    async fn wait_for_claude_ready(transport: &crate::transport::ServerTransport, target: &str) -> bool {
        let timeout = std::time::Duration::from_secs(60);
        let poll_interval = std::time::Duration::from_secs(2);
        let start = std::time::Instant::now();

        // Wait a minimum of 5 seconds before polling
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        while start.elapsed() < timeout {
            if let Ok(content) = proxy::capture_pane(transport, target, 50).await {
                // Claude Code shows a "❯" or ">" prompt when ready
                // Also check for the tips/help text that appears on startup
                if content.contains('❯')
                    || content.contains("What can I help")
                    || content.contains("/help")
                {
                    tracing::info!("Claude ready in pane {target}");
                    return true;
                }
            }
            tokio::time::sleep(poll_interval).await;
        }

        tracing::warn!("Timed out waiting for Claude ready in pane {target}");
        false
    }

    /// Build a Swarm model from an existing tmux session.
    async fn build_swarm_from_session(
        &self,
        session_name: &str,
        repo_path: PathBuf,
        agent_type: AgentType,
    ) -> Result<Swarm> {
        let project_name = Self::project_name(&repo_path);
        let session_info = session::list_panes(&self.transport, session_name).await?;

        // Convention:
        // Window 0 ("review"): manager in base repo
        // Windows 1..N ("worker-N"): one worker per window, each full-width
        let mut manager = AgentInfo {
            id: format!("{project_name}/manager"),
            role: "manager".to_string(),
            worktree_path: repo_path.clone(),
            tmux_target: format!("{session_name}:0.0"),
            status: AgentStatus::default(),
            is_manager: true,
            pane_content: String::new(),
            dispatched_issue: None,
            current_issue: None,
            current_issue_title: None,
            waiting_for_input: false,
        };

        let mut workers = Vec::new();

        // First pass: identify the manager pane (window named "review" or index 0)
        for window in &session_info.windows {
            if window.name == "review" || window.index == 0 {
                if let Some(pane) = window.panes.first() {
                    manager.tmux_target = pane.target.clone();
                }
            }
        }

        // Second pass: all windows NOT named "review" or "tester" are workers
        let mut worker_num = 1usize; // 1-indexed to match worktree naming
        for window in &session_info.windows {
            if window.name == "review"
                || window.name == "tester"
                || (window.index == 0 && !window.name.starts_with("worker-"))
            {
                continue; // Skip non-worker windows
            }
            for pane in &window.panes {
                let worktree_path = repo_path
                    .parent()
                    .unwrap_or(&repo_path)
                    .join(format!("{}-wt-{}", project_name, worker_num));

                let status_file = worktree_path
                    .join(agent_type.status_dir())
                    .join("fix-loop.status");

                let agent_status = status::read_status_file(&status_file);

                let role = format!("worker-{worker_num}");
                workers.push(AgentInfo {
                    id: format!("{project_name}/{role}"),
                    role,
                    worktree_path,
                    tmux_target: pane.target.clone(),
                    status: agent_status,
                    is_manager: false,
                    pane_content: String::new(),
                    dispatched_issue: None,
                    current_issue: None,
                    current_issue_title: None,
                    waiting_for_input: false,
                });
                worker_num += 1;
            }
        }

        // Sort workers by index
        workers.sort_by_key(|w| {
            w.role.strip_prefix("worker-")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0)
        });

        let stopped = crate::config::persistence::is_swarm_stopped(&project_name);
        Ok(Swarm {
            repo_path,
            project_name,
            agent_type,
            workflow: None, // Can't determine from session alone
            tmux_session: session_name.to_string(),
            manager,
            workers,
            issue_cache: Default::default(),
            stopped,
        })
    }

    /// Check if the autocoder plugin is installed, and install it if not.
    async fn ensure_plugin_installed(&self) -> Result<()> {
        // Check if autocoder plugin is already installed
        let output = self
            .output("claude", &["plugin".to_string(), "list".to_string()], None)
            .await
            .context("Failed to run 'claude plugin list'. Is claude CLI installed?")?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        if stdout.contains("autocoder") {
            tracing::info!("Autocoder plugin already installed");
            return Ok(());
        }

        tracing::info!("Autocoder plugin not found, installing...");

        // Step 1: Add the laird/agents marketplace (if not already added)
        let marketplace_output = self
            .output(
                "claude",
                &[
                    "plugin".to_string(),
                    "marketplace".to_string(),
                    "list".to_string(),
                ],
                None,
            )
            .await?;

        let marketplace_stdout = String::from_utf8_lossy(&marketplace_output.stdout);

        if !marketplace_stdout.contains("laird/agents")
            && !marketplace_stdout.contains("plugin-marketplace")
        {
            tracing::info!("Adding laird/agents marketplace...");
            let add_output = self
                .output(
                    "claude",
                    &[
                        "plugin".to_string(),
                        "marketplace".to_string(),
                        "add".to_string(),
                        "laird/agents".to_string(),
                    ],
                    None,
                )
                .await
                .context("Failed to add marketplace")?;

            if !add_output.status.success() {
                let stderr = String::from_utf8_lossy(&add_output.stderr);
                // Not fatal if it already exists
                if !stderr.contains("already") {
                    tracing::warn!("Marketplace add warning: {stderr}");
                }
            }
        }

        // Step 2: Install the autocoder plugin
        tracing::info!("Installing autocoder plugin...");
        let install_output = self
            .output(
                "claude",
                &[
                    "plugin".to_string(),
                    "install".to_string(),
                    "autocoder".to_string(),
                ],
                None,
            )
            .await
            .context("Failed to install autocoder plugin")?;

        if !install_output.status.success() {
            let stderr = String::from_utf8_lossy(&install_output.stderr);
            anyhow::bail!("Failed to install autocoder plugin: {stderr}");
        }

        tracing::info!("Autocoder plugin installed successfully");
        Ok(())
    }

    /// Remove worktrees for a swarm.
    async fn remove_worktrees(&self, repo_path: &Path, worktree_paths: &[PathBuf]) -> Result<()> {
        for wt in worktree_paths {
            let output = self
                .output(
                    "git",
                    &[
                        "worktree".to_string(),
                        "remove".to_string(),
                        "--force".to_string(),
                        wt.to_string_lossy().to_string(),
                    ],
                    Some(repo_path),
                )
                .await?;
            if !output.status.success() {
                tracing::warn!(
                    "Failed to remove worktree {}: {}",
                    wt.display(),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
        Ok(())
    }
}

impl AgentRuntime for ClaudeAdapter {
    async fn launch(&self, config: &SwarmConfig) -> Result<Swarm> {
        let runtime = &config.agent_type;
        let project_name = Self::project_name(&config.repo_path);
        let session_name = Self::session_name(runtime, &project_name);

        // Clear any stopped tombstone — the user is explicitly launching this swarm.
        crate::config::persistence::clear_swarm_stopped(&project_name);

        // Check if session already exists
        if session::has_session(&self.transport, &session_name).await {
            tracing::info!("♻️  Found existing session, reconnecting...\n");
            return self
                .build_swarm_from_session(&session_name, config.repo_path.clone(), runtime.clone())
                .await;
        }

        if matches!(runtime, AgentType::Claude) {
            // Ensure the autocoder plugin is installed
            tracing::info!("⏳ Checking autocoder plugin...\n");
            self.ensure_plugin_installed().await?;
            tracing::info!("✅ Plugin ready\n");
        }

        tracing::info!(
            "Launching swarm: {} workers for {}",
            config.num_workers,
            project_name,
        );

        // Ensure gh is authenticated as the correct user for this repo
        ensure_gh_auth_for_repo(&config.repo_path).await;

        // 1. Create git worktrees
        tracing::info!("Creating {} git worktrees", config.num_workers);
        let worktree_paths = self
            .create_worktrees(&config.repo_path, &project_name, config.num_workers)
            .await?;
        tracing::info!("Created {} worktrees", worktree_paths.len());

        // 2. Create tmux session
        tracing::info!("⏳ Creating tmux session...\n");
        self.create_tmux_session(&session_name, &config.repo_path, &worktree_paths)
            .await?;
        tracing::info!("tmux session: {session_name}");

        // 3. Launch claude in each worker window (1-indexed)
        for i in 0..worktree_paths.len() {
            let n = i + 1;
            tracing::info!("Starting worker-{n}");
            let target = format!("{session_name}:worker-{n}.0");
            if let Err(e) = self
                .launch_agent_in_pane(&target, &session_name, runtime)
                .await
            {
                tracing::warn!("Worker {i} launch failed: {e}");
                tracing::warn!("Failed to launch claude in pane {i}: {e}");
            } else {
                tracing::info!("worker-{i} started");
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        // 4. Launch claude in manager pane
        tracing::info!("⏳ Starting manager...\n");
        let manager_target = format!("{session_name}:review.0");
        if let Err(e) = self
            .launch_agent_in_pane(&manager_target, &session_name, runtime)
            .await
        {
            tracing::warn!("Manager launch failed: {e}");
            tracing::warn!("Failed to launch claude in manager pane: {e}");
        } else {
            tracing::info!("✅ Manager started\n");
            // Wait for Claude to be ready, then send the manage-loop bootstrap command
            if let Some(cmd) = manager_bootstrap_cmd(runtime) {
                if Self::wait_for_claude_ready(&self.transport, &manager_target).await {
                    tracing::info!("Sending manage-loop to manager: {cmd}");
                    proxy::send_keys(&self.transport, &manager_target, &cmd)
                        .await
                        .ok();
                } else {
                    tracing::warn!("Manager not ready in time, skipping manage-loop bootstrap");
                }
            }
        }

        tracing::info!("\n🎉 Swarm launched! Waiting for sessions to initialize...\n");

        // Resize the tmux session to match the current terminal size
        if let Err(e) = session::resize_session_to_terminal(&session_name).await {
            tracing::warn!("Failed to resize session {session_name}: {e}");
        }

        // 5. Build swarm model
        let swarm = self
            .build_swarm_from_session(&session_name, config.repo_path.clone(), runtime.clone())
            .await?;

        // Start worker fix-loops after agents have had time to initialize
        let loop_cmd = runtime.worker_loop_cmd().to_string();
        if !loop_cmd.is_empty() {
            let transport = self.transport.clone();
            let targets: Vec<String> = swarm.workers.iter().map(|w| w.tmux_target.clone()).collect();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                for target in &targets {
                    if let Err(e) = proxy::send_keys(&transport, target, &loop_cmd).await {
                        tracing::warn!("Failed to send {loop_cmd} to {target}: {e}");
                    } else {
                        tracing::info!("Sent {loop_cmd} to worker at {target}");
                    }
                }
            });
        }

        Ok(swarm)
    }

    async fn discover(&self, _agents_dir: &Path) -> Result<Vec<Swarm>> {
        let sessions = session::discover_agent_sessions(&self.transport).await?;
        let mut swarms = Vec::new();

        for session_name in sessions {
            // Infer agent type and project name from session name prefix
            let (agent_type, project_name) = if let Some(rest) = session_name.strip_prefix("claude-") {
                (AgentType::Claude, rest.to_string())
            } else if let Some(rest) = session_name.strip_prefix("codex-") {
                (AgentType::Codex, rest.to_string())
            } else if let Some(rest) = session_name.strip_prefix("droid-") {
                (AgentType::Droid, rest.to_string())
            } else if let Some(rest) = session_name.strip_prefix("gemini-") {
                (AgentType::Gemini, rest.to_string())
            } else {
                continue;
            };

            let repo_path = find_repo_path(&self.transport, &session_name, &project_name).await;

            if let Some(repo_path) = repo_path {
                // Ensure gh is authenticated as the correct user for this repo
                ensure_gh_auth_for_repo(&repo_path).await;

                // Resize discovered session to match current terminal
                if let Err(e) = session::resize_session_to_terminal(&session_name).await {
                    tracing::warn!("Failed to resize session {session_name}: {e}");
                }

                match self
                    .build_swarm_from_session(&session_name, repo_path, agent_type)
                    .await
                {
                    Ok(swarm) => {
                        if let Err(e) = self.ensure_swarm_agents_running(&swarm).await {
                            tracing::warn!("Failed to start agents for {session_name}: {e}");
                        }
                        swarms.push(swarm)
                    }
                    Err(e) => tracing::warn!("Failed to build swarm from {session_name}: {e}"),
                }
            } else {
                tracing::warn!("Could not determine repo path for project {project_name}");
            }
        }

        Ok(swarms)
    }

    async fn send_input(&self, tmux_target: &str, input: &str) -> Result<()> {
        proxy::send_keys(&self.transport, tmux_target, input).await
    }

    async fn send_raw_key(&self, tmux_target: &str, key: &str, literal: bool) -> Result<()> {
        if literal {
            proxy::send_literal(tmux_target, key).await
        } else {
            proxy::send_named_key(tmux_target, key).await
        }
    }

    async fn capture_output(&self, tmux_target: &str) -> Result<String> {
        proxy::capture_pane(&self.transport, tmux_target, 500).await
    }

    async fn add_worker(&self, swarm: &Swarm) -> Result<AgentInfo> {
        let next_num = swarm.workers.len() + 1; // 1-indexed
        let project_name = &swarm.project_name;
        let repo_path = &swarm.repo_path;
        let session_name = &swarm.tmux_session;

        // Create a git worktree for the new worker
        let worktree_path = repo_path
            .parent()
            .unwrap_or(repo_path)
            .join(format!("{project_name}-wt-{next_num}"));

        let current_branch = self
            .output(
                "git",
                &[
                    "rev-parse".to_string(),
                    "--abbrev-ref".to_string(),
                    "HEAD".to_string(),
                ],
                Some(repo_path),
            )
            .await
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "main".to_string());

        let worktree_branch = format!("{current_branch}-wt-{next_num}");

        if !worktree_path.exists() {
            // Create branch if it doesn't exist
            let _ = self
                .output(
                    "git",
                    &[
                        "branch".to_string(),
                        worktree_branch.clone(),
                        current_branch.clone(),
                    ],
                    Some(repo_path),
                )
                .await;

            let output = self
                .output(
                    "git",
                    &[
                        "worktree".to_string(),
                        "add".to_string(),
                        worktree_path.to_string_lossy().to_string(),
                        worktree_branch.clone(),
                    ],
                    Some(repo_path),
                )
                .await
                .context("Failed to create git worktree")?;

            if !output.status.success() {
                anyhow::bail!(
                    "git worktree add failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        // Create a new tmux window for this worker (full terminal width)
        let window_name = format!("worker-{next_num}");
        let output = self
            .output(
                "tmux",
                &[
                    "new-window".to_string(),
                    "-t".to_string(),
                    session_name.to_string(),
                    "-n".to_string(),
                    window_name.clone(),
                    "-c".to_string(),
                    worktree_path.to_string_lossy().to_string(),
                ],
                None,
            )
            .await
            .context("Failed to create tmux window")?;

        if !output.status.success() {
            anyhow::bail!(
                "tmux new-window failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let tmux_target = format!("{session_name}:{window_name}.0");

        proxy::send_keys(&self.transport, &tmux_target, swarm.agent_type.launch_cmd()).await?;

        let role = format!("worker-{next_num}");
        Ok(AgentInfo {
            id: format!("{}/{role}", swarm.project_name),
            role,
            worktree_path,
            tmux_target,
            status: AgentStatus::default(),
            is_manager: false,
            pane_content: String::new(),
            dispatched_issue: None,
            current_issue: None,
            current_issue_title: None,
            waiting_for_input: false,
        })
    }

    async fn start_worker_loop(&self, tmux_target: &str) -> Result<()> {
        let worker_loop_cmd = self.runtime.worker_loop_cmd();
        if worker_loop_cmd.is_empty() {
            return Ok(());
        }
        proxy::send_keys(&self.transport, tmux_target, worker_loop_cmd).await
    }

    async fn stop(&self, swarm: &Swarm) -> Result<()> {
        // Send Ctrl+C to each worker pane to interrupt claude
        for worker in &swarm.workers {
            self.output(
                "tmux",
                &[
                    "send-keys".to_string(),
                    "-t".to_string(),
                    worker.tmux_target.clone(),
                    "C-c".to_string(),
                    String::new(),
                ],
                None,
            )
            .await
            .ok();
        }
        Ok(())
    }

    async fn heal_workers(&self, swarm: &mut Swarm) -> Result<Vec<String>> {
        if swarm.stopped || crate::config::persistence::is_swarm_stopped(&swarm.project_name) {
            tracing::info!("Skipping heal_workers for stopped swarm {}", swarm.project_name);
            return Ok(vec![]);
        }

        let mut repairs = Vec::new();
        let session_name = &swarm.tmux_session;
        let repo_path = &swarm.repo_path;
        let launch_cmd = swarm.agent_type.launch_cmd().to_string();
        let loop_cmd = swarm.agent_type.worker_loop_cmd().to_string();

        // Check which tmux panes actually exist in the agents window
        let session_exists = session::has_session(&self.transport, session_name).await;
        let existing_panes = if session_exists {
            session::list_panes(&self.transport, session_name).await.ok()
        } else {
            None
        };
        let agents_window = existing_panes
            .as_ref()
            .and_then(|info| {
                info.windows
                    .iter()
                    .find(|w| w.name == "agents" || w.index == 0)
            });
        let agents_window_exists = agents_window.is_some();
        let agents_window_panes: Vec<String> = agents_window
            .map(|w| w.panes.iter().map(|p| p.target.clone()).collect())
            .unwrap_or_default();

        for worker in &mut swarm.workers {
            let wt_path = &worker.worktree_path;

            // 1. Check worktree exists
            if !wt_path.exists() {
                tracing::info!("Healing {}: recreating worktree at {}", worker.id, wt_path.display());

                // Determine branch name from path
                let wt_name = wt_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                let current_branch = Command::new("git")
                    .args(["rev-parse", "--abbrev-ref", "HEAD"])
                    .current_dir(repo_path)
                    .output()
                    .await
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_else(|_| "main".to_string());

                // Extract wt index from name like "project-wt-3"
                let branch_suffix = wt_name
                    .rsplit_once("-wt-")
                    .map(|(_, idx)| format!("-wt-{idx}"))
                    .unwrap_or_else(|| format!("-{}", worker.id));
                let worktree_branch = format!("{current_branch}{branch_suffix}");

                // Create branch if needed
                let _ = Command::new("git")
                    .args(["branch", &worktree_branch, &current_branch])
                    .current_dir(repo_path)
                    .output()
                    .await;

                let output = Command::new("git")
                    .args([
                        "worktree",
                        "add",
                        &wt_path.to_string_lossy(),
                        &worktree_branch,
                    ])
                    .current_dir(repo_path)
                    .output()
                    .await;

                match output {
                    Ok(o) if o.status.success() => {
                        repairs.push(format!("Recreated worktree for {}", worker.id));
                    }
                    Ok(o) => {
                        let err = String::from_utf8_lossy(&o.stderr);
                        tracing::warn!("Failed to recreate worktree for {}: {err}", worker.id);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to recreate worktree for {}: {e}", worker.id);
                    }
                }
            }

            // 2. Check tmux pane exists; recreate session/window/pane as needed
            let pane_exists = agents_window_panes.contains(&worker.tmux_target);
            if !pane_exists {
                tracing::info!("Healing {}: pane missing, recreating (session_exists={}, window_exists={})",
                    worker.id, session_exists, agents_window_exists);

                let pane_created = if !session_exists {
                    // Session completely gone — do not recreate; user likely killed it intentionally.
                    // They can use 'N' in the TUI to relaunch.
                    tracing::info!(
                        "Session {session_name} is gone; skipping recreation for {}",
                        worker.id
                    );
                    false
                } else if !agents_window_exists {
                    // Session exists but agents window is gone — create a new window
                    let output = Command::new("tmux")
                        .args(["new-window", "-t", session_name, "-n", "agents"])
                        .output()
                        .await;
                    output.map(|o| o.status.success()).unwrap_or(false)
                } else {
                    // Window exists but this pane is missing — split to add a pane
                    let output = Command::new("tmux")
                        .args(["split-window", "-h", "-t", &format!("{session_name}:0")])
                        .output()
                        .await;
                    if let Ok(ref o) = output {
                        if o.status.success() {
                            // Rebalance panes
                            let _ = Command::new("tmux")
                                .args(["select-layout", "-t", &format!("{session_name}:0"), "even-horizontal"])
                                .output()
                                .await;
                        }
                    }
                    output.map(|o| o.status.success()).unwrap_or(false)
                };

                if pane_created {
                    // Get the new pane's target
                    let pane_output = Command::new("tmux")
                        .args([
                            "list-panes",
                            "-t",
                            &format!("{session_name}:0"),
                            "-F",
                            "#{pane_index}",
                        ])
                        .output()
                        .await;

                    if let Ok(po) = pane_output {
                        let max_idx: u32 = String::from_utf8_lossy(&po.stdout)
                            .lines()
                            .filter_map(|l| l.parse().ok())
                            .max()
                            .unwrap_or(0);
                        worker.tmux_target = format!("{session_name}:0.{max_idx}");
                    }

                    // cd to worktree
                    if wt_path.exists() {
                        let _ = proxy::send_keys(
                            &self.transport,
                            &worker.tmux_target,
                            &format!("cd '{}'", wt_path.display()),
                        )
                        .await;
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                    }

                    // Launch agent
                    let _ = proxy::send_keys(
                        &self.transport,
                        &worker.tmux_target,
                        &launch_cmd,
                    )
                    .await;

                    // Wait for agent to initialize, then start fix-loop
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    let _ = proxy::send_keys(&self.transport, &worker.tmux_target, &loop_cmd).await;

                    repairs.push(format!("Recreated tmux pane and launched agent for {}", worker.id));
                    continue; // Skip step 3 since we just launched
                }
            }

            // 3. Check pane state: feedback prompt, shell prompt, etc.
            if pane_exists && wt_path.exists() {
                match proxy::capture_pane(&self.transport, &worker.tmux_target, 10).await {
                    Ok(content) => {
                        let trimmed = content.trim();

                        // 3a. Detect Claude feedback prompt and auto-dismiss
                        if is_feedback_prompt(trimmed) {
                            tracing::info!(
                                "Healing {}: feedback prompt detected, auto-dismissing",
                                worker.id
                            );
                            let _ = proxy::send_keys(&self.transport, &worker.tmux_target, "0").await;
                            repairs.push(format!("Auto-dismissed feedback prompt for {}", worker.id));
                            continue;
                        }

                        // 3b. Detect bare shell prompt (agent not running)
                        // Must distinguish from an active Claude session which also shows ❯
                        let is_bare_shell = is_bare_shell_prompt(trimmed);

                        if is_bare_shell {
                            tracing::info!(
                                "Healing {}: agent not running (shell prompt detected), restarting",
                                worker.id
                            );
                            // cd to worktree and restart agent
                            let _ = proxy::send_keys(
                                &self.transport,
                                &worker.tmux_target,
                                &format!("cd '{}' && {}", wt_path.display(), launch_cmd),
                            )
                            .await;

                            // Wait for agent to initialize, then start fix-loop
                            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                            let _ = proxy::send_keys(&self.transport, &worker.tmux_target, &loop_cmd).await;

                            repairs.push(format!("Restarted agent for {} (was at shell prompt)", worker.id));
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Could not capture pane for {}: {e}", worker.id);
                    }
                }
            }
        }

        Ok(repairs)
    }

    async fn teardown(&self, swarm: &Swarm) -> Result<()> {
        // Mark as intentionally stopped before killing so heal_workers won't respawn it.
        crate::config::persistence::mark_swarm_stopped(&swarm.project_name);

        // Kill the tmux session
        let output = self
            .output(
                "tmux",
                &[
                    "kill-session".to_string(),
                    "-t".to_string(),
                    swarm.tmux_session.clone(),
                ],
                None,
            )
            .await
            .context("Failed to kill tmux session")?;

        if !output.status.success() {
            tracing::warn!(
                "tmux kill-session failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Remove worktrees
        let worktree_paths: Vec<PathBuf> = swarm
            .workers
            .iter()
            .map(|w| w.worktree_path.clone())
            .collect();
        self.remove_worktrees(&swarm.repo_path, &worktree_paths)
            .await?;

        Ok(())
    }

    async fn revive_agents(&self, swarm: &Swarm) -> Result<()> {
        self.ensure_swarm_agents_running(swarm).await
    }
}

/// Classified state of a tmux pane.
#[derive(Debug, Clone, PartialEq)]
enum PaneState {
    /// Shell prompt visible — no agent running, needs launch
    NeedsLaunch,
    /// Agent is running and actively working
    AgentBusy,
    /// Agent is running but idle at prompt
    AgentIdle,
    /// Can't determine state from content alone — need to probe
    Unknown,
}

/// Classify pane state by analyzing recent output lines.
///
/// An AI agent session looks very different from a bare shell:
/// - Shell: ends with `%`, `$`, `#` prompt chars
/// - Claude: shows `❯` prompt, `bypass permissions` status bar, thinking/working indicators
/// - Codex: shows `›` prompt, `gpt-` model indicator
/// - Active agents show "thinking", "working", "reading", "writing", etc.
/// - Idle agents show "how can i help", `>` or `❯` prompt with no activity
fn classify_pane_state(content: &str) -> PaneState {
    let stripped = strip_ansi(content);
    let non_empty_lines: Vec<&str> = stripped
        .lines()
        .rev()
        .filter(|line| !line.trim().is_empty())
        .collect();

    // Empty pane — either shell hasn't loaded or pane was just created
    if non_empty_lines.is_empty() {
        return PaneState::Unknown;
    }

    let mut saw_agent_indicator = false;
    let mut saw_idle_prompt = false;
    let mut saw_busy_indicator = false;

    for line in non_empty_lines.iter().take(8) {
        let lower = line.trim().to_lowercase();

        // Busy agent indicators — actively working
        if lower.contains("thinking")
            || lower.contains("working")
            || lower.contains("reading")
            || lower.contains("writing")
            || lower.contains("analyzing")
            || lower.contains("esc to interrupt")
        {
            saw_busy_indicator = true;
        }

        // Agent UI elements (proves an agent is running, but not whether busy/idle)
        if lower.contains("bypass permissions")
            || lower.contains("permissions on")
            || lower.contains("permissions off")
            || lower.contains("gpt-")
            || lower.contains("codex")
            || lower.contains("claude")
            || lower.contains("gemini")
            || lower.contains("droid")
            || lower.starts_with('\u{23f5}') // ⏵ (Claude/Codex status bar)
        {
            saw_agent_indicator = true;
        }

        // Idle agent prompt (waiting for user input)
        if lower.contains("how can i help")
            || lower.contains("what would you like")
            || lower.starts_with('>')
            || lower.starts_with('\u{276f}') // ❯ (Claude prompt)
            || lower.starts_with('\u{203a}') // › (Codex prompt)
        {
            saw_idle_prompt = true;
        }
    }

    // Decide based on what we found
    if saw_busy_indicator {
        return PaneState::AgentBusy;
    }
    if saw_idle_prompt || (saw_agent_indicator && !saw_busy_indicator) {
        return PaneState::AgentIdle;
    }

    // Agent exited with an "update yourself and restart" message
    for line in non_empty_lines.iter().take(5) {
        let lower = line.trim().to_lowercase();
        if lower.contains("please restart codex")
            || lower.contains("update ran successfully")
            || lower.contains("please restart claude")
            || lower.contains("restart to apply")
        {
            return PaneState::NeedsLaunch;
        }
    }

    // Check for shell prompt (bare command line, no agent)
    if let Some(last_line) = non_empty_lines.first() {
        let trimmed = last_line.trim();
        if trimmed.ends_with('%') || trimmed.ends_with('$') || trimmed.ends_with('#') {
            return PaneState::NeedsLaunch;
        }
    }

    PaneState::Unknown
}

// Legacy wrappers used by tests
#[cfg(test)]
fn pane_needs_runtime_launch(content: &str) -> bool {
    classify_pane_state(content) == PaneState::NeedsLaunch
}

#[cfg(test)]
fn pane_agent_is_idle(content: &str) -> bool {
    classify_pane_state(content) == PaneState::AgentIdle
}

fn manager_bootstrap_cmd(runtime: &AgentType) -> Option<String> {
    match runtime {
        AgentType::Claude => Some("/autocoder:monitor-loop".to_string()),
        AgentType::Gemini => Some("/manage-loop".to_string()),
        AgentType::Codex => Some("/manage-loop".to_string()),
        AgentType::Droid => Some("/manage-loop".to_string()),
    }
}

fn worker_dispatch_cmd(runtime: &AgentType, issue_number: u32) -> Option<String> {
    match runtime {
        AgentType::Claude => Some(format!("/autocoder:fix {issue_number}")),
        AgentType::Gemini => Some(format!("/fix {issue_number}")),
        AgentType::Codex => Some(format!(
            "Use the repository's Codex autocoder workflow to work issue #{issue_number} specifically. Start by reading AGENTS.md, skills/autocoder/SKILL.md, skills/autocoder/references/workflow-map.md, and skills/autocoder/references/command-mapping.md. Translate the legacy /fix behavior into direct Codex actions. Do one issue-focused pass, run relevant tests, and summarize the outcome."
        )),
        AgentType::Droid => Some(format!("/fix {issue_number}")),
    }
}

fn generic_worker_bootstrap_cmd(runtime: &AgentType) -> Option<String> {
    let loop_cmd = runtime.worker_loop_cmd();
    if !loop_cmd.is_empty() {
        return Some(loop_cmd.to_string());
    }
    match runtime {
        AgentType::Codex => Some(
            "Use the repository's Codex autocoder workflow to pick the next available issue and work it. Start by reading AGENTS.md, skills/autocoder/SKILL.md, skills/autocoder/references/workflow-map.md, and skills/autocoder/references/command-mapping.md. Choose the highest-priority available issue, do one focused pass, run relevant tests, and summarize the outcome.".to_string(),
        ),
        _ => None,
    }
}

impl ClaudeAdapter {
    async fn bootstrap_command(&self, runtime: &AgentType, agent: &AgentInfo) -> Option<String> {
        if agent.is_manager {
            return manager_bootstrap_cmd(runtime);
        }

        let issue_num = match &agent.status.state {
            status::AgentState::Working { issue: Some(n) } => Some(*n),
            _ => agent.dispatched_issue.or(self.issue_from_branch(agent).await),
        };

        if let Some(issue_number) = issue_num {
            return worker_dispatch_cmd(runtime, issue_number);
        }

        generic_worker_bootstrap_cmd(runtime)
    }

    async fn issue_from_branch(&self, agent: &AgentInfo) -> Option<u32> {
        if agent.is_manager {
            return None;
        }

        let output = self
            .output(
                "git",
                &[
                    "rev-parse".to_string(),
                    "--abbrev-ref".to_string(),
                    "HEAD".to_string(),
                ],
                Some(&agent.worktree_path),
            )
            .await
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        extract_issue_number_from_branch(&branch)
    }
}

fn extract_issue_number_from_branch(branch: &str) -> Option<u32> {
    let bytes = branch.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }

        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }

        let prev_ok = start == 0
            || matches!(
                bytes[start - 1] as char,
                '/' | '-' | '_' | '.'
            );
        let next_ok = i == bytes.len()
            || matches!(
                bytes[i] as char,
                '/' | '-' | '_' | '.'
            );

        if prev_ok && next_ok {
            if let Ok(issue) = branch[start..i].parse::<u32>() {
                if issue > 0 {
                    return Some(issue);
                }
            }
        }
    }

    None
}

fn strip_ansi(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '\x1b' {
            out.push(ch);
            continue;
        }

        if matches!(chars.peek(), Some('[')) {
            chars.next();
            while let Some(next) = chars.next() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::{
        classify_pane_state, extract_issue_number_from_branch, generic_worker_bootstrap_cmd,
        is_bare_shell_prompt, is_feedback_prompt, manager_bootstrap_cmd, pane_agent_is_idle,
        pane_needs_runtime_launch, worker_dispatch_cmd, PaneState,
    };
    use crate::model::swarm::AgentType;

    #[test]
    fn empty_pane_is_unknown() {
        // Empty pane could be shell loading or agent starting — probe needed
        assert_eq!(classify_pane_state(""), PaneState::Unknown);
        assert_eq!(classify_pane_state("\n\n\n"), PaneState::Unknown);
    }

    #[test]
    fn shell_prompt_needs_launch() {
        assert_eq!(classify_pane_state("user@host repo %"), PaneState::NeedsLaunch);
        assert_eq!(classify_pane_state("root@host:/tmp#"), PaneState::NeedsLaunch);
        assert_eq!(classify_pane_state("laird@mac src $"), PaneState::NeedsLaunch);
        // Legacy wrapper still works
        assert!(pane_needs_runtime_launch("user@host repo %"));
    }

    #[test]
    fn agent_busy_detected() {
        assert_eq!(classify_pane_state("thinking about the next edit"), PaneState::AgentBusy);
        assert_eq!(classify_pane_state("  reading src/main.rs\n"), PaneState::AgentBusy);
        assert_eq!(
            classify_pane_state("  bypass permissions on\n  esc to interrupt\n"),
            PaneState::AgentBusy,
        );
    }

    #[test]
    fn agent_idle_detected() {
        assert_eq!(classify_pane_state("> \n"), PaneState::AgentIdle);
        assert_eq!(classify_pane_state("how can i help you today?\n"), PaneState::AgentIdle);
        // Claude status bar without busy indicator = idle
        assert_eq!(
            classify_pane_state(
                "  nextgen-CDD [integration]\n  \u{23f5}\u{23f5} bypass permissions on (shift+tab to cycle)\n\n"
            ),
            PaneState::AgentIdle,
        );
        // Codex at prompt
        assert_eq!(
            classify_pane_state("  gpt-5.4 default \u{00b7} 100% left\n"),
            PaneState::AgentIdle,
        );
        // Legacy wrapper
        assert!(pane_agent_is_idle("> \n"));
        assert!(!pane_agent_is_idle("thinking about the next step\n"));
    }

    #[test]
    fn unknown_content_returns_unknown() {
        // Random text that's not a shell prompt or agent indicator
        assert_eq!(classify_pane_state("some random output\n"), PaneState::Unknown);
        assert_eq!(classify_pane_state("building project...\n"), PaneState::Unknown);
    }

    #[test]
    fn worker_dispatch_matches_issue() {
        assert_eq!(
            worker_dispatch_cmd(&AgentType::Claude, 42),
            Some("/autocoder:fix 42".to_string())
        );
    }

    #[test]
    fn manager_bootstrap_uses_monitor_loop() {
        assert_eq!(
            manager_bootstrap_cmd(&AgentType::Claude),
            Some("/autocoder:monitor-loop".to_string())
        );
    }

    #[test]
    fn generic_worker_bootstrap_uses_fix_loop() {
        assert_eq!(
            generic_worker_bootstrap_cmd(&AgentType::Claude),
            Some("/autocoder:fix-loop".to_string())
        );
        assert_eq!(
            generic_worker_bootstrap_cmd(&AgentType::Gemini),
            Some("/fix-loop".to_string())
        );
        assert_eq!(
            generic_worker_bootstrap_cmd(&AgentType::Droid),
            None
        );
    }

    #[test]
    fn branch_issue_number_is_extracted() {
        assert_eq!(
            extract_issue_number_from_branch("feature/1234-fix-bug"),
            Some(1234)
        );
        assert_eq!(
            extract_issue_number_from_branch("bugfix_987_repro"),
            Some(987)
        );
    }

    #[test]
    fn branch_without_issue_number_is_ignored() {
        assert_eq!(extract_issue_number_from_branch("main"), None);
        assert_eq!(extract_issue_number_from_branch("feature/fix-bug"), None);
    }

    // --- is_feedback_prompt tests ---

    #[test]
    fn detects_feedback_prompt_full() {
        let content = "some output\n● How is Claude doing this session? (optional)\n  1: Bad    2: Fine   3: Good   0: Dismiss\n";
        assert!(is_feedback_prompt(content));
    }

    #[test]
    fn detects_feedback_prompt_partial() {
        assert!(is_feedback_prompt("How is Claude doing this session?"));
        assert!(is_feedback_prompt("1: Bad 2: Fine 3: Good 0: Dismiss"));
    }

    #[test]
    fn no_false_positive_feedback() {
        assert!(!is_feedback_prompt("Working on issue #42"));
        assert!(!is_feedback_prompt("idle"));
        assert!(!is_feedback_prompt(""));
    }

    // --- is_bare_shell_prompt tests ---

    #[test]
    fn detects_bash_prompt() {
        assert!(is_bare_shell_prompt("user@host:~/project$"));
        assert!(is_bare_shell_prompt("~ $"));
    }

    #[test]
    fn detects_zsh_prompt() {
        assert!(is_bare_shell_prompt("user@host %"));
        assert!(is_bare_shell_prompt("❯"));
    }

    #[test]
    fn not_bare_shell_with_claude_indicators() {
        // Claude session with ❯ prompt should NOT be detected as bare shell
        assert!(!is_bare_shell_prompt("╭─ some output\n╰─ ❯"));
        assert!(!is_bare_shell_prompt("bypass permissions enabled\n❯"));
        assert!(!is_bare_shell_prompt("Claude Code session\nBrewed for you\n❯"));
    }

    #[test]
    fn empty_not_bare_shell() {
        assert!(!is_bare_shell_prompt(""));
        assert!(!is_bare_shell_prompt("\n\n"));
    }

    #[test]
    fn normal_output_not_bare_shell() {
        assert!(!is_bare_shell_prompt("Working on issue #42"));
        assert!(!is_bare_shell_prompt("IDLE_NO_WORK_AVAILABLE"));
    }
}

/// Try to find a repo path given a project name.
async fn find_repo_path(
    transport: &ServerTransport,
    session_name: &str,
    project_name: &str,
) -> Option<PathBuf> {
    // Check tmux environment (best-effort, don't bail on failure)
    if let Ok(output) = transport
        .output(
            "tmux",
            &[
                "show-environment".to_string(),
                "-t".to_string(),
                session_name.to_string(),
                "PWD".to_string(),
            ],
            None,
        )
        .await
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(path_str) = stdout.trim().strip_prefix("PWD=") {
                let path = PathBuf::from(path_str);
                if transport.is_remote() || path.exists() {
                    return Some(path);
                }
            }
        }
    }

    // Try current directory and children/siblings
    if let Ok(cwd) = std::env::current_dir() {
        // cwd IS the project
        if cwd.file_name().map(|n| n.to_string_lossy().to_string())
            == Some(project_name.to_string())
        {
            return Some(cwd.clone());
        }
        // project is a child of cwd (e.g., cwd=~/src/, project=agents)
        let child = cwd.join(project_name);
        if child.exists() {
            return Some(child);
        }
        // project is a sibling of cwd
        if let Some(parent) = cwd.parent() {
            let sibling = parent.join(project_name);
            if sibling.exists() {
                return Some(sibling);
            }
        }
    }

    // Try ~/src
    if let Some(home) = dirs::home_dir() {
        let candidate = home.join("src").join(project_name);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

/// Detect the GitHub owner of a repo from its git remote URL and switch
/// `gh auth` to a matching account if one is available.
///
/// This handles the case where a repo is owned by a different GitHub account
/// than the currently active `gh` CLI profile.
async fn ensure_gh_auth_for_repo(repo_path: &Path) {
    // Get the remote URL
    let remote = match Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(repo_path)
        .output()
        .await
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => return,
    };

    // Extract owner from URL patterns:
    // https://github.com/OWNER/REPO.git
    // git@github.com:OWNER/REPO.git
    let owner = if let Some(rest) = remote.strip_prefix("https://github.com/") {
        rest.split('/').next()
    } else if let Some(rest) = remote.strip_prefix("git@github.com:") {
        rest.split('/').next()
    } else {
        None
    };

    let owner = match owner {
        Some(o) if !o.is_empty() => o.to_string(),
        _ => return,
    };

    // Check current gh user
    let current_user = match Command::new("gh")
        .args(["api", "user", "--jq", ".login"])
        .output()
        .await
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => return,
    };

    if current_user == owner {
        return; // Already correct
    }

    // Try to switch to the repo owner's account
    tracing::info!("Switching gh auth from {current_user} to {owner} for repo at {}", repo_path.display());
    match Command::new("gh")
        .args(["auth", "switch", "--user", &owner])
        .output()
        .await
    {
        Ok(o) if o.status.success() => {
            tracing::info!("Successfully switched gh auth to {owner}");
        }
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            tracing::warn!("Failed to switch gh auth to {owner}: {err}");
        }
        Err(e) => {
            tracing::warn!("Failed to run gh auth switch: {e}");
        }
    }
}

/// Check if pane content shows a bare shell prompt (no active Claude session).
///
/// Returns true if the last line looks like a shell prompt AND there are no
/// indicators of an active Claude session in the content. This avoids false
/// positives where Claude's own prompt (❯) is mistaken for a shell.
pub(crate) fn is_bare_shell_prompt(content: &str) -> bool {
    let last_line = content.lines().last().unwrap_or("").trim();
    if last_line.is_empty() {
        return false;
    }

    // Check if last line looks like a shell prompt
    let has_shell_prompt = last_line.ends_with('$')
        || last_line.ends_with('%')
        || last_line.ends_with('#')
        || last_line.ends_with("❯");

    if !has_shell_prompt {
        return false;
    }

    // Check for Claude session indicators — if any are present, this is NOT a bare shell
    let lower = content.to_lowercase();
    let claude_indicators = [
        "bypass permissions",
        "claude code",
        "brewed for",
        "co-authored-by",
        "tool use",
        "read(",
        "edit(",
        "bash(",
        "write(",
        "╭─",  // Claude's box drawing
        "╰─",
        "idle_no_work_available",
    ];

    for indicator in &claude_indicators {
        if lower.contains(indicator) {
            return false;
        }
    }

    true
}

/// Check if pane content contains a Claude feedback prompt.
/// Matches patterns like "How is Claude doing this session?" and
/// "1: Bad    2: Fine   3: Good   0: Dismiss".
pub(crate) fn is_feedback_prompt(content: &str) -> bool {
    let lower = content.to_lowercase();
    lower.contains("how is claude doing this session")
        || (lower.contains("1: bad") && lower.contains("0: dismiss"))
}


