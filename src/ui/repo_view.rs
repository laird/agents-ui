use chrono::Local;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::model::swarm::{AgentInfo, Swarm};
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

        if self.focus_manager {
            let input_display = format!("> {}█", self.input);
            lines.push(Line::from(Span::styled(input_display, theme::title_style())));
        }

        let manager_para = Paragraph::new(lines).block(manager_block);
        f.render_widget(manager_para, chunks[1]);

        // Workers table
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
                Span::styled(" stop worker  ", theme::help_style()),
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
