use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::model::status::{self, AgentStatus};
use crate::model::swarm::{AgentInfo, AgentType, Swarm};
use crate::tmux::{proxy, session};

use super::traits::{AgentRuntime, SwarmConfig};

pub struct ClaudeAdapter;

impl ClaudeAdapter {
    pub fn new() -> Self {
        Self
    }

    /// Derive the project name from a repo path (last directory component).
    fn project_name(repo_path: &Path) -> String {
        repo_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// Expected tmux session name for a given project.
    fn session_name(project: &str) -> String {
        format!("claude-{project}")
    }

    /// Create git worktrees for workers.
    async fn create_worktrees(repo_path: &Path, project_name: &str, num_workers: u32) -> Result<Vec<PathBuf>> {
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
            let output = Command::new("git")
                .args(["branch", &branch_name])
                .current_dir(repo_path)
                .output()
                .await
                .context("Failed to create branch")?;
            // Ignore errors (branch may already exist)
            if !output.status.success() {
                tracing::info!("Branch {branch_name} may already exist, continuing");
            }

            // Create the worktree
            let output = Command::new("git")
                .args([
                    "worktree",
                    "add",
                    wt_path.to_string_lossy().as_ref(),
                    &branch_name,
                ])
                .current_dir(repo_path)
                .output()
                .await
                .context("Failed to create worktree")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // If worktree already exists for this branch, try without branch
                if stderr.contains("already checked out") || stderr.contains("already exists") {
                    tracing::warn!("Worktree issue for {branch_name}: {stderr}");
                    // Try with a detached head
                    let output2 = Command::new("git")
                        .args([
                            "worktree",
                            "add",
                            "--detach",
                            wt_path.to_string_lossy().as_ref(),
                        ])
                        .current_dir(repo_path)
                        .output()
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

    /// Create a tmux session with manager + worker panes.
    async fn create_tmux_session(
        session_name: &str,
        repo_path: &Path,
        worktree_paths: &[PathBuf],
    ) -> Result<()> {
        // Create session with first window "agents" starting in first worktree
        let first_wt = worktree_paths
            .first()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| repo_path.to_string_lossy().to_string());

        let output = Command::new("tmux")
            .args([
                "new-session",
                "-d",
                "-s",
                session_name,
                "-n",
                "agents",
                "-c",
                &first_wt,
            ])
            .output()
            .await
            .context("Failed to create tmux session")?;

        if !output.status.success() {
            anyhow::bail!(
                "tmux new-session failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Set environment variables in the tmux session to force tmux mode
        // This tells the autocoder plugin to use tmux, not cmux
        Command::new("tmux")
            .args(["set-environment", "-t", session_name, "AGENTS_MUX", "tmux"])
            .output()
            .await
            .ok();
        Command::new("tmux")
            .args(["set-environment", "-t", session_name, "MUX", "tmux"])
            .output()
            .await
            .ok();

        // Create additional panes for remaining workers
        for (i, wt_path) in worktree_paths.iter().enumerate().skip(1) {
            let output = Command::new("tmux")
                .args([
                    "split-window",
                    "-t",
                    &format!("{session_name}:agents"),
                    "-c",
                    &wt_path.to_string_lossy(),
                ])
                .output()
                .await?;
            if !output.status.success() {
                tracing::warn!(
                    "Failed to split pane for worker {}: {}",
                    i,
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            // Tile the layout evenly
            Command::new("tmux")
                .args([
                    "select-layout",
                    "-t",
                    &format!("{session_name}:agents"),
                    "tiled",
                ])
                .output()
                .await
                .ok();
        }

        // Create "review" window for manager, in the base repo
        let output = Command::new("tmux")
            .args([
                "new-window",
                "-t",
                session_name,
                "-n",
                "review",
                "-c",
                &repo_path.to_string_lossy(),
            ])
            .output()
            .await?;

        if !output.status.success() {
            tracing::warn!(
                "Failed to create review window: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Select the agents window as default
        Command::new("tmux")
            .args(["select-window", "-t", &format!("{session_name}:agents")])
            .output()
            .await
            .ok();

        Ok(())
    }

    /// Launch claude in a specific tmux pane.
    async fn launch_claude_in_pane(target: &str, session_name: &str) -> Result<()> {
        // Launch claude with system prompt instructing it to use tmux (not cmux)
        let cmd = format!(
            "claude --dangerously-skip-permissions --append-system-prompt 'This session is managed by agents-ui via tmux. \
            IMPORTANT: Always use tmux commands (tmux capture-pane, tmux send-keys, etc.) \
            for reading worker screens and dispatching work. Do NOT use cmux. \
            The tmux session is named {session_name}.'"
        );
        proxy::send_keys(target, &cmd).await
    }

    /// Build a Swarm model from an existing tmux session.
    async fn build_swarm_from_session(
        session_name: &str,
        repo_path: PathBuf,
    ) -> Result<Swarm> {
        let project_name = Self::project_name(&repo_path);
        let session_info = session::list_panes(session_name).await?;

        // Convention:
        // Window 0 ("agents"): panes 0..N-1 are workers
        // Window 1 ("review"): pane 0 is manager
        let mut manager = AgentInfo {
            id: "manager".to_string(),
            worktree_path: repo_path.clone(),
            tmux_target: format!("{session_name}:1.0"),
            status: AgentStatus::default(),
            is_manager: true,
            pane_content: String::new(),
        };

        let mut workers = Vec::new();

        // First pass: identify the manager pane
        for window in &session_info.windows {
            if window.name == "review" || window.index == 1 {
                if let Some(pane) = window.panes.first() {
                    manager.tmux_target = pane.target.clone();
                }
            }
        }

        // Second pass: all panes NOT in the manager window are workers.
        // This handles dynamically added workers in any window.
        let mut worker_count = 0usize;
        for window in &session_info.windows {
            if window.name == "review" || window.index == 1 {
                continue; // Skip the manager window
            }
            for pane in &window.panes {
                let worktree_path = repo_path
                    .parent()
                    .unwrap_or(&repo_path)
                    .join(format!("{}-wt-{}", project_name, worker_count + 1));

                let status_file = worktree_path
                    .join(AgentType::Claude.status_dir())
                    .join("fix-loop.status");

                let agent_status = status::read_status_file(&status_file);

                workers.push(AgentInfo {
                    id: format!("worker-{worker_count}"),
                    worktree_path,
                    tmux_target: pane.target.clone(),
                    status: agent_status,
                    is_manager: false,
                    pane_content: String::new(),
                });
                worker_count += 1;
            }
        }

        Ok(Swarm {
            repo_path,
            project_name,
            agent_type: AgentType::Claude,
            workflow: None,
            tmux_session: session_name.to_string(),
            manager,
            workers,
        })
    }

    /// Check if the autocoder plugin is installed, and install it if not.
    async fn ensure_plugin_installed() -> Result<()> {
        // Check if autocoder plugin is already installed
        let output = Command::new("claude")
            .args(["plugin", "list"])
            .output()
            .await
            .context("Failed to run 'claude plugin list'. Is claude CLI installed?")?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        if stdout.contains("autocoder") {
            tracing::info!("Autocoder plugin already installed");
            return Ok(());
        }

        tracing::info!("Autocoder plugin not found, installing...");

        // Step 1: Add the laird/agents marketplace (if not already added)
        let marketplace_output = Command::new("claude")
            .args(["plugin", "marketplace", "list"])
            .output()
            .await?;

        let marketplace_stdout = String::from_utf8_lossy(&marketplace_output.stdout);

        if !marketplace_stdout.contains("laird/agents") && !marketplace_stdout.contains("plugin-marketplace") {
            tracing::info!("Adding laird/agents marketplace...");
            let add_output = Command::new("claude")
                .args(["plugin", "marketplace", "add", "laird/agents"])
                .output()
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
        let install_output = Command::new("claude")
            .args(["plugin", "install", "autocoder"])
            .output()
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
    async fn remove_worktrees(repo_path: &Path, worktree_paths: &[PathBuf]) -> Result<()> {
        for wt in worktree_paths {
            let output = Command::new("git")
                .args(["worktree", "remove", "--force", &wt.to_string_lossy()])
                .current_dir(repo_path)
                .output()
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
        let project_name = Self::project_name(&config.repo_path);
        let session_name = Self::session_name(&project_name);

        // Check if session already exists
        if session::has_session(&session_name).await {
            tracing::info!("Session {session_name} already exists, reconnecting");
            return Self::build_swarm_from_session(&session_name, config.repo_path.clone()).await;
        }

        // Ensure the autocoder plugin is installed
        Self::ensure_plugin_installed().await?;

        tracing::info!(
            "Launching swarm: {} workers for {}",
            config.num_workers,
            project_name,
        );

        // 1. Create git worktrees
        let worktree_paths =
            Self::create_worktrees(&config.repo_path, &project_name, config.num_workers).await?;

        // 2. Create tmux session with windows/panes
        Self::create_tmux_session(&session_name, &config.repo_path, &worktree_paths).await?;

        // 3. Launch claude in each worker pane
        for i in 0..worktree_paths.len() {
            let target = format!("{session_name}:agents.{i}");
            if let Err(e) = Self::launch_claude_in_pane(&target, &session_name).await {
                tracing::warn!("Failed to launch claude in pane {i}: {e}");
            }
            // Small delay between launches to avoid overwhelming
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        // 4. Launch claude in manager pane
        let manager_target = format!("{session_name}:review.0");
        if let Err(e) = Self::launch_claude_in_pane(&manager_target, &session_name).await {
            tracing::warn!("Failed to launch claude in manager pane: {e}");
        }

        // 6. Build swarm model
        let swarm = Self::build_swarm_from_session(&session_name, config.repo_path.clone()).await?;

        // Auto-start /manage-loop on the manager session
        tracing::info!("Sending /manage-loop to manager pane {}", swarm.manager.tmux_target);
        if let Err(e) = proxy::send_keys(&swarm.manager.tmux_target, "/manage-loop").await {
            tracing::warn!("Failed to send /manage-loop to manager: {e}");
        }

        Ok(swarm)
    }

    async fn discover(&self, _agents_dir: &Path) -> Result<Vec<Swarm>> {
        let sessions = session::discover_agent_sessions().await?;
        let mut swarms = Vec::new();

        for session_name in sessions {
            if !session_name.starts_with("claude-") {
                continue;
            }

            let project_name = session_name
                .strip_prefix("claude-")
                .unwrap_or(&session_name)
                .to_string();

            let repo_path = find_repo_path(&project_name).await;

            if let Some(repo_path) = repo_path {
                match Self::build_swarm_from_session(&session_name, repo_path).await {
                    Ok(swarm) => swarms.push(swarm),
                    Err(e) => tracing::warn!("Failed to build swarm from {session_name}: {e}"),
                }
            } else {
                tracing::warn!("Could not determine repo path for session {session_name}");
            }
        }

        Ok(swarms)
    }

    async fn send_input(&self, tmux_target: &str, input: &str) -> Result<()> {
        proxy::send_keys(tmux_target, input).await
    }

    async fn capture_output(&self, tmux_target: &str) -> Result<String> {
        proxy::capture_pane(tmux_target, 500).await
    }

    async fn add_worker(&self, swarm: &Swarm) -> Result<AgentInfo> {
        let next_idx = swarm.workers.len();
        let project_name = &swarm.project_name;
        let repo_path = &swarm.repo_path;
        let session_name = &swarm.tmux_session;

        // Create a git worktree for the new worker
        let worktree_path = repo_path
            .parent()
            .unwrap_or(repo_path)
            .join(format!("{project_name}-wt-{}", next_idx + 1));

        let current_branch = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(repo_path)
            .output()
            .await
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "main".to_string());

        let worktree_branch = format!("{current_branch}-wt-{}", next_idx + 1);

        if !worktree_path.exists() {
            // Create branch if it doesn't exist
            let _ = Command::new("git")
                .args(["branch", &worktree_branch, &current_branch])
                .current_dir(repo_path)
                .output()
                .await;

            let output = Command::new("git")
                .args([
                    "worktree",
                    "add",
                    &worktree_path.to_string_lossy(),
                    &worktree_branch,
                ])
                .current_dir(repo_path)
                .output()
                .await
                .context("Failed to create git worktree")?;

            if !output.status.success() {
                anyhow::bail!(
                    "git worktree add failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        // Create a new tmux pane in window 0 (agents window)
        let output = Command::new("tmux")
            .args(["split-window", "-h", "-t", &format!("{session_name}:0")])
            .output()
            .await
            .context("Failed to create tmux pane")?;

        if !output.status.success() {
            anyhow::bail!(
                "tmux split-window failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Rebalance panes
        let _ = Command::new("tmux")
            .args(["select-layout", "-t", &format!("{session_name}:0"), "even-horizontal"])
            .output()
            .await;

        // Figure out the new pane index (it's the highest pane index now)
        let pane_output = Command::new("tmux")
            .args([
                "list-panes",
                "-t",
                &format!("{session_name}:0"),
                "-F",
                "#{pane_index}",
            ])
            .output()
            .await
            .context("Failed to list panes")?;

        let pane_indices: Vec<u32> = String::from_utf8_lossy(&pane_output.stdout)
            .lines()
            .filter_map(|l| l.parse().ok())
            .collect();
        let new_pane_idx = pane_indices.into_iter().max().unwrap_or(next_idx as u32);
        let tmux_target = format!("{session_name}:0.{new_pane_idx}");

        // cd to worktree
        proxy::send_keys(&tmux_target, &format!("cd '{}'", worktree_path.display())).await?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Launch the agent with autonomous permissions
        proxy::send_keys(&tmux_target, swarm.agent_type.launch_cmd()).await?;

        // Wait for agent to initialize
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        // Send the worker loop command
        proxy::send_keys(&tmux_target, swarm.agent_type.worker_loop_cmd()).await?;

        Ok(AgentInfo {
            id: format!("worker-{next_idx}"),
            worktree_path,
            tmux_target,
            status: AgentStatus::default(),
            is_manager: false,
            pane_content: String::new(),
        })
    }

    async fn start_worker_loop(&self, tmux_target: &str) -> Result<()> {
        proxy::send_keys(tmux_target, AgentType::Claude.worker_loop_cmd()).await
    }

    async fn stop(&self, swarm: &Swarm) -> Result<()> {
        // Send Ctrl+C to each worker pane to interrupt claude
        for worker in &swarm.workers {
            Command::new("tmux")
                .args(["send-keys", "-t", &worker.tmux_target, "C-c", ""])
                .output()
                .await
                .ok();
        }
        Ok(())
    }

    async fn teardown(&self, swarm: &Swarm) -> Result<()> {
        // Kill the tmux session
        let output = Command::new("tmux")
            .args(["kill-session", "-t", &swarm.tmux_session])
            .output()
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
        Self::remove_worktrees(&swarm.repo_path, &worktree_paths).await?;

        Ok(())
    }
}

/// Try to find a repo path given a project name.
async fn find_repo_path(project_name: &str) -> Option<PathBuf> {
    // Check tmux environment
    let output = Command::new("tmux")
        .args([
            "show-environment",
            "-t",
            &format!("claude-{project_name}"),
            "PWD",
        ])
        .output()
        .await
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(path_str) = stdout.trim().strip_prefix("PWD=") {
            let path = PathBuf::from(path_str);
            if path.exists() {
                return Some(path);
            }
        }
    }

    // Try current directory
    if let Ok(cwd) = std::env::current_dir() {
        if cwd.file_name().map(|n| n.to_string_lossy().to_string())
            == Some(project_name.to_string())
        {
            return Some(cwd);
        }
        if let Some(parent) = cwd.parent() {
            let candidate = parent.join(project_name);
            if candidate.exists() {
                return Some(candidate);
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
