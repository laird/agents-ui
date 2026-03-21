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
    /// Scroll offset for manager pane output.
    pub manager_scroll: u16,
    /// Height of the visible manager pane area.
    pub manager_visible_height: u16,
    /// Whether manager view auto-follows new content.
    pub manager_following: bool,
}

impl RepoView {
    pub fn new() -> Self {
        let mut worker_table_state = TableState::default();
        worker_table_state.select(Some(0));
        Self {
            worker_table_state,
            focus_manager: false,
            input: String::new(),
            manager_scroll: 0,
            manager_visible_height: 20,
            manager_following: true,
        }
    }

    pub fn manager_scroll_up(&mut self, amount: u16) {
        self.manager_scroll = self.manager_scroll.saturating_sub(amount);
        self.manager_following = false;
    }

    pub fn manager_scroll_down(&mut self, amount: u16) {
        self.manager_scroll = self.manager_scroll.saturating_add(amount);
    }

    pub fn manager_page_up(&mut self) {
        let page = self.manager_visible_height.max(1);
        self.manager_scroll_up(page);
    }

    pub fn manager_page_down(&mut self) {
        let page = self.manager_visible_height.max(1);
        self.manager_scroll_down(page);
    }

    pub fn manager_scroll_to_top(&mut self) {
        self.manager_scroll = 0;
        self.manager_following = false;
    }

    pub fn manager_scroll_to_bottom(&mut self) {
        self.manager_scroll = u16::MAX;
        self.manager_following = true;
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, swarm: &Swarm) {
        // When manager is focused, give it more space
        let manager_height = if self.focus_manager {
            Constraint::Percentage(60)
        } else {
            Constraint::Length(8)
        };

        let chunks = Layout::vertical([
            Constraint::Length(3),
            manager_height,
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
        let manager_title = if self.focus_manager {
            format!(" Manager — {} ", swarm.manager.tmux_target)
        } else {
            " Manager ".to_string()
        };
        let manager_block = Block::default()
            .borders(Borders::ALL)
            .title(manager_title)
            .border_style(if self.focus_manager {
                theme::title_style()
            } else {
                ratatui::style::Style::default()
            });

        if self.focus_manager {
            // Full interactive view with scrolling (like AgentView)
            let inner = manager_block.inner(chunks[1]);

            // Split into content area + input line
            let mgr_chunks = Layout::vertical([
                Constraint::Min(3),
                Constraint::Length(1),
            ])
            .split(inner);

            f.render_widget(manager_block, chunks[1]);

            let content = &swarm.manager.pane_content;
            let lines: Vec<Line> = content.lines().map(|l| Line::from(l.to_string())).collect();
            let total_lines = lines.len() as u16;

            let visible_height = mgr_chunks[0].height;
            self.manager_visible_height = visible_height;
            let max_scroll = total_lines.saturating_sub(visible_height);

            if self.manager_following {
                self.manager_scroll = max_scroll;
            } else if self.manager_scroll > max_scroll {
                self.manager_scroll = max_scroll;
            }
            if self.manager_scroll >= max_scroll {
                self.manager_following = true;
            }

            let pane_output = Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((self.manager_scroll, 0));
            f.render_widget(pane_output, mgr_chunks[0]);

            // Input line
            let input_display = format!("> {}█", self.input);
            let input_widget = Paragraph::new(Line::from(Span::styled(
                input_display,
                theme::input_style(),
            )));
            f.render_widget(input_widget, mgr_chunks[1]);
        } else {
            // Compact view: status + last few lines
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
        }

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
                Span::styled(" Enter", theme::title_style()),
                Span::styled(" send  ", theme::help_style()),
                Span::styled("↑/↓", theme::title_style()),
                Span::styled(" line  ", theme::help_style()),
                Span::styled("PgUp/PgDn", theme::title_style()),
                Span::styled(" page  ", theme::help_style()),
                Span::styled("Home/End", theme::title_style()),
                Span::styled(" top/btm  ", theme::help_style()),
                Span::styled("Esc", theme::title_style()),
                Span::styled(" back to workers  ", theme::help_style()),
            ]))
        } else {
            Paragraph::new(Line::from(vec![
                Span::styled(" Enter", theme::title_style()),
                Span::styled(" drill into agent  ", theme::help_style()),
                Span::styled("m", theme::title_style()),
                Span::styled(" manager session  ", theme::help_style()),
                Span::styled("d", theme::title_style()),
                Span::styled(" shutdown  ", theme::help_style()),
                Span::styled("f", theme::title_style()),
                Span::styled(" fix-loop  ", theme::help_style()),
                Span::styled("a", theme::title_style()),
                Span::styled(" add worker  ", theme::help_style()),
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
