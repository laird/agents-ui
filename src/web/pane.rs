//! Reading and typing into an agent's terminal, whichever multiplexer it runs
//! in. tmux targets go through `crate::tmux::proxy`; herdr panes through the
//! `herdr pane` CLI. Keys arrive in tmux's names (the web API validates them
//! with `tmux_key_name`) and are translated for herdr here.

use anyhow::{Context, Result, bail};

use super::Mux;
use crate::tmux::proxy::{self, PaneCapture};
use crate::transport::ServerTransport;

/// An agent's terminal: where it is and what runs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pane {
    pub target: String,
    pub mux: Mux,
}

/// The screen plus scrollback, with ANSI colours, and the cursor when the
/// multiplexer reports it (herdr doesn't, so `pane_height` is 0: no caret).
pub async fn capture(transport: &ServerTransport, pane: &Pane, lines: u32) -> Result<PaneCapture> {
    match pane.mux {
        Mux::Tmux => proxy::capture_pane_with_cursor(transport, &pane.target, lines).await,
        Mux::Herdr => Ok(PaneCapture { content: herdr_read(transport, &pane.target, lines, "ansi").await?, ..Default::default() }),
    }
}

/// The screen as text, for reading what an agent is asking.
pub async fn capture_text(transport: &ServerTransport, pane: &Pane, lines: u32) -> Result<String> {
    match pane.mux {
        Mux::Tmux => proxy::capture_pane(transport, &pane.target, lines).await,
        Mux::Herdr => herdr_read(transport, &pane.target, lines, "text").await,
    }
}

/// Press one key, named as tmux names it (`Enter`, `C-c`, `Up`, or a single character).
pub async fn send_key(transport: &ServerTransport, pane: &Pane, key: &str) -> Result<()> {
    match pane.mux {
        Mux::Tmux => proxy::send_keys_no_enter(transport, &pane.target, key).await,
        Mux::Herdr => match herdr_key(key) {
            HerdrInput::Key(k) => herdr(transport, &["pane", "send-keys", &pane.target, &k]).await,
            HerdrInput::Text(t) => herdr(transport, &["pane", "send-text", &pane.target, &t]).await,
        },
    }
}

/// Type text literally, without Enter.
pub async fn send_text(transport: &ServerTransport, pane: &Pane, text: &str) -> Result<()> {
    match pane.mux {
        Mux::Tmux => proxy::send_literal_ordered(transport, &pane.target, text).await,
        Mux::Herdr => herdr(transport, &["pane", "send-text", &pane.target, text]).await,
    }
}

/// Type a line and press Enter.
pub async fn send_line(transport: &ServerTransport, pane: &Pane, text: &str) -> Result<()> {
    match pane.mux {
        Mux::Tmux => proxy::send_keys(transport, &pane.target, text).await,
        Mux::Herdr => {
            send_text(transport, pane, text).await?;
            herdr(transport, &["pane", "send-keys", &pane.target, "enter"]).await
        }
    }
}

/// Watch an agent's pane for the terminal view: each change of its content
/// is sent as `PaneOutput`. tmux panes use tmux's own watcher; a herdr pane
/// is read through `herdr pane read`, and after `HERDR_GONE_AFTER` failed
/// reads in a row it is reported `PaneDead` and the watch ends.
pub fn spawn_watcher(
    transport: ServerTransport,
    pane: Pane,
    agent_id: String,
    tx: tokio::sync::mpsc::UnboundedSender<crate::event::Event>,
    poll_interval: std::time::Duration,
) -> tokio::task::JoinHandle<()> {
    if pane.mux == Mux::Tmux {
        return proxy::spawn_pane_watcher(transport, pane.target, agent_id, tx, poll_interval);
    }
    tokio::spawn(async move {
        let mut last = String::new();
        let mut interval = tokio::time::interval(poll_interval);
        let mut failures: u32 = 0;
        loop {
            interval.tick().await;
            match capture(&transport, &pane, 500).await {
                Ok(c) => {
                    failures = 0;
                    if c.content != last {
                        last = c.content.clone();
                        if tx.send(crate::event::Event::PaneOutput { agent_id: agent_id.clone(), content: c.content }).is_err() {
                            break;
                        }
                    }
                }
                Err(e) => {
                    failures += 1;
                    tracing::warn!("herdr pane {} read failed ({failures}): {e}", pane.target);
                    if failures >= HERDR_GONE_AFTER {
                        let _ = tx.send(crate::event::Event::PaneDead { agent_id: agent_id.clone() });
                        break;
                    }
                }
            }
        }
    })
}
const HERDR_GONE_AFTER: u32 = 10;

