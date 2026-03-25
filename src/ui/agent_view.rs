use ansi_to_tui::IntoText;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::model::swarm::AgentInfo;
use super::theme;

pub struct AgentView {
    pub input: String,
    pub scroll_offset: u16,
}

impl AgentView {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            scroll_offset: 0,
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, agent: &AgentInfo) {
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(area);

        // Title
        let role = if agent.is_manager {
            "Manager"
        } else {
            "Worker"
        };
        let id_label = format!("  {} ", agent.id);
        let role_label = format!("[{role}] ");
        let status_label = agent.status.state.to_string();
        let path_label = format!("  {}", agent.worktree_path.display());

        let title = Paragraph::new(Line::from(vec![
            Span::styled(id_label, theme::title_style()),
            Span::styled(role_label, theme::help_style()),
            Span::styled(status_label, theme::status_style(&agent.status.state)),
            Span::styled(path_label, theme::help_style()),
        ]))
        .block(Block::default().borders(Borders::BOTTOM));
        f.render_widget(title, chunks[0]);

        // Pane output — parse ANSI escape codes for colors
        let content = &agent.pane_content;
        let text = content
            .as_bytes()
            .into_text()
            .unwrap_or_else(|_| Text::raw(content.clone()));
        let total_lines = text.lines.len() as u16;

        let visible_height = chunks[1].height.saturating_sub(2);
        let max_scroll = total_lines.saturating_sub(visible_height);
        if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }

        let at_bottom = self.scroll_offset >= max_scroll;
        let scroll_indicator = if !at_bottom && total_lines > 0 {
            format!(
                " Session — {} [line {}/{}] ",
                agent.tmux_target,
                self.scroll_offset + 1,
                total_lines,
            )
        } else {
            format!(" Session — {} ", agent.tmux_target)
        };
        let pane_output = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(scroll_indicator))
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset, 0));
        f.render_widget(pane_output, chunks[1]);

        // Passthrough mode indicator
        let mode_info = Paragraph::new(Line::from(Span::styled(
            " Keys forwarded to agent — Tab, /, etc. work natively",
            theme::help_style(),
        )))
        .block(Block::default().borders(Borders::ALL).title(" Passthrough Mode "));
        f.render_widget(mode_info, chunks[2]);

        // Help
        let help = Paragraph::new(Line::from(vec![
            Span::styled(" Esc Esc", theme::title_style()),
            Span::styled(" back  ", theme::help_style()),
            Span::styled("⌥0", theme::title_style()),
            Span::styled(" overview  ", theme::help_style()),
            Span::styled("⌥m", theme::title_style()),
            Span::styled(" mgr  ", theme::help_style()),
            Span::styled("⌥1-9", theme::title_style()),
            Span::styled(" worker  ", theme::help_style()),
            Span::styled("PgUp/Dn", theme::title_style()),
            Span::styled(" scroll  ", theme::help_style()),
            Span::styled("Home/End", theme::title_style()),
            Span::styled(" top/bottom  ", theme::help_style()),
        ]))
        .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, chunks[3]);
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = u16::MAX;
    }
}
