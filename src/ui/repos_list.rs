use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::model::swarm::Swarm;
use super::theme;

pub struct ReposListView {
    pub table_state: TableState,
}

impl ReposListView {
    pub fn new() -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self { table_state }
    }

    pub fn render(
        &mut self,
        f: &mut Frame,
        area: Rect,
        swarms: &[Swarm],
        status_msg: Option<&str>,
    ) {
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(1), // Status line
            Constraint::Length(3),
        ])
        .split(area);

        // Title
        let repo_count = format!(
            "  ({} repo{})",
            swarms.len(),
            if swarms.len() == 1 { "" } else { "s" }
        );
        let title = Paragraph::new(Line::from(vec![
            Span::styled("  Agents UI", theme::title_style()),
            Span::styled(repo_count, theme::help_style()),
        ]))
        .block(Block::default().borders(Borders::BOTTOM));
        f.render_widget(title, chunks[0]);

        // Table
        if swarms.is_empty() {
            let empty = Paragraph::new(Line::from(vec![
                Span::styled("  No active swarms. Press ", theme::help_style()),
                Span::styled("n", theme::title_style()),
                Span::styled(" to launch a new one.", theme::help_style()),
            ]));
            f.render_widget(empty, chunks[1]);
        } else {
            let header = Row::new(vec![
                Cell::from("Repo"),
                Cell::from("Workflow"),
                Cell::from("Runtime"),
                Cell::from("Agents"),
                Cell::from("Status"),
                Cell::from("Attention"),
            ])
            .style(theme::header_style());

            let rows: Vec<Row> = swarms
                .iter()
                .map(|s| {
                    let busy = s.busy_count();
                    let total = s.workers.len();
                    let attention = s.attention_count();
                    Row::new(vec![
                        Cell::from(s.project_name.clone()),
                        Cell::from(
                            s.workflow
                                .as_ref()
                                .map(|w| w.to_string())
                                .unwrap_or_else(|| "—".to_string()),
                        ),
                        Cell::from(s.agent_type.to_string()),
                        Cell::from(format!("{busy}/{total} busy")),
                        Cell::from("Active"),
                        Cell::from(if attention > 0 {
                            format!("{attention} items")
                        } else {
                            "—".to_string()
                        })
                        .style(if attention > 0 {
                            theme::attention_style()
                        } else {
                            theme::help_style()
                        }),
                    ])
                })
                .collect();

            let table = Table::new(
                rows,
                [
                    Constraint::Percentage(25),
                    Constraint::Percentage(15),
                    Constraint::Percentage(15),
                    Constraint::Percentage(15),
                    Constraint::Percentage(15),
                    Constraint::Percentage(15),
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
            Span::styled(" Enter", theme::title_style()),
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
