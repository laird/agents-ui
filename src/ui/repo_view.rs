use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
    Frame,
};

use crate::model::swarm::Swarm;
use super::theme;

pub struct RepoView {
    pub worker_table_state: TableState,
    pub focus_manager: bool,
    pub input: String,
}

impl RepoView {
    pub fn new() -> Self {
        let mut worker_table_state = TableState::default();
        worker_table_state.select(Some(0));
        Self {
            worker_table_state,
            focus_manager: false,
            input: String::new(),
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, swarm: &Swarm) {
        // Workers table height: header + 1 row per worker + borders, capped
        let worker_rows = swarm.workers.len().max(1);
        let workers_height = (worker_rows as u16 + 3).min(12); // header + rows + borders

        // Calculate input height based on content (2 for borders, content lines capped at 5)
        let input_display = format!("> {}█", self.input);
        let avail_width = area.width.saturating_sub(2) as usize; // minus borders
        let input_lines = if avail_width > 0 {
            ((input_display.len() + avail_width - 1) / avail_width).max(1)
        } else {
            1
        };
        let input_height = (input_lines as u16).min(5) + 2; // content + borders

        let chunks = Layout::vertical([
            Constraint::Length(1),        // Title bar
            Constraint::Min(8),           // Manager session (primary, fills screen)
            Constraint::Length(input_height), // Input line (expands with content)
            Constraint::Length(workers_height), // Workers table (compact)
            Constraint::Length(1),         // Help bar
        ])
        .split(area);

        // Title bar (compact)
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
        let status_text = swarm.manager.status.state.to_string();
        let title = Paragraph::new(Line::from(vec![
            Span::styled(project_label, theme::title_style()),
            Span::styled(workflow_label, theme::help_style()),
            Span::styled(runtime_label, theme::help_style()),
            Span::styled(" Manager: ", theme::help_style()),
            Span::styled(status_text, theme::status_style(&swarm.manager.status.state)),
        ]));
        f.render_widget(title, chunks[0]);

        // Manager session output (primary content area — fills available space)
        let inner_height = chunks[1].height.saturating_sub(2) as usize; // minus borders
        let content_lines: Vec<Line> = swarm
            .manager
            .pane_content
            .lines()
            .rev()
            .take(inner_height)
            .map(|s| Line::from(s.to_string()))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        let manager_block = Block::default()
            .borders(Borders::ALL)
            .title(" Manager Session ")
            .border_style(if self.focus_manager {
                theme::title_style()
            } else {
                ratatui::style::Style::default()
            });

        let manager_para = Paragraph::new(content_lines).block(manager_block);
        f.render_widget(manager_para, chunks[1]);

        // Input line (always visible, wraps to show all content)
        let input_style = if self.focus_manager {
            theme::title_style()
        } else {
            theme::help_style()
        };
        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(input_style);
        let input_para = Paragraph::new(Span::styled(&input_display, input_style))
            .block(input_block)
            .wrap(Wrap { trim: false });
        f.render_widget(input_para, chunks[2]);

        // Workers table (compact summary)
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

        f.render_stateful_widget(table, chunks[3], &mut self.worker_table_state);

        // Help bar (compact, single line)
        let help = if self.focus_manager {
            Paragraph::new(Line::from(vec![
                Span::styled(" Enter", theme::title_style()),
                Span::styled(" send  ", theme::help_style()),
                Span::styled("↓", theme::title_style()),
                Span::styled(" workers  ", theme::help_style()),
                Span::styled("Esc", theme::title_style()),
                Span::styled(" workers  ", theme::help_style()),
            ]))
        } else {
            Paragraph::new(Line::from(vec![
                Span::styled(" Enter", theme::title_style()),
                Span::styled(" drill in  ", theme::help_style()),
                Span::styled("↑", theme::title_style()),
                Span::styled(" manager  ", theme::help_style()),
                Span::styled("a", theme::title_style()),
                Span::styled(" add worker  ", theme::help_style()),
                Span::styled("Esc", theme::title_style()),
                Span::styled(" back  ", theme::help_style()),
                Span::styled("q", theme::title_style()),
                Span::styled(" quit", theme::help_style()),
            ]))
        };
        f.render_widget(help, chunks[4]);
    }

    pub fn next_worker(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let i = self.worker_table_state.selected().unwrap_or(0);
        self.worker_table_state.select(Some((i + 1) % len));
    }

    /// Move selection up. Returns `true` if already at the top (caller should
    /// move focus to the manager panel).
    pub fn previous_worker(&mut self, len: usize) -> bool {
        if len == 0 {
            return true;
        }
        let i = self.worker_table_state.selected().unwrap_or(0);
        if i == 0 {
            return true; // At top — signal to focus manager
        }
        self.worker_table_state.select(Some(i - 1));
        false
    }

    pub fn selected_worker(&self) -> Option<usize> {
        self.worker_table_state.selected()
    }
}
