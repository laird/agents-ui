mod adapter;
mod app;
mod config;
mod event;
mod github;
mod model;
mod runtime;
mod scripts;
mod tmux;
mod transport;
mod tui;
mod ui;
mod web;

use anyhow::Result;
use model::swarm::AgentType;
use std::path::Path;
use transport::ServerTransport;

#[derive(Debug, Clone, PartialEq)]
struct CliOptions {
    agent_type: Option<AgentType>,
    server: Option<String>,
    web_port: Option<u16>,
}

fn parse_cli_options<I, S>(args: I) -> Result<CliOptions>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut has_claude = false;
    let mut has_codex = false;
    let mut has_droid = false;
    let mut has_gemini = false;
    let mut server = None;
    let mut web_port: Option<u16> = None;

    let mut iter = args.into_iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_ref() {
            "--claude" => has_claude = true,
            "--codex" => has_codex = true,
            "--droid" => has_droid = true,
            "--gemini" => has_gemini = true,
            "--web" => {
                if web_port.is_none() {
                    web_port = Some(web::server::DEFAULT_PORT);
                }
            }
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
            "--web-port" => {
                let Some(value) = iter.next() else {
                    anyhow::bail!("--web-port requires a port number");
                };
                let raw = value.as_ref();
                let port: u16 = raw.parse().map_err(|_| {
                    anyhow::anyhow!("--web-port value '{raw}' is not a valid port number")
                })?;
                web_port = Some(port);
            }
            value if value.starts_with("--web-port=") => {
                let raw = value.trim_start_matches("--web-port=");
                let port: u16 = raw.parse().map_err(|_| {
                    anyhow::anyhow!("--web-port value '{raw}' is not a valid port number")
                })?;
                web_port = Some(port);
            }
            _ => {}
        }
    }

    let selected = [has_claude, has_codex, has_droid, has_gemini]
        .into_iter()
        .filter(|flag| *flag)
        .count();
    if selected > 1 {
        anyhow::bail!("Use only one runtime flag: --claude, --codex, --droid, or --gemini");
    }

    let agent_type = if has_gemini {
        Some(AgentType::Gemini)
    } else if has_droid {
        Some(AgentType::Droid)
    } else if has_codex {
        Some(AgentType::Codex)
    } else if has_claude {
        Some(AgentType::Claude)
    } else {
        None
    };

    Ok(CliOptions { agent_type, server, web_port })
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = parse_cli_options(std::env::args())?;
    let transport = ServerTransport::new(cli.server.clone());

    let cwd = std::env::current_dir().ok();
    let repo_root = cwd
        .as_deref()
        .and_then(crate::config::persistence::find_repo_root);

    let initial_agent_type =
        select_initial_agent_type(cli.agent_type.clone(), repo_root.as_deref())?;
    let startup_warning = crate::runtime::validate_environment(
        &transport,
        initial_agent_type.as_ref(),
        repo_root.as_deref(),
    )
    .await?
    .gh_warning;

    // Initialize logging to file (not stdout, since we own the terminal)
    tracing_subscriber::fmt()
        .with_writer(|| {
            let log_dir = dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("agents-ui");
            if let Err(e) = std::fs::create_dir_all(&log_dir) {
                eprintln!(
                    "agents-tui: failed to create log directory {}: {}",
                    log_dir.display(),
                    e
                );
                return Box::new(std::io::sink()) as Box<dyn std::io::Write + Send>;
            }

            match std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_dir.join("agents-ui.log"))
            {
                Ok(file) => Box::new(file) as Box<dyn std::io::Write + Send>,
                Err(e) => {
                    eprintln!(
                        "agents-tui: failed to open log file in {}: {}",
                        log_dir.display(),
                        e
                    );
                    Box::new(std::io::sink()) as Box<dyn std::io::Write + Send>
                }
            }
        })
        .with_ansi(false)
        .init();

    let mut terminal = tui::init()?;

    // Start web server if requested
    let web_state = cli.web_port.map(|port| {
        let shared = web::new_shared_state();
        let state_clone = shared.clone();
        tokio::spawn(async move {
            if let Err(e) = web::server::run(port, state_clone).await {
                tracing::error!("Web server exited with error: {e:#}");
            }
        });
        shared
    });

    let mut app = app::App::new(
        initial_agent_type,
        cli.agent_type.is_some(),
        repo_root,
        cli.server,
        startup_warning,
        web_state,
    )
    .await?;
    let result = app.run(&mut terminal).await;
    if let Err(e) = &result {
        tracing::error!("TUI runtime failed: {e:#}");
    }
    if let Err(e) = tui::restore() {
        tracing::error!("Failed to restore terminal state: {e:#}");
        return Err(e);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{CliOptions, parse_cli_options, select_initial_agent_type};
    use crate::model::swarm::AgentType;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "agents-ui-main-{name}-{}-{nanos}",
            std::process::id()
        ))
    }

    #[test]
    fn defaults_to_none() {
        let args = vec!["agents-tui"];
        assert_eq!(
            parse_cli_options(args).unwrap(),
            CliOptions {
                agent_type: None,
                server: None,
                web_port: None,
            }
        );
    }

    #[test]
    fn picks_droid_when_flag_present() {
        let args = vec!["agents-tui", "--droid"];
        assert_eq!(
            parse_cli_options(args).unwrap().agent_type,
            Some(AgentType::Droid)
        );
    }

    #[test]
    fn picks_codex_when_flag_present() {
        let args = vec!["agents-tui", "--codex"];
        assert_eq!(
            parse_cli_options(args).unwrap().agent_type,
            Some(AgentType::Codex)
        );
    }

    #[test]
    fn picks_claude_when_flag_present() {
        let args = vec!["agents-tui", "--claude"];
        assert_eq!(
            parse_cli_options(args).unwrap().agent_type,
            Some(AgentType::Claude)
        );
    }

    #[test]
    fn picks_gemini_when_flag_present() {
        let args = vec!["agents-tui", "--gemini"];
        assert_eq!(
            parse_cli_options(args).unwrap().agent_type,
            Some(AgentType::Gemini)
        );
    }

    #[test]
    fn rejects_conflicting_runtime_flags() {
        let args = vec!["agents-tui", "--claude", "--gemini"];
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
                web_port: None,
            }
        );

        let args = vec!["agents-tui", "--server=builder"];
        assert_eq!(
            parse_cli_options(args).unwrap(),
            CliOptions {
                agent_type: None,
                server: Some("builder".to_string()),
                web_port: None,
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

    #[test]
    fn web_flag_sets_default_port() {
        let args = vec!["agents-tui", "--web"];
        let opts = parse_cli_options(args).unwrap();
        assert_eq!(
            opts.web_port,
            Some(crate::web::server::DEFAULT_PORT)
        );
    }

    #[test]
    fn web_port_flag_space_separated() {
        let args = vec!["agents-tui", "--web-port", "9000"];
        let opts = parse_cli_options(args).unwrap();
        assert_eq!(opts.web_port, Some(9000u16));
    }

    #[test]
    fn web_port_flag_equals_separated() {
        let args = vec!["agents-tui", "--web-port=8080"];
        let opts = parse_cli_options(args).unwrap();
        assert_eq!(opts.web_port, Some(8080u16));
    }

    #[test]
    fn web_port_flag_rejects_invalid_port() {
        let args = vec!["agents-tui", "--web-port", "notaport"];
        assert!(parse_cli_options(args).is_err());
    }

    #[test]
    fn no_web_flags_gives_none_port() {
        let args = vec!["agents-tui", "--claude"];
        let opts = parse_cli_options(args).unwrap();
        assert_eq!(opts.web_port, None);
    }
}
