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

    /// Expected tmux session name for a given project (manager session).
    fn session_name(project: &str) -> String {
        format!("claude-{project}")
    }

    /// Worker session name: claude-<project>-w<N>
    fn worker_session_name(project: &str, worker_idx: usize) -> String {
        format!("claude-{project}-w{worker_idx}")
    }

    /// Build a Swarm model by discovering the manager session and worker sessions.
    async fn build_swarm(
        project_name: &str,
        repo_path: PathBuf,
    ) -> Result<Swarm> {
        let manager_session = Self::session_name(project_name);

        // Manager lives in its own session (or window 1 of the main session for legacy compat)
        let manager_target = if session::has_session(&manager_session).await {
            // Check if legacy layout: window 1 is "review" (manager)
            let session_info = session::list_panes(&manager_session).await?;
            let review_window = session_info.windows.iter().find(|w| w.name == "review" || w.index == 1);
            if let Some(window) = review_window {
                window.panes.first().map(|p| p.target.clone())
                    .unwrap_or_else(|| format!("{manager_session}:0.0"))
            } else {
                format!("{manager_session}:0.0")
            }
        } else {
            format!("{manager_session}:0.0")
        };

        let manager = AgentInfo {
            id: "manager".to_string(),
            worktree_path: repo_path.clone(),
            tmux_target: manager_target,
            status: AgentStatus::default(),
            is_manager: true,
            pane_content: String::new(),
        };

        // Discover worker sessions: claude-<project>-wN
        let all_sessions = session::list_sessions().await?;
        let worker_prefix = format!("claude-{project_name}-w");
        let mut workers = Vec::new();

        for sess_name in &all_sessions {
            if let Some(suffix) = sess_name.strip_prefix(&worker_prefix) {
                if let Ok(idx) = suffix.parse::<usize>() {
                    let worktree_path = repo_path
                        .parent()
                        .unwrap_or(&repo_path)
                        .join(format!("{project_name}-wt-{}", idx + 1));

                    let status_file = worktree_path
                        .join(AgentType::Claude.status_dir())
                        .join("fix-loop.status");

                    let agent_status = status::read_status_file(&status_file);

                    workers.push(AgentInfo {
                        id: format!("worker-{idx}"),
                        worktree_path,
                        tmux_target: format!("{sess_name}:0.0"),
                        status: agent_status,
                        is_manager: false,
                        pane_content: String::new(),
                    });
                }
            }
        }

        // Also check legacy layout: panes in window 0 of the main session
        if workers.is_empty() && session::has_session(&manager_session).await {
            if let Ok(session_info) = session::list_panes(&manager_session).await {
                for window in &session_info.windows {
                    if window.name == "agents" || window.index == 0 {
                        for pane in &window.panes {
                            let worker_idx = pane.index as usize;
                            let worktree_path = repo_path
                                .parent()
                                .unwrap_or(&repo_path)
                                .join(format!("{project_name}-wt-{}", worker_idx + 1));

                            let status_file = worktree_path
                                .join(AgentType::Claude.status_dir())
                                .join("fix-loop.status");

                            let agent_status = status::read_status_file(&status_file);

                            workers.push(AgentInfo {
                                id: format!("worker-{worker_idx}"),
                                worktree_path,
                                tmux_target: pane.target.clone(),
                                status: agent_status,
                                is_manager: false,
                                pane_content: String::new(),
                            });
                        }
                    }
                }
            }
        }

        // Sort workers by index
        workers.sort_by_key(|w| {
            w.id.strip_prefix("worker-")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0)
        });

        Ok(Swarm {
            repo_path,
            project_name: project_name.to_string(),
            agent_type: AgentType::Claude,
            workflow: None,
            tmux_session: manager_session,
            manager,
            workers,
        })
    }
}