/// What a tmux key becomes for herdr: a logical key, or text to type.
#[derive(Debug, PartialEq, Eq)]
pub enum HerdrInput {
    Key(String),
    Text(String),
}

/// Translate a tmux key name into herdr's (`Escape` → `esc`, `C-c` → `ctrl+c`,
/// `BTab` → `shift+tab`). Anything that isn't a key name (`1`, `y`) is typed.
pub fn herdr_key(tmux: &str) -> HerdrInput {
    const NAMED: &[(&str, &str)] = &[
        ("Enter", "enter"),
        ("Escape", "esc"),
        ("Tab", "tab"),
        ("BTab", "shift+tab"),
        ("BSpace", "backspace"),
        ("Space", "space"),
        ("Up", "up"),
        ("Down", "down"),
        ("Left", "left"),
        ("Right", "right"),
        ("Home", "home"),
        ("End", "end"),
        ("PageUp", "pageup"),
        ("PageDown", "pagedown"),
        ("DC", "delete"),
    ];
    if let Some((_, k)) = NAMED.iter().find(|(t, _)| *t == tmux) {
        return HerdrInput::Key((*k).to_string());
    }
    if let Some(c) = tmux.strip_prefix("C-").filter(|c| c.len() == 1) {
        return HerdrInput::Key(format!("ctrl+{}", c.to_ascii_lowercase()));
    }
    if let Some(n) = tmux.strip_prefix('F').filter(|n| n.parse::<u8>().is_ok_and(|n| (1..=12).contains(&n))) {
        return HerdrInput::Key(format!("f{n}"));
    }
    HerdrInput::Text(tmux.to_string())
}

/// `herdr pane read <pane> --lines N --format F`: the terminal as plain text or with ANSI escapes.
pub fn herdr_read_args(target: &str, lines: u32, format: &str) -> Vec<String> {
    ["pane", "read", target, "--source", "recent", "--lines", &lines.to_string(), "--format", format]
        .map(String::from)
        .to_vec()
}

async fn herdr_read(transport: &ServerTransport, target: &str, lines: u32, format: &str) -> Result<String> {
    let output = transport
        .output("herdr", &herdr_read_args(target, lines, format), None)
        .await
        .with_context(|| format!("couldn't read herdr pane {target}"))?;
    if !output.status.success() {
        bail!("couldn't read herdr pane {target}: {}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

async fn herdr(transport: &ServerTransport, args: &[&str]) -> Result<()> {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let output = transport.output("herdr", &args, None).await.context("couldn't run herdr")?;
    if !output.status.success() {
        bail!("herdr {} failed: {}", args[..2].join(" "), String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn herdr_keys_from_tmux_names() {
        assert_eq!(herdr_key("Enter"), HerdrInput::Key("enter".into()));
        assert_eq!(herdr_key("Escape"), HerdrInput::Key("esc".into()));
        assert_eq!(herdr_key("BTab"), HerdrInput::Key("shift+tab".into()));
        assert_eq!(herdr_key("BSpace"), HerdrInput::Key("backspace".into()));
        assert_eq!(herdr_key("C-c"), HerdrInput::Key("ctrl+c".into()));
        assert_eq!(herdr_key("F5"), HerdrInput::Key("f5".into()));
        assert_eq!(herdr_key("Up"), HerdrInput::Key("up".into()));
    }

    #[test]
    fn non_key_names_are_typed_for_herdr() {
        // An answer key such as a menu number or `y` is a character, not a key name.
        assert_eq!(herdr_key("1"), HerdrInput::Text("1".into()));
        assert_eq!(herdr_key("y"), HerdrInput::Text("y".into()));
        assert_eq!(herdr_key("F99"), HerdrInput::Text("F99".into()));
    }

    #[test]
    fn herdr_read_asks_for_recent_output_in_the_format() {
        assert_eq!(
            herdr_read_args("wA:p1", 500, "ansi"),
            ["pane", "read", "wA:p1", "--source", "recent", "--lines", "500", "--format", "ansi"]
        );
    }
}
