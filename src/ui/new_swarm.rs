use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::NewSwarmField;
use crate::model::swarm::{AgentType, ALL_AGENT_TYPES};
use super::text_input::TextInput;
use super::theme;

pub fn render_new_swarm_dialog(
    f: &mut Frame,
    area: Rect,
    field: &NewSwarmField,
    input: &TextInput,
    repo_path: &str,
    agent_type: &AgentType,
) {
    // Center a dialog box
    let dialog_area = centered_rect(60, 14, area);

    // Clear background
    f.render_widget(Clear, dialog_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Launch New Swarm ")
        .border_style(theme::title_style());

    let inner = block.inner(dialog_area);
    f.render_widget(block, dialog_area);

    let chunks = Layout::vertical([
        Constraint::Length(2), // Instructions / context
        Constraint::Length(2), // Field 1
        Constraint::Length(2), // Field 2
        Constraint::Length(2), // Field 3
        Constraint::Length(2), // Help
    ])
    .split(inner);

    match field {
        NewSwarmField::RepoPath => {
            let instructions = Paragraph::new(Line::from(Span::styled(
                " Enter the path to the repository:",
                theme::help_style(),
            )));
            f.render_widget(instructions, chunks[0]);

            let input_line = input.render_line(" > ");
            let input_widget = Paragraph::new(input_line);
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
        NewSwarmField::AgentRuntime => {
            let repo_display = format!(" Repo: {repo_path}");
            let repo_line = Paragraph::new(Line::from(Span::styled(
                repo_display,
                theme::help_style(),
            )));
            f.render_widget(repo_line, chunks[0]);

            let prompt = Paragraph::new(Line::from(Span::styled(
                " Select agent runtime:",
                theme::help_style(),
            )));
            f.render_widget(prompt, chunks[1]);

            // Show all agent types with the selected one highlighted
            let type_spans: Vec<Span> = ALL_AGENT_TYPES
                .iter()
                .flat_map(|t| {
                    let label = format!(" {} ", t);
                    if *t == *agent_type {
                        vec![Span::styled(
                            format!("[{label}]"),
                            theme::selected_style(),
                        )]
                    } else {
                        vec![Span::styled(
                            format!(" {label} "),
                            theme::help_style(),
                        )]
                    }
                })
                .collect();
            let type_widget = Paragraph::new(Line::from(type_spans));
            f.render_widget(type_widget, chunks[2]);

            let help = Paragraph::new(Line::from(vec![
                Span::styled(" ↑/↓", theme::title_style()),
                Span::styled(" select  ", theme::help_style()),
                Span::styled("Enter", theme::title_style()),
                Span::styled(" confirm  ", theme::help_style()),
                Span::styled("Esc", theme::title_style()),
                Span::styled(" back", theme::help_style()),
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

            let runtime_display = format!(" Runtime: {agent_type}");
            let runtime_line = Paragraph::new(Line::from(Span::styled(
                runtime_display,
                theme::help_style(),
            )));
            f.render_widget(runtime_line, chunks[1]);

            let prompt = Paragraph::new(Line::from(Span::styled(
                " Number of workers:",
                theme::help_style(),
            )));
            f.render_widget(prompt, chunks[2]);

            let input_line = input.render_line(" > ");
            let input_widget = Paragraph::new(input_line);
            f.render_widget(input_widget, chunks[3]);

            let help = Paragraph::new(Line::from(vec![
                Span::styled(" Enter", theme::title_style()),
                Span::styled(" launch  ", theme::help_style()),
                Span::styled("Esc", theme::title_style()),
                Span::styled(" back", theme::help_style()),
            ]));
            f.render_widget(help, chunks[4]);
        }
        NewSwarmField::Launching => {
            let msg = Paragraph::new(Line::from(Span::styled(
                " Launching swarm... please wait",
                theme::title_style(),
            )));
            f.render_widget(msg, chunks[1]);

            let help = Paragraph::new(Line::from(vec![
                Span::styled(" Esc", theme::title_style()),
                Span::styled(" cancel", theme::help_style()),
            ]));
            f.render_widget(help, chunks[4]);
        }
    }
}

/// Create a centered rect of given percentage width and fixed height.
fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
