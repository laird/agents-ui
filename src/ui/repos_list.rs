use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
};
use std::collections::HashMap;
use std::path::PathBuf;

use super::theme;
use crate::model::swarm::Swarm;

pub struct ReposListView {
    pub table_state: TableState,
    issue_count_deltas: HashMap<String, usize>,
    issue_counts: HashMap<String, usize>,
}

impl ReposListView {
    pub fn new() -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            table_state,
            issue_count_deltas: HashMap::new(),
            issue_counts: HashMap::new(),
        }
    }

    pub fn update_issue_count(&mut self, repo_key: &str, new_count: usize) {
        if let Some(prev) = self.issue_counts.get(repo_key).copied() {
            if new_count > prev {
                self.issue_count_deltas
                    .insert(repo_key.to_string(), new_count - prev);
            } else {
                self.issue_count_deltas.remove(repo_key);
            }
        }
        self.issue_counts.insert(repo_key.to_string(), new_count);
    }

    pub fn clear_issue_delta(&mut self, repo_key: &str) {
        self.issue_count_deltas.remove(repo_key);
    }

    pub fn render(
        &mut self,
        f: &mut Frame,
        area: Rect,
        swarms: &[Swarm],
        available: &[PathBuf],
        status_msg: Option<&str>,
    ) {
        let total_items = swarms.len() + available.len();

        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(area);

        // Title
        let active_count = swarms.len();
        let avail_count = available.len();
        let title_info = if active_count > 0 && avail_count > 0 {
            format!("  ({active_count} active, {avail_count} available)")
        } else if active_count > 0 {
            format!("  ({active_count} active)")
        } else if avail_count > 0 {
            format!("  ({avail_count} repos found)")
        } else {
            String::new()
        };
        let title = Paragraph::new(Line::from(vec![
            Span::styled("  Agents UI", theme::title_style()),
            Span::styled(title_info, theme::help_style()),
        ]))
        .block(Block::default().borders(Borders::BOTTOM));
        f.render_widget(title, chunks[0]);

        // Table
        if total_items == 0 {
            let empty = Paragraph::new(Line::from(vec![
                Span::styled("  No repos found. Press ", theme::help_style()),
                Span::styled("n", theme::title_style()),
                Span::styled(" to launch a new swarm.", theme::help_style()),
            ]));
            f.render_widget(empty, chunks[1]);
        } else {
            let header = Row::new(vec![
                Cell::from("#"),
                Cell::from("Repo"),
                Cell::from("Status"),
                Cell::from("Workflow"),
                Cell::from("Runtime"),
                Cell::from("Agents"),
                Cell::from("Attention"),
            ])
            .style(theme::header_style());

            let mut rows: Vec<Row> = Vec::new();
            let mut row_num = 1;

            // Active swarms first
            for s in swarms {
                let busy = s.busy_count();
                let total = s.workers.len();
                let attention = s.attention_count();
                let mut attention_spans = vec![Span::styled(
                    if attention > 0 {
                        format!("{attention} items")
                    } else {
                        "—".to_string()
                    },
                    if attention > 0 {
                        theme::attention_style()
                    } else {
                        theme::help_style()
                    },
                )];
                if let Some(delta) = self.issue_count_deltas.get(&s.project_name) {
                    if *delta > 0 {
                        attention_spans
                            .push(Span::styled(format!(" +{delta}"), theme::attention_style()));
                    }
                }
                rows.push(Row::new(vec![
                    Cell::from(format!("{row_num}")).style(theme::title_style()),
                    Cell::from(s.project_name.clone()),
                    Cell::from("Active").style(Style::default().fg(ratatui::style::Color::Green)),
                    Cell::from(
                        s.workflow
                            .as_ref()
                            .map(|w| w.to_string())
                            .unwrap_or_else(|| "—".to_string()),
                    ),
                    Cell::from(s.agent_type.to_string()),
                    Cell::from(format!("{busy}/{total} busy")),
                    Cell::from(Line::from(attention_spans)),
                ]));
                row_num += 1;
            }

            // Available repos
            for repo in available {
                let name = repo
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| repo.to_string_lossy().to_string());
                rows.push(Row::new(vec![
                    Cell::from(format!("{row_num}")).style(theme::help_style()),
                    Cell::from(name),
                    Cell::from("—").style(theme::help_style()),
                    Cell::from("—").style(theme::help_style()),
                    Cell::from("—").style(theme::help_style()),
                    Cell::from("—").style(theme::help_style()),
                    Cell::from("—").style(theme::help_style()),
                ]));
                row_num += 1;
            }

            let table = Table::new(
                rows,
                [
                    Constraint::Length(3),
                    Constraint::Percentage(22),
                    Constraint::Percentage(12),
                    Constraint::Percentage(14),
                    Constraint::Percentage(12),
                    Constraint::Percentage(14),
                    Constraint::Percentage(14),
                ],
            )
            .header(header)
            .block(Block::default().borders(Borders::ALL).title(" Repos "))
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
            Span::styled(" 1-9", theme::title_style()),
            Span::styled("/", theme::help_style()),
            Span::styled("Enter", theme::title_style()),
            Span::styled(" select  ", theme::help_style()),
            Span::styled("n", theme::title_style()),
            Span::styled(" new swarm  ", theme::help_style()),
            Span::styled("d", theme::title_style()),
            Span::styled(" stop swarm  ", theme::help_style()),
            Span::styled("r", theme::title_style()),
            Span::styled(" refresh  ", theme::help_style()),
            Span::styled("q", theme::title_style()),
            Span::styled(" quit", theme::help_style()),
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

#[cfg(test)]
mod tests {
    use super::ReposListView;

    #[test]
    fn issue_delta_only_tracks_increases() {
        let mut view = ReposListView::new();

        view.update_issue_count("repo-a", 3);
        assert_eq!(view.issue_count_deltas.get("repo-a"), None);

        view.update_issue_count("repo-a", 5);
        assert_eq!(view.issue_count_deltas.get("repo-a"), Some(&2));

        view.update_issue_count("repo-a", 4);
        assert_eq!(view.issue_count_deltas.get("repo-a"), None);
    }

    #[test]
    fn clear_issue_delta_removes_indicator() {
        let mut view = ReposListView::new();
        view.update_issue_count("repo-a", 1);
        view.update_issue_count("repo-a", 2);
        assert_eq!(view.issue_count_deltas.get("repo-a"), Some(&1));

        view.clear_issue_delta("repo-a");
        assert_eq!(view.issue_count_deltas.get("repo-a"), None);
    }
}
