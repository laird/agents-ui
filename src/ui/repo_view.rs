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
    pub manager_scroll: u16,
}

impl RepoView {
    pub fn new() -> Self {
        let mut worker_table_state = TableState::default();
        worker_table_state.select(Some(0));
        Self {
            worker_table_state,
            focus_manager: false,
            input: String::new(),
            manager_scroll: u16::MAX, // Start scrolled to bottom
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, swarm: &Swarm) {
        // Calculate worker table height: header + rows + borders, min 4 max 10
        let agent_rows = (1 + swarm.workers.len()) as u16; // manager + workers
        let table_height = (agent_rows + 3).max(5).min(12); // +3 for header, borders

        let chunks = Layout::vertical([
            Constraint::Length(3),          // Title
            Constraint::Length(table_height), // Workers table (compact)
            Constraint::Min(8),             // Manager session output
            Constraint::Length(3),          // Manager input
            Constraint::Length(3),          // Help bar
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

        // Workers table (compact)
        let header = Row::new(vec![
            Cell::from("Worker"),
            Cell::from("Status"),
            Cell::from("Current Task"),
            Cell::from("Worktree"),
        ])
        .style(theme::header_style());

        let agent_to_row = |agent: &crate::model::swarm::AgentInfo| -> Row {
            let task = match &agent.status.state {
                crate::model::status::AgentState::Working { issue: Some(n) } => {
                    format!("Issue #{n}")
                }
                _ => "—".to_string(),
            };
            let wt_name = agent
                .worktree_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            Row::new(vec![
                Cell::from(agent.id.clone()),
                Cell::from(agent.status.state.to_string())
                    .style(theme::status_style(&agent.status.state)),
                Cell::from(task),
                Cell::from(wt_name),
            ])
        };

        // Manager as first row, then workers
        let mut rows: Vec<Row> = vec![agent_to_row(&swarm.manager)];
        rows.extend(swarm.workers.iter().map(|w| agent_to_row(w)));

        let agents_title = format!(" Agents ({}) ", 1 + swarm.workers.len());
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
                .title(agents_title)
                .border_style(if !self.focus_manager {
                    theme::title_style()
                } else {
                    ratatui::style::Style::default()
                }),
        )
        .row_highlight_style(theme::selected_style());

        f.render_stateful_widget(table, chunks[1], &mut self.worker_table_state);

        // Manager session output (scrollable, like AgentView)
        let content = &swarm.manager.pane_content;
        let lines: Vec<Line> = content.lines().map(|l| Line::from(l.to_string())).collect();
        let total_lines = lines.len() as u16;

        let visible_height = chunks[2].height.saturating_sub(2); // subtract borders
        let max_scroll = total_lines.saturating_sub(visible_height);
        if self.manager_scroll > max_scroll {
            self.manager_scroll = max_scroll;
        }

        let session_title = format!(" Manager — {} ", swarm.manager.tmux_target);
        let manager_output = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(session_title)
                    .border_style(if self.focus_manager {
                        theme::title_style()
                    } else {
                        ratatui::style::Style::default()
                    }),
            )
            .wrap(Wrap { trim: false })
            .scroll((self.manager_scroll, 0));
        f.render_widget(manager_output, chunks[2]);

        // Manager input line (always visible)
        let input_display = format!("> {}█", self.input);
        let input_style = if self.focus_manager {
            theme::input_style()
        } else {
            theme::help_style()
        };
        let input = Paragraph::new(Line::from(Span::styled(input_display, input_style)))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Input ")
                    .border_style(if self.focus_manager {
                        theme::title_style()
                    } else {
                        ratatui::style::Style::default()
                    }),
            );
        f.render_widget(input, chunks[3]);

        // Help bar
        let help = if self.focus_manager {
            Paragraph::new(Line::from(vec![
                Span::styled(" Enter", theme::title_style()),
                Span::styled(" send  ", theme::help_style()),
                Span::styled("PgUp/PgDn", theme::title_style()),
                Span::styled(" scroll  ", theme::help_style()),
                Span::styled("Esc", theme::title_style()),
                Span::styled(" back to workers  ", theme::help_style()),
            ]))
        } else {
            Paragraph::new(Line::from(vec![
                Span::styled("1-9", theme::title_style()),
                Span::styled(" jump to worker  ", theme::help_style()),
                Span::styled("Enter", theme::title_style()),
                Span::styled(" drill in  ", theme::help_style()),
                Span::styled("m", theme::title_style()),
                Span::styled(" manager input  ", theme::help_style()),
                Span::styled("d", theme::title_style()),
                Span::styled(" shutdown  ", theme::help_style()),
                Span::styled("f", theme::title_style()),
                Span::styled(" fix-loop  ", theme::help_style()),
                Span::styled("a", theme::title_style()),
                Span::styled(" add worker  ", theme::help_style()),
                Span::styled("Alt+z", theme::title_style()),
                Span::styled(" stop swarm  ", theme::help_style()),
                Span::styled("Esc", theme::title_style()),
                Span::styled(" back  ", theme::help_style()),
                Span::styled("q", theme::title_style()),
                Span::styled(" quit", theme::help_style()),
            ]))
        };
        f.render_widget(help.block(Block::default().borders(Borders::TOP)), chunks[4]);
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

    pub fn scroll_manager_up(&mut self, amount: u16) {
        self.manager_scroll = self.manager_scroll.saturating_sub(amount);
    }

    pub fn scroll_manager_down(&mut self, amount: u16) {
        self.manager_scroll = self.manager_scroll.saturating_add(amount);
    }
}
