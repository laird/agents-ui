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
    pub comments: Vec<(String, String)>, // (author, body)
}

impl IssueDetailView {
    pub fn new(issue_number: u32, title: String, body: String, labels: Vec<String>, state: String, comments: Vec<(String, String)>) -> Self {
        Self {
            scroll_offset: 0,
            issue_number,
            title,
            body,
            labels,
            state,
            comments,
        }
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::vertical([
            Constraint::Length(4), // Header
            Constraint::Min(5),   // Body
            Constraint::Length(3), // Help bar
        ])
        .split(area);

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

        // Body + comments content (combined into one scrollable area)
        let mut all_lines: Vec<Line> = Vec::new();

        let body_text = if self.body.is_empty() {
            " (No description provided)".to_string()
        } else {
            format!(" {}", self.body.replace('\r', ""))
        };
        for l in body_text.lines() {
            all_lines.push(Line::from(l.to_string()));
        }

        if !self.comments.is_empty() {
            all_lines.push(Line::from(""));
            for (author, comment_body) in &self.comments {
                all_lines.push(Line::from(vec![
                    Span::styled(format!(" @{author}:"), theme::title_style()),
                ]));
                let comment_text = format!(" {}", comment_body.replace('\r', ""));
                for l in comment_text.lines() {
                    all_lines.push(Line::from(l.to_string()));
                }
                all_lines.push(Line::from(Span::styled(
                    " ─────────────────────────────────────",
                    theme::help_style(),
                )));
            }
        }

        let body = Paragraph::new(all_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Issue Body "),
            )
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset, 0));
        f.render_widget(body, chunks[1]);

        // Help bar
        let help = Paragraph::new(Line::from(vec![
            Span::styled(" PgUp/PgDn", theme::title_style()),
            Span::styled(" scroll  ", theme::help_style()),
            Span::styled("g", theme::title_style()),
            Span::styled(" open in browser  ", theme::help_style()),
            Span::styled("Esc", theme::title_style()),
            Span::styled(" back  ", theme::help_style()),
            Span::styled("q", theme::title_style()),
            Span::styled(" quit", theme::help_style()),
        ]))
        .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, chunks[2]);
    }
}
