use anyhow::{Context, Result};
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

        loop {
            interval.tick().await;

            match capture_pane(&transport, &target, 500).await {
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
                        &transport,
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
