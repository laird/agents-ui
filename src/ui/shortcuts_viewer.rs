use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::config::shortcuts::ShortcutsConfig;
use super::theme;

/// Render the shortcuts viewer as a centered overlay.
pub fn render_shortcuts_viewer(
    f: &mut Frame,
    area: Rect,
    config: &ShortcutsConfig,
    active_panel: &str,
) {
    // Center the popup: 60% width, 70% height
    let popup_width = (area.width as f32 * 0.6).max(40.0).min(80.0) as u16;
    let popup_height = (area.height as f32 * 0.7).max(10.0).min(30.0) as u16;
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(format!(" Shortcuts — {} ", active_panel))
        .borders(Borders::ALL)
        .border_style(theme::title_style());

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Build lines for each panel's shortcuts
    let mut lines: Vec<Line> = Vec::new();

    // Active panel first
    let panel_shortcuts = config.for_panel(active_panel);
    if !panel_shortcuts.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("  [{active_panel}]"),
            theme::header_style(),
        )));
        for (key, shortcut) in panel_shortcuts {
            lines.push(Line::from(vec![
                Span::styled(format!("    {:<8}", key), theme::title_style()),
                Span::styled(&shortcut.label, ratatui::style::Style::default().fg(ratatui::style::Color::White)),
                Span::styled(format!("  → {}", shortcut.command), theme::help_style()),
            ]));
        }
        lines.push(Line::from(""));
    }

    // Global shortcuts
    if active_panel != "global" && !config.global.is_empty() {
        lines.push(Line::from(Span::styled(
            "  [global]",
            theme::header_style(),
        )));
        for (key, shortcut) in &config.global {
            lines.push(Line::from(vec![
                Span::styled(format!("    {:<8}", key), theme::title_style()),
                Span::styled(&shortcut.label, ratatui::style::Style::default().fg(ratatui::style::Color::White)),
                Span::styled(format!("  → {}", shortcut.command), theme::help_style()),
            ]));
        }
        lines.push(Line::from(""));
    }

    // Help footer
    lines.push(Line::from(vec![
        Span::styled("  e", theme::title_style()),
        Span::styled(" edit config  ", theme::help_style()),
        Span::styled("?", theme::title_style()),
        Span::styled(" close  ", theme::help_style()),
        Span::styled("any key", theme::title_style()),
        Span::styled(" dismiss", theme::help_style()),
    ]));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(paragraph, inner);
}
