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
    pub comment_count: u32,
    pub assignees: Vec<String>,
    pub created_at_age: String,
}

impl IssueDetailView {
    pub fn new(
        issue_number: u32,
        title: String,
        body: String,
        labels: Vec<String>,
        state: String,
        comment_count: u32,
        assignees: Vec<String>,
        created_at_age: String,
    ) -> Self {
        Self {
            scroll_offset: 0,
            issue_number,
            title,
            body,
            labels,
            state,
            comment_count,
            assignees,
            created_at_age,
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
            Constraint::Length(5), // Header (extra line for metadata)
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

        let assignee_text = if self.assignees.is_empty() {
            "unassigned".to_string()
        } else {
            self.assignees.join(", ")
        };
        let comment_text = match self.comment_count {
            0 => "no comments".to_string(),
            1 => "1 comment".to_string(),
            n => format!("{n} comments"),
        };
        let age_text = if self.created_at_age.is_empty() {
            String::new()
        } else {
            format!("created {}ago", self.created_at_age)
        };
        let mut meta_parts = vec![assignee_text, comment_text];
        if !age_text.is_empty() {
            meta_parts.push(age_text);
        }

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
            Line::from(Span::styled(
                format!(" {}", meta_parts.join("  ·  ")),
                theme::help_style(),
            )),
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

#[cfg(test)]
mod tests {
    use super::IssueDetailView;
    use ratatui::{backend::TestBackend, Terminal};

    fn make_view(comment_count: u32, assignees: Vec<&str>, created_at_age: &str) -> IssueDetailView {
        IssueDetailView::new(
            42,
            "Test issue".to_string(),
            "Body text".to_string(),
            vec!["bug".to_string(), "P2".to_string()],
            "OPEN".to_string(),
            comment_count,
            assignees.into_iter().map(|s| s.to_string()).collect(),
            created_at_age.to_string(),
        )
    }

    fn render_to_string(view: &IssueDetailView) -> String {
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| view.render(f, f.area())).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn renders_issue_number_and_title() {
        let view = make_view(0, vec![], "");
        let rendered = render_to_string(&view);
        assert!(rendered.contains("#42"), "should show issue number");
        assert!(rendered.contains("Test issue"), "should show title");
    }

    #[test]
    fn renders_unassigned_when_no_assignees() {
        let view = make_view(0, vec![], "");
        let rendered = render_to_string(&view);
        assert!(rendered.contains("unassigned"));
    }

    #[test]
    fn renders_assignee_login() {
        let view = make_view(0, vec!["alice"], "");
        let rendered = render_to_string(&view);
        assert!(rendered.contains("alice"));
    }

    #[test]
    fn renders_comment_count_singular() {
        let view = make_view(1, vec![], "");
        let rendered = render_to_string(&view);
        assert!(rendered.contains("1 comment"));
    }

    #[test]
    fn renders_comment_count_plural() {
        let view = make_view(5, vec![], "");
        let rendered = render_to_string(&view);
        assert!(rendered.contains("5 comments"));
    }

    #[test]
    fn renders_no_comments_when_zero() {
        let view = make_view(0, vec![], "");
        let rendered = render_to_string(&view);
        assert!(rendered.contains("no comments"));
    }

    #[test]
    fn renders_created_at_age_when_present() {
        let view = make_view(0, vec![], "3d ");
        let rendered = render_to_string(&view);
        assert!(rendered.contains("created 3d ago") || rendered.contains("3d"));
    }

    #[test]
    fn omits_created_at_when_empty() {
        let view = make_view(0, vec![], "");
        let rendered = render_to_string(&view);
        assert!(!rendered.contains("created"));
    }
}
