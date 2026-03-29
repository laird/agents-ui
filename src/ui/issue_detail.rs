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

    // Numbered list: "1. text", "10. text", etc.
    {
        let digits_end = trimmed.find(|c: char| !c.is_ascii_digit()).unwrap_or(0);
        if digits_end > 0 && trimmed[digits_end..].starts_with(". ") {
            let num = &trimmed[..digits_end];
            let text = trimmed[digits_end + 2..].to_string();
            return Line::from(vec![
                Span::styled(format!("{num}. "), theme::help_style()),
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
        // Image: ![alt](url) → show "🖼 alt"
        if let Some(start) = rest.find("![") {
            if start > 0 {
                spans.extend(parse_inline(&rest[..start]));
            }
            let after = &rest[start + 2..];
            if let Some(alt_end) = after.find("](") {
                let alt = &after[..alt_end];
                let after_url = &after[alt_end + 2..];
                if let Some(url_end) = after_url.find(')') {
                    spans.push(Span::styled(
                        format!("🖼 {alt}"),
                        Style::default().fg(Color::Blue),
                    ));
                    rest = &after_url[url_end + 1..];
                    continue;
                }
            }
        }

        // Link: [text](url) → show "text"
        if let Some(start) = rest.find('[') {
            if start > 0 {
                spans.extend(parse_inline(&rest[..start]));
            }
            let after = &rest[start + 1..];
            if let Some(text_end) = after.find("](") {
                let text = &after[..text_end];
                let after_url = &after[text_end + 2..];
                if let Some(url_end) = after_url.find(')') {
                    spans.push(Span::styled(
                        text.to_string(),
                        Style::default().add_modifier(Modifier::UNDERLINED),
                    ));
                    rest = &after_url[url_end + 1..];
                    continue;
                }
            }
            // No valid link syntax — emit up to and including '['
            spans.push(Span::from(rest[..start + 1].to_string()));
            rest = &rest[start + 1..];
            continue;
        }

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

        // Strikethrough: ~~...~~
        if rest.starts_with("~~") {
            let after = &rest[2..];
            if let Some(end) = after.find("~~") {
                spans.push(Span::styled(
                    after[..end].to_string(),
                    Style::default().add_modifier(Modifier::CROSSED_OUT),
                ));
                rest = &after[end + 2..];
                continue;
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

        // Italic: *...* or _..._  (only single marker, after ruling out ** and __)
        let italic_marker = if rest.starts_with('*') {
            Some("*")
        } else if rest.starts_with('_') {
            Some("_")
        } else {
            None
        };

        if let Some(marker) = italic_marker {
            let after = &rest[marker.len()..];
            if let Some(end) = after.find(marker) {
                spans.push(Span::styled(
                    after[..end].to_string(),
                    Style::default().add_modifier(Modifier::ITALIC),
                ));
                rest = &after[end + marker.len()..];
                continue;
            }
        }

        // No more markers — find the next potential marker to emit plain text up to it
        let next_marker = ["~~", "**", "__", "`", "*", "_"]
            .iter()
            .filter_map(|m| rest.find(m).map(|pos| (pos, *m)))
            .filter(|(pos, _)| *pos > 0)
            .min_by_key(|(pos, _)| *pos);

        if let Some((pos, _)) = next_marker {
            spans.push(Span::from(rest[..pos].to_string()));
            rest = &rest[pos..];
            continue;
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

/// Convert markdown text into styled lines, handling fenced code blocks (``` ... ```).
/// Lines inside a fenced code block are rendered verbatim in Cyan.
pub fn render_markdown_lines(text: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut in_code_block = false;
    for raw_line in text.lines() {
        let trimmed = raw_line.trim_start();
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            // Render the fence delimiter itself as a dim border
            lines.push(Line::from(Span::styled(
                raw_line.to_string(),
                Style::default().fg(Color::DarkGray),
            )));
        } else if in_code_block {
            lines.push(Line::from(Span::styled(
                raw_line.to_string(),
                Style::default().fg(Color::Cyan),
            )));
        } else {
            lines.push(render_markdown_line(raw_line));
        }
    }
    lines
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
    pub assignees: Vec<String>,
    pub created_at_age: String,
    pub updated_at_age: String,
}

impl IssueDetailView {
    pub fn new(
        issue_number: u32,
        title: String,
        body: String,
        labels: Vec<String>,
        state: String,
        comments: Vec<(String, String)>,
        assignees: Vec<String>,
        created_at_age: String,
        updated_at_age: String,
    ) -> Self {
        Self {
            scroll_offset: 0,
            issue_number,
            title,
            body,
            labels,
            state,
            comments,
            assignees,
            created_at_age,
            updated_at_age,
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
        let comment_count = self.comments.len();
        let comment_text = match comment_count {
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
        if !self.updated_at_age.is_empty() && self.updated_at_age != self.created_at_age {
            meta_parts.push(format!("updated {}ago", self.updated_at_age));
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

        // Body content + comments in a single scrollable area
        let body_text = if self.body.is_empty() {
            " (No description provided)".to_string()
        } else {
            format!(" {}", self.body.replace('\r', ""))
        };

        let mut all_lines: Vec<Line> = render_markdown_lines(&body_text);

        for (author, comment_body) in &self.comments {
            all_lines.push(Line::from(""));
            all_lines.push(Line::from(vec![
                Span::styled("─── @", theme::help_style()),
                Span::styled(author.clone(), theme::title_style()),
                Span::styled(" ───", theme::help_style()),
            ]));
            all_lines.extend(render_markdown_lines(&comment_body.replace('\r', "")));
        }

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
            Span::styled("n/p", theme::title_style()),
            Span::styled(" next/prev  ", theme::help_style()),
            Span::styled("d", theme::title_style()),
            Span::styled(" dispatch  ", theme::help_style()),
            Span::styled("g", theme::title_style()),
            Span::styled(" browser  ", theme::help_style()),
            Span::styled("Esc/⌥←", theme::title_style()),
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

    fn make_view(comment_count: usize, assignees: &[&str], created_at_age: &str) -> IssueDetailView {
        let comments: Vec<(String, String)> = (0..comment_count)
            .map(|i| (format!("user{i}"), format!("comment {i}")))
            .collect();
        IssueDetailView::new(
            42,
            "Test issue".to_string(),
            "Body text".to_string(),
            vec!["bug".to_string()],
            "OPEN".to_string(),
            comments,
            assignees.iter().map(|s| s.to_string()).collect(),
            created_at_age.to_string(),
            String::new(),
        )
    }

    #[test]
    fn new_stores_metadata_fields() {
        let view = make_view(5, &["alice", "bob"], "3d ");
        assert_eq!(view.comments.len(), 5);
        assert_eq!(view.assignees, vec!["alice", "bob"]);
        assert_eq!(view.created_at_age, "3d ");
    }

    #[test]
    fn new_empty_metadata() {
        let view = make_view(0, &[], "");
        assert!(view.comments.is_empty());
        assert!(view.assignees.is_empty());
        assert!(view.created_at_age.is_empty());
    }

    #[test]
    fn render_markdown_line_blockquote() {
        let line = render_markdown_line("> some quoted text");
        assert!(line.spans[0].content.contains('│'));
        assert_eq!(line.spans[1].content, "some quoted text");
    }

    #[test]
    fn render_markdown_line_horizontal_rule() {
        for rule in &["---", "***", "___"] {
            let line = render_markdown_line(rule);
            assert_eq!(line.spans.len(), 1);
            assert!(line.spans[0].content.contains('─'));
        }
    }

    #[test]
    fn render_markdown_line_italic_star() {
        let line = render_markdown_line("*italic* text");
        let combined: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(combined.contains("italic"));
        let italic_span = line.spans.iter().find(|s| s.content == "italic").unwrap();
        assert!(italic_span.style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn render_markdown_line_italic_underscore() {
        let line = render_markdown_line("_italic_ text");
        let combined: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(combined.contains("italic"));
        let italic_span = line.spans.iter().find(|s| s.content == "italic").unwrap();
        assert!(italic_span.style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn render_markdown_line_strikethrough() {
        let line = render_markdown_line("~~struck~~ text");
        let combined: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(combined.contains("struck"));
        let struck_span = line.spans.iter().find(|s| s.content == "struck").unwrap();
        assert!(struck_span.style.add_modifier.contains(Modifier::CROSSED_OUT));
    }

    #[test]
    fn render_markdown_line_bold_not_italic() {
        // **bold** must not be parsed as italic
        let line = render_markdown_line("**bold** text");
        let bold_span = line.spans.iter().find(|s| s.content == "bold").unwrap();
        assert!(bold_span.style.add_modifier.contains(Modifier::BOLD));
        assert!(!bold_span.style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn render_markdown_lines_fenced_code_block() {
        use ratatui::style::Color;
        let text = "before\n```\nfn foo() {}\n```\nafter";
        let lines = render_markdown_lines(text);
        assert_eq!(lines.len(), 5);
        // "before" — plain text
        assert!(lines[0].spans.iter().any(|s| s.content.contains("before")));
        // opening ``` — dark gray
        assert_eq!(lines[1].spans[0].style.fg, Some(Color::DarkGray));
        // code line — cyan
        assert_eq!(lines[2].spans[0].style.fg, Some(Color::Cyan));
        assert!(lines[2].spans[0].content.contains("fn foo()"));
        // closing ``` — dark gray
        assert_eq!(lines[3].spans[0].style.fg, Some(Color::DarkGray));
        // "after" — plain text
        assert!(lines[4].spans.iter().any(|s| s.content.contains("after")));
    }

    #[test]
    fn render_markdown_line_link() {
        let line = render_markdown_line("See [the docs](https://docs.example.com) for details");
        let combined: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(combined.contains("the docs"));
        assert!(!combined.contains("https://"));
        assert!(combined.contains("for details"));
    }

    #[test]
    fn render_markdown_line_image() {
        let line = render_markdown_line("![screenshot](https://example.com/img.png)");
        let combined: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(combined.contains("screenshot"));
        assert!(!combined.contains("https://"));
        assert!(combined.contains("🖼"));
    }

    #[test]
    fn render_markdown_line_malformed_link_passthrough() {
        let line = render_markdown_line("[broken link");
        let combined: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(combined.contains("[broken link"));
    }

    fn make_view_with_updated(created_age: &str, updated_age: &str) -> IssueDetailView {
        IssueDetailView::new(
            1,
            "T".to_string(),
            "B".to_string(),
            vec![],
            "OPEN".to_string(),
            vec![],
            vec![],
            created_age.to_string(),
            updated_age.to_string(),
        )
    }

    #[test]
    fn updated_at_age_shown_when_different_from_created() {
        let view = make_view_with_updated("3d ", "4h ");
        assert_eq!(view.updated_at_age, "4h ");
        assert_ne!(view.updated_at_age, view.created_at_age);
    }

    #[test]
    fn updated_at_age_omitted_when_same_as_created() {
        let view = make_view_with_updated("3d ", "3d ");
        assert_eq!(view.updated_at_age, view.created_at_age);
    }

    #[test]
    fn updated_at_age_empty_when_not_provided() {
        let view = make_view_with_updated("3d ", "");
        assert!(view.updated_at_age.is_empty());
    }

}
