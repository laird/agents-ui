use anyhow::{Context, Result};
use std::env;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::mpsc;

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

fn parse_env_u16(name: &str) -> Option<u16> {
    env::var(name).ok()?.parse::<u16>().ok()
}

/// Return the active terminal size (columns, rows), if available.
pub fn current_terminal_size() -> Option<(u16, u16)> {
    if let Ok((cols, rows)) = crossterm::terminal::size() {
        return Some((cols, rows));
    }
    Some((parse_env_u16("COLUMNS")?, parse_env_u16("LINES")?))
}

/// Resize all windows in a tmux session to the given dimensions.
pub async fn resize_session_windows(session_name: &str, cols: u16, rows: u16) -> Result<()> {
    let output = Command::new("tmux")
        .args(["list-windows", "-t", session_name, "-F", "#{window_index}"])
        .output()
        .await
        .context("Failed to list tmux windows")?;

    if !output.status.success() {
        anyhow::bail!(
            "tmux list-windows failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let cols = cols.to_string();
    let rows = rows.to_string();
    for window_idx in String::from_utf8_lossy(&output.stdout).lines() {
        let target = format!("{session_name}:{window_idx}");
        let resize = Command::new("tmux")
            .args([
                "resize-window",
                "-t",
                &target,
                "-x",
                cols.as_str(),
                "-y",
                rows.as_str(),
            ])
            .output()
            .await
            .with_context(|| format!("Failed to resize tmux window {target}"))?;

        if !resize.status.success() {
            tracing::warn!(
                "tmux resize-window failed for {target}: {}",
                String::from_utf8_lossy(&resize.stderr)
            );
        }
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
        const MAX_CONSECUTIVE_FAILURES: u32 = 3;

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
                    if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                        tracing::info!(
                            "Pane {target} failed {consecutive_failures} times consecutively, stopping watcher"
                        );
                        break;
                    }
                    tracing::warn!("Pane capture failed for {target}: {e}");
                }
            }
        }
    })
}
