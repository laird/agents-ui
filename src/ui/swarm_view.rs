use ansi_to_tui::IntoText;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
    Frame,
};

use crate::model::issue::{GitHubIssue, IssueFilter, IssuePriority, IssueType};
use crate::model::swarm::Swarm;
use super::theme;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SwarmPanel {
    Manager,
    Workers,
    Issues,
}

impl SwarmPanel {
    pub fn next(self) -> Self {
        match self {
            SwarmPanel::Manager => SwarmPanel::Workers,
            SwarmPanel::Workers => SwarmPanel::Issues,
            SwarmPanel::Issues => SwarmPanel::Manager,
        }
    }
}

pub struct SwarmView {
    pub manager_scroll: u16,
    pub workers_table: TableState,
    pub issues_table: TableState,
    pub issue_filter: IssueFilter,
    /// Active type filter: None = all types, Some(t) = only issues of that type.
    pub issue_type_filter: Option<IssueType>,
    /// Active priority filter: None = all priorities, Some(p) = only issues of that priority.
    pub priority_filter: Option<IssuePriority>,
    /// Active search query (None = not searching, Some("") = searching with empty query).
    pub search_query: Option<String>,
}

impl SwarmView {
    pub fn new() -> Self {
        let mut workers_table = TableState::default();
        workers_table.select(Some(0));
        let mut issues_table = TableState::default();
        issues_table.select(Some(0));
        Self {
            manager_scroll: 0,
            workers_table,
            issues_table,
            issue_filter: IssueFilter::All,
            issue_type_filter: None,
            priority_filter: None,
            search_query: None,
        }
    }

