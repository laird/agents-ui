use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use super::theme;

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
    pub fn new(issue_number: u32, title: String, body: String, labels: Vec<String>, state: String) -> Self {
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

    pub fn render(&self, f: &mut Frame, area: Rect, comment_input: Option<&str>) {
        let chunks = if comment_input.is_some() {
            Layout::vertical([
                Constraint::Length(4), // Header
                Constraint::Min(4),    // Body
                Constraint::Length(3), // Comment input
                Constraint::Length(3), // Help bar
            ])
            .split(area)
        } else {
            Layout::vertical([
                Constraint::Length(4), // Header
                Constraint::Min(5),    // Body
                Constraint::Length(3), // Help bar
            ])
            .split(area)
        };

        // Header
        let label_text = if self.labels.is_empty() {
            String::new()
        } else {
            self.labels.join(" · ")
        };

        let header_lines = vec![
            Line::from(vec![
                Span::styled(
                    format!(" #{}: ", self.issue_number),
                    theme::title_style(),
                ),
                Span::styled(&self.title, theme::title_style()),
            ]),
            Line::from(vec![
                Span::styled(format!(" {} ", self.state), theme::help_style()),
                Span::raw(" · "),
                Span::styled(label_text, theme::help_style()),
            ]),
        ];

        let header = Paragraph::new(header_lines)
            .block(Block::default().borders(Borders::BOTTOM));
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
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Issue Body "),
            )
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset, 0));
        f.render_widget(body, chunks[1]);

        let help_chunk = if let Some(input) = comment_input {
            let input_line = Line::from(vec![
                Span::styled(" Comment: ", theme::title_style()),
                Span::raw(input),
                Span::styled("█", theme::title_style()),
            ]);
            let input_box = Paragraph::new(input_line).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Post Comment (Enter submit, Esc cancel) "),
            );
            f.render_widget(input_box, chunks[2]);
            chunks[3]
        } else {
            chunks[2]
        };

        // Help bar
        let help = Paragraph::new(Line::from(vec![
            Span::styled(" PgUp/PgDn", theme::title_style()),
            Span::styled(" scroll  ", theme::help_style()),
            Span::styled("g", theme::title_style()),
            Span::styled(" open in browser  ", theme::help_style()),
            Span::styled("c", theme::title_style()),
            Span::styled(" copy #  ", theme::help_style()),
            Span::styled("C", theme::title_style()),
            Span::styled(" comment  ", theme::help_style()),
            Span::styled("x", theme::title_style()),
            Span::styled(" close/reopen  ", theme::help_style()),
            Span::styled("p", theme::title_style()),
            Span::styled(" approve proposal  ", theme::help_style()),
            Span::styled("Esc", theme::title_style()),
            Span::styled(" back  ", theme::help_style()),
            Span::styled("q", theme::title_style()),
            Span::styled(" quit", theme::help_style()),
        ]))
        .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, help_chunk);
    }
}
