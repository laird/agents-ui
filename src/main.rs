mod app;
mod config;
mod event;
mod github;
mod model;
mod scripts;
mod tmux;
mod tui;
mod adapter;
mod transport;
mod ui;

use anyhow::Result;
use model::swarm::AgentType;
use std::path::Path;
use transport::ServerTransport;

#[derive(Debug, Clone, PartialEq)]
struct CliOptions {
    agent_type: Option<AgentType>,
    server: Option<String>,
}

fn parse_cli_options<I, S>(args: I) -> Result<CliOptions>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut has_claude = false;
    let mut has_codex = false;
    let mut has_droid = false;
    let mut server = None;

    let mut iter = args.into_iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_ref() {
            "--claude" => has_claude = true,
            "--codex" => has_codex = true,
            "--droid" => has_droid = true,
            "--server" => {
                let Some(value) = iter.next() else {
                    anyhow::bail!("--server requires a hostname");
                };
                server = Some(value.as_ref().to_string());
            }
            value if value.starts_with("--server=") => {
                let host = value.trim_start_matches("--server=");
                if host.is_empty() {
                    anyhow::bail!("--server requires a hostname");
                }
                server = Some(host.to_string());
            }
            _ => {}
        }
    }

    let selected = [has_claude, has_codex, has_droid]
        .into_iter()
        .filter(|flag| *flag)
        .count();
    if selected > 1 {
        anyhow::bail!("Use only one runtime flag: --claude, --codex, or --droid");
    }

    let agent_type = if has_droid {
        Some(AgentType::Droid)
    } else if has_codex {
        Some(AgentType::Codex)
    } else if has_claude {
        Some(AgentType::Claude)
    } else {
        None
    };

    Ok(CliOptions { agent_type, server })
}

fn select_initial_agent_type(
    cli_agent_type: Option<AgentType>,
    repo_root: Option<&Path>,
) -> Result<Option<AgentType>> {
    if let Some(agent_type) = cli_agent_type {
        Ok(Some(agent_type))
    } else if let Some(root) = repo_root {
        crate::config::persistence::load_repo_agent_type(root)
    } else {
        Ok(None)
    }
}

/// Returns Ok(optional_warning) — fatal errors bail, non-fatal gh issues return a warning string.
async fn validate_startup_requirements(
    transport: &ServerTransport,
    agent_type: Option<&AgentType>,
) -> Result<Option<String>> {
    let location = transport.server().unwrap_or("this machine");

    let tmux_hint = if cfg!(target_os = "macos") {
        "brew install tmux"
    } else {
        "sudo apt install tmux"
    };
    if !transport.command_exists("tmux").await {
        anyhow::bail!(
            "tmux is not installed on {location}. Install with: {tmux_hint}"
        );
    }

    if let Some(agent_type) = agent_type {
        let (binary, hint) = match agent_type {
            AgentType::Claude => ("claude", "See https://docs.anthropic.com/en/docs/claude-code"),
            AgentType::Codex => ("codex", "npm install -g @openai/codex"),
            AgentType::Droid => ("droid", "See https://droid.dev"),
            AgentType::Gemini => ("gemini", "See https://ai.google.dev"),
        };

        if !transport.command_exists(binary).await {
            anyhow::bail!(
                "{binary} is not installed on {location}. {hint}"
            );
        }
    }

    // Non-fatal: check gh CLI auth status
    if let Some(gh_err) = crate::github::check_gh_auth(transport).await {
        let warning = gh_err.to_string();
        // Log but don't bail — gh is optional for basic operation
        eprintln!("Warning: {warning}");
        return Ok(Some(warning));
    }

    Ok(None)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = parse_cli_options(std::env::args())?;
    let transport = ServerTransport::new(cli.server.clone());

    let cwd = std::env::current_dir().ok();
    let repo_root = cwd
        .as_deref()
        .and_then(crate::config::persistence::find_repo_root);

    let initial_agent_type = select_initial_agent_type(cli.agent_type.clone(), repo_root.as_deref())?;
    let startup_warning = validate_startup_requirements(&transport, initial_agent_type.as_ref()).await?;

    // Initialize logging to file (not stdout, since we own the terminal)
    tracing_subscriber::fmt()
        .with_writer(|| {
            let log_dir = dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("agents-ui");
            std::fs::create_dir_all(&log_dir).ok();
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_dir.join("agents-ui.log"))
                .unwrap()
        })
        .with_ansi(false)
        .init();

    let mut terminal = tui::init()?;
    let mut app = app::App::new(
        initial_agent_type,
        cli.agent_type.is_some(),
        repo_root,
        cli.server,
        startup_warning,
    )
    .await?;
    let result = app.run(&mut terminal).await;
    tui::restore()?;
    result
}

#[cfg(test)]
mod tests {
    use super::{parse_cli_options, select_initial_agent_type, CliOptions};
    use crate::model::swarm::AgentType;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("agents-ui-main-{name}-{}-{nanos}", std::process::id()))
    }

    #[test]
    fn defaults_to_none() {
        let args = vec!["agents-tui"];
        assert_eq!(
            parse_cli_options(args).unwrap(),
            CliOptions {
                agent_type: None,
                server: None,
            }
        );
    }

    #[test]
    fn picks_droid_when_flag_present() {
        let args = vec!["agents-tui", "--droid"];
        assert_eq!(parse_cli_options(args).unwrap().agent_type, Some(AgentType::Droid));
    }

    #[test]
    fn picks_codex_when_flag_present() {
        let args = vec!["agents-tui", "--codex"];
        assert_eq!(parse_cli_options(args).unwrap().agent_type, Some(AgentType::Codex));
    }

    #[test]
    fn picks_claude_when_flag_present() {
        let args = vec!["agents-tui", "--claude"];
        assert_eq!(parse_cli_options(args).unwrap().agent_type, Some(AgentType::Claude));
    }

    #[test]
    fn rejects_conflicting_runtime_flags() {
        let args = vec!["agents-tui", "--claude", "--codex"];
        assert!(parse_cli_options(args).is_err());
    }

    #[test]
    fn parses_server_flag_variants() {
        let args = vec!["agents-tui", "--codex", "--server", "builder"];
        assert_eq!(
            parse_cli_options(args).unwrap(),
            CliOptions {
                agent_type: Some(AgentType::Codex),
                server: Some("builder".to_string()),
            }
        );

        let args = vec!["agents-tui", "--server=builder"];
        assert_eq!(
            parse_cli_options(args).unwrap(),
            CliOptions {
                agent_type: None,
                server: Some("builder".to_string()),
            }
        );
    }

    #[test]
    fn cli_runtime_overrides_saved_repo_runtime() {
        let root = temp_path("runtime-override");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        crate::config::persistence::save_repo_agent_type(&root, &AgentType::Claude).unwrap();

        let selected = select_initial_agent_type(Some(AgentType::Droid), Some(&root)).unwrap();
        assert_eq!(selected, Some(AgentType::Droid));

        std::fs::remove_dir_all(root).ok();
    }
}