    pub fn render(
        &mut self,
        f: &mut Frame,
        area: Rect,
        swarm: &Swarm,
        issues: &[GitHubIssue],
        focus: SwarmPanel,
        blink: bool,
        issues_loading: bool,
    ) {
        let mut filtered_issues: Vec<&GitHubIssue> = issues
            .iter()
            .filter(|i| i.matches_filter(self.issue_filter))
            .filter(|i| {
                if let Some(ref tf) = self.issue_type_filter {
                    &i.issue_type == tf
                } else {
                    true
                }
            })
            .filter(|i| {
                if let Some(q) = &self.search_query {
                    if q.is_empty() {
                        return true;
                    }
                    let q_lower = q.to_lowercase();
                    if q.starts_with('#') {
                        if let Ok(n) = q[1..].parse::<u32>() {
                            return i.number == n;
                        }
                    }
                    i.title.to_lowercase().contains(&q_lower)
                        || i.number.to_string().contains(q.as_str())
                } else {
                    true
                }
            })
            .collect();
        filtered_issues.sort_by_key(|i| (&i.priority, i.number));

        // Pre-compute attention data before layout (needed for dynamic sizing)
        let attention = count_attention(swarm, issues);
        let working = swarm.busy_count();
        let total_workers = swarm.workers.len();
        let idle = total_workers - working;
        let avail_issues = issues.iter().filter(|i| !i.is_blocked() && !i.is_being_worked() && i.state == crate::model::issue::IssueState::Open).count();
        let blocked_issues: Vec<&GitHubIssue> = issues.iter().filter(|i| i.is_blocked()).collect();
        let blocked_count = blocked_issues.len();

        // Header is 1 line normally; 2 lines when there are blocked issues to surface inline
        let header_height: u16 = if blocked_count > 0 { 2 } else { 1 };

        let chunks = Layout::vertical([
            Constraint::Length(header_height),  // Header line(s)
            Constraint::Min(4),                 // Body (manager + workers/issues)
            Constraint::Length(2),              // Help bar
        ])
        .split(area);

        // Size bottom panel to fit the longer of workers or issues (+3 for borders+header row)
        // but never more than 50% of the body area so the manager always has room
        let max_bottom = chunks[1].height / 2;
        let bottom_rows = ((swarm.workers.len().max(filtered_issues.len()) + 3) as u16).min(max_bottom);
        let body_chunks = Layout::vertical([
            Constraint::Min(4),               // Manager gets all remaining space
            Constraint::Length(bottom_rows),   // Workers/Issues: sized to fit content
        ])
        .split(chunks[1]);

        let mut header_spans = vec![
            Span::styled(format!(" {} ", swarm.project_name), theme::title_style()),
            Span::styled("Active ", Style::default().fg(ratatui::style::Color::Green)),
            Span::styled(
                format!("{}W: {} working, {} idle  ", total_workers, working, idle),
                theme::help_style(),
            ),
            Span::styled(
                format!("Issues: {} avail, {} blocked  ", avail_issues, blocked_count),
                theme::help_style(),
            ),
        ];
        if attention > 0 {
            let style = theme::attention_blink_style(blink);
            header_spans.push(Span::styled(format!("⚠ {attention} need attention"), style));
        }
        let left_len: usize = header_spans.iter().map(|s| s.content.len()).sum();
        header_spans.push(theme::hostname_right_span(left_len, chunks[0].width as usize));

        // Build header lines: status line + optional inline attention row
        let mut header_lines = vec![Line::from(header_spans)];
        if blocked_count > 0 {
            let mut attn_spans = vec![
                Span::styled(" ⚠ ", theme::attention_style()),
            ];
            let show_n = blocked_count.min(3);
            for (idx, issue) in blocked_issues.iter().take(show_n).enumerate() {
                if idx > 0 {
                    attn_spans.push(Span::styled("  ", theme::help_style()));
                }
                let blocking_label = issue.labels
                    .iter()
                    .find(|l| crate::model::issue::BLOCKING_LABELS.contains(&l.as_str()))
                    .map(|s| s.as_str())
                    .unwrap_or("blocked");
                let guidance = crate::model::issue::blocking_guidance(blocking_label);
                attn_spans.push(Span::styled(
                    format!("#{} [{}] {}", issue.number, blocking_label, truncate(&issue.title, 25)),
                    theme::attention_style(),
                ));
                attn_spans.push(Span::styled(
                    format!(" → {}", guidance),
                    theme::help_style(),
                ));
            }
            if blocked_count > 3 {
                attn_spans.push(Span::styled(
                    format!("  … and {} more (Tab→Issues)", blocked_count - 3),
                    theme::help_style(),
                ));
            }
            header_lines.push(Line::from(attn_spans));
        }

        let header = Paragraph::new(header_lines);
        f.render_widget(header, chunks[0]);

        // --- Manager panel ---
        let manager_content = &swarm.manager.pane_content;
        let text = manager_content
            .as_bytes()
            .into_text()
            .unwrap_or_else(|_| Text::raw(manager_content.clone()));
        let total_lines = text.lines.len() as u16;
        let visible = body_chunks[0].height.saturating_sub(2);
        let max_scroll = total_lines.saturating_sub(visible);
        if self.manager_scroll > max_scroll {
            self.manager_scroll = max_scroll;
        }

        let manager_block = Block::default()
            .borders(Borders::ALL)
            .title(" Manager ")
            .border_style(if focus == SwarmPanel::Manager {
                theme::title_style()
            } else {
                Style::default()
            });

        let manager = Paragraph::new(text)
            .block(manager_block)
            .wrap(Wrap { trim: false })
            .scroll((self.manager_scroll, 0));
        f.render_widget(manager, body_chunks[0]);

        // --- Bottom split: Workers (left) | Issues (right) ---
        let bottom_cols = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(60),
        ])
        .split(body_chunks[1]);

        // Workers table
        let worker_header = Row::new(vec![
            Cell::from("#"),
            Cell::from("Status"),
            Cell::from("Task"),
            Cell::from("Age"),
        ])
        .style(theme::header_style());

        let worker_rows: Vec<Row> = swarm
            .workers
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let needs_input = agent_needs_input(&w.pane_content);
                let status_str = if needs_input {
                    "⚠ input".to_string()
                } else {
                    w.status.state.to_string()
                };
                let status_style = if needs_input {
                    theme::attention_blink_style(blink)
                } else {
                    theme::status_style(&w.status.state)
                };
                let task = if let Some(issue_num) = w.current_issue {
                    let title = w.current_issue_title.as_deref().unwrap_or("");
                    if title.is_empty() {
                        format!("#{issue_num}")
                    } else {
                        format!("#{issue_num} {}", truncate(title, 25))
                    }
                } else {
                    match &w.status.state {
                        crate::model::status::AgentState::Working { issue: Some(n) } => {
                            format!("#{n}")
                        }
                        _ => "\u{2014}".to_string(),
                    }
                };
                let task = match w.dispatched_issue {
                    Some(n) if !task.contains(&format!("#{n}")) => {
                        if task == "\u{2014}" {
                            format!("→#{n}")
                        } else {
                            format!("{task} →#{n}")
                        }
                    }
                    _ => task,
                };
                let age = crate::model::status::elapsed_display(w.status.timestamp);
                Row::new(vec![
                    Cell::from(format!("{}", i + 1)),
                    Cell::from(status_str).style(status_style),
                    Cell::from(task),
                    Cell::from(age).style(Style::default().fg(ratatui::style::Color::DarkGray)),
                ])
            })
            .collect();

        let workers_block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Workers ({}) ", swarm.workers.len()))
            .border_style(if focus == SwarmPanel::Workers {
                theme::title_style()
            } else {
                Style::default()
            });

        let workers_table = Table::new(
            worker_rows,
            [
                Constraint::Length(3),
                Constraint::Percentage(35),
                Constraint::Percentage(45),
                Constraint::Length(5),
            ],
        )
        .header(worker_header)
        .block(workers_block)
        .row_highlight_style(if focus == SwarmPanel::Workers {
            theme::selected_style()
        } else {
            Style::default()
        });

        f.render_stateful_widget(workers_table, bottom_cols[0], &mut self.workers_table);

        // Issues table
        let filter_label = self.issue_filter.label();
        let type_label = match &self.issue_type_filter {
            Some(IssueType::Bug) => " · bug",
            Some(IssueType::Enhancement) => " · enh",
            Some(IssueType::Proposal) => " · prop",
            Some(IssueType::Other) => " · other",
            None => "",
        };
        let priority_label = match &self.priority_filter {
            Some(IssuePriority::P0) => " · P0",
            Some(IssuePriority::P1) => " · P1",
            Some(IssuePriority::P2) => " · P2",
            Some(IssuePriority::P3) => " · P3",
            _ => "",
        };

        // Split issues area: optional 1-line search bar + table
        let is_searching = self.search_query.is_some();
        let issues_col = bottom_cols[1];
        let (search_area, table_area) = if is_searching {
            let parts = Layout::vertical([
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(issues_col);
            (Some(parts[0]), parts[1])
        } else {
            (None, issues_col)
        };

        // Render search bar
        if let (Some(area), Some(query)) = (search_area, &self.search_query) {
            let bar = Paragraph::new(Line::from(vec![
                Span::styled(" / ", theme::title_style()),
                Span::styled(query.as_str(), Style::default().fg(ratatui::style::Color::White)),
                Span::styled("█", Style::default().fg(ratatui::style::Color::White)),
                Span::styled("  Esc clear  Enter confirm", theme::help_style()),
            ]));
            f.render_widget(bar, area);
        }

        let issue_header = Row::new(vec![
            Cell::from("T"),
            Cell::from("#"),
            Cell::from("Pri"),
            Cell::from("Title"),
            Cell::from("Status"),
        ])
        .style(theme::header_style());

        let issue_rows: Vec<Row> = filtered_issues
            .iter()
            .map(|issue| {
                let status = issue.status_label();
                let status_style = if issue.is_being_worked() {
                    Style::default().fg(ratatui::style::Color::Green)
                } else if issue.is_blocked() {
                    Style::default().fg(ratatui::style::Color::Yellow)
                } else {
                    Style::default().fg(ratatui::style::Color::Gray)
                };
                let type_cell = if issue.is_recently_updated() {
                    Cell::from(format!("{}★", issue.type_char()))
                        .style(theme::issue_type_style(&issue.issue_type))
                } else {
                    Cell::from(issue.type_char()).style(theme::issue_type_style(&issue.issue_type))
                };
                Row::new(vec![
                    type_cell,
                    Cell::from(format!("{}", issue.number)),
                    Cell::from(issue.priority_label()).style(theme::priority_style(&issue.priority)),
                    Cell::from(truncate(&issue.title, 30)),
                    Cell::from(status).style(status_style),
                ])
            })
            .collect();

        let total_issue_count = issues.len();
        let is_filtered = filtered_issues.len() < total_issue_count;
        let count_str = if is_filtered {
            format!("{}/{}", filtered_issues.len(), total_issue_count)
        } else {
            filtered_issues.len().to_string()
        };
        let issues_block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Issues ({filter_label}{type_label}{priority_label}: {}{}) ", count_str, if issues_loading { ", loading\u{2026}" } else { "" }))
            .border_style(if focus == SwarmPanel::Issues {
                theme::title_style()
            } else {
                Style::default()
            });

        let issues_table = Table::new(
            issue_rows,
            [
                Constraint::Length(3),
                Constraint::Length(5),
                Constraint::Length(4),
                Constraint::Min(15),
                Constraint::Length(18),
            ],
        )
        .header(issue_header)
        .block(issues_block)
        .row_highlight_style(if focus == SwarmPanel::Issues {
            theme::selected_style()
        } else {
            Style::default()
        });

        f.render_stateful_widget(issues_table, table_area, &mut self.issues_table);

        // --- Help bar ---
        let help_spans = match focus {
            SwarmPanel::Manager => vec![
                Span::styled(" Tab", theme::title_style()),
                Span::styled(" cycle  ", theme::help_style()),
                Span::styled("PgUp/Dn", theme::title_style()),
                Span::styled(" scroll  ", theme::help_style()),
                Span::styled("Enter", theme::title_style()),
                Span::styled(" fullscreen  ", theme::help_style()),
                Span::styled("⌥d", theme::title_style()),
                Span::styled(" deploy  ", theme::help_style()),
                Span::styled("⌥a", theme::title_style()),
                Span::styled(" next alert  ", theme::help_style()),
                Span::styled("⌥f", theme::title_style()),
                Span::styled(" feedback", theme::help_style()),
            ],
            SwarmPanel::Workers => vec![
                Span::styled(" Tab", theme::title_style()),
                Span::styled(" cycle  ", theme::help_style()),
                Span::styled("Enter", theme::title_style()),
                Span::styled(" drill in  ", theme::help_style()),
                Span::styled("f", theme::title_style()),
                Span::styled(" fix-loop  ", theme::help_style()),
                Span::styled("g", theme::title_style()),
                Span::styled(" browser  ", theme::help_style()),
                Span::styled("d", theme::title_style()),
                Span::styled(" shutdown  ", theme::help_style()),
                Span::styled("a", theme::title_style()),
                Span::styled(" add  ", theme::help_style()),
                Span::styled("⌥a", theme::title_style()),
                Span::styled(" next alert", theme::help_style()),
            ],
            SwarmPanel::Issues => vec![
                Span::styled(" Tab", theme::title_style()),
                Span::styled(" cycle  ", theme::help_style()),
                Span::styled("d", theme::title_style()),
                Span::styled(" dispatch  ", theme::help_style()),
                Span::styled("a", theme::title_style()),
                Span::styled(" add  ", theme::help_style()),
                Span::styled("p", theme::title_style()),
                Span::styled(" approve  ", theme::help_style()),
                Span::styled("b", theme::title_style()),
                Span::styled(" next blocked  ", theme::help_style()),
                Span::styled("r", theme::title_style()),
                Span::styled(" review-blocked  ", theme::help_style()),
                Span::styled("f", theme::title_style()),
                Span::styled(" filter  ", theme::help_style()),
                Span::styled("t", theme::title_style()),
                Span::styled(" type  ", theme::help_style()),
                Span::styled("P", theme::title_style()),
                Span::styled(" priority  ", theme::help_style()),
                Span::styled("/", theme::title_style()),
                Span::styled(" search  ", theme::help_style()),
                Span::styled("Enter", theme::title_style()),
                Span::styled(" view  ", theme::help_style()),
                Span::styled("g", theme::title_style()),
                Span::styled(" browser  ", theme::help_style()),
                Span::styled("⌥a", theme::title_style()),
                Span::styled(" next alert", theme::help_style()),
            ],
        };
        let help = Paragraph::new(Line::from(help_spans));
        f.render_widget(help, chunks[2]);
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

    pub fn next_worker(&mut self, len: usize) {
        if len == 0 { return; }
        let i = self.workers_table.selected().unwrap_or(0);
        self.workers_table.select(Some((i + 1) % len));
    }

    /// Returns `true` if already at the top (caller should focus the manager panel).
    pub fn prev_worker(&mut self, len: usize) -> bool {
        if len == 0 { return true; }
        let i = self.workers_table.selected().unwrap_or(0);
        if i == 0 {
            return true;
        }
        self.workers_table.select(Some(i - 1));
        false
    }

    pub fn selected_worker(&self) -> Option<usize> {
        self.workers_table.selected()
    }

    pub fn next_issue(&mut self, len: usize) {
        if len == 0 { return; }
        let i = self.issues_table.selected().unwrap_or(0);
        self.issues_table.select(Some((i + 1) % len));
    }

    pub fn prev_issue(&mut self, len: usize) {
        if len == 0 { return; }
        let i = self.issues_table.selected().unwrap_or(0);
        self.issues_table.select(Some(if i == 0 { len - 1 } else { i - 1 }));
    }

    pub fn selected_issue(&self) -> Option<usize> {
        self.issues_table.selected()
    }

    /// Return issues matching both the active filter and the current search query.
    #[allow(dead_code)]
    pub fn issues_matching_search<'a>(&self, issues: &'a [GitHubIssue]) -> Vec<&'a GitHubIssue> {
        issues
            .iter()
            .filter(|i| i.matches_filter(self.issue_filter))
            .filter(|i| {
                if let Some(q) = &self.search_query {
                    if q.is_empty() {
                        return true;
                    }
                    let q_lower = q.to_lowercase();
                    if q.starts_with('#') {
                        if let Ok(n) = q[1..].parse::<u32>() {
                            return i.number == n;
                        }
                    }
                    i.title.to_lowercase().contains(&q_lower)
                        || i.number.to_string().contains(q.as_str())
                } else {
                    true
                }
            })
            .collect()
    }

    /// Return the subset of `issues` that pass both the state filter and the type filter,
    /// sorted by priority then issue number — matching the order rendered by `render()`.
    pub fn apply_filters<'a>(&self, issues: &'a [GitHubIssue]) -> Vec<&'a GitHubIssue> {
        let mut result: Vec<&'a GitHubIssue> = issues
            .iter()
            .filter(|i| i.matches_filter(self.issue_filter))
            .filter(|i| self.issue_type_filter.as_ref().map_or(true, |tf| &i.issue_type == tf))
            .filter(|i| self.priority_filter.as_ref().map_or(true, |pf| &i.priority == pf))
            .collect();
        result.sort_by_key(|i| (&i.priority, i.number));
        result
    }

    /// Cycle the type filter: None → Bug → Enhancement → Proposal → None.
    pub fn cycle_issue_type_filter(&mut self) {
        self.issue_type_filter = match &self.issue_type_filter {
            None => Some(IssueType::Bug),
            Some(IssueType::Bug) => Some(IssueType::Enhancement),
            Some(IssueType::Enhancement) => Some(IssueType::Proposal),
            Some(IssueType::Proposal) | Some(IssueType::Other) => None,
        };
        // Reset table selection when filter changes.
        self.issues_table.select(Some(0));
    }

    /// Cycle the priority filter: None → P0 → P1 → P2 → P3 → None.
    pub fn cycle_priority_filter(&mut self) {
        self.priority_filter = match &self.priority_filter {
            None => Some(IssuePriority::P0),
            Some(IssuePriority::P0) => Some(IssuePriority::P1),
            Some(IssuePriority::P1) => Some(IssuePriority::P2),
            Some(IssuePriority::P2) => Some(IssuePriority::P3),
            Some(IssuePriority::P3) | Some(IssuePriority::None) => None,
        };
        self.issues_table.select(Some(0));
    }
}

