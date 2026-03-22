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
    pub focus_workers: bool,
    pub input: String,
    pub manager_scroll_offset: u16,
}

impl RepoView {
    pub fn new() -> Self {
        let mut worker_table_state = TableState::default();
        worker_table_state.select(Some(0));
        Self {
            worker_table_state,
            focus_workers: false,
            input: String::new(),
            manager_scroll_offset: u16::MAX, // Start scrolled to bottom
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, swarm: &Swarm) {
        // Top-level 70/30 split: manager area vs worker list
        let main_chunks = Layout::vertical([
            Constraint::Percentage(70), // Manager area (title + session + input + help)
            Constraint::Percentage(30), // Workers table
        ])
        .split(area);

        // Manager area: title, session output, input, help
        let mgr_chunks = Layout::vertical([
            Constraint::Length(3),  // Title bar
            Constraint::Min(5),    // Manager session (fills remaining)
            Constraint::Length(3),  // Input line
            Constraint::Length(3),  // Help bar
        ])
        .split(main_chunks[0]);

        // Title bar
        let project_label = format!("  {} ", swarm.project_name);
        let manager_status = &swarm.manager.status.state;
        let status_text = format!(" [{}] ", manager_status);
        let runtime_label = format!(" {} ", swarm.agent_type);
        let title = Paragraph::new(Line::from(vec![
            Span::styled(project_label, theme::title_style()),
            Span::styled(status_text, theme::status_style(manager_status)),
            Span::styled(runtime_label, theme::help_style()),
        ]))
        .block(Block::default().borders(Borders::BOTTOM));
        f.render_widget(title, mgr_chunks[0]);

        // Manager session output (primary content, scrollable)
        let content = &swarm.manager.pane_content;
        let lines: Vec<Line> = content.lines().map(|l| Line::from(l.to_string())).collect();
        let total_lines = lines.len() as u16;

        let visible_height = mgr_chunks[1].height.saturating_sub(2); // borders
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
        f.render_widget(manager_output, mgr_chunks[1]);

        // Input line
        let input_display = format!("> {}█", self.input);
        let input_widget = Paragraph::new(Line::from(Span::styled(
            input_display,
            theme::input_style(),
        )))
        .block(Block::default().borders(Borders::ALL).title(" Input "));
        f.render_widget(input_widget, mgr_chunks[2]);

        // Help bar
        let help = Paragraph::new(Line::from(vec![
            Span::styled(" Enter", theme::title_style()),
            Span::styled(" send  ", theme::help_style()),
            Span::styled("PgUp/PgDn", theme::title_style()),
            Span::styled(" scroll  ", theme::help_style()),
            Span::styled("Tab", theme::title_style()),
            Span::styled(" workers  ", theme::help_style()),
            Span::styled("⌥0", theme::title_style()),
            Span::styled(" back  ", theme::help_style()),
            Span::styled("q", theme::title_style()),
            Span::styled(" quit", theme::help_style()),
        ]));
        f.render_widget(help.block(Block::default().borders(Borders::TOP)), mgr_chunks[3]);

        // Workers table (bottom 30%)
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

        f.render_stateful_widget(table, main_chunks[1], &mut self.worker_table_state);
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

    /// Move to next worker. Returns true if wrapped past the end (should focus manager).
    pub fn next_worker(&mut self, len: usize) -> bool {
        if len == 0 {
            return true;
        }
        let i = self.worker_table_state.selected().unwrap_or(0);
        if i + 1 >= len {
            return true;
        }
        self.worker_table_state.select(Some(i + 1));
        false
    }

    /// Move to previous worker. Returns true if wrapped past the top (should focus manager).
    pub fn previous_worker(&mut self, len: usize) -> bool {
        if len == 0 {
            return true;
        }
        let i = self.worker_table_state.selected().unwrap_or(0);
        if i == 0 {
            return true;
        }
        self.worker_table_state.select(Some(i - 1));
        false
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
            if let Some(title) = extract_issue_title_from_pane(&agent.pane_content, *n) {
                format!("#{n}: {title}")
            } else {
                format!("Issue #{n}")
            }
        }
        AgentState::Working { issue: None } => {
            extract_task_hint_from_pane(&agent.pane_content)
                .unwrap_or_else(|| "Working...".to_string())
        }
        AgentState::Idle => "idle".to_string(),
        AgentState::Starting => "starting...".to_string(),
        AgentState::Completed { detail } => {
            if detail.len() > 40 {
                format!("{}...", &detail[..37])
            } else {
                detail.clone()
            }
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
    for line in pane_content.lines().rev().take(10) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- extract_issue_title_from_pane tests ---

    #[test]
    fn extract_issue_title_found() {
        let content = "Some output\nWorking on #42: Fix the login bug\nMore output";
        assert_eq!(
            extract_issue_title_from_pane(content, 42),
            Some("Fix the login bug".to_string())
        );
    }

    #[test]
    fn extract_issue_title_not_found() {
        let content = "No issue references here";
        assert_eq!(extract_issue_title_from_pane(content, 42), None);
    }

    #[test]
    fn extract_issue_title_truncates_long() {
        let content = format!(
            "#99 - {}",
            "A".repeat(60)
        );
        let result = extract_issue_title_from_pane(&content, 99).unwrap();
        assert!(result.len() <= 53); // 50 + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn extract_issue_title_short_ignored() {
        // Title <= 3 chars should be ignored
        let content = "#42: ab";
        assert_eq!(extract_issue_title_from_pane(content, 42), None);
    }

    #[test]
    fn extract_issue_title_prefers_last_line() {
        let content = "#5: first mention\n#5: second better title";
        // Iterates in reverse, so finds "second better title" first
        assert_eq!(
            extract_issue_title_from_pane(content, 5),
            Some("second better title".to_string())
        );
    }

    // --- extract_task_hint_from_pane tests ---

    #[test]
    fn extract_task_hint_fix() {
        let content = "random output\nFix the broken test\n";
        assert_eq!(
            extract_task_hint_from_pane(content),
            Some("Fix the broken test".to_string())
        );
    }

    #[test]
    fn extract_task_hint_working_on() {
        let content = "Working on issue #42";
        assert_eq!(
            extract_task_hint_from_pane(content),
            Some("Working on issue #42".to_string())
        );
    }

    #[test]
    fn extract_task_hint_none() {
        let content = "just some random output\nnothing relevant";
        assert_eq!(extract_task_hint_from_pane(content), None);
    }

    #[test]
    fn extract_task_hint_truncates_long() {
        let hint = format!("Fix {}", "x".repeat(60));
        let result = extract_task_hint_from_pane(&hint).unwrap();
        assert!(result.len() <= 53);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn extract_task_hint_issue_prefix() {
        let content = "debug info\nIssue #10 needs attention";
        assert_eq!(
            extract_task_hint_from_pane(content),
            Some("Issue #10 needs attention".to_string())
        );
    }

    #[test]
    fn extract_task_hint_starting_work() {
        let content = "Starting work on feature X";
        assert_eq!(
            extract_task_hint_from_pane(content),
            Some("Starting work on feature X".to_string())
        );
    }

    #[test]
    fn extract_task_hint_only_recent_lines() {
        // Only checks last 10 lines
        let mut lines: Vec<String> = (0..20)
            .map(|i| format!("line {i}"))
            .collect();
        // Put a match at line 0 (beyond the 10-line window)
        lines[0] = "Fix the early bug".to_string();
        let content = lines.join("\n");
        // Should NOT find it since it's more than 10 lines from the end
        assert_eq!(extract_task_hint_from_pane(&content), None);
    }
}
