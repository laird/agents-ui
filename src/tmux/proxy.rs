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

/// Send a literal string to a tmux pane (no key name lookups, no Enter appended).
/// Uses fire-and-forget spawn for lower latency on interactive keystrokes.
pub async fn send_literal(target: &str, text: &str) -> Result<()> {
    Command::new("tmux")
        .args(["send-keys", "-t", target, "-l", text])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to spawn tmux send-keys literal")?;

    Ok(())
}

/// Send a named key (e.g., "Enter", "BSpace", "C-c") to a tmux pane.
/// Uses fire-and-forget spawn for lower latency on interactive keystrokes.
pub async fn send_named_key(target: &str, key: &str) -> Result<()> {
    Command::new("tmux")
        .args(["send-keys", "-t", target, key])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to spawn tmux send-keys named")?;

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
                    if consecutive_failures <= 3 {
                        tracing::warn!("Pane capture failed for {target}: {e}");
                    }
                    // Stop after 5 consecutive failures (pane likely gone)
                    if consecutive_failures >= 5 {
                        tracing::info!("Pane {target} unreachable after {consecutive_failures} failures, stopping watcher");
                        break;
                    }
                }
            }
        }
    })
}
