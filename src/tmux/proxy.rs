use anyhow::{Context, Result};
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::tmux::session::pane_exists;
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

/// A pane capture together with the cursor position at the moment of capture.
///
/// `cursor_y` is a row within the *visible* pane, while `content` also carries
/// scrollback, so the two only line up via `pane_height`: the last
/// `pane_height` lines of `content` are the visible region, and the cursor sits
/// on line `content_lines - pane_height + cursor_y`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaneCapture {
    /// Pane text, including scrollback and ANSI escapes.
    pub content: String,
    /// Cursor column within the visible pane, 0-based, measured in terminal
    /// cells (not characters -- a wide glyph advances this by two).
    pub cursor_x: u16,
    /// Cursor row within the visible pane, 0-based from the top of the
    /// visible region.
    pub cursor_y: u16,
    /// Height of the visible pane in rows. Zero means the cursor position
    /// could not be read, and callers should render no cursor rather than
    /// guess at one.
    pub pane_height: u16,
}

/// Build the argv for the combined cursor + capture invocation.
///
/// Split out from [`capture_pane_with_cursor`] so the flag set can be asserted
/// on: the caret's position depends on it, and the dependency is not obvious
/// from reading the flags.
///
/// Deliberately WITHOUT `-J`. [`capture_pane`] joins wrapped lines because it
/// only has to read well, but `cursor_y` counts *physical* pane rows, and the
/// page finds the caret's line by counting back `pane_height` lines from the
/// end of the capture. `-J` collapses a wrapped row and its continuation into
/// one line, so every wrap at or below the caret shifts it a row too high --
/// and a completion menu, the one thing this caret exists to make usable,
/// opens directly below the input line and wraps freely. One line per pane row
/// keeps that arithmetic exact.
fn capture_with_cursor_args(target: &str, scrollback_lines: u32) -> Vec<String> {
    vec![
        "display-message".to_string(),
        "-p".to_string(),
        "-t".to_string(),
        target.to_string(),
        "-F".to_string(),
        "#{cursor_x} #{cursor_y} #{pane_height}".to_string(),
        // A tmux command separator. The transport shell-quotes every argument,
        // so it reaches tmux as a literal ';' rather than being eaten by the
        // shell -- true for both the local and the ssh transport.
        ";".to_string(),
        "capture-pane".to_string(),
        "-p".to_string(),
        "-e".to_string(),
        "-t".to_string(),
        target.to_string(),
        "-S".to_string(),
        format!("-{scrollback_lines}"),
    ]
}

/// Capture a pane's content and its cursor position.
///
/// This issues ONE tmux invocation for both. Two calls would not just double
/// the round trip on the streaming path, where this runs several times a
/// second -- they could also straddle a redraw and pair a cursor position with
/// content from a different frame, drawing the caret in the wrong place.
pub async fn capture_pane_with_cursor(
    transport: &ServerTransport,
    target: &str,
    scrollback_lines: u32,
) -> Result<PaneCapture> {
    let output = transport
        .output("tmux", &capture_with_cursor_args(target, scrollback_lines), None)
        .await
        .context("Failed to capture tmux pane with cursor")?;

    if !output.status.success() {
        anyhow::bail!(
            "tmux capture-pane (with cursor) failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(parse_capture_with_cursor(
        &String::from_utf8_lossy(&output.stdout),
    ))
}

/// Split the combined `display-message` + `capture-pane` output into a
/// [`PaneCapture`].
///
/// The first line is the cursor header. If it is not exactly three integers
/// the whole payload is treated as content with `pane_height: 0` -- losing the
/// caret is survivable, silently eating the pane's first line is not.
pub fn parse_capture_with_cursor(raw: &str) -> PaneCapture {
    let (header, rest) = match raw.split_once('\n') {
        Some(parts) => parts,
        // No newline at all: nothing to split off, so treat it as content.
        None => {
            return PaneCapture {
                content: raw.to_string(),
                ..Default::default()
            };
        }
    };

    let fields: Vec<&str> = header.split_whitespace().collect();
    if fields.len() == 3
        && let (Ok(x), Ok(y), Ok(h)) = (
            fields[0].parse::<u16>(),
            fields[1].parse::<u16>(),
            fields[2].parse::<u16>(),
        )
    {
        return PaneCapture {
            content: rest.to_string(),
            cursor_x: x,
            cursor_y: y,
            pane_height: h,
        };
    }

    PaneCapture {
        content: raw.to_string(),
        ..Default::default()
    }
}

