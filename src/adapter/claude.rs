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

    /// Build a Swarm model from an existing tmux session.
    async fn build_swarm_from_session(
        session_name: &str,
        repo_path: PathBuf,
    ) -> Result<Swarm> {
        let project_name = Self::project_name(&repo_path);
        let session_info = session::list_panes(session_name).await?;

        // Convention from start-parallel-agents.sh:
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

        for window in &session_info.windows {
            if window.name == "review" || window.index == 1 {
                // Manager pane
                if let Some(pane) = window.panes.first() {
                    manager.tmux_target = pane.target.clone();
                }
            } else if window.name == "agents" || window.index == 0 {
                // Worker panes
                for pane in &window.panes {
                    let worker_idx = pane.index as usize;
                    let worktree_path = repo_path
                        .parent()
                        .unwrap_or(&repo_path)
                        .join(format!("{}-wt-{}", project_name, worker_idx + 1));

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

        Ok(Swarm {
            repo_path,
            project_name,
            agent_type: AgentType::Claude,
            workflow: None, // Can't determine from session alone
            tmux_session: session_name.to_string(),
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
            return Self::build_swarm_from_session(&session_name, config.repo_path.clone()).await;
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
        // start-parallel-agents.sh ends with `tmux attach`, so we spawn it
        // in the background and wait for the session to appear.
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

        // Wait for the tmux session to appear (the script creates it before attaching)
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

        Self::build_swarm_from_session(&session_name, config.repo_path.clone()).await
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

            // Try to find the repo path from git worktree in one of the panes,
            // or fall back to looking in common locations
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

    async fn stop(&self, swarm: &Swarm) -> Result<()> {
        // Write stop files for each worker
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
        let script_path = crate::scripts::launcher::find_script("stop-parallel-agents.sh")
            .unwrap_or_else(|| PathBuf::from("../agents/plugins/autocoder/scripts/stop-parallel-agents.sh"));

        let output = Command::new("bash")
            .arg(&script_path)
            .current_dir(&swarm.repo_path)
            .output()
            .await
            .context("Failed to run stop-parallel-agents.sh")?;

        if !output.status.success() {
            tracing::warn!(
                "stop-parallel-agents.sh failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(())
    }
}

/// Try to find a repo path given a project name.
/// Checks the current directory and common parent directories.
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
        // Check siblings
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
