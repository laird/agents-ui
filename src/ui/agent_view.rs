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
        let path_label = format!("  {}", agent.worktree_path.display());
        let id_len = id_label.len();
        let role_len = role_label.len();
        let path_len = path_label.len();

        // Build status spans with the issue number highlighted separately
        let mut title_spans = vec![
            Span::styled(id_label, theme::title_style()),
            Span::styled(role_label, theme::help_style()),
        ];
        match &agent.status.state {
            crate::model::status::AgentState::Working { issue: Some(n) } => {
                title_spans.push(Span::styled(
                    "Working ",
                    theme::status_style(&agent.status.state),
                ));
                title_spans.push(Span::styled(
                    format!("#{n}"),
                    theme::title_style(),
                ));
            }
            state => {
                title_spans.push(Span::styled(
                    state.to_string(),
                    theme::status_style(&agent.status.state),
                ));
            }
        }
        if agent.waiting_for_input {
            title_spans.push(Span::styled(" NEEDS INPUT", theme::waiting_style()));
        }
        title_spans.push(Span::styled(path_label, theme::help_style()));
        let left_len = id_len + role_len + path_len
            + if agent.waiting_for_input { " NEEDS INPUT".len() } else { 0 }
            + 10; // approximate state label
        title_spans.push(theme::hostname_right_span(left_len, chunks[0].width as usize));

        let title = Paragraph::new(Line::from(title_spans))
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

        // Help bar with key shortcuts
        let help = Paragraph::new(Line::from(vec![
            Span::styled(" keys → session  ", theme::help_style()),
            Span::styled("PgUp/Dn", theme::title_style()),
            Span::styled(" scroll  ", theme::help_style()),
            Span::styled("Home/End", theme::title_style()),
            Span::styled(" top/bottom  ", theme::help_style()),
            Span::styled("Alt+0", theme::title_style()),
            Span::styled(" back  ", theme::help_style()),
            Span::styled("Alt+1-9", theme::title_style()),
            Span::styled(" sessions  ", theme::help_style()),
            Span::styled("Alt+a", theme::waiting_style()),
            Span::styled(" next waiting", theme::help_style()),
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
