use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{CreateIssueField, CreateIssueForm, InstallScope, NewSwarmField, BLOCKING_LABELS};
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
    form: &CreateIssueForm,
) {
    use ratatui::style::{Modifier, Style};

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Create GitHub Issue ")
        .border_style(theme::title_style());

    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(1), // Title label
        Constraint::Length(1), // Title input
        Constraint::Length(1), // Priority label + selector
        Constraint::Length(1), // Type label + selector
        Constraint::Length(1), // Labels label
        Constraint::Length(1), // Labels row 1
        Constraint::Length(1), // Labels row 2
        Constraint::Min(0),   // spacer
        Constraint::Length(1), // help
    ])
    .split(inner);

    let focused_style = Style::default().fg(ratatui::style::Color::Cyan).add_modifier(Modifier::BOLD);
    let normal_label = theme::help_style();

    // -- Title field --
    let title_label_style = if form.field == CreateIssueField::Title { focused_style } else { normal_label };
    f.render_widget(Paragraph::new(Line::from(Span::styled(" Title:", title_label_style))), chunks[0]);

    let cursor = if form.field == CreateIssueField::Title { "█" } else { "" };
    let input_display = format!(" > {}{}", form.title, cursor);
    f.render_widget(Paragraph::new(Line::from(Span::styled(input_display, theme::input_style()))), chunks[1]);

    // -- Priority field --
    let pri_label_style = if form.field == CreateIssueField::Priority { focused_style } else { normal_label };
    let priorities = [
        crate::app::IssuePriority::P0,
        crate::app::IssuePriority::P1,
        crate::app::IssuePriority::P2,
        crate::app::IssuePriority::P3,
    ];
    let mut pri_spans = vec![Span::styled(" Priority: ", pri_label_style)];
    for p in &priorities {
        let selected = *p == form.priority;
        let marker = if selected { "(●) " } else { "( ) " };
        let label = format!("{}{} ", marker, p.desc());
        let style = if selected {
            Style::default().fg(ratatui::style::Color::White).add_modifier(Modifier::BOLD)
        } else {
            normal_label
        };
        pri_spans.push(Span::styled(label, style));
    }
    f.render_widget(Paragraph::new(Line::from(pri_spans)), chunks[2]);

    // -- Type field --
    let type_label_style = if form.field == CreateIssueField::IssueType { focused_style } else { normal_label };
    let bug_sel = form.issue_type == crate::app::IssueType::Bug;
    let mut type_spans = vec![Span::styled(" Type:     ", type_label_style)];
    for (is_sel, label) in [(bug_sel, "bug"), (!bug_sel, "enhancement")] {
        let marker = if is_sel { "(●) " } else { "( ) " };
        let text = format!("{}{} ", marker, label);
        let style = if is_sel {
            Style::default().fg(ratatui::style::Color::White).add_modifier(Modifier::BOLD)
        } else {
            normal_label
        };
        type_spans.push(Span::styled(text, style));
    }
    f.render_widget(Paragraph::new(Line::from(type_spans)), chunks[3]);

    // -- Labels field --
    let lbl_label_style = if form.field == CreateIssueField::Labels { focused_style } else { normal_label };
    f.render_widget(Paragraph::new(Line::from(Span::styled(" Labels:", lbl_label_style))), chunks[4]);

    // Render blocking labels in two rows of 3
    for row in 0..2 {
        let start = row * 3;
        let end = (start + 3).min(BLOCKING_LABELS.len());
        let mut spans = vec![Span::styled(" ", normal_label)];
        for i in start..end {
            let checked = if form.label_toggles[i] { "[x] " } else { "[ ] " };
            let text = format!("{}{}", checked, BLOCKING_LABELS[i]);
            let is_cursor = form.field == CreateIssueField::Labels && form.label_cursor == i;
            let style = if is_cursor {
                Style::default().fg(ratatui::style::Color::White).add_modifier(Modifier::UNDERLINED)
            } else if form.label_toggles[i] {
                Style::default().fg(ratatui::style::Color::Yellow)
            } else {
                normal_label
            };
            spans.push(Span::styled(text, style));
            spans.push(Span::styled("  ", normal_label));
        }
        f.render_widget(Paragraph::new(Line::from(spans)), chunks[5 + row]);
    }

    // -- Help bar --
    let help = Paragraph::new(Line::from(vec![
        Span::styled(" Enter", theme::title_style()),
        Span::styled(" create  ", theme::help_style()),
        Span::styled("Tab", theme::title_style()),
        Span::styled("/", theme::help_style()),
        Span::styled("↑↓", theme::title_style()),
        Span::styled(" navigate  ", theme::help_style()),
        Span::styled("←→", theme::title_style()),
        Span::styled("/", theme::help_style()),
        Span::styled("Space", theme::title_style()),
        Span::styled(" select  ", theme::help_style()),
        Span::styled("Esc", theme::title_style()),
        Span::styled(" cancel", theme::help_style()),
    ]));
    f.render_widget(help, chunks[8]);
}

#[cfg(test)]
mod tests {
    use super::{
        render_create_issue_dialog, render_install_scope_dialog, render_new_swarm_dialog,
        render_runtime_dialog,
    };
    use crate::app::{CreateIssueForm, InstallScope, NewSwarmField};
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
    fn create_issue_dialog_shows_form_fields() {
        let mut form = CreateIssueForm::new();
        form.title = "Broken reconnect flow".to_string();
        let rendered = rendered_text(|terminal| {
            terminal
                .draw(|f| render_create_issue_dialog(f, f.area(), &form))
                .unwrap();
        });

        assert!(rendered.contains("Create GitHub Issue"));
        assert!(rendered.contains("Broken reconnect flow"));
        assert!(rendered.contains("Priority"));
        assert!(rendered.contains("Medium")); // Default P2
        assert!(rendered.contains("bug")); // Default type
        assert!(rendered.contains("enhancement"));
        assert!(rendered.contains("Labels"));
        assert!(rendered.contains("needs-design"));
        assert!(rendered.contains("proposal"));
        assert!(rendered.contains("create"));
    }
}
