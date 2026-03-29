use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use super::theme;

/// Convert a single markdown line into a styled ratatui `Line`.
/// Handles: headings, checkboxes, list items, inline code, bold, plain text.
pub fn render_markdown_line(line: &str) -> Line<'static> {
    let trimmed = line.trim_start();

    // Headings: # ## ### etc.
    if trimmed.starts_with('#') {
        let text = trimmed.trim_start_matches('#').trim().to_string();
        return Line::from(vec![Span::styled(
            text,
            theme::title_style(),
        )]);
    }

    // Checked checkbox: - [x] or - [X]
    if trimmed.starts_with("- [x]") || trimmed.starts_with("- [X]") {
        let text = trimmed[5..].trim().to_string();
        return Line::from(vec![
            Span::styled("✓ ", Style::default().fg(Color::Green)),
            Span::styled(text, Style::default().fg(Color::DarkGray)),
        ]);
    }

    // Unchecked checkbox: - [ ]
    if trimmed.starts_with("- [ ]") {
        let text = trimmed[5..].trim().to_string();
        return Line::from(vec![
            Span::styled("☐ ", Style::default().fg(Color::DarkGray)),
            Span::from(text),
        ]);
    }

    // List item: - text or * text
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        let text = trimmed[2..].to_string();
        return Line::from(vec![
            Span::styled("• ", theme::help_style()),
            Span::from(text),
        ]);
    }

    // Numbered list: 1. text, 12. text, etc.
    let num_end = trimmed.find(|c: char| !c.is_ascii_digit());
    if let Some(n) = num_end {
        if n > 0 && trimmed[n..].starts_with(". ") {
            let number = &trimmed[..n];
            let text = trimmed[n + 2..].to_string();
            return Line::from(vec![
                Span::styled(format!("{number}. "), theme::help_style()),
                Span::from(text),
            ]);
        }
    }

    // Blockquote: > text
    if let Some(text) = trimmed.strip_prefix("> ").or_else(|| trimmed.strip_prefix(">")) {
        return Line::from(vec![
            Span::styled("│ ", theme::help_style()),
            Span::styled(text.to_string(), Style::default().fg(Color::Gray)),
        ]);
    }

    // Horizontal rule: ---, ***, ___
    if trimmed == "---" || trimmed == "***" || trimmed == "___" {
        return Line::from(Span::styled("─────────────────────", theme::help_style()));
    }

    // Plain line — handle inline code and bold inline
    let owned = line.to_string();
    let spans = parse_inline(&owned);
    Line::from(spans)
}

/// Parse a line for inline `code` and **bold**/__bold__ markers into styled spans.
fn parse_inline(line: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut rest = line;

    while !rest.is_empty() {
        // Inline code: `...`
        if let Some(start) = rest.find('`') {
            if start > 0 {
                spans.push(Span::from(rest[..start].to_string()));
            }
            let after = &rest[start + 1..];
            if let Some(end) = after.find('`') {
                spans.push(Span::styled(
                    after[..end].to_string(),
                    Style::default().fg(Color::Cyan),
                ));
                rest = &after[end + 1..];
                continue;
            } else {
                // Unmatched backtick — treat as plain text
                spans.push(Span::from(rest[start..].to_string()));
                break;
            }
        }

        // Bold: **...** or __...__
        let bold_marker = if rest.starts_with("**") {
            Some("**")
        } else if rest.starts_with("__") {
            Some("__")
        } else {
            None
        };

        if let Some(marker) = bold_marker {
            let after = &rest[marker.len()..];
            if let Some(end) = after.find(marker) {
                spans.push(Span::styled(
                    after[..end].to_string(),
                    Style::default().add_modifier(Modifier::BOLD),
                ));
                rest = &after[end + marker.len()..];
                continue;
            }
        }

        // No more markers — emit the remainder
        spans.push(Span::from(rest.to_string()));
        break;
    }

    if spans.is_empty() {
        spans.push(Span::from(String::new()));
    }
    spans
}

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
                all_lines.push(render_markdown_line(line));
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
    fn render_markdown_line_plain_text() {
        let line = render_markdown_line("Hello world");
        assert!(!line.spans.is_empty());
        // Plain text is returned as-is in the first span
        assert!(line.spans.iter().any(|s| s.content.contains("Hello world")));
    }

    #[test]
    fn render_markdown_line_heading() {
        let line = render_markdown_line("## Section heading");
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].content, "Section heading");
    }

    #[test]
    fn render_markdown_line_list_item() {
        let line = render_markdown_line("- some item");
        assert_eq!(line.spans.len(), 2);
        assert!(line.spans[0].content.contains('•'));
        assert_eq!(line.spans[1].content, "some item");
    }

    #[test]
    fn render_markdown_line_checked_checkbox() {
        let line = render_markdown_line("- [x] done task");
        assert!(line.spans[0].content.contains('✓'));
        assert!(line.spans[1].content.contains("done task"));
    }

    #[test]
    fn render_markdown_line_unchecked_checkbox() {
        let line = render_markdown_line("- [ ] pending task");
        assert!(line.spans[0].content.contains('☐'));
        assert!(line.spans[1].content.contains("pending task"));
    }

    #[test]
    fn render_markdown_line_inline_code() {
        let line = render_markdown_line("Use `cargo test` to run");
        // Should have spans: "Use ", "cargo test", " to run"
        let combined: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(combined.contains("cargo test"));
    }

    #[test]
    fn render_markdown_line_empty() {
        let line = render_markdown_line("");
        assert!(!line.spans.is_empty());
    }

    #[test]
    fn render_markdown_line_numbered_list() {
        let line = render_markdown_line("1. First step");
        assert_eq!(line.spans.len(), 2);
        assert!(line.spans[0].content.contains("1."));
        assert_eq!(line.spans[1].content, "First step");
    }

    #[test]
    fn render_markdown_line_numbered_list_multi_digit() {
        let line = render_markdown_line("12. Twelfth step");
        assert_eq!(line.spans.len(), 2);
        assert!(line.spans[0].content.contains("12."));
        assert_eq!(line.spans[1].content, "Twelfth step");
    }

    #[test]
    fn render_markdown_line_numbered_list_not_triggered_on_plain_number() {
        // "123" alone is not a list item (no ". " after)
        let line = render_markdown_line("123");
        let combined: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(combined, "123");
    }

    #[test]
    fn render_markdown_line_blockquote() {
        let line = render_markdown_line("> quoted text");
        assert_eq!(line.spans.len(), 2);
        assert!(line.spans[0].content.contains('│'));
        assert_eq!(line.spans[1].content, "quoted text");
    }

    #[test]
    fn render_markdown_line_horizontal_rule() {
        let line = render_markdown_line("---");
        assert_eq!(line.spans.len(), 1);
        assert!(line.spans[0].content.contains('─'));
    }
}
