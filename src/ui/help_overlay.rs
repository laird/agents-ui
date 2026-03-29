use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Row, Table};
use ratatui::Frame;

use crate::config::keybindings::KeyBindings;

/// Render a help overlay showing current keybindings.
pub fn render_help_overlay(f: &mut Frame, area: Rect, keybindings: &KeyBindings) {
    // Center a box in the middle of the screen
    let width = 50u16.min(area.width.saturating_sub(4));
    let entries = keybindings.help_entries();
    let height = (entries.len() as u16 + 4).min(area.height.saturating_sub(2));

    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    // Clear the area behind the popup
    f.render_widget(Clear, popup_area);

    let rows: Vec<Row> = entries
        .iter()
        .map(|(action, keys)| {
            Row::new(vec![
                Span::styled(keys.clone(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(action.clone(), Style::default().fg(Color::White)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [Constraint::Length(16), Constraint::Fill(1)],
    )
    .header(
        Row::new(vec![
            Span::styled("Key", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled("Action", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ])
    )
    .block(
        Block::default()
            .title(" Keyboard Shortcuts ")
            .title_bottom(Line::from(" Press ? or Esc to close ").style(Style::default().fg(Color::DarkGray)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(table, popup_area);
}
