use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::model::issue::GithubIssue;
use super::theme;

pub struct IssueListView {
    pub table_state: TableState,
    pub issues: Vec<GithubIssue>,
    /// Message shown in title bar (e.g., "Dispatched #42 to worker-1")
    pub status_msg: Option<String>,
}

impl IssueListView {
    pub fn new(issues: Vec<GithubIssue>) -> Self {
        let mut table_state = TableState::default();
        if !issues.is_empty() {
            table_state.select(Some(0));
        }
        Self { table_state, issues, status_msg: None }
    }

    pub fn next(&mut self) {
        if self.issues.is_empty() {
            return;
        }
        let i = self.table_state.selected().unwrap_or(0);
        self.table_state.select(Some((i + 1) % self.issues.len()));
    }

    pub fn previous(&mut self) {
        if self.issues.is_empty() {
            return;
        }
        let i = self.table_state.selected().unwrap_or(0);
        self.table_state
            .select(Some(if i == 0 { self.issues.len() - 1 } else { i - 1 }));
    }

    pub fn selected_issue(&self) -> Option<&GithubIssue> {
        self.table_state
            .selected()
            .and_then(|i| self.issues.get(i))
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, idle_workers: usize) {
        let chunks = Layout::vertical([
            Constraint::Length(3), // Title
            Constraint::Min(5),    // Table
            Constraint::Length(3), // Help
        ])
        .split(area);

        // Title bar
        let status_part = self
            .status_msg
            .as_deref()
            .unwrap_or("");
        let title_line = Line::from(vec![
            Span::styled(" GitHub Issues ", theme::title_style()),
            Span::styled(
                format!(" ({} idle workers) ", idle_workers),
                theme::help_style(),
            ),
            Span::styled(status_part, theme::title_style()),
        ]);
        f.render_widget(
            Paragraph::new(title_line).block(Block::default().borders(Borders::BOTTOM)),
            chunks[0],
        );

        // Table
        let header = Row::new(vec![
            Cell::from("#"),
            Cell::from("Priority"),
            Cell::from("Title"),
            Cell::from("Labels"),
        ])
        .style(theme::header_style());

        let rows: Vec<Row> = self
            .issues
            .iter()
            .map(|issue| {
                let row_style = if issue.is_blocked() {
                    theme::attention_style()
                } else {
                    Style::default()
                };
                let labels_str = issue
                    .labels
                    .iter()
                    .filter(|l| {
                        !matches!(l.as_str(), "P0" | "P1" | "P2" | "P3" | "enhancement" | "bug")
                    })
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" · ");
                Row::new(vec![
                    Cell::from(issue.number.to_string()),
                    Cell::from(issue.priority_label()),
                    Cell::from(issue.title.clone()),
                    Cell::from(labels_str),
                ])
                .style(row_style)
            })
            .collect();

        let title = format!(" Issues ({}) ", self.issues.len());
        let table = Table::new(
            rows,
            [
                Constraint::Length(6),
                Constraint::Length(8),
                Constraint::Min(40),
                Constraint::Percentage(25),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(theme::title_style()),
        )
        .row_highlight_style(theme::selected_style());

        f.render_stateful_widget(table, chunks[1], &mut self.table_state);

        // Help bar
        let help = Paragraph::new(Line::from(vec![
            Span::styled(" Enter", theme::title_style()),
            Span::styled(" view detail  ", theme::help_style()),
            Span::styled("Space/d", theme::title_style()),
            Span::styled(" dispatch to idle worker  ", theme::help_style()),
            Span::styled("r", theme::title_style()),
            Span::styled(" refresh  ", theme::help_style()),
            Span::styled("Esc", theme::title_style()),
            Span::styled(" back  ", theme::help_style()),
            Span::styled("q", theme::title_style()),
            Span::styled(" quit", theme::help_style()),
        ]))
        .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, chunks[2]);
    }
}
