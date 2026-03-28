use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::model::issue::{GitHubIssue, IssueFilter};
use super::theme;

pub struct IssueListView {
    pub table_state: TableState,
    pub filter: IssueFilter,
}

impl IssueListView {
    pub fn new() -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            table_state,
            filter: IssueFilter::All,
        }
    }

    pub fn render(
        &mut self,
        f: &mut Frame,
        area: Rect,
        issues: &[GitHubIssue],
        idle_workers: usize,
    ) {
        let chunks = Layout::vertical([
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

        let filter = self.filter;
        let filtered: Vec<&GitHubIssue> = issues
            .iter()
            .filter(|i| i.matches_filter(filter))
            .collect();

        let filter_label = filter.label();
        let idle_text = if idle_workers == 1 {
            " — 1 worker idle".to_string()
        } else if idle_workers > 1 {
            format!(" — {idle_workers} workers idle")
        } else {
            String::new()
        };

        let header = Row::new(vec![
            Cell::from("#"),
            Cell::from("Pri"),
            Cell::from("Title"),
            Cell::from("Labels"),
            Cell::from("Status"),
        ])
        .style(theme::header_style());

        let rows: Vec<Row> = filtered
            .iter()
            .map(|issue| {
                let label_text = issue
                    .labels
                    .iter()
                    .filter(|l| !matches!(l.as_str(), "bug" | "enhancement"))
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ");

                let status = issue.status_label();
                let row_style = if issue.is_being_worked() {
                    Style::default().fg(ratatui::style::Color::Green)
                } else if issue.is_blocked() {
                    theme::attention_style()
                } else {
                    Style::default()
                };

                Row::new(vec![
                    Cell::from(format!("{}", issue.number)),
                    Cell::from(issue.priority_label()),
                    Cell::from(issue.title.as_str()),
                    Cell::from(label_text),
                    Cell::from(status),
                ])
                .style(row_style)
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(6),
                Constraint::Length(4),
                Constraint::Min(20),
                Constraint::Percentage(25),
                Constraint::Length(20),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Issue List ({filter_label}: {}){idle_text} ", filtered.len()))
                .border_style(theme::title_style()),
        )
        .row_highlight_style(theme::selected_style());

        f.render_stateful_widget(table, chunks[0], &mut self.table_state);

        let help = Paragraph::new(Line::from(vec![
            Span::styled(" j/k", theme::title_style()),
            Span::styled(" navigate  ", theme::help_style()),
            Span::styled("Enter", theme::title_style()),
            Span::styled(" view  ", theme::help_style()),
            Span::styled("d", theme::title_style()),
            Span::styled("/", theme::help_style()),
            Span::styled("Space", theme::title_style()),
            Span::styled(" dispatch  ", theme::help_style()),
            Span::styled("f", theme::title_style()),
            Span::styled(" filter  ", theme::help_style()),
            Span::styled("r", theme::title_style()),
            Span::styled(" refresh  ", theme::help_style()),
            Span::styled("Esc", theme::title_style()),
            Span::styled(" back  ", theme::help_style()),
            Span::styled("q", theme::title_style()),
            Span::styled(" quit", theme::help_style()),
        ]))
        .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, chunks[1]);
    }

    pub fn next(&mut self, len: usize) {
        if len == 0 { return; }
        let i = self.table_state.selected().unwrap_or(0);
        self.table_state.select(Some((i + 1) % len));
    }

    pub fn prev(&mut self, len: usize) {
        if len == 0 { return; }
        let i = self.table_state.selected().unwrap_or(0);
        self.table_state.select(Some(if i == 0 { len - 1 } else { i - 1 }));
    }

    pub fn selected(&self) -> Option<usize> {
        self.table_state.selected()
    }
}
