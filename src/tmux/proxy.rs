use anyhow::{Context, Result};
use tokio::process::Command;
use tokio::sync::mpsc;
use std::time::Duration;

/// Capture the current content of a tmux pane.
pub async fn capture_pane(target: &str, scrollback_lines: u32) -> Result<String> {
    let output = Command::new("tmux")
        .args([
            "capture-pane",
            "-p",
            "-t",
            target,
            "-S",
            &format!("-{scrollback_lines}"),
        ])
        .output()
        .await
        .context("Failed to capture tmux pane")?;

    if !output.status.success() {
        anyhow::bail!(
            "tmux capture-pane failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Send keys (text + Enter) to a tmux pane.
pub async fn send_keys(target: &str, input: &str) -> Result<()> {
    let output = Command::new("tmux")
        .args(["send-keys", "-t", target, input, "Enter"])
        .output()
        .await
        .context("Failed to send keys to tmux pane")?;

    if !output.status.success() {
        anyhow::bail!(
            "tmux send-keys failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

/// Resize a tmux pane to given dimensions.
pub async fn resize_pane(target: &str, width: u16, height: u16) -> Result<()> {
    let output = Command::new("tmux")
        .args([
            "resize-pane",
            "-t",
            target,
            "-x",
            &width.to_string(),
            "-y",
            &height.to_string(),
        ])
        .output()
        .await
        .context("Failed to resize tmux pane")?;

    if !output.status.success() {
        anyhow::bail!(
            "tmux resize-pane failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

/// Kill a specific tmux pane.
pub async fn kill_pane(target: &str) -> Result<()> {
    let output = Command::new("tmux")
        .args(["kill-pane", "-t", target])
        .output()
        .await
        .context("Failed to kill tmux pane")?;

    if !output.status.success() {
        anyhow::bail!(
            "tmux kill-pane failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

/// Kill an entire tmux session.
pub async fn kill_session(session: &str) -> Result<()> {
    let output = Command::new("tmux")
        .args(["kill-session", "-t", session])
        .output()
        .await
        .context("Failed to kill tmux session")?;

    if !output.status.success() {
        anyhow::bail!(
            "tmux kill-session failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

/// Spawn a background task that polls a tmux pane and sends content updates.
pub fn spawn_pane_watcher(
    target: String,
    agent_id: String,
    tx: mpsc::UnboundedSender<crate::event::Event>,
    poll_interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut last_content = String::new();
        let mut interval = tokio::time::interval(poll_interval);
        let mut consecutive_failures: u32 = 0;
        const MAX_FAILURES: u32 = 3;

        loop {
            interval.tick().await;

            match capture_pane(&target, 500).await {
                Ok(content) => {
                    consecutive_failures = 0;
                    if content != last_content {
                        last_content = content.clone();
                        if tx
                            .send(crate::event::Event::PaneOutput {
                                agent_id: agent_id.clone(),
                                content,
                            })
                            .is_err()
                        {
                            break; // Channel closed
                        }
                    }
                }
                Err(e) => {
                    consecutive_failures += 1;
                    if consecutive_failures >= MAX_FAILURES {
                        tracing::info!(
                            "Pane {target} failed {consecutive_failures} times, stopping watcher: {e}"
                        );
                        break;
                    }
                    tracing::warn!("Pane capture failed for {target}: {e}");
                }
            }
        }
    })
}
