use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Row, Table};
use ratatui::Frame;

use crate::config::keybindings::KeyBindings;

/// Render a help overlay showing current keybindings.
///
/// If `context_entries` is `Some`, a second section titled "Issues Panel" is rendered
/// below the global keys showing context-specific bindings.
pub fn render_help_overlay(f: &mut Frame, area: Rect, keybindings: &KeyBindings, context_entries: Option<&[(&str, &str)]>) {
    // Center a box in the middle of the screen
    let width = 50u16.min(area.width.saturating_sub(4));
    let global_entries = keybindings.help_entries();

    // Total rows: global header + global entries + optional (separator + context header + context entries)
    let context_row_count = context_entries
        .map(|e| e.len() as u16 + 2) // separator row + header row + entries
        .unwrap_or(0);
    let total_rows = global_entries.len() as u16 + 2 + context_row_count; // +2 for global header row
    let height = (total_rows + 2).min(area.height.saturating_sub(2)); // +2 for border

    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    // Clear the area behind the popup
    f.render_widget(Clear, popup_area);

    let mut rows: Vec<Row> = global_entries
        .iter()
        .map(|(action, keys)| {
            Row::new(vec![
                Span::styled(keys.clone(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(action.clone(), Style::default().fg(Color::White)),
            ])
        })
        .collect();

    if let Some(ctx) = context_entries {
        // Blank separator row
        rows.push(Row::new(vec![Span::raw(""), Span::raw("")]));
        // Section header row
        rows.push(Row::new(vec![
            Span::styled("── Issues Panel ─", Style::default().fg(Color::Yellow)),
            Span::raw(""),
        ]));
        for (action, key) in ctx {
            rows.push(Row::new(vec![
                Span::styled(*key, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(*action, Style::default().fg(Color::White)),
            ]));
        }
    }

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
