use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{InstallScope, NewSwarmField};
use crate::model::swarm::AgentType;
use super::theme;

pub fn render_runtime_dialog(
    f: &mut Frame,
    area: Rect,
    selected: AgentType,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Select Runtime ")
        .border_style(theme::title_style());

    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .split(inner);

    let instructions = Paragraph::new(Line::from(Span::styled(
        " Choose default runtime (saved in repo):",
        theme::help_style(),
    )));
    f.render_widget(instructions, chunks[0]);

    let claude_style = if selected == AgentType::Claude {
        theme::input_style()
    } else {
        theme::help_style()
    };
    let codex_style = if selected == AgentType::Codex {
        theme::input_style()
    } else {
        theme::help_style()
    };
    let droid_style = if selected == AgentType::Droid {
        theme::input_style()
    } else {
        theme::help_style()
    };

    let claude = Paragraph::new(Line::from(Span::styled(
        " > Claude Code (autocoder plugin from ../agents)",
        claude_style,
    )));
    f.render_widget(claude, chunks[1]);

    let codex = Paragraph::new(Line::from(Span::styled(
        " > Codex (.codex workflows)",
        codex_style,
    )));
    f.render_widget(codex, chunks[2]);

    let droid = Paragraph::new(Line::from(Span::styled(
        " > Droid (.factory workflows)",
        droid_style,
    )));
    f.render_widget(droid, chunks[3]);

    let help = Paragraph::new(Line::from(vec![
        Span::styled(" ↑/↓", theme::title_style()),
        Span::styled(" choose  ", theme::help_style()),
        Span::styled("c", theme::title_style()),
        Span::styled(" claude  ", theme::help_style()),
        Span::styled("x", theme::title_style()),
        Span::styled(" codex  ", theme::help_style()),
        Span::styled("d", theme::title_style()),
        Span::styled(" droid  ", theme::help_style()),
        Span::styled("Enter", theme::title_style()),
        Span::styled(" confirm", theme::help_style()),
    ]));
    f.render_widget(help, chunks[4]);
}

pub fn render_install_scope_dialog(
    f: &mut Frame,
    area: Rect,
    selected: InstallScope,
    agent_type: AgentType,
    repo_path: String,
) {
    let (title_text, message_text) = match agent_type {
        AgentType::Droid => (
            " Install Droid Agents ",
            " `laird/agents` is not installed for Droid.",
        ),
        AgentType::Codex => (
            " Install Codex Agents ",
            " Codex runtime assets are not installed for this repo.",
        ),
        _ => (
            " Install Runtime Assets ",
            " Required runtime assets are not installed.",
        ),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title_text)
        .border_style(theme::title_style());

    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .split(inner);

    let title = Paragraph::new(Line::from(Span::styled(
        message_text,
        theme::help_style(),
    )));
    f.render_widget(title, chunks[0]);

    let repo = Paragraph::new(Line::from(Span::styled(
        format!(" Repo: {repo_path}"),
        theme::help_style(),
    )));
    f.render_widget(repo, chunks[1]);

    let user_style = if selected == InstallScope::User {
        theme::input_style()
    } else {
        theme::help_style()
    };
    let repo_style = if selected == InstallScope::Repo {
        theme::input_style()
    } else {
        theme::help_style()
    };

    let user_option = Paragraph::new(Line::from(Span::styled(
        " > Install for user (--scope user)",
        user_style,
    )));
    f.render_widget(user_option, chunks[2]);

    let repo_option = Paragraph::new(Line::from(Span::styled(
        " > Install for repo (--scope project)",
        repo_style,
    )));
    f.render_widget(repo_option, chunks[3]);

    let help = Paragraph::new(Line::from(vec![
        Span::styled(" ↑/↓", theme::title_style()),
        Span::styled(" choose  ", theme::help_style()),
        Span::styled("u", theme::title_style()),
        Span::styled(" user  ", theme::help_style()),
        Span::styled("r", theme::title_style()),
        Span::styled(" repo  ", theme::help_style()),
        Span::styled("Enter", theme::title_style()),
        Span::styled(" install", theme::help_style()),
    ]));
    f.render_widget(help, chunks[5]);
}

