mod app;
mod event;
mod tui;
mod model;
mod tmux;
mod adapter;
mod scripts;
mod ui;
mod config;

use anyhow::Result;
use model::swarm::AgentType;

#[tokio::main]
async fn main() -> Result<()> {
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

    let default_agent_type = parse_agent_type(std::env::args().skip(1));

    let mut terminal = tui::init()?;
    let mut app = app::App::new(default_agent_type).await?;
    let result = app.run(&mut terminal).await;
    tui::restore()?;
    result
}

fn parse_agent_type<I>(args: I) -> AgentType
where
    I: IntoIterator<Item = String>,
{
    let mut selected = AgentType::Claude;
    let mut expect_agent_value = false;

    for arg in args {
        if expect_agent_value {
            selected = match arg.as_str() {
                "claude" => AgentType::Claude,
                "codex" => AgentType::Codex,
                "droid" => AgentType::Droid,
                "gemini" => AgentType::Gemini,
                _ => selected,
            };
            expect_agent_value = false;
            continue;
        }

        if arg == "--agent" {
            expect_agent_value = true;
            continue;
        }

        selected = match arg.as_str() {
            "--claude" => AgentType::Claude,
            "--codex" => AgentType::Codex,
            "--droid" => AgentType::Droid,
            "--gemini" => AgentType::Gemini,
            _ if arg.starts_with("--agent=") => {
                match arg.trim_start_matches("--agent=") {
                    "claude" => AgentType::Claude,
                    "codex" => AgentType::Codex,
                    "droid" => AgentType::Droid,
                    "gemini" => AgentType::Gemini,
                    _ => selected,
                }
            }
            _ => selected,
        };
    }

    selected
}

#[cfg(test)]
mod tests {
    use super::parse_agent_type;
    use crate::model::swarm::AgentType;

    #[test]
    fn parse_defaults_to_claude() {
        let args: Vec<String> = vec![];
        assert_eq!(parse_agent_type(args), AgentType::Claude);
    }

    #[test]
    fn parse_short_runtime_flags() {
        assert_eq!(
            parse_agent_type(vec!["--droid".to_string()]),
            AgentType::Droid
        );
        assert_eq!(
            parse_agent_type(vec!["--codex".to_string()]),
            AgentType::Codex
        );
        assert_eq!(
            parse_agent_type(vec!["--gemini".to_string()]),
            AgentType::Gemini
        );
    }

    #[test]
    fn parse_agent_value_flag() {
        assert_eq!(
            parse_agent_type(vec!["--agent".to_string(), "droid".to_string()]),
            AgentType::Droid
        );
        assert_eq!(
            parse_agent_type(vec!["--agent=codex".to_string()]),
            AgentType::Codex
        );
    }
}
