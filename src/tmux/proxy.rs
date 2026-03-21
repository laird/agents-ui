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
            "-e", // Preserve ANSI escape sequences (colors, etc.)
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

/// Send Ctrl+C followed by kill to a tmux pane to shut down the session.
pub async fn kill_pane(target: &str) -> Result<()> {
    // Send Ctrl+C to interrupt any running process
    let _ = Command::new("tmux")
        .args(["send-keys", "-t", target, "C-c", ""])
        .output()
        .await;

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Send "exit" to close the shell
    let _ = Command::new("tmux")
        .args(["send-keys", "-t", target, "exit", "Enter"])
        .output()
        .await;

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

        loop {
            interval.tick().await;

            match capture_pane(&target, 500).await {
                Ok(content) => {
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
                    tracing::warn!("Pane capture failed for {target}: {e}");
                    // Pane might have been destroyed — check and break
                    if !crate::tmux::session::has_session(
                        target.split(':').next().unwrap_or(&target),
                    )
                    .await
                    {
                        tracing::info!("Session gone for {target}, stopping watcher");
                        break;
                    }
                }
            }
        }
    })
}
