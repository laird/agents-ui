use anyhow::{Context, Result};
use tokio::process::Command;
use tokio::sync::mpsc;
use std::time::Duration;

use crate::transport::ServerTransport;

/// Capture the current content of a tmux pane.
pub async fn capture_pane(
    transport: &ServerTransport,
    target: &str,
    scrollback_lines: u32,
) -> Result<String> {
    let output = transport
        .output(
            "tmux",
            &[
                "capture-pane".to_string(),
                "-p".to_string(),
                "-e".to_string(),
                "-J".to_string(), // join wrapped lines (prevents truncation at pane width)
                "-t".to_string(),
                target.to_string(),
                "-S".to_string(),
                format!("-{scrollback_lines}"),
            ],
            None,
        )
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

/// Send keys without appending Enter.
pub async fn send_keys_no_enter(
    transport: &ServerTransport,
    target: &str,
    input: &str,
) -> Result<()> {
    let output = transport
        .output(
            "tmux",
            &[
                "send-keys".to_string(),
                "-t".to_string(),
                target.to_string(),
                input.to_string(),
            ],
            None,
        )
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

/// Send keys (text + Enter) to a tmux pane.
pub async fn send_keys(transport: &ServerTransport, target: &str, input: &str) -> Result<()> {
    let output = transport
        .output(
            "tmux",
            &[
                "send-keys".to_string(),
                "-t".to_string(),
                target.to_string(),
                input.to_string(),
                "Enter".to_string(),
            ],
            None,
        )
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

/// Resize a tmux pane to given dimensions.
pub async fn resize_pane(transport: &ServerTransport, target: &str, width: u16, height: u16) -> Result<()> {
    let output = transport
        .output(
            "tmux",
            &[
                "resize-pane".to_string(),
                "-t".to_string(),
                target.to_string(),
                "-x".to_string(),
                width.to_string(),
                "-y".to_string(),
                height.to_string(),
            ],
            None,
        )
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

/// Send Ctrl+C followed by kill to a tmux pane to shut down the session.
pub async fn kill_pane(transport: &ServerTransport, target: &str) -> Result<()> {
    // Send Ctrl+C to interrupt any running process
    let _ = transport
        .output(
            "tmux",
            &[
                "send-keys".to_string(),
                "-t".to_string(),
                target.to_string(),
                "C-c".to_string(),
                String::new(),
            ],
            None,
        )
        .await;

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Send "exit" to close the shell
    let _ = transport
        .output(
            "tmux",
            &[
                "send-keys".to_string(),
                "-t".to_string(),
                target.to_string(),
                "exit".to_string(),
                "Enter".to_string(),
            ],
            None,
        )
        .await;

    Ok(())
}

/// Spawn a background task that polls a tmux pane and sends content updates.
pub fn spawn_pane_watcher(
    transport: ServerTransport,
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

            match capture_pane(&transport, &target, 500).await {
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
