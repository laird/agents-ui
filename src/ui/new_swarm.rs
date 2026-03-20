use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::NewSwarmField;
use super::theme;

pub fn render_new_swarm_dialog(
    f: &mut Frame,
    area: Rect,
    field: &NewSwarmField,
    input: &str,
    repo_path: &str,
) {
    // Center a dialog box
    let dialog_area = centered_rect(60, 12, area);

    // Clear background
    f.render_widget(Clear, dialog_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Launch New Swarm ")
        .border_style(theme::title_style());

    let inner = block.inner(dialog_area);
    f.render_widget(block, dialog_area);

    let chunks = Layout::vertical([
        Constraint::Length(2), // Instructions
        Constraint::Length(2), // Repo path field
        Constraint::Length(2), // Workers field
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
            f.render_widget(help, chunks[3]);
        }
        NewSwarmField::NumWorkers => {
            let repo_display = format!(" Repo: {repo_path}");
            let repo_line = Paragraph::new(Line::from(Span::styled(
                repo_display,
                theme::help_style(),
            )));
            f.render_widget(repo_line, chunks[0]);

            let prompt = Paragraph::new(Line::from(Span::styled(
                " Number of workers:",
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
            f.render_widget(help, chunks[3]);
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
            f.render_widget(help, chunks[3]);
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
