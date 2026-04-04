use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::theme;
use crate::ui::text_input::TextInput;

/// State for the issue detail view.
pub struct IssueDetailView {
    pub scroll_offset: u16,
    pub issue_number: u32,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
    pub state: String,
}

impl IssueDetailView {
    pub fn new(
        issue_number: u32,
        title: String,
        body: String,
        labels: Vec<String>,
        state: String,
    ) -> Self {
        Self {
            scroll_offset: 0,
            issue_number,
            title,
            body,
            labels,
            state,
        }
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
    }

    pub fn render(&self, f: &mut Frame, area: Rect, comment_input: Option<&TextInput>) {
        let constraints = if comment_input.is_some() {
            vec![
                Constraint::Length(4), // Header
                Constraint::Min(5),    // Body
                Constraint::Length(3), // Comment input
                Constraint::Length(3), // Help bar
            ]
        } else {
            vec![
                Constraint::Length(4), // Header
                Constraint::Min(5),    // Body
                Constraint::Length(3), // Help bar
            ]
        };
        let chunks = Layout::vertical(constraints).split(area);

        // Header
        let label_text = if self.labels.is_empty() {
            String::new()
        } else {
            self.labels.join(" · ")
        };

        let header_lines = vec![
            Line::from(vec![
                Span::styled(format!(" #{}: ", self.issue_number), theme::title_style()),
                Span::styled(&self.title, theme::title_style()),
            ]),
            Line::from(vec![
                Span::styled(format!(" {} ", self.state), theme::help_style()),
                Span::raw(" · "),
                Span::styled(label_text, theme::help_style()),
            ]),
        ];

        let header = Paragraph::new(header_lines).block(Block::default().borders(Borders::BOTTOM));
        f.render_widget(header, chunks[0]);

        // Body content
        let body_text = if self.body.is_empty() {
            " (No description provided)".to_string()
        } else {
            format!(" {}", self.body.replace('\r', ""))
        };

        let body_lines: Vec<Line> = body_text
            .lines()
            .map(|l| Line::from(l.to_string()))
            .collect();

        let body = Paragraph::new(body_lines)
            .block(Block::default().borders(Borders::ALL).title(" Issue Body "))
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset, 0));
        f.render_widget(body, chunks[1]);

        if let Some(input) = comment_input {
            let comment = Paragraph::new(input.render_line(" Comment: ")).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Post Comment "),
            );
            f.render_widget(comment, chunks[2]);
        }

        let help_chunk = chunks[chunks.len() - 1];
        // Help bar
        let help = Paragraph::new(Line::from(vec![
            Span::styled(" PgUp/PgDn", theme::title_style()),
            Span::styled(" scroll  ", theme::help_style()),
            Span::styled("C", theme::title_style()),
            Span::styled(" comment  ", theme::help_style()),
            Span::styled("g", theme::title_style()),
            Span::styled(" open in browser  ", theme::help_style()),
            Span::styled("Esc", theme::title_style()),
            Span::styled(" back  ", theme::help_style()),
            Span::styled("q", theme::title_style()),
            Span::styled(" quit", theme::help_style()),
        ]))
        .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, help_chunk);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_view() -> IssueDetailView {
        IssueDetailView::new(
            42,
            "Test issue".into(),
            "Body text".into(),
            vec!["bug".into()],
            "open".into(),
        )
    }

    #[test]
    fn new_sets_fields() {
        let v = make_view();
        assert_eq!(v.issue_number, 42);
        assert_eq!(v.title, "Test issue");
        assert_eq!(v.body, "Body text");
        assert_eq!(v.labels, vec!["bug"]);
        assert_eq!(v.state, "open");
    }

    #[test]
    fn new_scroll_starts_at_zero() {
        let v = make_view();
        assert_eq!(v.scroll_offset, 0);
    }

    #[test]
    fn scroll_down_increments_offset() {
        let mut v = make_view();
        v.scroll_down(5);
        assert_eq!(v.scroll_offset, 5);
        v.scroll_down(3);
        assert_eq!(v.scroll_offset, 8);
    }

    #[test]
    fn scroll_up_decrements_offset() {
        let mut v = make_view();
        v.scroll_down(10);
        v.scroll_up(4);
        assert_eq!(v.scroll_offset, 6);
    }

    #[test]
    fn scroll_up_saturates_at_zero() {
        let mut v = make_view();
        v.scroll_up(10); // offset is 0, can't go negative
        assert_eq!(v.scroll_offset, 0);
    }

    #[test]
    fn scroll_down_saturates_at_max() {
        let mut v = make_view();
        v.scroll_offset = u16::MAX;
        v.scroll_down(1); // saturating_add → stays at u16::MAX
        assert_eq!(v.scroll_offset, u16::MAX);
    }
}
