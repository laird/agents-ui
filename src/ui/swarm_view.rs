use ansi_to_tui::IntoText;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
};
use std::time::Instant;

use crate::model::issue::{GitHubIssue, IssueFilter, IssuePriority, IssueType};
use crate::model::swarm::Swarm;
use super::text_input::TextInput;
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

    pub fn prev(self) -> Self {
        match self {
            SwarmPanel::Manager => SwarmPanel::Issues,
            SwarmPanel::Workers => SwarmPanel::Manager,
            SwarmPanel::Issues => SwarmPanel::Workers,
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
    /// Active issue search query (`/` activates search mode)
    pub issue_search: Option<TextInput>,
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
            issue_search: None,
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
        last_fetched: Option<Instant>,
        manager_input: &TextInput,
    ) {
        let filtered_issues = self.apply_filters(issues);

        // Pre-compute attention data before layout (needed for dynamic sizing)
        let attention = count_attention(swarm, issues);
        let working = swarm.busy_count();
        let total_workers = swarm.workers.len();
        let idle = total_workers - working;
        let avail_issues = issues
            .iter()
            .filter(|i| {
                !i.is_blocked()
                    && !i.is_being_worked()
                    && i.state == crate::model::issue::IssueState::Open
            })
            .count();
        let blocked_issues: Vec<&GitHubIssue> = issues.iter().filter(|i| i.is_blocked()).collect();
        let blocked_count = blocked_issues.len();

        // Header is 1 line normally; 2 lines when there are blocked issues to surface inline
        let header_height: u16 = if blocked_count > 0 { 2 } else { 1 };

        let chunks = Layout::vertical([
            Constraint::Length(header_height), // Header line(s)
            Constraint::Min(4),                // Body (manager + workers/issues)
            Constraint::Length(2),             // Help bar
        ])
        .split(area);

        // Size bottom panel to fit the longer of workers or issues (+3 for borders+header row)
        // but never more than 50% of the body area so the manager always has room
        let max_bottom = chunks[1].height / 2;
        let bottom_rows =
            ((swarm.workers.len().max(filtered_issues.len()) + 3) as u16).min(max_bottom);
        let body_chunks = Layout::vertical([
            Constraint::Min(4),              // Manager gets all remaining space
            Constraint::Length(bottom_rows), // Workers/Issues: sized to fit content
        ])
        .split(chunks[1]);

        use crate::model::swarm::HealthStatus;
        let healthy = swarm.workers.iter().filter(|w| w.health.status() == HealthStatus::Healthy).count();
        let stalled = swarm.workers.iter().filter(|w| w.health.status() == HealthStatus::Stalled).count();
        let dead = swarm.workers.iter().filter(|w| w.health.status() == HealthStatus::Dead).count();
        let restarting = swarm.workers.iter().filter(|w| w.health.status() == HealthStatus::Restarting).count();

        let mut header_spans = vec![
            Span::styled(format!(" {} ", swarm.project_name), theme::title_style()),
            Span::styled("Active ", Style::default().fg(ratatui::style::Color::Green)),
            Span::styled(
                format!("{}W: {} working, {} idle  ", total_workers, working, idle),
                theme::help_style(),
            ),
            Span::styled(
                format!(
                    "Issues: {} avail, {} blocked  ",
                    avail_issues, blocked_count
                ),
                theme::help_style(),
            ),
        ];
        if stalled > 0 || dead > 0 || restarting > 0 {
            header_spans.push(Span::styled(
                format!("Health: ✓{} ⚠{} ↺{} ✗{}  ", healthy, stalled, restarting, dead),
                if dead > 0 {
                    Style::default().fg(ratatui::style::Color::Red)
                } else {
                    Style::default().fg(ratatui::style::Color::Yellow)
                },
            ));
        }
        if attention > 0 {
            let style = theme::attention_blink_style(blink);
            header_spans.push(Span::styled(format!("⚠ {attention} need attention"), style));
        }
        let left_len: usize = header_spans.iter().map(|s| s.content.len()).sum();
        header_spans.push(theme::hostname_right_span(
            left_len,
            chunks[0].width as usize,
        ));

        // Build header lines: status line + optional inline attention row
        let mut header_lines = vec![Line::from(header_spans)];
        if blocked_count > 0 {
            let mut attn_spans = vec![Span::styled(" ⚠ ", theme::attention_style())];
            let show_n = blocked_count.min(3);
            for (idx, issue) in blocked_issues.iter().take(show_n).enumerate() {
                if idx > 0 {
                    attn_spans.push(Span::styled("  ", theme::help_style()));
                }
                let blocking_label = issue
                    .labels
                    .iter()
                    .find(|l| crate::model::issue::BLOCKING_LABELS.contains(&l.as_str()))
                    .map(|s| s.as_str())
                    .unwrap_or("blocked");
                let guidance = crate::model::issue::blocking_guidance(blocking_label);
                attn_spans.push(Span::styled(
                    format!(
                        "#{} [{}] {}",
                        issue.number,
                        blocking_label,
                        truncate(&issue.title, 25)
                    ),
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
        let manager_block = Block::default()
            .borders(Borders::ALL)
            .title(" Manager ")
            .border_style(if focus == SwarmPanel::Manager {
                theme::title_style()
            } else {
                Style::default()
            });

        let manager_rows =
            Layout::vertical([Constraint::Min(4), Constraint::Length(1)]).split(body_chunks[0]);
        let content_width = manager_rows[0].width.saturating_sub(2).max(1);
        let total_lines = wrapped_line_count(&text, content_width);
        let visible = manager_rows[0].height.saturating_sub(2);
        let max_scroll = total_lines.saturating_sub(visible);
        if self.manager_scroll > max_scroll {
            self.manager_scroll = max_scroll;
        }

        let manager = Paragraph::new(text)
            .block(manager_block)
            .wrap(Wrap { trim: false })
            .scroll((self.manager_scroll, 0));
        f.render_widget(manager, manager_rows[0]);

        let manager_input = Paragraph::new(manager_input.render_line("> ")).block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(if focus == SwarmPanel::Manager {
                    theme::title_style()
                } else {
                    Style::default()
                }),
        );
        f.render_widget(manager_input, manager_rows[1]);

        // --- Bottom split: Workers (left) | Issues (right) ---
        let bottom_cols =
            Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(body_chunks[1]);

        // Workers table
        let worker_header = Row::new(vec![
            Cell::from("#"),
            Cell::from("H"),
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
                use crate::model::swarm::HealthStatus;
                let (health_sym, health_style) = match w.health.status() {
                    HealthStatus::Healthy => ("✓", Style::default().fg(ratatui::style::Color::Green)),
                    HealthStatus::Stalled => ("⚠", Style::default().fg(ratatui::style::Color::Yellow)),
                    HealthStatus::Restarting => ("↺", Style::default().fg(ratatui::style::Color::Cyan)),
                    HealthStatus::Dead => ("✗", Style::default().fg(ratatui::style::Color::Red)),
                };
                Row::new(vec![
                    Cell::from(format!("{}", i + 1)),
                    Cell::from(health_sym).style(health_style),
                    Cell::from(status_str).style(status_style),
                    Cell::from(task),
                    Cell::from(age).style(Style::default().fg(ratatui::style::Color::DarkGray)),
                ])
            })
            .collect();

        let workers_block = Block::default()
            .borders(Borders::ALL)
            .title({
                let busy = swarm.busy_count();
                let total = swarm.workers.len();
                let idle = total.saturating_sub(busy);
                if total == 0 {
                    " Workers (0) ".to_string()
                } else if busy == 0 {
                    format!(" Workers ({total} idle) ")
                } else if idle == 0 {
                    format!(" Workers ({busy} busy) ")
                } else {
                    format!(" Workers ({busy} busy, {idle} idle) ")
                }
            })
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
                Constraint::Length(2),
                Constraint::Percentage(40),
                Constraint::Percentage(50),
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
            let parts =
                Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(issues_col);
            (Some(parts[0]), parts[1])
        } else {
            (None, issues_col)
        };

        // Render search bar
        if let (Some(area), Some(query)) = (search_area, &self.search_query) {
            let bar = Paragraph::new(Line::from(vec![
                Span::styled(" / ", theme::title_style()),
                Span::styled(
                    query.as_str(),
                    Style::default().fg(ratatui::style::Color::White),
                ),
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

        // T(3) + #(5) + Pri(4) + Status(18) + 2 borders + ~7 column spacing = 37 overhead
        let title_col_width = (table_area.width.saturating_sub(37) as usize).max(15);

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
                    Cell::from(issue.priority_label())
                        .style(theme::priority_style(&issue.priority)),
                    Cell::from(truncate(&issue.title, title_col_width)),
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
        let staleness_secs = last_fetched.map(|t| t.elapsed().as_secs());
        let staleness_str = match staleness_secs {
            Some(s) if s >= 120 => format!(
                " · {}m old{}",
                s / 60,
                if s >= 600 { " \u{27F3}" } else { "" }
            ),
            _ => String::new(),
        };
        let issues_block = Block::default()
            .borders(Borders::ALL)
            .title(format!(
                " Issues ({filter_label}{type_label}{priority_label}: {}{}{}) ",
                count_str,
                if issues_loading {
                    ", loading\u{2026}"
                } else {
                    ""
                },
                staleness_str
            ))
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

        // Split issues area: search bar (when active) + issues table
        let (search_area, issues_table_area) = if self.issue_search.is_some() {
            let split = Layout::vertical([
                Constraint::Length(3),
                Constraint::Min(3),
            ])
            .split(bottom_cols[1]);
            (Some(split[0]), split[1])
        } else {
            (None, bottom_cols[1])
        };

        if let (Some(area), Some(search)) = (search_area, &self.issue_search) {
            let search_block = Block::default()
                .borders(Borders::ALL)
                .title(" Search ")
                .border_style(theme::title_style());
            let search_widget = Paragraph::new(search.render_line(" / ")).block(search_block);
            f.render_widget(search_widget, area);
        }

        f.render_stateful_widget(issues_table, issues_table_area, &mut self.issues_table);

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
                Span::styled("r", theme::title_style()),
                Span::styled(" refresh  ", theme::help_style()),
                Span::styled("f", theme::title_style()),
                Span::styled(" fix-loop  ", theme::help_style()),
                Span::styled("F", theme::title_style()),
                Span::styled(" all-idle  ", theme::help_style()),
                Span::styled("g", theme::title_style()),
                Span::styled(" browser  ", theme::help_style()),
                Span::styled("d", theme::title_style()),
                Span::styled(" shutdown  ", theme::help_style()),
                Span::styled("a", theme::title_style()),
                Span::styled(" add  ", theme::help_style()),
                Span::styled("S", theme::title_style()),
                Span::styled(" switch agent  ", theme::help_style()),
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
                Span::styled(" refresh  ", theme::help_style()),
                Span::styled("R", theme::title_style()),
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
                Span::styled("u", theme::title_style()),
                Span::styled(" release  ", theme::help_style()),
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
        if len == 0 {
            return;
        }
        let i = self.workers_table.selected().unwrap_or(0);
        self.workers_table.select(Some((i + 1) % len));
    }

    /// Returns `true` if already at the top (caller should focus the manager panel).
    pub fn prev_worker(&mut self, len: usize) -> bool {
        if len == 0 {
            return true;
        }
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
        if len == 0 {
            return;
        }
        let i = self.issues_table.selected().unwrap_or(0);
        self.issues_table.select(Some((i + 1) % len));
    }

    pub fn prev_issue(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let i = self.issues_table.selected().unwrap_or(0);
        self.issues_table
            .select(Some(if i == 0 { len - 1 } else { i - 1 }));
    }

    pub fn selected_issue(&self) -> Option<usize> {
        self.issues_table.selected()
    }

    /// Return all issues passing every active filter (status, type, priority, search query),
    /// sorted by priority then issue number — exactly matching the order rendered by `render()`.
    pub fn apply_filters<'a>(&self, issues: &'a [GitHubIssue]) -> Vec<&'a GitHubIssue> {
        let mut result: Vec<&'a GitHubIssue> = issues
            .iter()
            .filter(|i| i.matches_filter(self.issue_filter))
            .filter(|i| {
                self.issue_type_filter
                    .as_ref()
                    .map_or(true, |tf| &i.issue_type == tf)
            })
            .filter(|i| {
                self.priority_filter
                    .as_ref()
                    .map_or(true, |pf| &i.priority == pf)
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
        result.sort_by_key(|i| (&i.priority, i.number));
        result
    }

    /// Render a full-screen issue list for the given project.
    pub fn render_issue_list(
        &mut self,
        f: &mut Frame,
        area: Rect,
        project_name: &str,
        issues: &[GitHubIssue],
    ) {
        let search_query = self.issue_search.as_ref().map(|s| s.text().to_lowercase());
        let filtered: Vec<&GitHubIssue> = issues
            .iter()
            .filter(|i| i.matches_filter(self.issue_filter))
            .filter(|i| {
                if let Some(ref q) = search_query {
                    if q.is_empty() { return true; }
                    if let Some(num_str) = q.strip_prefix('#') {
                        if let Ok(num) = num_str.parse::<u32>() {
                            return i.number == num;
                        }
                    }
                    i.title.to_lowercase().contains(q.as_str())
                } else {
                    true
                }
            })
            .collect();

        let avail = issues.iter().filter(|i| !i.is_blocked() && !i.is_being_worked()).count();
        let blocked = issues.iter().filter(|i| i.is_blocked()).count();
        let filter_label = self.issue_filter.label();

        let chunks = Layout::vertical([
            Constraint::Length(1),  // Header
            Constraint::Min(3),     // Table (with optional search bar)
            Constraint::Length(2),  // Help bar
        ])
        .split(area);

        // Header
        let mut header_spans = vec![
            Span::styled(format!(" {} ", project_name), theme::title_style()),
            Span::styled(
                format!("Issues: {} avail  {} blocked  filter: {}  showing {}", avail, blocked, filter_label, filtered.len()),
                theme::help_style(),
            ),
        ];
        let left_len: usize = header_spans.iter().map(|s| s.content.len()).sum();
        header_spans.push(theme::hostname_right_span(left_len, chunks[0].width as usize));
        f.render_widget(Paragraph::new(Line::from(header_spans)), chunks[0]);

        // Split table area for optional search bar
        let (search_area, table_area) = if self.issue_search.is_some() {
            let split = Layout::vertical([
                Constraint::Length(3),
                Constraint::Min(3),
            ])
            .split(chunks[1]);
            (Some(split[0]), split[1])
        } else {
            (None, chunks[1])
        };

        if let (Some(area), Some(search)) = (search_area, &self.issue_search) {
            let search_block = Block::default()
                .borders(Borders::ALL)
                .title(" Search ")
                .border_style(theme::title_style());
            let search_widget = Paragraph::new(search.render_line(" / ")).block(search_block);
            f.render_widget(search_widget, area);
        }

        // Issues table (full-width, title grows with available space)
        let issue_header = Row::new(vec![
            Cell::from("#"),
            Cell::from("Pri"),
            Cell::from("Title"),
            Cell::from("Status"),
        ])
        .style(theme::header_style());

        let issue_rows: Vec<Row> = filtered
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
                Row::new(vec![
                    Cell::from(format!("{}", issue.number)),
                    Cell::from(issue.priority_label()),
                    Cell::from(issue.title.clone()),
                    Cell::from(status).style(status_style),
                ])
            })
            .collect();

        let issues_block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Issues ({filter_label}: {}) ", filtered.len()))
            .border_style(theme::title_style());

        let table = Table::new(
            issue_rows,
            [
                Constraint::Length(5),
                Constraint::Length(4),
                Constraint::Min(20),
                Constraint::Length(20),
            ],
        )
        .header(issue_header)
        .block(issues_block)
        .row_highlight_style(theme::selected_style());

        f.render_stateful_widget(table, table_area, &mut self.issues_table);

        // Help bar
        let help = Paragraph::new(Line::from(vec![
            Span::styled(" Esc", theme::title_style()),
            Span::styled(" back  ", theme::help_style()),
            Span::styled("Enter", theme::title_style()),
            Span::styled(" view  ", theme::help_style()),
            Span::styled("d/Space", theme::title_style()),
            Span::styled(" dispatch  ", theme::help_style()),
            Span::styled("f", theme::title_style()),
            Span::styled(" filter  ", theme::help_style()),
            Span::styled("/", theme::title_style()),
            Span::styled(" search  ", theme::help_style()),
            Span::styled("r/F5", theme::title_style()),
            Span::styled(" refresh", theme::help_style()),
        ]));
        f.render_widget(help, chunks[2]);
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
/// Re-exported so the TUI and the web dashboard cannot drift apart again; the
/// implementation lives in `model::status`.
pub use crate::model::status::agent_needs_input;

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

/// Truncate to `max` characters, appending an ellipsis when anything is cut.
///
/// Counts characters, not bytes. `s.len()` is a byte count, so the byte slice
/// this used to do both measured the wrong thing and panicked outright when
/// `max - 1` landed inside a multi-byte character: one `↔` in a pane title was
/// enough to take the whole TUI down.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}

fn wrapped_line_count(text: &Text<'_>, content_width: u16) -> u16 {
    let width = content_width.max(1) as usize;
    let total: usize = text
        .lines
        .iter()
        .map(|line| {
            let line_width = line.width();
            if line_width == 0 {
                1
            } else {
                line_width.div_ceil(width)
            }
        })
        .sum();
    total.min(u16::MAX as usize) as u16
}

#[cfg(test)]
mod tests {

    /// The crash that motivated char-based truncation: a pane line containing
    /// `↔` panicked with "byte index N is not a char boundary".
    #[test]
    fn truncate_does_not_split_a_multibyte_character() {
        let s = "worker-1 ↔ manager sync in progress";
        for max in 0..=s.chars().count() + 2 {
            let out = truncate(s, max);
            assert!(
                out.chars().count() <= max.max(1),
                "truncate({s:?}, {max}) returned {out:?}"
            );
        }
    }

    #[test]
    fn truncate_counts_characters_not_bytes() {
        // Ten characters, thirty bytes: a byte-based check would truncate this
        // even though it fits.
        let s = "↔↔↔↔↔↔↔↔↔↔";
        assert_eq!(truncate(s, 10), s);
        assert_eq!(truncate(s, 4), "↔↔↔…");
    }
    use super::{SwarmPanel, SwarmView, agent_needs_input, truncate};
    use crate::model::issue::{GitHubIssue, IssueFilter, IssueState};
    use crate::model::status::{AgentState, AgentStatus};
    use crate::model::swarm::{AgentInfo, AgentType, Swarm};
    use crate::ui::text_input::TextInput;
    use ratatui::{Terminal, backend::TestBackend};
    use std::path::PathBuf;

    fn make_agent(id: &str, is_manager: bool, pane_content: &str, state: AgentState) -> AgentInfo {
        AgentInfo {
            id: format!("test/{id}"),
            role: id.to_string(),
            worktree_path: PathBuf::new(),
            branch: None,
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
            resurrection_attempts: 0,
            completed_issue_count: 0,
            health: crate::model::swarm::WorkerHealth::default(),
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
    fn swarm_panel_next_cycles() {
        assert!(matches!(SwarmPanel::Manager.next(), SwarmPanel::Workers));
        assert!(matches!(SwarmPanel::Workers.next(), SwarmPanel::Issues));
        assert!(matches!(SwarmPanel::Issues.next(), SwarmPanel::Manager));
    }

    #[test]
    fn scroll_manager_up_saturates_at_zero() {
        let mut view = SwarmView::new();
        view.scroll_manager_down(5);
        view.scroll_manager_up(3);
        assert_eq!(view.manager_scroll, 2);
        view.scroll_manager_up(100);
        assert_eq!(view.manager_scroll, 0);
    }

    #[test]
    fn scroll_manager_down_increments() {
        let mut view = SwarmView::new();
        view.scroll_manager_down(10);
        assert_eq!(view.manager_scroll, 10);
    }

    #[test]
    fn next_worker_increments_selection() {
        let mut view = SwarmView::new();
        view.next_worker(3);
        assert_eq!(view.selected_worker(), Some(1));
    }

    #[test]
    fn next_worker_wraps_at_end() {
        let mut view = SwarmView::new();
        view.next_worker(3); // 0->1
        view.next_worker(3); // 1->2
        view.next_worker(3); // 2->0
        assert_eq!(view.selected_worker(), Some(0));
    }

    #[test]
    fn next_worker_with_empty_list_does_not_panic() {
        let mut view = SwarmView::new();
        view.next_worker(0);
        assert_eq!(view.selected_worker(), Some(0));
    }

    #[test]
    fn prev_worker_returns_true_at_top() {
        let mut view = SwarmView::new(); // starts at 0
        let at_top = view.prev_worker(3);
        assert!(at_top);
        assert_eq!(view.selected_worker(), Some(0));
    }

    #[test]
    fn prev_worker_decrements_selection() {
        let mut view = SwarmView::new();
        view.next_worker(3); // 0->1
        let at_top = view.prev_worker(3); // 1->0
        assert!(!at_top);
        assert_eq!(view.selected_worker(), Some(0));
    }

    #[test]
    fn prev_worker_with_empty_list_returns_true() {
        let mut view = SwarmView::new();
        let at_top = view.prev_worker(0);
        assert!(at_top);
    }

    #[test]
    fn next_issue_increments_selection() {
        let mut view = SwarmView::new();
        view.next_issue(3);
        assert_eq!(view.selected_issue(), Some(1));
    }

    #[test]
    fn next_issue_wraps_at_end() {
        let mut view = SwarmView::new();
        view.next_issue(3); // 0->1
        view.next_issue(3); // 1->2
        view.next_issue(3); // 2->0
        assert_eq!(view.selected_issue(), Some(0));
    }

    #[test]
    fn prev_issue_wraps_to_last() {
        let mut view = SwarmView::new();
        view.prev_issue(3); // 0->2
        assert_eq!(view.selected_issue(), Some(2));
    }

    #[test]
    fn next_issue_with_empty_list_does_not_panic() {
        let mut view = SwarmView::new();
        view.next_issue(0);
        assert_eq!(view.selected_issue(), Some(0));
    }

    #[test]
    fn prev_issue_with_empty_list_does_not_panic() {
        let mut view = SwarmView::new();
        view.prev_issue(0);
        assert_eq!(view.selected_issue(), Some(0));
    }

    #[test]
    fn count_attention_counts_waiting_agents_and_blocked_issues() {
        use super::count_attention;
        use crate::model::issue::GitHubIssue;

        let mut swarm = make_swarm();
        // Give manager waiting pane content
        swarm.manager.pane_content = "Do you want to proceed? (y/n)".to_string();
        // Give worker-1 waiting pane content (matches "should i proceed" pattern)
        swarm.workers[0].pane_content = "should i proceed with this change?".to_string();

        let issues: Vec<GitHubIssue> = vec![];
        // 2 agents waiting, 0 blocked issues
        assert_eq!(count_attention(&swarm, &issues), 2);
    }

    #[test]
    fn count_attention_zero_when_all_idle() {
        use super::count_attention;
        let swarm = make_swarm();
        let issues: Vec<crate::model::issue::GitHubIssue> = vec![];
        assert_eq!(count_attention(&swarm, &issues), 0);
    }

    #[test]
    fn issue_type_filter_cycles() {
        let mut view = SwarmView::new();
        assert_eq!(view.issue_type_filter, None);
        view.cycle_issue_type_filter();
        assert_eq!(
            view.issue_type_filter,
            Some(crate::model::issue::IssueType::Bug)
        );
        view.cycle_issue_type_filter();
        assert_eq!(
            view.issue_type_filter,
            Some(crate::model::issue::IssueType::Enhancement)
        );
        view.cycle_issue_type_filter();
        assert_eq!(
            view.issue_type_filter,
            Some(crate::model::issue::IssueType::Proposal)
        );
        view.cycle_issue_type_filter();
        assert_eq!(view.issue_type_filter, None);
    }

    #[test]
    fn issue_type_filter_excludes_non_matching() {
        use crate::model::issue::{IssuePriority, IssueState, IssueType};
        let mut view = SwarmView::new();
        view.cycle_issue_type_filter(); // → Bug

        let bug = GitHubIssue {
            number: 1,
            title: "a bug".into(),
            state: IssueState::Open,
            priority: IssuePriority::P2,
            issue_type: IssueType::Bug,
            labels: vec!["bug".into()],
            is_working: false,
            assigned_worker: None,
            updated_at: None,
        };
        let enh = GitHubIssue {
            number: 2,
            title: "an enh".into(),
            state: IssueState::Open,
            priority: IssuePriority::P3,
            issue_type: IssueType::Enhancement,
            labels: vec!["enhancement".into()],
            is_working: false,
            assigned_worker: None,
            updated_at: None,
        };
        let issues = vec![bug, enh];
        // Only bug should pass the type filter
        let filtered: Vec<_> = issues
            .iter()
            .filter(|i| {
                if let Some(ref tf) = view.issue_type_filter {
                    &i.issue_type == tf
                } else {
                    true
                }
            })
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].number, 1);
    }

    #[test]
    fn apply_filters_sorts_by_priority_then_number() {
        use crate::model::issue::{IssuePriority, IssueState, IssueType};
        let issues = vec![
            GitHubIssue {
                number: 10,
                title: "a".into(),
                state: IssueState::Open,
                priority: IssuePriority::P3,
                issue_type: IssueType::Other,
                labels: vec![],
                is_working: false,
                assigned_worker: None,
                updated_at: None,
            },
            GitHubIssue {
                number: 5,
                title: "b".into(),
                state: IssueState::Open,
                priority: IssuePriority::P1,
                issue_type: IssueType::Bug,
                labels: vec![],
                is_working: false,
                assigned_worker: None,
                updated_at: None,
            },
            GitHubIssue {
                number: 3,
                title: "c".into(),
                state: IssueState::Open,
                priority: IssuePriority::P1,
                issue_type: IssueType::Bug,
                labels: vec![],
                is_working: false,
                assigned_worker: None,
                updated_at: None,
            },
            GitHubIssue {
                number: 8,
                title: "d".into(),
                state: IssueState::Open,
                priority: IssuePriority::P2,
                issue_type: IssueType::Enhancement,
                labels: vec![],
                is_working: false,
                assigned_worker: None,
                updated_at: None,
            },
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
        assert_eq!(
            view.priority_filter,
            Some(crate::model::issue::IssuePriority::P0)
        );
        view.cycle_priority_filter();
        assert_eq!(
            view.priority_filter,
            Some(crate::model::issue::IssuePriority::P1)
        );
        view.cycle_priority_filter();
        assert_eq!(
            view.priority_filter,
            Some(crate::model::issue::IssuePriority::P2)
        );
        view.cycle_priority_filter();
        assert_eq!(
            view.priority_filter,
            Some(crate::model::issue::IssuePriority::P3)
        );
        view.cycle_priority_filter();
        assert_eq!(view.priority_filter, None);
    }

    #[test]
    fn priority_filter_shows_only_matching_priority() {
        use crate::model::issue::{IssuePriority, IssueState, IssueType};
        let issues = vec![
            GitHubIssue {
                number: 1,
                title: "a".into(),
                state: IssueState::Open,
                priority: IssuePriority::P1,
                issue_type: IssueType::Bug,
                labels: vec![],
                is_working: false,
                assigned_worker: None,
                updated_at: None,
            },
            GitHubIssue {
                number: 2,
                title: "b".into(),
                state: IssueState::Open,
                priority: IssuePriority::P2,
                issue_type: IssueType::Enhancement,
                labels: vec![],
                is_working: false,
                assigned_worker: None,
                updated_at: None,
            },
            GitHubIssue {
                number: 3,
                title: "c".into(),
                state: IssueState::Open,
                priority: IssuePriority::P3,
                issue_type: IssueType::Other,
                labels: vec![],
                is_working: false,
                assigned_worker: None,
                updated_at: None,
            },
        ];
        let mut view = SwarmView::new();
        view.priority_filter = Some(IssuePriority::P1);
        let filtered = view.apply_filters(&issues);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].number, 1);
    }

    #[test]
    fn detects_confirmation_prompts() {
        assert!(agent_needs_input(
            "Would you like to proceed?\nPress enter to confirm"
        ));
        assert!(!agent_needs_input("All good, continuing work"));
    }

    #[test]
    fn render_smoke_writes_swarm_sections() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut view = SwarmView::new();
        let manager_input = TextInput::new();
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
                view.render(
                    f,
                    f.area(),
                    &swarm,
                    &issues,
                    SwarmPanel::Manager,
                    false,
                    false,
                    None,
                    &manager_input,
                );
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let rendered = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("Manager"));
        assert!(rendered.contains("Workers (1 busy)"));
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
        let results = view.apply_filters(&issues);
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
        let results = view.apply_filters(&issues);
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
        let results = view.apply_filters(&issues);
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
        let results = view.apply_filters(&issues);
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
        let results = view.apply_filters(&issues);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].number, 2);
    }

    #[test]
    fn issues_title_shows_fraction_when_filter_active() {
        use crate::model::issue::{IssuePriority, IssueState, IssueType};
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut view = SwarmView::new();
        let manager_input = TextInput::new();
        view.cycle_issue_type_filter(); // → Bug filter
        let swarm = make_swarm();
        let issues = vec![
            GitHubIssue {
                number: 1,
                title: "A bug".into(),
                state: IssueState::Open,
                priority: IssuePriority::P1,
                issue_type: IssueType::Bug,
                labels: vec!["bug".into()],
                is_working: false,
                assigned_worker: None,
                updated_at: None,
            },
            GitHubIssue {
                number: 2,
                title: "An enhancement".into(),
                state: IssueState::Open,
                priority: IssuePriority::P2,
                issue_type: IssueType::Enhancement,
                labels: vec!["enhancement".into()],
                is_working: false,
                assigned_worker: None,
                updated_at: None,
            },
        ];
        terminal
            .draw(|f| {
                view.render(
                    f,
                    f.area(),
                    &swarm,
                    &issues,
                    SwarmPanel::Issues,
                    false,
                    false,
                    None,
                    &manager_input,
                );
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        // Bug filter active: 1 bug shown out of 2 total
        assert!(
            rendered.contains("1/2"),
            "Expected '1/2' in rendered output, got: {rendered}"
        );
    }

    #[test]
    fn issues_title_shows_plain_count_when_no_filter() {
        use crate::model::issue::{IssuePriority, IssueState, IssueType};
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut view = SwarmView::new();
        let manager_input = TextInput::new();
        let swarm = make_swarm();
        let issues = vec![
            GitHubIssue {
                number: 1,
                title: "A bug".into(),
                state: IssueState::Open,
                priority: IssuePriority::P1,
                issue_type: IssueType::Bug,
                labels: vec!["bug".into()],
                is_working: false,
                assigned_worker: None,
                updated_at: None,
            },
            GitHubIssue {
                number: 2,
                title: "An enhancement".into(),
                state: IssueState::Open,
                priority: IssuePriority::P2,
                issue_type: IssueType::Enhancement,
                labels: vec!["enhancement".into()],
                is_working: false,
                assigned_worker: None,
                updated_at: None,
            },
        ];
        terminal
            .draw(|f| {
                view.render(
                    f,
                    f.area(),
                    &swarm,
                    &issues,
                    SwarmPanel::Issues,
                    false,
                    false,
                    None,
                    &manager_input,
                );
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        // No filter: plain count "all: 2", no fraction
        assert!(
            rendered.contains("all: 2"),
            "Expected 'all: 2' in rendered output"
        );
        assert!(
            !rendered.contains("2/2"),
            "Should not show fraction when unfiltered"
        );
    }

    #[test]
    fn issues_title_shows_staleness_indicator() {
        use crate::model::issue::{IssuePriority, IssueState, IssueType};
        use std::time::{Duration, Instant};
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let swarm = make_swarm();
        let manager_input = TextInput::new();
        let issues = vec![GitHubIssue {
            number: 1,
            title: "A bug".into(),
            state: IssueState::Open,
            priority: IssuePriority::P1,
            issue_type: IssueType::Bug,
            labels: vec!["bug".into()],
            is_working: false,
            assigned_worker: None,
            updated_at: None,
        }];

        // Fresh: no indicator
        let fresh = Instant::now();
        let mut view = SwarmView::new();
        terminal
            .draw(|f| {
                view.render(
                    f,
                    f.area(),
                    &swarm,
                    &issues,
                    SwarmPanel::Issues,
                    false,
                    false,
                    Some(fresh),
                    &manager_input,
                );
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(
            !rendered.contains("old"),
            "Fresh data should show no staleness indicator"
        );

        // Stale (simulate 5 min old): expect "5m old"
        let stale = Instant::now()
            .checked_sub(Duration::from_secs(300))
            .unwrap_or(Instant::now());
        terminal
            .draw(|f| {
                view.render(
                    f,
                    f.area(),
                    &swarm,
                    &issues,
                    SwarmPanel::Issues,
                    false,
                    false,
                    Some(stale),
                    &manager_input,
                );
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(
            rendered.contains("m old"),
            "Stale data should show staleness indicator"
        );
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
        use crate::model::issue::{IssuePriority, IssueState, IssueType};
        let issues = vec![
            GitHubIssue {
                number: 1,
                title: "p1 bug".into(),
                state: IssueState::Open,
                priority: IssuePriority::P1,
                issue_type: IssueType::Bug,
                labels: vec![],
                is_working: false,
                assigned_worker: None,
                updated_at: None,
            },
            GitHubIssue {
                number: 2,
                title: "p2 enh".into(),
                state: IssueState::Open,
                priority: IssuePriority::P2,
                issue_type: IssueType::Enhancement,
                labels: vec![],
                is_working: false,
                assigned_worker: None,
                updated_at: None,
            },
            GitHubIssue {
                number: 3,
                title: "p3 other".into(),
                state: IssueState::Open,
                priority: IssuePriority::P3,
                issue_type: IssueType::Other,
                labels: vec![],
                is_working: false,
                assigned_worker: None,
                updated_at: None,
            },
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

    #[test]
    fn render_applies_priority_filter() {
        use crate::model::issue::{IssuePriority, IssueState, IssueType};
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut view = SwarmView::new();
        let manager_input = TextInput::new();
        view.cycle_priority_filter(); // → P0 (no issues match)
        view.cycle_priority_filter(); // → P1
        let swarm = make_swarm();
        let issues = vec![
            GitHubIssue {
                number: 1,
                title: "p1 issue".into(),
                state: IssueState::Open,
                priority: IssuePriority::P1,
                issue_type: IssueType::Bug,
                labels: vec![],
                is_working: false,
                assigned_worker: None,
                updated_at: None,
            },
            GitHubIssue {
                number: 2,
                title: "p2 issue".into(),
                state: IssueState::Open,
                priority: IssuePriority::P2,
                issue_type: IssueType::Enhancement,
                labels: vec![],
                is_working: false,
                assigned_worker: None,
                updated_at: None,
            },
        ];
        terminal
            .draw(|f| {
                view.render(
                    f,
                    f.area(),
                    &swarm,
                    &issues,
                    SwarmPanel::Issues,
                    false,
                    false,
                    None,
                    &manager_input,
                );
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        // P1 filter active: 1 of 2 issues shown
        assert!(
            rendered.contains("1/2"),
            "Priority filter should reduce rendered count: {rendered}"
        );
        assert!(rendered.contains("p1 issue"), "P1 issue should be visible");
        assert!(
            !rendered.contains("p2 issue"),
            "P2 issue should be hidden by P1 filter"
        );
    }

    #[test]
    fn scroll_manager_to_bottom_sets_to_max() {
        let mut view = SwarmView::new();
        view.scroll_manager_to_bottom();
        assert_eq!(view.manager_scroll, u16::MAX);
    }

    #[test]
    fn next_issue_increments_and_wraps() {
        let mut view = SwarmView::new();
        view.next_issue(3);
        assert_eq!(view.selected_issue(), Some(1));
        view.next_issue(3);
        view.next_issue(3);
        assert_eq!(view.selected_issue(), Some(0));
    }

    #[test]
    fn count_attention_counts_waiting_agents() {
        use super::count_attention;
        let mut swarm = make_swarm();
        swarm.workers[0].pane_content = "should i proceed with this change?".to_string();
        let issues: Vec<GitHubIssue> = vec![];
        assert_eq!(count_attention(&swarm, &issues), 1);
    }

    #[test]
    fn count_attention_includes_blocked_issues() {
        use super::count_attention;
        let swarm = make_swarm();
        let issues = vec![
            make_issue(1, "blocked", &["needs-design"]),
            make_issue(2, "normal", &[]),
        ];
        assert_eq!(count_attention(&swarm, &issues), 1);
    }

    #[test]
    fn count_attention_combines_blocked_and_waiting() {
        use super::count_attention;
        let mut swarm = make_swarm();
        swarm.workers[0].pane_content = "should i proceed with this?".to_string();
        let issues = vec![make_issue(1, "blocked", &["needs-approval"])];
        assert_eq!(count_attention(&swarm, &issues), 2);
    }
}