/// Send a literal string to a pane and wait for tmux to accept it.
///
/// [`send_literal`] is fire-and-forget, which is the right trade for a single
/// keystroke but destroys ordering when several are sent in sequence: two
/// spawned processes can reach the pane in either order. Batched input has to
/// await each send, so it uses this.
pub async fn send_literal_ordered(
    transport: &ServerTransport,
    target: &str,
    text: &str,
) -> Result<()> {
    let output = transport
        .output(
            "tmux",
            &[
                "send-keys".to_string(),
                "-t".to_string(),
                target.to_string(),
                "-l".to_string(),
                text.to_string(),
            ],
            None,
        )
        .await
        .context("Failed to send literal text to tmux pane")?;

    if !output.status.success() {
        anyhow::bail!(
            "tmux send-keys -l failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
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
pub async fn resize_pane(
    transport: &ServerTransport,
    target: &str,
    width: u16,
    height: u16,
) -> Result<()> {
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
    if let Err(e) = transport
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
        .await
    {
        tracing::warn!("Failed sending Ctrl+C to pane {target}: {e}");
    }

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Send "exit" to close the shell
    if let Err(e) = transport
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
        .await
    {
        tracing::warn!("Failed sending exit to pane {target}: {e}");
    }

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
        const MAX_CONSECUTIVE_FAILURES: u32 = 3;

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
                    if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                        // Confirm the pane is truly gone before declaring it dead,
                        // to avoid false positives from transient capture failures.
                        if !pane_exists(&transport, &target).await {
                            tracing::info!(
                                "Pane {target} confirmed gone after {consecutive_failures} failures, stopping watcher"
                            );
                            let _ = tx.send(crate::event::Event::PaneDead {
                                agent_id: agent_id.clone(),
                            });
                            break;
                        }
                        tracing::warn!(
                            "Pane {target} failed {consecutive_failures} times but still exists — resetting failure count"
                        );
                        consecutive_failures = 0;
                    } else {
                        tracing::warn!("Pane capture failed for {target}: {e}");
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // `-J` would collapse a wrapped pane row into one line, and the page
    // locates the caret's row by counting back `pane_height` lines from the
    // end of the capture -- so a joined capture draws the caret a row too high
    // for every wrap at or below it. One output line per pane row is what
    // makes that arithmetic exact.
    #[test]
    fn capture_with_cursor_does_not_join_wrapped_lines() {
        let args = capture_with_cursor_args("sess:0.1", 500);
        assert!(
            !args.iter().any(|a| a == "-J"),
            "-J breaks cursor_y row alignment: {args:?}"
        );
    }

    #[test]
    fn capture_with_cursor_args_request_cursor_then_content() {
        let args = capture_with_cursor_args("sess:0.1", 500);
        let sep = args.iter().position(|a| a == ";").expect("no ';' separator");
        assert_eq!(args[0], "display-message");
        assert!(
            args[..sep].contains(&"#{cursor_x} #{cursor_y} #{pane_height}".to_string()),
            "cursor header must come first so it can be split off: {args:?}"
        );
        assert_eq!(args[sep + 1], "capture-pane");
        // ANSI escapes, and the target and scrollback depth, on the capture.
        assert!(args[sep..].contains(&"-e".to_string()));
        assert!(args[sep..].contains(&"-500".to_string()));
        assert_eq!(args.iter().filter(|a| *a == "sess:0.1").count(), 2);
    }

    #[test]
    fn parses_cursor_header_and_content() {
        let cap = parse_capture_with_cursor("37 9 10\nline one\nline two\n");
        assert_eq!(cap.cursor_x, 37);
        assert_eq!(cap.cursor_y, 9);
        assert_eq!(cap.pane_height, 10);
        assert_eq!(cap.content, "line one\nline two\n");
    }

    #[test]
    fn parses_zero_cursor() {
        let cap = parse_capture_with_cursor("0 0 24\n");
        assert_eq!((cap.cursor_x, cap.cursor_y, cap.pane_height), (0, 0, 24));
        assert_eq!(cap.content, "");
    }

    #[test]
    fn keeps_ansi_escapes_in_content() {
        let cap = parse_capture_with_cursor("1 2 3\n\x1b[31mred\x1b[0m\n");
        assert_eq!(cap.content, "\x1b[31mred\x1b[0m\n");
        assert_eq!(cap.pane_height, 3);
    }

    // A malformed header must not be swallowed: dropping the caret is fine,
    // dropping a line of pane output is not.
    #[test]
    fn malformed_header_is_treated_as_content() {
        let raw = "not a cursor header\nreal content\n";
        let cap = parse_capture_with_cursor(raw);
        assert_eq!(cap.content, raw);
        assert_eq!(cap.pane_height, 0, "pane_height 0 signals 'no cursor'");
    }

    #[test]
    fn wrong_field_count_is_treated_as_content() {
        for raw in ["1 2\ncontent\n", "1 2 3 4\ncontent\n", "a b c\ncontent\n"] {
            let cap = parse_capture_with_cursor(raw);
            assert_eq!(cap.content, raw, "raw: {raw:?}");
            assert_eq!(cap.pane_height, 0, "raw: {raw:?}");
        }
    }

    #[test]
    fn payload_without_newline_is_content() {
        let cap = parse_capture_with_cursor("no newline here");
        assert_eq!(cap.content, "no newline here");
        assert_eq!(cap.pane_height, 0);
    }

    // Values that do not fit u16 are a malformed header, not a silent zero.
    #[test]
    fn out_of_range_header_is_treated_as_content() {
        let raw = "99999 1 10\ncontent\n";
        let cap = parse_capture_with_cursor(raw);
        assert_eq!(cap.content, raw);
        assert_eq!(cap.pane_height, 0);
    }
}
