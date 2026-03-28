use ansi_to_tui::IntoText;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::model::swarm::AgentInfo;
use super::text_input::TextInput;
use super::theme;

pub struct AgentView {
    pub input: TextInput,
    pub scroll_offset: u16,
    /// Height of the visible pane area (updated each render).
    pub visible_height: u16,
    /// Whether the view should auto-follow new content (true when at bottom).
    pub following: bool,
}

impl AgentView {
    pub fn new() -> Self {
        Self {
            input: TextInput::new(),
            scroll_offset: 0,
            visible_height: 20,
            following: true,
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, agent: &AgentInfo) {
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

        // Title
        let role = if agent.role == "tester" {
            "Tester"
        } else if agent.is_manager {
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
        self.visible_height = visible_height;
        let max_scroll = total_lines.saturating_sub(visible_height);

        // Auto-follow: if following mode is on, snap to bottom
        if self.following {
            self.scroll_offset = max_scroll;
        } else if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }

        // If we're at the bottom, re-enable following
        if self.scroll_offset >= max_scroll {
            self.following = true;
        }

        let session_title = format!(" Session — {} ", agent.tmux_target);
        let pane_output = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(session_title))
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset, 0));
        f.render_widget(pane_output, chunks[1]);

        // Input line
        let input_line = self.input.render_line("> ");
        let input_widget = Paragraph::new(input_line)
            .block(Block::default().borders(Borders::ALL).title(" Input "));
        f.render_widget(input_widget, chunks[2]);

        // Help
        let help = Paragraph::new(Line::from(vec![
            Span::styled(" Enter", theme::title_style()),
            Span::styled(" send  ", theme::help_style()),
            Span::styled("↑/↓", theme::title_style()),
            Span::styled(" line  ", theme::help_style()),
            Span::styled("PgUp/PgDn", theme::title_style()),
            Span::styled(" page  ", theme::help_style()),
            Span::styled("Home/End", theme::title_style()),
            Span::styled(" top/btm  ", theme::help_style()),
            Span::styled("Esc", theme::title_style()),
            Span::styled(" back  ", theme::help_style()),
            Span::styled("⌥0", theme::title_style()),
            Span::styled(" overview  ", theme::help_style()),
            Span::styled("⌥1-9", theme::title_style()),
            Span::styled(" worker  ", theme::help_style()),
        ]))
        .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, chunks[2]);
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
        self.following = false;
    }

    pub fn scroll_down(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
        // following will be re-enabled in render if we hit the bottom
    }

    pub fn page_up(&mut self) {
        let page = self.visible_height.max(1);
        self.scroll_up(page);
    }

    pub fn page_down(&mut self) {
        let page = self.visible_height.max(1);
        self.scroll_down(page);
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
        self.following = false;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = u16::MAX;
        self.following = true;
    }
}