/// Count items needing human attention: blocked GitHub issues + agents waiting for input.
pub fn count_attention(swarm: &Swarm, issues: &[crate::model::issue::GitHubIssue]) -> usize {
    let blocked = issues.iter().filter(|i| i.is_blocked()).count();
    let mut agents_waiting = 0;
    if agent_needs_input(&swarm.manager.pane_content) {
        agents_waiting += 1;
    }
    for w in &swarm.workers {
        if agent_needs_input(&w.pane_content) {
            agents_waiting += 1;
        }
    }
    blocked + agents_waiting
}

/// Check if an agent's pane content indicates it's waiting for human input.
pub fn agent_needs_input(pane_content: &str) -> bool {
    if pane_content.is_empty() {
        return false;
    }
    // Strip ANSI for matching
    let stripped: String = strip_ansi(pane_content);
    let tail: String = stripped
        .lines()
        .rev()
        .take(10)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" ");
    let lower = tail.to_lowercase();

    lower.contains("what should claude do instead")
        || lower.contains("interrupted")
        || lower.contains("do you want to proceed")
        || lower.contains("should i proceed")
        || lower.contains("would you like")
        || lower.contains("please confirm")
        || lower.contains("askuserquestion")
        || lower.contains("enter to select")
}

fn strip_ansi(s: &str) -> String {
    s.chars()
        .fold((String::new(), false), |(mut out, in_esc), c| {
            if c == '\x1b' {
                (out, true)
            } else if in_esc {
                (out, !(c.is_ascii_alphabetic()))
            } else {
                out.push(c);
                (out, false)
            }
        })
        .0
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}

