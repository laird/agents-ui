use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
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

/// Render a single markdown line into a ratatui `Line` with styled spans.
/// Handles headings, list items, checkboxes, inline code, and bold.
pub fn render_markdown_line(line: &str) -> Line<'static> {
    let trimmed = line.trim_start();

    // Headings: ## Heading
    if let Some(rest) = trimmed.strip_prefix("### ") {
        return Line::from(Span::styled(
            format!("  {rest}"),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    }
    if let Some(rest) = trimmed.strip_prefix("## ") {
        return Line::from(Span::styled(
            rest.to_string(),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    }
    if let Some(rest) = trimmed.strip_prefix("# ") {
        return Line::from(Span::styled(
            rest.to_string(),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    }

    // Checkboxes: - [x] / - [ ]
    if trimmed.starts_with("- [x]") || trimmed.starts_with("- [X]") {
        let rest = trimmed[5..].trim_start();
        return Line::from(vec![
            Span::styled("☑ ", Style::default().fg(Color::Green)),
            Span::styled(
                rest.to_string(),
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::CROSSED_OUT),
            ),
        ]);
    }
    if let Some(rest) = trimmed.strip_prefix("- [ ]") {
        let rest = rest.trim_start();
        return Line::from(vec![
            Span::styled("☐ ", Style::default().fg(Color::DarkGray)),
            Span::raw(rest.to_string()),
        ]);
    }

    // List items: - item
    if let Some(rest) = trimmed.strip_prefix("- ") {
        let indent = " ".repeat(line.len() - trimmed.len());
        return Line::from(vec![
            Span::styled(format!("{indent}• "), Style::default().fg(Color::DarkGray)),
            Span::raw(rest.to_string()),
        ]);
    }

    // Plain line: handle inline code and bold inline
    render_inline(line)
}

/// Render a line with potential inline `code` and **bold** spans.
fn render_inline(line: &str) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut remaining = line.to_string();

    while !remaining.is_empty() {
        // Look for inline code first
        if let Some(start) = remaining.find('`') {
            if let Some(end) = remaining[start + 1..].find('`') {
                let end = start + 1 + end;
                if start > 0 {
                    let before = remaining[..start].to_string();
                    spans.extend(render_bold_spans(&before));
                }
                let code = remaining[start + 1..end].to_string();
                spans.push(Span::styled(code, Style::default().fg(Color::Cyan)));
                remaining = remaining[end + 1..].to_string();
                continue;
            }
        }
        // No more inline code
        spans.extend(render_bold_spans(&remaining));
        break;
    }

    Line::from(spans)
}

/// Split text on **bold** markers and return styled spans.
fn render_bold_spans(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut remaining = text.to_string();

    while !remaining.is_empty() {
        if let Some(start) = remaining.find("**") {
            if let Some(end_rel) = remaining[start + 2..].find("**") {
                let end = start + 2 + end_rel;
                if start > 0 {
                    spans.push(Span::raw(remaining[..start].to_string()));
                }
                let bold_text = remaining[start + 2..end].to_string();
                spans.push(Span::styled(bold_text, Style::default().add_modifier(Modifier::BOLD)));
                remaining = remaining[end + 2..].to_string();
                continue;
            }
        }
        spans.push(Span::raw(remaining));
        break;
    }

    spans
}

/// State for the issue detail view.
pub struct IssueDetailView {
    pub scroll_offset: u16,
    pub issue_number: u32,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
    pub state: String,
    /// Recent comments as (author, body) pairs.
    pub comments: Vec<(String, String)>,
}

impl IssueDetailView {
    pub fn new(
        issue_number: u32,
        title: String,
        body: String,
        labels: Vec<String>,
        state: String,
        comments: Vec<(String, String)>,
    ) -> Self {
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
        ];

        let header = Paragraph::new(header_lines)
            .block(Block::default().borders(Borders::BOTTOM));
        f.render_widget(header, chunks[0]);

        // Body content + comments in a single scrollable area
        let body_text = if self.body.is_empty() {
            " (No description provided)".to_string()
        } else {
            format!(" {}", self.body.replace('\r', ""))
        };

        let mut all_lines: Vec<Line> = body_text
            .lines()
            .map(render_markdown_line)
            .collect();

        for (author, comment_body) in &self.comments {
            all_lines.push(Line::from(""));
            all_lines.push(Line::from(vec![
                Span::styled("─── @", theme::help_style()),
                Span::styled(author.clone(), theme::title_style()),
                Span::styled(" ───", theme::help_style()),
            ]));
            for line in comment_body.replace('\r', "").lines() {
                all_lines.push(Line::from(format!(" {line}")));
            }
        }

        let comment_count = self.comments.len();
        let block_title = if comment_count > 0 {
            format!(" Issue Body + {} comment{} ", comment_count, if comment_count == 1 { "" } else { "s" })
        } else {
            " Issue Body ".to_string()
        };

        let body = Paragraph::new(all_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(block_title),
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

    #[test]
    fn render_markdown_line_empty() {
        let line = render_markdown_line("");
        assert!(line.spans.is_empty() || line.spans.iter().all(|s| s.content.is_empty()));
    }

    #[test]
    fn render_markdown_line_heading() {
        let line = render_markdown_line("## My Heading");
        assert_eq!(line.spans.len(), 1);
        assert!(line.spans[0].content.contains("My Heading"));
        assert!(line.spans[0].style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn render_markdown_line_list_item() {
        let line = render_markdown_line("- some item");
        assert!(line.spans.iter().any(|s| s.content.contains('•')));
        assert!(line.spans.iter().any(|s| s.content.contains("some item")));
    }

    #[test]
    fn render_markdown_line_checked_checkbox() {
        let line = render_markdown_line("- [x] Done task");
        assert!(line.spans.iter().any(|s| s.content.contains('☑')));
    }

    #[test]
    fn render_markdown_line_unchecked_checkbox() {
        let line = render_markdown_line("- [ ] Todo task");
        assert!(line.spans.iter().any(|s| s.content.contains('☐')));
        assert!(line.spans.iter().any(|s| s.content.contains("Todo task")));
    }

    #[test]
    fn render_markdown_line_inline_code() {
        let line = render_markdown_line("Use `cargo test` to run");
        let cyan_span = line.spans.iter().find(|s| s.style.fg == Some(Color::Cyan));
        assert!(cyan_span.is_some());
        assert_eq!(cyan_span.unwrap().content, "cargo test");
    }

    #[test]
    fn render_markdown_line_bold() {
        let line = render_markdown_line("This is **bold** text");
        let bold_span = line.spans.iter().find(|s| s.style.add_modifier.contains(Modifier::BOLD));
        assert!(bold_span.is_some());
        assert_eq!(bold_span.unwrap().content, "bold");
    }

    #[test]
    fn render_markdown_line_plain_text() {
        let line = render_markdown_line("Just plain text here");
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "Just plain text here");
    }
}
