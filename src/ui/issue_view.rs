use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::model::issue::GitHubIssue;
use super::theme;

pub struct IssueView {
    pub scroll_offset: u16,
    /// The rendered body text fetched from `gh issue view`.
    pub body: String,
    /// Issue number being viewed.
    pub issue_number: u32,
}

impl IssueView {
    pub fn new(issue_number: u32) -> Self {
        Self {
            scroll_offset: 0,
            body: "Loading…".to_string(),
            issue_number,
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, issue: Option<&GitHubIssue>) {
        let chunks = Layout::vertical([
            Constraint::Length(3),  // Title
            Constraint::Min(5),    // Body
            Constraint::Length(3), // Help
        ])
        .split(area);

        // Title bar
        let title_text = if let Some(issue) = issue {
            let status = issue.status_label();
            let pri = issue.priority_label();
            vec![
                Span::styled(format!("  #{} ", issue.number), theme::title_style()),
                Span::styled(format!("[{}] ", pri), theme::help_style()),
                Span::styled(&issue.title, Style::default().fg(ratatui::style::Color::White)),
                Span::styled(format!("  {status}"), theme::help_style()),
            ]
        } else {
            vec![
                Span::styled(format!("  #{} ", self.issue_number), theme::title_style()),
            ]
        };

        let title = Paragraph::new(Line::from(title_text))
            .block(Block::default().borders(Borders::BOTTOM));
        f.render_widget(title, chunks[0]);

        // Body
        let text = Text::raw(&self.body);
        let total_lines = text.lines.len() as u16;
        let visible_height = chunks[1].height.saturating_sub(2);
        let max_scroll = total_lines.saturating_sub(visible_height);
        if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }

        let body = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" Issue #{} ", self.issue_number)),
            )
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset, 0));
        f.render_widget(body, chunks[1]);

        // Help bar
        let help = Paragraph::new(Line::from(vec![
            Span::styled(" Esc/⌥←", theme::title_style()),
            Span::styled(" back  ", theme::help_style()),
            Span::styled("PgUp/Dn", theme::title_style()),
            Span::styled(" scroll  ", theme::help_style()),
            Span::styled("g", theme::title_style()),
            Span::styled(" open in browser", theme::help_style()),
        ]))
        .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, chunks[2]);
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
    }
}
