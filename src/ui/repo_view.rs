use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
    Frame,
};

use crate::model::swarm::Swarm;
use super::theme;

/// Focus target within the Repo View.
#[derive(Debug, Clone, PartialEq)]
pub enum RepoViewFocus {
    /// Manager session pane (scrollable output + input).
    Manager,
    /// Workers table.
    Workers,
}

pub struct RepoView {
    pub worker_table_state: TableState,
    pub focus: RepoViewFocus,
    pub input: String,
    /// Scroll offset for the embedded manager session output.
    pub manager_scroll: u16,
}

impl RepoView {
    pub fn new() -> Self {
        let mut worker_table_state = TableState::default();
        worker_table_state.select(Some(0));
        Self {
            worker_table_state,
            focus: RepoViewFocus::Workers,
            input: String::new(),
            manager_scroll: u16::MAX, // Start scrolled to bottom
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, swarm: &Swarm) {
        let chunks = Layout::vertical([
            Constraint::Length(3),  // Title bar
            Constraint::Min(10),   // Manager session (takes remaining space)
            Constraint::Length(2 + swarm.workers.len().min(8) as u16 + 1), // Workers table (compact)
            Constraint::Length(3), // Help bar
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

        // Manager session panel (embedded live session)
        self.render_manager_session(f, chunks[1], swarm);

        // Workers table (compact)
        self.render_workers_table(f, chunks[2], swarm);

        // Help bar
        self.render_help_bar(f, chunks[3]);
    }

    fn render_manager_session(&mut self, f: &mut Frame, area: Rect, swarm: &Swarm) {
        let manager_focused = self.focus == RepoViewFocus::Manager;

        let manager_status = &swarm.manager.status.state;
        let status_text = manager_status.to_string();

        let session_title = format!(
            " Manager — {} ",
            swarm.manager.tmux_target
        );
        let border_style = if manager_focused {
            theme::title_style()
        } else {
            ratatui::style::Style::default()
        };

        if manager_focused {
            // When focused: show session output + input field
            let inner_chunks = Layout::vertical([
                Constraint::Min(3),    // Session output
                Constraint::Length(3), // Input field
            ])
            .split(area);

            // Session output
            let content = &swarm.manager.pane_content;
            let lines: Vec<Line> = content.lines().map(|l| Line::from(l.to_string())).collect();
            let total_lines = lines.len() as u16;

            let visible_height = inner_chunks[0].height.saturating_sub(2);
            let max_scroll = total_lines.saturating_sub(visible_height);
            if self.manager_scroll > max_scroll {
                self.manager_scroll = max_scroll;
            }

            let pane_output = Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(session_title)
                        .title_bottom(Line::from(vec![
                            Span::styled(" Status: ", theme::help_style()),
                            Span::styled(status_text, theme::status_style(manager_status)),
                            Span::raw(" "),
                        ]).right_aligned())
                        .border_style(border_style),
                )
                .wrap(Wrap { trim: false })
                .scroll((self.manager_scroll, 0));
            f.render_widget(pane_output, inner_chunks[0]);

            // Input field
            let input_display = format!("> {}█", self.input);
            let input_widget = Paragraph::new(Line::from(Span::styled(
                input_display,
                theme::input_style(),
            )))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Input ")
                    .border_style(border_style),
            );
            f.render_widget(input_widget, inner_chunks[1]);
        } else {
            // When not focused: show session output only (no input field)
            let content = &swarm.manager.pane_content;
            let lines: Vec<Line> = content.lines().map(|l| Line::from(l.to_string())).collect();
            let total_lines = lines.len() as u16;

            let visible_height = area.height.saturating_sub(2);
            let max_scroll = total_lines.saturating_sub(visible_height);
            if self.manager_scroll > max_scroll {
                self.manager_scroll = max_scroll;
            }

            let pane_output = Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(session_title)
                        .title_bottom(Line::from(vec![
                            Span::styled(" Status: ", theme::help_style()),
                            Span::styled(status_text, theme::status_style(manager_status)),
                            Span::raw(" "),
                        ]).right_aligned())
                        .border_style(border_style),
                )
                .wrap(Wrap { trim: false })
                .scroll((self.manager_scroll, 0));
            f.render_widget(pane_output, area);
        }
    }

    fn render_workers_table(&mut self, f: &mut Frame, area: Rect, swarm: &Swarm) {
        let workers_focused = self.focus == RepoViewFocus::Workers;

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
                .border_style(if workers_focused {
                    theme::title_style()
                } else {
                    ratatui::style::Style::default()
                }),
        )
        .row_highlight_style(if workers_focused {
            theme::selected_style()
        } else {
            ratatui::style::Style::default()
        });

        f.render_stateful_widget(table, area, &mut self.worker_table_state);
    }

    fn render_help_bar(&self, f: &mut Frame, area: Rect) {
        let help = match self.focus {
            RepoViewFocus::Manager => {
                Paragraph::new(Line::from(vec![
                    Span::styled(" Enter", theme::title_style()),
                    Span::styled(" send  ", theme::help_style()),
                    Span::styled("PgUp/PgDn", theme::title_style()),
                    Span::styled(" scroll  ", theme::help_style()),
                    Span::styled("↓/Tab", theme::title_style()),
                    Span::styled(" workers  ", theme::help_style()),
                    Span::styled("F", theme::title_style()),
                    Span::styled(" fullscreen  ", theme::help_style()),
                    Span::styled("Esc", theme::title_style()),
                    Span::styled(" back  ", theme::help_style()),
                ]))
            }
            RepoViewFocus::Workers => {
                Paragraph::new(Line::from(vec![
                    Span::styled(" ↑", theme::title_style()),
                    Span::styled(" manager  ", theme::help_style()),
                    Span::styled("Enter", theme::title_style()),
                    Span::styled(" fullscreen agent  ", theme::help_style()),
                    Span::styled("a", theme::title_style()),
                    Span::styled(" add worker  ", theme::help_style()),
                    Span::styled("Esc", theme::title_style()),
                    Span::styled(" back  ", theme::help_style()),
                    Span::styled("q", theme::title_style()),
                    Span::styled(" quit", theme::help_style()),
                ]))
            }
        };
        f.render_widget(help.block(Block::default().borders(Borders::TOP)), area);
    }

    pub fn next_worker(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let i = self.worker_table_state.selected().unwrap_or(0);
        self.worker_table_state.select(Some((i + 1) % len));
    }

    pub fn previous_worker(&mut self, len: usize) -> bool {
        if len == 0 {
            return true; // Navigate to manager
        }
        let i = self.worker_table_state.selected().unwrap_or(0);
        if i == 0 {
            // At top of workers list — navigate up to manager
            return true;
        }
        self.worker_table_state.select(Some(i - 1));
        false
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

    pub fn scroll_manager_to_bottom(&mut self) {
        self.manager_scroll = u16::MAX;
    }

    pub fn focus_manager(&mut self) {
        self.focus = RepoViewFocus::Manager;
        self.scroll_manager_to_bottom();
    }

    pub fn focus_workers(&mut self) {
        self.focus = RepoViewFocus::Workers;
    }

    /// Returns true if the manager session is scrolled to the bottom (following new output).
    pub fn is_manager_at_bottom(&self) -> bool {
        self.manager_scroll >= u16::MAX - 500
    }
}