pub fn render_new_swarm_dialog(
    f: &mut Frame,
    area: Rect,
    field: &NewSwarmField,
    input: &str,
    repo_path: &str,
) {
    // Use the full screen area with a bordered block
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Launch New Swarm ")
        .border_style(theme::title_style());

    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .split(inner);

    match field {
        NewSwarmField::RepoPath => {
            let instructions = Paragraph::new(Line::from(Span::styled(
                " Enter the path to the repository:",
                theme::help_style(),
            )));
            f.render_widget(instructions, chunks[0]);

            let input_display = format!(" > {}█", input);
            let input_widget = Paragraph::new(Line::from(Span::styled(
                input_display,
                theme::input_style(),
            )));
            f.render_widget(input_widget, chunks[1]);

            let help = Paragraph::new(Line::from(vec![
                Span::styled(" Enter", theme::title_style()),
                Span::styled(" confirm  ", theme::help_style()),
                Span::styled("Tab", theme::title_style()),
                Span::styled(" complete  ", theme::help_style()),
                Span::styled("Esc", theme::title_style()),
                Span::styled(" cancel", theme::help_style()),
            ]));
            f.render_widget(help, chunks[4]);
        }
        NewSwarmField::NumWorkers => {
            let repo_display = format!(" Repo: {repo_path}");
            let repo_line = Paragraph::new(Line::from(Span::styled(
                repo_display,
                theme::help_style(),
            )));
            f.render_widget(repo_line, chunks[0]);

            let prompt = Paragraph::new(Line::from(Span::styled(
                " Number of workers (↑/↓ to adjust):",
                theme::help_style(),
            )));
            f.render_widget(prompt, chunks[1]);

            let input_display = format!(" > {}█", input);
            let input_widget = Paragraph::new(Line::from(Span::styled(
                input_display,
                theme::input_style(),
            )));
            f.render_widget(input_widget, chunks[2]);

            let help = Paragraph::new(Line::from(vec![
                Span::styled(" Enter", theme::title_style()),
                Span::styled(" launch  ", theme::help_style()),
                Span::styled("Esc", theme::title_style()),
                Span::styled(" back", theme::help_style()),
            ]));
            f.render_widget(help, chunks[4]);
        }
    }
}

pub fn render_create_issue_dialog(
    f: &mut Frame,
    area: Rect,
    input: &str,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Create GitHub Issue ")
        .border_style(theme::title_style());

    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .split(inner);

    let instructions = Paragraph::new(Line::from(Span::styled(
        " Describe the issue:",
        theme::help_style(),
    )));
    f.render_widget(instructions, chunks[0]);

    let input_display = format!(" > {}█", input);
    let input_widget = Paragraph::new(Line::from(Span::styled(
        input_display,
        theme::input_style(),
    )));
    f.render_widget(input_widget, chunks[1]);

    let help = Paragraph::new(Line::from(vec![
        Span::styled(" Enter", theme::title_style()),
        Span::styled(" create  ", theme::help_style()),
        Span::styled("Esc", theme::title_style()),
        Span::styled(" cancel", theme::help_style()),
    ]));
    f.render_widget(help, chunks[3]);
}

#[cfg(test)]
mod tests {
    use super::{
        render_create_issue_dialog, render_install_scope_dialog, render_new_swarm_dialog,
        render_runtime_dialog,
    };
    use crate::app::{InstallScope, NewSwarmField};
    use crate::model::swarm::AgentType;
    use ratatui::{backend::TestBackend, Terminal};

    fn rendered_text<F>(draw_fn: F) -> String
    where
        F: FnOnce(&mut Terminal<TestBackend>),
    {
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        draw_fn(&mut terminal);
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
    }

    #[test]
    fn runtime_dialog_shows_supported_runtimes() {
        let rendered = rendered_text(|terminal| {
            terminal
                .draw(|f| render_runtime_dialog(f, f.area(), AgentType::Codex))
                .unwrap();
        });

        assert!(rendered.contains("Select Runtime"));
        assert!(rendered.contains("Claude Code"));
        assert!(rendered.contains("Codex"));
        assert!(rendered.contains("Droid"));
    }

    #[test]
    fn install_scope_dialog_shows_repo_and_options() {
        let rendered = rendered_text(|terminal| {
            terminal
                .draw(|f| {
                    render_install_scope_dialog(
                        f,
                        f.area(),
                        InstallScope::Repo,
                        AgentType::Codex,
                        "/tmp/demo".to_string(),
                    )
                })
                .unwrap();
        });

        assert!(rendered.contains("Install Codex Agents"));
        assert!(rendered.contains("/tmp/demo"));
        assert!(rendered.contains("--scope user"));
        assert!(rendered.contains("--scope project"));
    }

    #[test]
    fn new_swarm_dialog_shows_input_state() {
        let rendered = rendered_text(|terminal| {
            terminal
                .draw(|f| {
                    render_new_swarm_dialog(
                        f,
                        f.area(),
                        &NewSwarmField::NumWorkers,
                        "3",
                        "/tmp/demo",
                    )
                })
                .unwrap();
        });

        assert!(rendered.contains("Launch New Swarm"));
        assert!(rendered.contains("Repo: /tmp/demo"));
        assert!(rendered.contains("> 3"));
    }

    #[test]
    fn create_issue_dialog_shows_prompt() {
        let rendered = rendered_text(|terminal| {
            terminal
                .draw(|f| render_create_issue_dialog(f, f.area(), "Broken reconnect flow"))
                .unwrap();
        });

        assert!(rendered.contains("Create GitHub Issue"));
        assert!(rendered.contains("Broken reconnect flow"));
        assert!(rendered.contains("create"));
    }
}
