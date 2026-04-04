use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::theme;
use crate::model::issue::GitHubIssue;

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
            Constraint::Length(3), // Title
            Constraint::Min(5),    // Body
            Constraint::Length(3), // Help
        ])
        .split(area);

        // Title bar
        let width = chunks[0].width as usize;
        let mut title_text = if let Some(issue) = issue {
            let status = issue.status_label();
            let pri = issue.priority_label();
            vec![
                Span::styled(format!("  #{} ", issue.number), theme::title_style()),
                Span::styled(format!("[{}] ", pri), theme::help_style()),
                Span::styled(
                    &issue.title,
                    Style::default().fg(ratatui::style::Color::White),
                ),
                Span::styled(format!("  {status}"), theme::help_style()),
            ]
        } else {
            vec![Span::styled(
                format!("  #{} ", self.issue_number),
                theme::title_style(),
            )]
        };
        let left_len: usize = title_text.iter().map(|s| s.content.len()).sum();
        title_text.push(theme::hostname_right_span(left_len, width));

        let title =
            Paragraph::new(Line::from(title_text)).block(Block::default().borders(Borders::BOTTOM));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sets_initial_state() {
        let v = IssueView::new(7);
        assert_eq!(v.issue_number, 7);
        assert_eq!(v.scroll_offset, 0);
        assert_eq!(v.body, "Loading…");
    }

    #[test]
    fn scroll_down_increments_offset() {
        let mut v = IssueView::new(1);
        v.scroll_down(3);
        assert_eq!(v.scroll_offset, 3);
        v.scroll_down(2);
        assert_eq!(v.scroll_offset, 5);
    }

    #[test]
    fn scroll_up_decrements_offset() {
        let mut v = IssueView::new(1);
        v.scroll_down(10);
        v.scroll_up(4);
        assert_eq!(v.scroll_offset, 6);
    }

    #[test]
    fn scroll_up_saturates_at_zero() {
        let mut v = IssueView::new(1);
        v.scroll_up(5);
        assert_eq!(v.scroll_offset, 0);
    }

    #[test]
    fn scroll_down_saturates_at_max() {
        let mut v = IssueView::new(1);
        v.scroll_offset = u16::MAX;
        v.scroll_down(1);
        assert_eq!(v.scroll_offset, u16::MAX);
    }

    #[test]
    fn scroll_roundtrip_returns_to_zero() {
        let mut v = IssueView::new(1);
        v.scroll_down(20);
        v.scroll_up(20);
        assert_eq!(v.scroll_offset, 0);
    }
}
