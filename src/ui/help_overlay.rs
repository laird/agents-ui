use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Row, Table};

use crate::config::keybindings::KeyBindings;

/// Render a help overlay showing current keybindings.
///
/// `context_entries` renders an additional "Issues Panel" section when provided
/// (used when the Issues panel is focused in Swarm View).
pub fn render_help_overlay(
    f: &mut Frame,
    area: Rect,
    keybindings: &KeyBindings,
    context_entries: Option<&[(&str, &str)]>,
) {
    let global_entries = keybindings.help_entries();

    // Build rows: global entries, then optional context section
    let mut rows: Vec<Row> = global_entries
        .iter()
        .map(|(action, keys)| {
            Row::new(vec![
                Span::styled(
                    keys.clone(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(action.clone(), Style::default().fg(Color::White)),
            ])
        })
        .collect();

    if let Some(ctx) = context_entries {
        // Separator row
        rows.push(Row::new(vec![
            Span::styled("── Issues Panel ──", Style::default().fg(Color::DarkGray)),
            Span::raw(""),
        ]));
        for (action, key) in ctx {
            rows.push(Row::new(vec![
                Span::styled(
                    *key,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(*action, Style::default().fg(Color::White)),
            ]));
        }
    }

    let row_count = rows.len() as u16;
    let width = 50u16.min(area.width.saturating_sub(4));
    let height = (row_count + 4).min(area.height.saturating_sub(2));

    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    f.render_widget(Clear, popup_area);

    let table = Table::new(rows, [Constraint::Length(16), Constraint::Fill(1)])
        .header(Row::new(vec![
            Span::styled(
                "Key",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Action",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .block(
            Block::default()
                .title(" Keyboard Shortcuts ")
                .title_bottom(
                    Line::from(" Press ? or Esc to close ")
                        .style(Style::default().fg(Color::DarkGray)),
                )
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

    f.render_widget(table, popup_area);
}

/// Issue panel context entries for the help overlay.
pub const ISSUES_PANEL_CONTEXT: &[(&str, &str)] = &[
    ("cycle status filter", "f"),
    ("cycle type filter", "t"),
    ("cycle priority filter", "P"),
    ("clear all filters", "c"),
    ("search issues", "/"),
    ("add new issue", "a"),
    ("dispatch to agent", "d / Space"),
    ("approve issue", "p"),
    ("next blocked issue", "b"),
    ("refresh issues", "r"),
    ("review-blocked", "R"),
    ("open in browser", "g"),
    ("release working label", "u"),
    ("view issue detail", "Enter"),
];