impl AgentRuntime for ClaudeAdapter {
    async fn launch(&self, config: &SwarmConfig) -> Result<Swarm> {
        let project_name = Self::project_name(&config.repo_path);
        let session_name = Self::session_name(&project_name);

        // Check if session already exists
        if session::has_session(&session_name).await {
            tracing::info!("Session {session_name} already exists, reconnecting");
            return Self::build_swarm(&project_name, config.repo_path.clone()).await;
        }

        let script_path = crate::scripts::launcher::find_script("start-parallel-agents.sh")
            .or_else(|| {
                let p = config.agents_dir.join("plugins/autocoder/scripts/start-parallel-agents.sh");
                p.exists().then_some(p)
            });

        let script_path = match script_path {
            Some(p) => p,
            None => anyhow::bail!(
                "start-parallel-agents.sh not found. Install the autocoder plugin or set AGENTS_DIR."
            ),
        };

        tracing::info!(
            "Launching swarm: {} workers for {} via {}",
            config.num_workers,
            project_name,
            script_path.display()
        );

        // Launch the script in a detached manner.
        let mut child = Command::new("bash")
            .args([
                script_path.to_string_lossy().as_ref(),
                &config.num_workers.to_string(),
                "--mux",
                "tmux",
                "--agent",
                config.agent_type.script_flag(),
            ])
            .current_dir(&config.repo_path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("Failed to spawn start-parallel-agents.sh")?;

        // Wait for the tmux session to appear
        let mut attempts = 0;
        loop {
            if session::has_session(&session_name).await {
                break;
            }
            attempts += 1;
            if attempts > 60 {
                child.kill().await.ok();
                anyhow::bail!("Timed out waiting for tmux session {session_name}");
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }

        // Kill the script process (it's blocking on tmux attach which we don't need)
        child.kill().await.ok();

        Self::build_swarm(&project_name, config.repo_path.clone()).await
    }

    async fn discover(&self, _agents_dir: &Path) -> Result<Vec<Swarm>> {
        let sessions = session::list_sessions().await?;
        let mut seen_projects = std::collections::HashSet::new();
        let mut swarms = Vec::new();

        for session_name in &sessions {
            if !session_name.starts_with("claude-") {
                continue;
            }

            // Extract project name, handling both "claude-<project>" and "claude-<project>-wN"
            let after_prefix = session_name.strip_prefix("claude-").unwrap_or(session_name);

            // Strip -wN suffix if present to get base project name
            let project_name = if let Some(base) = after_prefix.strip_suffix(
                &after_prefix.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<String>()
                    .chars().rev().collect::<String>()
            ) {
                if let Some(base) = base.strip_suffix("-w") {
                    base.to_string()
                } else {
                    after_prefix.to_string()
                }
            } else {
                after_prefix.to_string()
            };

            if !seen_projects.insert(project_name.clone()) {
                continue; // Already processed this project
            }

            let repo_path = find_repo_path(&project_name).await;
            if let Some(repo_path) = repo_path {
                match Self::build_swarm(&project_name, repo_path).await {
                    Ok(swarm) => swarms.push(swarm),
                    Err(e) => tracing::warn!("Failed to build swarm for {project_name}: {e}"),
                }
            } else {
                tracing::warn!("Could not determine repo path for project {project_name}");
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

        // Create a new tmux SESSION for this worker (full terminal width)
        let worker_session = Self::worker_session_name(project_name, next_idx);

        let output = Command::new("tmux")
            .args([
                "new-session",
                "-d",
                "-s",
                &worker_session,
                "-c",
                &worktree_path.to_string_lossy(),
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

        let tmux_target = format!("{worker_session}:0.0");

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
        for worker in &swarm.workers {
            let stop_file = worker
                .worktree_path
                .join(swarm.agent_type.status_dir())
                .join("fix-loop.stop");
            if let Err(e) = std::fs::write(&stop_file, "stop") {
                tracing::warn!("Failed to write stop file for {}: {e}", worker.id);
            }
        }
        Ok(())
    }

    async fn teardown(&self, swarm: &Swarm) -> Result<()> {
        // Kill worker sessions
        for worker in &swarm.workers {
            let session = worker.tmux_target.split(':').next().unwrap_or("");
            if !session.is_empty() {
                let _ = Command::new("tmux")
                    .args(["kill-session", "-t", session])
                    .output()
                    .await;
            }
        }

        // Kill manager session
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", &swarm.tmux_session])
            .output()
            .await;

        Ok(())
    }
}

/// Try to find a repo path given a project name.
async fn find_repo_path(project_name: &str) -> Option<PathBuf> {
    // Check if there's a tmux environment variable with the path
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
        if cwd.file_name().map(|n| n.to_string_lossy().to_string()) == Some(project_name.to_string())
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

    // Try home/src
    if let Some(home) = dirs::home_dir() {
        let candidate = home.join("src").join(project_name);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}
