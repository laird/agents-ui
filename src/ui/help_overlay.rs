use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Row, Table};
use ratatui::Frame;

use crate::config::keybindings::KeyBindings;

/// Render a help overlay showing current keybindings.
///
/// When `context_entries` is `Some`, a second section titled with the context label is
/// rendered below the global keys, separated by a blank row.
pub fn render_help_overlay(
    f: &mut Frame,
    area: Rect,
    keybindings: &KeyBindings,
    context_entries: Option<(&str, &[(&str, &str)])>,
) {
    let width = 50u16.min(area.width.saturating_sub(4));
    let global_entries = keybindings.help_entries();

    // Extra rows: blank separator + section header + context entries
    let context_extra = context_entries
        .map(|(_, entries)| 1 + 1 + entries.len()) // blank + header + entries
        .unwrap_or(0);
    let height = (global_entries.len() as u16 + 4 + context_extra as u16)
        .min(area.height.saturating_sub(2));

    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

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

    if let Some((section_label, entries)) = context_entries {
        // Blank separator row
        rows.push(Row::new(vec![Span::raw(""), Span::raw("")]));

        // Section header row
        rows.push(Row::new(vec![
            Span::raw(""),
            Span::styled(
                format!("── {} ──", section_label),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ]));

        // Context-specific key entries
        for (key, action) in entries {
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
