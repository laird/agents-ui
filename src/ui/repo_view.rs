#![allow(dead_code)]

use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::model::swarm::Swarm;
use super::theme;

pub struct RepoView {
    pub worker_table_state: TableState,
    pub focus_manager: bool,
}

impl RepoView {
    pub fn new() -> Self {
        let mut worker_table_state = TableState::default();
        worker_table_state.select(Some(0));
        Self {
            worker_table_state,
            focus_manager: false,
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, swarm: &Swarm) {
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

        // Title
        let project_label = format!("  {} ", swarm.project_name);
        let workflow_label = format!(
            " [{}] ",
            swarm
                .workflow
                .as_ref()
                .map(|w| w.to_string())
                .unwrap_or_else(|| "—".to_string())
        );
        let runtime_label = format!(" {} ", swarm.agent_type);
        let title = Paragraph::new(Line::from(vec![
            Span::styled(project_label, theme::title_style()),
            Span::styled(workflow_label, theme::help_style()),
            Span::styled(runtime_label, theme::help_style()),
        ]))
        .block(Block::default().borders(Borders::BOTTOM));
        f.render_widget(title, chunks[0]);

        // Manager panel
        let manager_status = &swarm.manager.status.state;
        let manager_block = Block::default()
            .borders(Borders::ALL)
            .title(" Manager ")
            .border_style(if self.focus_manager {
                theme::title_style()
            } else {
                ratatui::style::Style::default()
            });

        let status_text = manager_status.to_string();
        let worktree_text = format!("Worktree: {}", swarm.manager.worktree_path.display());

        let last_lines: Vec<String> = swarm
            .manager
            .pane_content
            .lines()
            .rev()
            .take(4)
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        let mut lines: Vec<Line> = vec![Line::from(vec![
            Span::styled("Status: ", theme::help_style()),
            Span::styled(status_text, theme::status_style(manager_status)),
            Span::raw("  "),
            Span::styled(worktree_text, theme::help_style()),
        ])];

        for l in &last_lines {
            lines.push(Line::from(l.clone()));
        }

        let manager_para = Paragraph::new(lines).block(manager_block);
        f.render_widget(manager_para, chunks[1]);

        // Workers table
        let header = Row::new(vec![
            Cell::from("Worker"),
            Cell::from("Status"),
            Cell::from("Current Task"),
            Cell::from("Worktree"),
        ])
        .style(theme::header_style());

        let rows: Vec<Row> = swarm
            .workers
            .iter()
            .map(|w| {
                let task = match &w.status.state {
                    crate::model::status::AgentState::Working { issue: Some(n) } => {
                        format!("Issue #{n}")
                    }
                    _ => "—".to_string(),
                };
                let wt_name = w
                    .worktree_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                Row::new(vec![
                    Cell::from(w.id.clone()),
                    Cell::from(w.status.state.to_string())
                        .style(theme::status_style(&w.status.state)),
                    Cell::from(task),
                    Cell::from(wt_name),
                ])
            })
            .collect();

        let workers_title = format!(" Workers ({}) ", swarm.workers.len());
        let table = Table::new(
            rows,
            [
                Constraint::Percentage(20),
                Constraint::Percentage(20),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(workers_title)
                .border_style(if !self.focus_manager {
                    theme::title_style()
                } else {
                    ratatui::style::Style::default()
                }),
        )
        .row_highlight_style(theme::selected_style());

        f.render_stateful_widget(table, chunks[2], &mut self.worker_table_state);

        // Help bar
        let help = if self.focus_manager {
            Paragraph::new(Line::from(vec![
                Span::styled(" Keys forwarded to manager", theme::help_style()),
                Span::styled("  Tab", theme::title_style()),
                Span::styled("/", theme::help_style()),
                Span::styled("Esc", theme::title_style()),
                Span::styled(" workers  ", theme::help_style()),
                Span::styled("⌥1-9", theme::title_style()),
                Span::styled(" worker", theme::help_style()),
            ]))
        } else {
            Paragraph::new(Line::from(vec![
                Span::styled(" Tab", theme::title_style()),
                Span::styled("/", theme::help_style()),
                Span::styled("m", theme::title_style()),
                Span::styled(" manager  ", theme::help_style()),
                Span::styled("1-9", theme::title_style()),
                Span::styled("/", theme::help_style()),
                Span::styled("Enter", theme::title_style()),
                Span::styled(" view  ", theme::help_style()),
                Span::styled("a", theme::title_style()),
                Span::styled(" add  ", theme::help_style()),
                Span::styled("f", theme::title_style()),
                Span::styled(" fix-loop  ", theme::help_style()),
                Span::styled("d", theme::title_style()),
                Span::styled(" shutdown  ", theme::help_style()),
                Span::styled("Esc", theme::title_style()),
                Span::styled(" back  ", theme::help_style()),
                Span::styled("q", theme::title_style()),
                Span::styled(" quit", theme::help_style()),
            ]))
        };
        f.render_widget(help.block(Block::default().borders(Borders::TOP)), chunks[3]);
    }

    pub fn next_worker(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let i = self.worker_table_state.selected().unwrap_or(0);
        self.worker_table_state.select(Some((i + 1) % len));
    }

    pub fn previous_worker(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let i = self.worker_table_state.selected().unwrap_or(0);
        self.worker_table_state
            .select(Some(if i == 0 { len - 1 } else { i - 1 }));
    }

    pub fn selected_worker(&self) -> Option<usize> {
        self.worker_table_state.selected()
    }
}