#[cfg(test)]
mod tests {
    use super::{agent_needs_input, SwarmPanel, SwarmView};
    use crate::model::issue::{GitHubIssue, IssueFilter, IssueState};
    use crate::model::status::{AgentState, AgentStatus};
    use crate::model::swarm::{AgentInfo, AgentType, Swarm};
    use ratatui::{backend::TestBackend, Terminal};
    use std::path::PathBuf;

    fn make_agent(id: &str, is_manager: bool, pane_content: &str, state: AgentState) -> AgentInfo {
        AgentInfo {
            id: format!("test/{id}"),
            role: id.to_string(),
            worktree_path: PathBuf::new(),
            tmux_target: String::new(),
            status: AgentStatus {
                timestamp: None,
                state,
            },
            is_manager,
            pane_content: pane_content.to_string(),
            dispatched_issue: None,
            current_issue: None,
            current_issue_title: None,
            waiting_for_input: false,
        }
    }

    fn make_swarm() -> Swarm {
        Swarm {
            repo_path: PathBuf::from("/tmp/repo"),
            project_name: "demo".to_string(),
            agent_type: AgentType::Codex,
            workflow: None,
            tmux_session: "codex-demo".to_string(),
            manager: make_agent("manager", true, "Manager output", AgentState::Idle),
            workers: vec![make_agent(
                "worker-1",
                false,
                "working issue #12",
                AgentState::Working { issue: Some(12) },
            )],
            issue_cache: crate::model::issue::IssueCache::default(),
            stopped: false,
        }
    }

