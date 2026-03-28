use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::model::issue::{GitHubIssue, IssueState};
use crate::model::swarm::Swarm;
use super::theme;

pub struct IssueListView {
    pub table_state: TableState,
}

impl IssueListView {
    pub fn new() -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self { table_state }
    }

    /// Sort issues: open non-blocked first (by priority, then number), then blocked.
    pub fn sorted_open<'a>(issues: &'a [GitHubIssue]) -> Vec<&'a GitHubIssue> {
        let mut sorted: Vec<&GitHubIssue> = issues
            .iter()
            .filter(|i| i.state == IssueState::Open)
            .collect();
        sorted.sort_by_key(|i| {
            let blocked = u8::from(i.is_blocked());
            let pri = i.priority_num().unwrap_or(9);
            (blocked, pri, i.number)
        });
        sorted
    }

    pub fn render(
        &mut self,
        f: &mut Frame,
        area: Rect,
        swarm: &Swarm,
        issues: &[GitHubIssue],
        status_msg: Option<&str>,
    ) {
        let sorted = Self::sorted_open(issues);
        let idle_count = swarm
            .workers
            .iter()
            .filter(|w| matches!(w.status.state, crate::model::status::AgentState::Idle))
            .count();

        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(area);

        // Title
        let title_text = format!(
            "  Issues — {}  ({} idle worker{})",
            swarm.project_name,
            idle_count,
            if idle_count == 1 { "" } else { "s" }
        );
        let width = chunks[0].width as usize;
        let left_len = title_text.len();
        let title_line = Line::from(vec![
            Span::styled(title_text, theme::title_style()),
            theme::hostname_right_span(left_len, width),
        ]);
        let title = Paragraph::new(title_line).block(Block::default().borders(Borders::BOTTOM));
        f.render_widget(title, chunks[0]);

        // Table
        if sorted.is_empty() {
            let empty = Paragraph::new(Line::from(Span::styled(
                "  No open issues.",
                theme::help_style(),
            )));
            f.render_widget(empty, chunks[1]);
        } else {
            let header = Row::new(vec![
                Cell::from("#"),
                Cell::from("Title"),
                Cell::from("Priority"),
                Cell::from("Status"),
                Cell::from("Labels"),
            ])
            .style(theme::header_style());

            let rows: Vec<Row> = sorted
                .iter()
                .map(|issue| {
                    let status_style = if issue.is_blocked() {
                        theme::attention_style()
                    } else if issue.is_being_worked() {
                        Style::default().fg(ratatui::style::Color::Blue)
                    } else {
                        theme::help_style()
                    };

                    let skip: &[&str] =
                        &["P0", "P1", "P2", "P3", "bug", "enhancement", "working", "proposal"];
                    let labels: Vec<&str> = issue
                        .labels
                        .iter()
                        .filter(|l| !skip.contains(&l.as_str()))
                        .map(|l| l.as_str())
                        .collect();
                    let labels_str = labels.join(", ");

                    Row::new(vec![
                        Cell::from(format!("#{}", issue.number)).style(theme::title_style()),
                        Cell::from(issue.title.clone()),
                        Cell::from(issue.priority_label()),
                        Cell::from(issue.status_label()).style(status_style),
                        Cell::from(labels_str).style(theme::help_style()),
                    ])
                })
                .collect();

            let table = Table::new(
                rows,
                [
                    Constraint::Length(7),
                    Constraint::Percentage(46),
                    Constraint::Length(8),
                    Constraint::Percentage(20),
                    Constraint::Percentage(24),
                ],
            )
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} Open Issues ", sorted.len())),
            )
            .row_highlight_style(theme::selected_style());

            f.render_stateful_widget(table, chunks[1], &mut self.table_state);
        }

        // Status line
        if let Some(msg) = status_msg {
            let status = Paragraph::new(Line::from(Span::styled(
                format!(" {msg}"),
                theme::help_style(),
            )));
            f.render_widget(status, chunks[2]);
        }

        // Help bar
        let help = Paragraph::new(Line::from(vec![
            Span::styled(" ↑↓/jk", theme::title_style()),
            Span::styled(" navigate  ", theme::help_style()),
            Span::styled("Enter", theme::title_style()),
            Span::styled(" view  ", theme::help_style()),
            Span::styled("Space/d", theme::title_style()),
            Span::styled(" dispatch  ", theme::help_style()),
            Span::styled("r", theme::title_style()),
            Span::styled(" refresh  ", theme::help_style()),
            Span::styled("Esc", theme::title_style()),
            Span::styled(" back", theme::help_style()),
        ]))
        .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, chunks[3]);
    }

    pub fn next(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let i = self.table_state.selected().unwrap_or(0);
        self.table_state.select(Some((i + 1) % len));
    }

    pub fn previous(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let i = self.table_state.selected().unwrap_or(0);
        self.table_state
            .select(Some(if i == 0 { len - 1 } else { i - 1 }));
    }

    pub fn selected(&self) -> Option<usize> {
        self.table_state.selected()
    }
}

impl Default for IssueListView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::issue::{IssuePriority, IssueType};

    fn make_issue(number: u32, priority: IssuePriority, blocked: bool) -> GitHubIssue {
        let mut labels = vec![format!("{priority}")];
        if blocked {
            labels.push("needs-design".to_string());
        }
        GitHubIssue {
            number,
            title: format!("Issue #{number}"),
            state: IssueState::Open,
            priority,
            issue_type: IssueType::Bug,
            labels,
            is_working: false,
            assigned_worker: None,
        }
    }

    #[test]
    fn sorted_open_puts_blocked_last() {
        let issues = vec![
            make_issue(1, IssuePriority::P1, true),
            make_issue(2, IssuePriority::P2, false),
            make_issue(3, IssuePriority::P0, false),
        ];
        let sorted = IssueListView::sorted_open(&issues);
        assert_eq!(sorted[0].number, 3); // P0 unblocked first
        assert_eq!(sorted[1].number, 2); // P2 unblocked second
        assert_eq!(sorted[2].number, 1); // blocked last
    }

    #[test]
    fn navigation_wraps() {
        let mut view = IssueListView::new();
        view.next(3);
        assert_eq!(view.selected(), Some(1));
        view.next(3);
        view.next(3);
        assert_eq!(view.selected(), Some(0)); // wrapped
        view.previous(3);
        assert_eq!(view.selected(), Some(2)); // wrapped backward
    }
}
