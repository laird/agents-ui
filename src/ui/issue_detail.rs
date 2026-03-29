use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use super::theme;

/// Count checked and total task items in a markdown body.
/// Scans for `- [x]` (checked) and `- [ ]` (unchecked) patterns.
/// Returns `(checked, total)`.
pub fn count_tasks(body: &str) -> (usize, usize) {
    let mut checked = 0;
    let mut total = 0;
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("- [x]") || trimmed.starts_with("- [X]") {
            checked += 1;
            total += 1;
        } else if trimmed.starts_with("- [ ]") {
            total += 1;
        }
    }
    (checked, total)
}

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
            {
                let (checked, total) = count_tasks(&self.body);
                let mut spans = vec![
                    Span::styled(format!(" {} ", self.state), theme::help_style()),
                    Span::raw(" · "),
                    Span::styled(label_text, theme::help_style()),
                ];
                if total > 0 {
                    let task_style = if checked == total {
                        Style::default().fg(ratatui::style::Color::Green)
                    } else {
                        Style::default().fg(ratatui::style::Color::DarkGray)
                    };
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled(format!("{}/{} ✓", checked, total), task_style));
                }
                Line::from(spans)
            },
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
    use super::*;

    fn make_view(comment_count: u32, assignees: &[&str], created_at_age: &str) -> IssueDetailView {
        IssueDetailView::new(
            42,
            "Test issue".to_string(),
            "Body text".to_string(),
            vec!["bug".to_string()],
            "OPEN".to_string(),
            comment_count,
            assignees.iter().map(|s| s.to_string()).collect(),
            created_at_age.to_string(),
        )
    }

    #[test]
    fn new_stores_metadata_fields() {
        let view = make_view(5, &["alice", "bob"], "3d ");
        assert_eq!(view.comment_count, 5);
        assert_eq!(view.assignees, vec!["alice", "bob"]);
        assert_eq!(view.created_at_age, "3d ");
    }

    #[test]
    fn new_empty_metadata() {
        let view = make_view(0, &[], "");
        assert_eq!(view.comment_count, 0);
        assert!(view.assignees.is_empty());
        assert!(view.created_at_age.is_empty());
    }

    #[test]
    fn count_tasks_no_tasks() {
        assert_eq!(count_tasks("No checkboxes here."), (0, 0));
        assert_eq!(count_tasks(""), (0, 0));
    }

    #[test]
    fn count_tasks_all_checked() {
        let body = "- [x] Task one\n- [X] Task two\n";
        assert_eq!(count_tasks(body), (2, 2));
    }

    #[test]
    fn count_tasks_none_checked() {
        let body = "- [ ] Task one\n- [ ] Task two\n- [ ] Task three\n";
        assert_eq!(count_tasks(body), (0, 3));
    }

    #[test]
    fn count_tasks_mixed() {
        let body = "Some intro text.\n- [x] Done\n- [ ] Not done\n- [x] Also done\n";
        assert_eq!(count_tasks(body), (2, 3));
    }

    #[test]
    fn count_tasks_ignores_non_checkbox_lines() {
        let body = "- regular list item\n- [x] checked\n* [ ] not a checkbox (wrong prefix)\n";
        assert_eq!(count_tasks(body), (1, 1));
    }
}
