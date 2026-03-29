use anyhow::{Context, Result};
use tokio::process::Command;

use crate::transport::ServerTransport;

/// A discovered tmux session.
#[derive(Debug, Clone)]
pub struct TmuxSessionInfo {
    #[allow(dead_code)] // Used for session identification in future features
    pub name: String,
    pub windows: Vec<TmuxWindowInfo>,
}

/// A tmux window within a session.
#[derive(Debug, Clone)]
pub struct TmuxWindowInfo {
    pub index: u32,
    pub name: String,
    pub panes: Vec<TmuxPaneInfo>,
}

/// A tmux pane within a window.
#[derive(Debug, Clone)]
pub struct TmuxPaneInfo {
    #[allow(dead_code)]
    pub index: u32,
    /// Full target string, e.g., "claude-myrepo:0.0"
    pub target: String,
}

/// Check if tmux is available.
#[allow(dead_code)] // Available for startup validation
pub async fn tmux_available() -> bool {
    ServerTransport::default()
        .output("tmux", &["-V".to_string()], None)
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// List all tmux sessions.
pub async fn list_sessions(transport: &ServerTransport) -> Result<Vec<String>> {
    let output = transport
        .output(
            "tmux",
            &[
                "list-sessions".to_string(),
                "-F".to_string(),
                "#{session_name}".to_string(),
            ],
            None,
        )
        .await
        .context("Failed to run tmux list-sessions")?;

    if !output.status.success() {
        // No server running = no sessions
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().map(|s| s.to_string()).collect())
}

/// Discover agent sessions matching known prefixes (claude-, codex-, droid-, gemini-).
pub async fn discover_agent_sessions(transport: &ServerTransport) -> Result<Vec<String>> {
    let prefixes = ["claude-", "codex-", "droid-", "gemini-"];
    let sessions = list_sessions(transport).await?;
    Ok(sessions
        .into_iter()
        .filter(|s| prefixes.iter().any(|p| s.starts_with(p)))
        .collect())
}

/// Resize all windows in a session to the given dimensions.
pub async fn resize_session(name: &str, width: u16, height: u16) -> Result<()> {
    // First, set the session's default size so new windows inherit it
    let _ = Command::new("tmux")
        .args([
            "set-option",
            "-t",
            name,
            "default-size",
            &format!("{width}x{height}"),
        ])
        .output()
        .await;

    // Resize all existing windows
    let output = Command::new("tmux")
        .args([
            "list-windows",
            "-t",
            name,
            "-F",
            "#{window_index}",
        ])
        .output()
        .await
        .context("Failed to list windows for resize")?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for win_idx in stdout.lines() {
            let target = format!("{name}:{win_idx}");
            let _ = Command::new("tmux")
                .args([
                    "resize-window",
                    "-t",
                    &target,
                    "-x",
                    &width.to_string(),
                    "-y",
                    &height.to_string(),
                ])
                .output()
                .await;
        }
    }

    Ok(())
}

/// Resize a session to match the current terminal size.
pub async fn resize_session_to_terminal(name: &str) -> Result<()> {
    let (width, height) = crossterm::terminal::size()
        .context("Failed to get terminal size")?;
    resize_session(name, width, height).await
}

/// Check if a specific session exists.
pub async fn has_session(transport: &ServerTransport, name: &str) -> bool {
    transport
        .output(
            "tmux",
            &["has-session".to_string(), "-t".to_string(), name.to_string()],
            None,
        )
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// List panes in a session, returning window/pane info.
pub async fn list_panes(transport: &ServerTransport, session: &str) -> Result<TmuxSessionInfo> {
    let output = transport
        .output(
            "tmux",
            &[
                "list-panes".to_string(),
                "-s".to_string(),
                "-t".to_string(),
                session.to_string(),
                "-F".to_string(),
                "#{window_index}\t#{window_name}\t#{pane_index}".to_string(),
            ],
            None,
        )
        .await
        .context("Failed to list tmux panes")?;

    if !output.status.success() {
        anyhow::bail!(
            "tmux list-panes failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_list_panes_output(session, &stdout))
}

fn parse_list_panes_output(session: &str, stdout: &str) -> TmuxSessionInfo {
    let mut windows: Vec<TmuxWindowInfo> = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        if parts.len() < 3 {
            continue;
        }
        let win_idx: u32 = parts[0].parse().unwrap_or(0);
        let win_name = parts[1].to_string();
        let pane_idx: u32 = parts[2].parse().unwrap_or(0);

        let target = format!("{session}:{win_idx}.{pane_idx}");
        let pane = TmuxPaneInfo {
            index: pane_idx,
            target,
        };

        if let Some(window) = windows.iter_mut().find(|w| w.index == win_idx) {
            window.panes.push(pane);
        } else {
            windows.push(TmuxWindowInfo {
                index: win_idx,
                name: win_name,
                panes: vec![pane],
            });
        }
    }

    TmuxSessionInfo {
        name: session.to_string(),
        windows,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_list_panes_output;

    #[test]
    fn parses_windows_and_panes_from_tmux_output() {
        let parsed = parse_list_panes_output(
            "codex-demo",
            "0\treview\t0\n1\tworker-1\t0\n1\tworker-1\t1\n2\tworker-2\t0\n",
        );

        assert_eq!(parsed.windows.len(), 3);
        assert_eq!(parsed.windows[0].name, "review");
        assert_eq!(parsed.windows[1].panes.len(), 2);
        assert_eq!(parsed.windows[1].panes[1].target, "codex-demo:1.1");
    }

    #[test]
    fn ignores_malformed_tmux_lines() {
        let parsed = parse_list_panes_output(
            "codex-demo",
            "broken-line\n2\tworker-2\t0\n",
        );

        assert_eq!(parsed.windows.len(), 1);
        assert_eq!(parsed.windows[0].index, 2);
        assert_eq!(parsed.windows[0].panes[0].target, "codex-demo:2.0");
    }
}
