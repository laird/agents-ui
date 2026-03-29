use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Row, Table};
use ratatui::Frame;

use crate::config::keybindings::KeyBindings;

/// Render a help overlay showing current keybindings.
/// When `context_entries` is provided, a second section with context-specific keys is shown.
pub fn render_help_overlay(
    f: &mut Frame,
    area: Rect,
    keybindings: &KeyBindings,
    context_entries: Option<&[(&str, &str)]>,
) {
    let width = 50u16.min(area.width.saturating_sub(4));
    let global_entries = keybindings.help_entries();

    // Build all rows: global header + global entries + optional context section
    let mut all_rows: Vec<Row> = global_entries
        .iter()
        .map(|(action, keys)| {
            Row::new(vec![
                Span::styled(keys.clone(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(action.clone(), Style::default().fg(Color::White)),
            ])
        })
        .collect();

    if let Some(ctx) = context_entries {
        // Separator row
        all_rows.push(Row::new(vec![
            Span::styled("── Issues Panel ──", Style::default().fg(Color::Yellow)),
            Span::raw(""),
        ]));
        for (key, action) in ctx {
            all_rows.push(Row::new(vec![
                Span::styled(*key, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(*action, Style::default().fg(Color::White)),
            ]));
        }
    }

    let total_rows = all_rows.len() as u16;
    // +4 for border (2) + header row (1) + padding (1)
    let height = (total_rows + 4).min(area.height.saturating_sub(2));

    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    f.render_widget(Clear, popup_area);

    let table = Table::new(
        all_rows,
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

/// Keybindings for the Issues panel in Swarm View.
pub const ISSUES_PANEL_ENTRIES: &[(&str, &str)] = &[
    ("f", "Cycle status filter"),
    ("t", "Cycle type filter"),
    ("P", "Cycle priority filter"),
    ("c", "Clear all filters"),
    ("/", "Search issues"),
    ("a", "Add new issue"),
    ("d / Space", "Dispatch to agent"),
    ("p", "Approve issue"),
    ("b", "Next blocked issue"),
    ("r", "Review-blocked (manager)"),
    ("g", "Open in browser"),
    ("u", "Release stuck issue"),
    ("Enter", "View issue detail"),
];
