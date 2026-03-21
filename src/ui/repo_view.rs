use chrono::Local;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
    Frame,
};

use crate::model::swarm::{AgentInfo, Swarm};
use super::theme;

pub struct RepoView {
    pub worker_table_state: TableState,
    pub focus_manager: bool,
    pub input: String,
    pub manager_scroll_offset: u16,
}

impl RepoView {
    pub fn new() -> Self {
        let mut worker_table_state = TableState::default();
        worker_table_state.select(Some(0));
        Self {
            worker_table_state,
            focus_manager: false,
            input: String::new(),
            manager_scroll_offset: u16::MAX, // Start scrolled to bottom
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, swarm: &Swarm) {
        // Worker table height: header + border + rows (compact)
        let worker_rows = swarm.workers.len() as u16;
        let worker_table_height = (worker_rows + 3).min(10); // header + border top/bottom + rows, max 10

        let chunks = Layout::vertical([
            Constraint::Length(1),             // Title bar
            Constraint::Min(5),               // Manager session (fills remaining space)
            Constraint::Length(3),             // Input line
            Constraint::Length(worker_table_height), // Workers table (compact)
            Constraint::Length(3),             // Help bar
        ])
        .split(area);

        // Title bar (compact, single line)
        let project_label = format!("  {} ", swarm.project_name);
        let manager_status = &swarm.manager.status.state;
        let status_text = format!(" [{}] ", manager_status);
        let runtime_label = format!(" {} ", swarm.agent_type);
        let title = Paragraph::new(Line::from(vec![
            Span::styled(project_label, theme::title_style()),
            Span::styled(status_text, theme::status_style(manager_status)),
            Span::styled(runtime_label, theme::help_style()),
        ]));
        f.render_widget(title, chunks[0]);

        // Manager session output (primary content, scrollable)
        let content = &swarm.manager.pane_content;
        let lines: Vec<Line> = content.lines().map(|l| Line::from(l.to_string())).collect();
        let total_lines = lines.len() as u16;

        let visible_height = chunks[1].height.saturating_sub(2); // borders
        let max_scroll = total_lines.saturating_sub(visible_height);
        if self.manager_scroll_offset > max_scroll {
            self.manager_scroll_offset = max_scroll;
        }

        let session_title = format!(" Manager — {} ", swarm.manager.tmux_target);
        let manager_output = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(session_title)
                    .border_style(theme::title_style()),
            )
            .wrap(Wrap { trim: false })
            .scroll((self.manager_scroll_offset, 0));
        f.render_widget(manager_output, chunks[1]);

        // Input line
        let input_display = format!("> {}█", self.input);
        let input_widget = Paragraph::new(Line::from(Span::styled(
            input_display,
            theme::input_style(),
        )))
        .block(Block::default().borders(Borders::ALL).title(" Input "));
        f.render_widget(input_widget, chunks[2]);

        // Workers table (compact)
        let header = Row::new(vec![
            Cell::from("Worker"),
            Cell::from("Status"),
            Cell::from("Current Task"),
            Cell::from("Last Activity"),
        ])
        .style(theme::header_style());

        let rows: Vec<Row> = swarm
            .workers
            .iter()
            .map(|w| {
                let task = current_task_display(w);
                let activity = format_last_activity(w);
                Row::new(vec![
                    Cell::from(w.id.clone()),
                    Cell::from(w.status.state.to_string())
                        .style(theme::status_style(&w.status.state)),
                    Cell::from(task),
                    Cell::from(activity),
                ])
            })
            .collect();

        let busy = swarm.busy_count();
        let total = swarm.workers.len();
        let workers_title = format!(" Workers ({busy}/{total} busy) ");
        let table = Table::new(
            rows,
            [
                Constraint::Percentage(15),
                Constraint::Percentage(15),
                Constraint::Percentage(45),
                Constraint::Percentage(25),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(workers_title),
        )
        .row_highlight_style(theme::selected_style());

        f.render_stateful_widget(table, chunks[3], &mut self.worker_table_state);

        // Help bar
        let help = Paragraph::new(Line::from(vec![
            Span::styled(" Enter", theme::title_style()),
            Span::styled(" send  ", theme::help_style()),
            Span::styled("PgUp/PgDn", theme::title_style()),
            Span::styled(" scroll  ", theme::help_style()),
            Span::styled("Tab", theme::title_style()),
            Span::styled(" workers  ", theme::help_style()),
            Span::styled("Esc", theme::title_style()),
            Span::styled(" back  ", theme::help_style()),
            Span::styled("q", theme::title_style()),
            Span::styled(" quit", theme::help_style()),
        ]));
        f.render_widget(help.block(Block::default().borders(Borders::TOP)), chunks[4]);
    }

    pub fn scroll_manager_up(&mut self, amount: u16) {
        self.manager_scroll_offset = self.manager_scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_manager_down(&mut self, amount: u16) {
        self.manager_scroll_offset = self.manager_scroll_offset.saturating_add(amount);
    }

    pub fn scroll_manager_to_bottom(&mut self) {
        self.manager_scroll_offset = u16::MAX;
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

/// Build the "Current Task" display string from status and pane content.
fn current_task_display(agent: &AgentInfo) -> String {
    use crate::model::status::AgentState;

    match &agent.status.state {
        AgentState::Working { issue: Some(n) } => {
            // Try to extract issue title from pane content
            if let Some(title) = extract_issue_title_from_pane(&agent.pane_content, *n) {
                format!("#{n}: {title}")
            } else {
                format!("Issue #{n}")
            }
        }
        AgentState::Working { issue: None } => {
            // Try to find what's being worked on from pane content
            extract_task_hint_from_pane(&agent.pane_content)
                .unwrap_or_else(|| "Working...".to_string())
        }
        AgentState::Idle => "idle".to_string(),
        AgentState::Starting => "starting...".to_string(),
        AgentState::Completed { detail } => {
            let short = if detail.len() > 40 {
                format!("{}...", &detail[..37])
            } else {
                detail.clone()
            };
            short
        }
        AgentState::Stopped => "—".to_string(),
        AgentState::Unknown(_) => "—".to_string(),
    }
}

/// Try to extract an issue title from pane content for a given issue number.
fn extract_issue_title_from_pane(pane_content: &str, issue_num: u32) -> Option<String> {
    let pattern = format!("#{issue_num}");
    for line in pane_content.lines().rev() {
        if let Some(pos) = line.find(&pattern) {
            // Look for text after the issue number like "#14: Some title" or "#14 - Some title"
            let after = &line[pos + pattern.len()..];
            let title = after
                .trim_start_matches(|c: char| c == ':' || c == '-' || c == ' ')
                .trim();
            if !title.is_empty() && title.len() > 3 {
                let truncated = if title.len() > 50 {
                    format!("{}...", &title[..47])
                } else {
                    title.to_string()
                };
                return Some(truncated);
            }
        }
    }
    None
}

/// Try to extract a task hint from pane content (e.g., "Fixing...", "Reading file...").
fn extract_task_hint_from_pane(pane_content: &str) -> Option<String> {
    // Look at the last few non-empty lines for activity hints
    for line in pane_content.lines().rev().take(10) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Look for lines that indicate current activity
        if trimmed.starts_with("Fix")
            || trimmed.starts_with("Working on")
            || trimmed.starts_with("Issue")
            || trimmed.starts_with("Starting work")
        {
            let display = if trimmed.len() > 50 {
                format!("{}...", &trimmed[..47])
            } else {
                trimmed.to_string()
            };
            return Some(display);
        }
    }
    None
}

/// Format the last activity time as a relative time string.
fn format_last_activity(agent: &AgentInfo) -> String {
    if let Some(ts) = agent.status.timestamp {
        let now = Local::now().naive_local();
        let duration = now.signed_duration_since(ts);

        if duration.num_seconds() < 0 {
            return "just now".to_string();
        }

        let secs = duration.num_seconds();
        if secs < 60 {
            format!("{secs}s ago")
        } else if secs < 3600 {
            format!("{}m ago", secs / 60)
        } else if secs < 86400 {
            format!("{}h ago", secs / 3600)
        } else {
            format!("{}d ago", secs / 86400)
        }
    } else {
        "—".to_string()
    }
}
