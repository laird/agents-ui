use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
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
    // Calculate dialog height based on input length (repo paths can be long)
    let dialog_width_pct = 60u16;
    let approx_inner_width = (area.width as usize * dialog_width_pct as usize / 100).saturating_sub(4);
    let input_text = format!(" > {}█", input);
    let input_lines = if approx_inner_width > 0 {
        ((input_text.len() + approx_inner_width - 1) / approx_inner_width).max(1)
    } else {
        1
    };
    let input_field_height = (input_lines as u16).min(4).max(2);
    let dialog_height = 8 + input_field_height; // instructions + input + workers + help + borders

    // Center a dialog box
    let dialog_area = centered_rect(dialog_width_pct, dialog_height, area);

    // Clear background
    f.render_widget(Clear, dialog_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Launch New Swarm ")
        .border_style(theme::title_style());

    let inner = block.inner(dialog_area);
    f.render_widget(block, dialog_area);

    let chunks = Layout::vertical([
        Constraint::Length(2),                // Instructions
        Constraint::Length(input_field_height), // Repo path / input field (expands)
        Constraint::Length(2),                // Workers field
        Constraint::Length(2),                // Help
    ])
    .split(inner);

    match field {
        NewSwarmField::RepoPath => {
            let instructions = Paragraph::new(Line::from(Span::styled(
                " Enter the path to the repository:",
                theme::help_style(),
            )));
            f.render_widget(instructions, chunks[0]);

            let input_widget = Paragraph::new(Span::styled(
                &input_text,
                theme::input_style(),
            ))
            .wrap(Wrap { trim: false });
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
            let repo_line = Paragraph::new(Span::styled(
                repo_display,
                theme::help_style(),
            ))
            .wrap(Wrap { trim: false });
            f.render_widget(repo_line, chunks[0]);

            let prompt = Paragraph::new(Line::from(Span::styled(
                " Number of workers (default: 2):",
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
                Span::styled("↑↓", theme::title_style()),
                Span::styled(" adjust  ", theme::help_style()),
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