    #[test]
    fn issue_type_filter_cycles() {
        let mut view = SwarmView::new();
        assert_eq!(view.issue_type_filter, None);
        view.cycle_issue_type_filter();
        assert_eq!(view.issue_type_filter, Some(crate::model::issue::IssueType::Bug));
        view.cycle_issue_type_filter();
        assert_eq!(view.issue_type_filter, Some(crate::model::issue::IssueType::Enhancement));
        view.cycle_issue_type_filter();
        assert_eq!(view.issue_type_filter, Some(crate::model::issue::IssueType::Proposal));
        view.cycle_issue_type_filter();
        assert_eq!(view.issue_type_filter, None);
    }

    #[test]
    fn issue_type_filter_excludes_non_matching() {
        use crate::model::issue::{IssueType, IssuePriority, IssueState};
        let mut view = SwarmView::new();
        view.cycle_issue_type_filter(); // → Bug

        let bug = GitHubIssue {
            number: 1, title: "a bug".into(), state: IssueState::Open,
            priority: IssuePriority::P2, issue_type: IssueType::Bug,
            labels: vec!["bug".into()], is_working: false, assigned_worker: None,
            updated_at: None,
        };
        let enh = GitHubIssue {
            number: 2, title: "an enh".into(), state: IssueState::Open,
            priority: IssuePriority::P3, issue_type: IssueType::Enhancement,
            labels: vec!["enhancement".into()], is_working: false, assigned_worker: None,
            updated_at: None,
        };
        let issues = vec![bug, enh];
        // Only bug should pass the type filter
        let filtered: Vec<_> = issues.iter()
            .filter(|i| {
                if let Some(ref tf) = view.issue_type_filter { &i.issue_type == tf } else { true }
            })
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].number, 1);
    }

    #[test]
    fn apply_filters_sorts_by_priority_then_number() {
        use crate::model::issue::{IssueType, IssuePriority, IssueState};
        let issues = vec![
            GitHubIssue { number: 10, title: "a".into(), state: IssueState::Open, priority: IssuePriority::P3, issue_type: IssueType::Other, labels: vec![], is_working: false, assigned_worker: None, updated_at: None },
            GitHubIssue { number: 5, title: "b".into(), state: IssueState::Open, priority: IssuePriority::P1, issue_type: IssueType::Bug, labels: vec![], is_working: false, assigned_worker: None, updated_at: None },
            GitHubIssue { number: 3, title: "c".into(), state: IssueState::Open, priority: IssuePriority::P1, issue_type: IssueType::Bug, labels: vec![], is_working: false, assigned_worker: None, updated_at: None },
            GitHubIssue { number: 8, title: "d".into(), state: IssueState::Open, priority: IssuePriority::P2, issue_type: IssueType::Enhancement, labels: vec![], is_working: false, assigned_worker: None, updated_at: None },
        ];
        // apply_filters returns issues sorted by priority then number
        let view = SwarmView::new();
        let filtered = view.apply_filters(&issues);
        let nums: Vec<u32> = filtered.iter().map(|i| i.number).collect();
        assert_eq!(nums, vec![3, 5, 8, 10]); // P1(3), P1(5), P2(8), P3(10)
    }

    #[test]
    fn priority_filter_cycles_p0_to_none() {
        let mut view = SwarmView::new();
        assert_eq!(view.priority_filter, None);
        view.cycle_priority_filter();
        assert_eq!(view.priority_filter, Some(crate::model::issue::IssuePriority::P0));
        view.cycle_priority_filter();
        assert_eq!(view.priority_filter, Some(crate::model::issue::IssuePriority::P1));
        view.cycle_priority_filter();
        assert_eq!(view.priority_filter, Some(crate::model::issue::IssuePriority::P2));
        view.cycle_priority_filter();
        assert_eq!(view.priority_filter, Some(crate::model::issue::IssuePriority::P3));
        view.cycle_priority_filter();
        assert_eq!(view.priority_filter, None);
    }

    #[test]
    fn priority_filter_excludes_non_matching() {
        use crate::model::issue::{IssueType, IssuePriority, IssueState};
        let issues = vec![
            GitHubIssue { number: 1, title: "a".into(), state: IssueState::Open, priority: IssuePriority::P1, issue_type: IssueType::Bug, labels: vec![], is_working: false, assigned_worker: None, updated_at: None },
            GitHubIssue { number: 2, title: "b".into(), state: IssueState::Open, priority: IssuePriority::P2, issue_type: IssueType::Enhancement, labels: vec![], is_working: false, assigned_worker: None, updated_at: None },
            GitHubIssue { number: 3, title: "c".into(), state: IssueState::Open, priority: IssuePriority::P3, issue_type: IssueType::Other, labels: vec![], is_working: false, assigned_worker: None, updated_at: None },
        ];
        let mut view = SwarmView::new();
        view.priority_filter = Some(IssuePriority::P1);
        let filtered = view.apply_filters(&issues);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].number, 1);
    }

    #[test]
    fn detects_confirmation_prompts() {
        assert!(agent_needs_input("Would you like to proceed?\nPress enter to confirm"));
        assert!(!agent_needs_input("All good, continuing work"));
    }

    #[test]
    fn render_smoke_writes_swarm_sections() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut view = SwarmView::new();
        let swarm = make_swarm();
        let issues = vec![GitHubIssue {
            number: 12,
            title: "Fix worker bootstrap after reconnect".to_string(),
            state: IssueState::Open,
            priority: crate::model::issue::IssuePriority::P1,
            issue_type: crate::model::issue::IssueType::Bug,
            labels: vec!["P1".to_string()],
            is_working: false,
            assigned_worker: Some("worker-1".to_string()),
            updated_at: None,
        }];

        terminal
            .draw(|f| {
                view.render(f, f.area(), &swarm, &issues, SwarmPanel::Manager, false, false);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let rendered = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("Manager"));
        assert!(rendered.contains("Workers (1)"));
        assert!(rendered.contains("Issues (all: 1)"));
        assert!(rendered.contains("demo"));
        assert!(rendered.contains("#12"));
    }

    fn make_issue(number: u32, title: &str, labels: &[&str]) -> GitHubIssue {
        GitHubIssue {
            number,
            title: title.to_string(),
            state: IssueState::Open,
            priority: crate::model::issue::IssuePriority::None,
            issue_type: crate::model::issue::IssueType::Other,
            labels: labels.iter().map(|s| s.to_string()).collect(),
            is_working: false,
            assigned_worker: None,
            updated_at: None,
        }
    }

    #[test]
    fn search_filters_by_title_substring() {
        let mut view = SwarmView::new();
        let issues = vec![
            make_issue(1, "Fix login bug", &[]),
            make_issue(2, "Add dark mode", &[]),
            make_issue(3, "Fix logout flow", &[]),
        ];
        view.search_query = Some("login".to_string());
        let results = view.issues_matching_search(&issues);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].number, 1);
    }

    #[test]
    fn search_filters_by_issue_number() {
        let mut view = SwarmView::new();
        let issues = vec![
            make_issue(42, "Some issue", &[]),
            make_issue(123, "Another issue", &[]),
        ];
        view.search_query = Some("42".to_string());
        let results = view.issues_matching_search(&issues);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].number, 42);
    }

    #[test]
    fn search_with_hash_prefix_filters_by_exact_number() {
        let mut view = SwarmView::new();
        let issues = vec![
            make_issue(42, "Some issue", &[]),
            make_issue(123, "Another issue", &[]),
        ];
        view.search_query = Some("#42".to_string());
        let results = view.issues_matching_search(&issues);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].number, 42);
    }

    #[test]
    fn empty_search_returns_all_filtered_issues() {
        let view = SwarmView::new();
        let issues = vec![
            make_issue(1, "Issue one", &[]),
            make_issue(2, "Issue two", &[]),
        ];
        let results = view.issues_matching_search(&issues);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_respects_issue_filter() {
        let mut view = SwarmView::new();
        view.issue_filter = IssueFilter::Blocked;
        let issues = vec![
            make_issue(1, "Open issue", &[]),
            make_issue(2, "Blocked issue", &["needs-design"]),
        ];
        let results = view.issues_matching_search(&issues);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].number, 2);
    }

    #[test]
    fn issues_title_shows_fraction_when_filter_active() {
        use crate::model::issue::{IssueType, IssuePriority, IssueState};
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut view = SwarmView::new();
        view.cycle_issue_type_filter(); // → Bug filter
        let swarm = make_swarm();
        let issues = vec![
            GitHubIssue {
                number: 1, title: "A bug".into(), state: IssueState::Open,
                priority: IssuePriority::P1, issue_type: IssueType::Bug,
                labels: vec!["bug".into()], is_working: false, assigned_worker: None, updated_at: None,
            },
            GitHubIssue {
                number: 2, title: "An enhancement".into(), state: IssueState::Open,
                priority: IssuePriority::P2, issue_type: IssueType::Enhancement,
                labels: vec!["enhancement".into()], is_working: false, assigned_worker: None, updated_at: None,
            },
        ];
        terminal.draw(|f| {
            view.render(f, f.area(), &swarm, &issues, SwarmPanel::Issues, false, false);
        }).unwrap();
        let rendered: String = terminal.backend().buffer().content().iter().map(|c| c.symbol()).collect();
        // Bug filter active: 1 bug shown out of 2 total
        assert!(rendered.contains("1/2"), "Expected '1/2' in rendered output, got: {rendered}");
    }

    #[test]
    fn issues_title_shows_plain_count_when_no_filter() {
        use crate::model::issue::{IssueType, IssuePriority, IssueState};
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut view = SwarmView::new();
        let swarm = make_swarm();
        let issues = vec![
            GitHubIssue {
                number: 1, title: "A bug".into(), state: IssueState::Open,
                priority: IssuePriority::P1, issue_type: IssueType::Bug,
                labels: vec!["bug".into()], is_working: false, assigned_worker: None, updated_at: None,
            },
            GitHubIssue {
                number: 2, title: "An enhancement".into(), state: IssueState::Open,
                priority: IssuePriority::P2, issue_type: IssueType::Enhancement,
                labels: vec!["enhancement".into()], is_working: false, assigned_worker: None, updated_at: None,
            },
        ];
        terminal.draw(|f| {
            view.render(f, f.area(), &swarm, &issues, SwarmPanel::Issues, false, false);
        }).unwrap();
        let rendered: String = terminal.backend().buffer().content().iter().map(|c| c.symbol()).collect();
        // No filter: plain count "all: 2", no fraction
        assert!(rendered.contains("all: 2"), "Expected 'all: 2' in rendered output");
        assert!(!rendered.contains("2/2"), "Should not show fraction when unfiltered");
    }

    #[test]
    fn priority_filter_cycles_correctly() {
        use crate::model::issue::IssuePriority;
        let mut view = SwarmView::new();
        assert_eq!(view.priority_filter, None);
        view.cycle_priority_filter();
        assert_eq!(view.priority_filter, Some(IssuePriority::P0));
        view.cycle_priority_filter();
        assert_eq!(view.priority_filter, Some(IssuePriority::P1));
        view.cycle_priority_filter();
        assert_eq!(view.priority_filter, Some(IssuePriority::P2));
        view.cycle_priority_filter();
        assert_eq!(view.priority_filter, Some(IssuePriority::P3));
        view.cycle_priority_filter();
        assert_eq!(view.priority_filter, None);
    }

    #[test]
    fn priority_filter_excludes_non_matching() {
        use crate::model::issue::{IssueType, IssuePriority, IssueState};
        let issues = vec![
            GitHubIssue { number: 1, title: "p1 bug".into(), state: IssueState::Open, priority: IssuePriority::P1, issue_type: IssueType::Bug, labels: vec![], is_working: false, assigned_worker: None, updated_at: None },
            GitHubIssue { number: 2, title: "p2 enh".into(), state: IssueState::Open, priority: IssuePriority::P2, issue_type: IssueType::Enhancement, labels: vec![], is_working: false, assigned_worker: None, updated_at: None },
            GitHubIssue { number: 3, title: "p3 other".into(), state: IssueState::Open, priority: IssuePriority::P3, issue_type: IssueType::Other, labels: vec![], is_working: false, assigned_worker: None, updated_at: None },
        ];
        let mut view = SwarmView::new();
        view.cycle_priority_filter(); // → P0 (nothing matches)
        assert_eq!(view.apply_filters(&issues).len(), 0);

        view.cycle_priority_filter(); // → P1
        let filtered = view.apply_filters(&issues);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].number, 1);

        view.cycle_priority_filter(); // → P2
        let filtered = view.apply_filters(&issues);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].number, 2);
    }
}
