use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::model::issue::IssuePriority;
use crate::model::swarm::Swarm;
use super::theme;

/// Which panel has focus in the repo view.
#[derive(Debug, Clone, PartialEq)]
pub enum RepoViewFocus {
    Workers,
    Issues,
    ManagerInput,
}

/// A notification banner shown temporarily.
#[derive(Debug, Clone)]
pub struct Banner {
    pub message: String,
    pub style: ratatui::style::Style,
    /// Ticks remaining before auto-dismiss (at 250ms per tick).
    pub ttl: u32,
}

pub struct RepoView {
    pub focus: RepoViewFocus,
    pub worker_list_state: ListState,
    pub issue_list_state: ListState,
    pub priority_filter: Option<IssuePriority>,
    /// Worker index for quick peek popup (None = no popup).
    pub peek_worker: Option<usize>,
    /// Active notification banners (newest first).
    pub banners: Vec<Banner>,
}

impl RepoView {
    pub fn new() -> Self {
        let mut worker_list_state = ListState::default();
        worker_list_state.select(Some(0));
        let mut issue_list_state = ListState::default();
        issue_list_state.select(Some(0));
        Self {
            focus: RepoViewFocus::Workers,
            worker_list_state,
            issue_list_state,
            priority_filter: None,
            peek_worker: None,
            banners: Vec::new(),
        }
    }

    /// Add a notification banner that auto-dismisses after ~4 seconds (16 ticks).
    pub fn add_banner(&mut self, message: String, style: ratatui::style::Style) {
        self.banners.insert(0, Banner {
            message,
            style,
            ttl: 16, // ~4 seconds at 250ms tick
        });
        // Keep max 5 banners
        self.banners.truncate(5);
    }

    /// Tick banners down, removing expired ones.
    pub fn tick_banners(&mut self) {
        for banner in &mut self.banners {
            banner.ttl = banner.ttl.saturating_sub(1);
        }
        self.banners.retain(|b| b.ttl > 0);
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, swarm: &Swarm) {
        // Reserve space for banners at top
        let banner_height = self.banners.len().min(3) as u16;

        let chunks = Layout::vertical([
            Constraint::Length(banner_height), // Banners
            Constraint::Length(1), // Title bar
            Constraint::Min(8),   // Two-column area
            Constraint::Length(1), // Help bar
        ])
        .split(area);

        self.render_banners(f, chunks[0]);
        self.render_title_bar(f, chunks[1], swarm);
        self.render_columns(f, chunks[2], swarm);
        self.render_help(f, chunks[3]);

        // Render peek popup overlay if active
        if let Some(worker_idx) = self.peek_worker {
            if let Some(worker) = swarm.workers.get(worker_idx) {
                self.render_peek_popup(f, area, worker);
            }
        }
    }

    fn render_banners(&self, f: &mut Frame, area: Rect) {
        for (i, banner) in self.banners.iter().take(3).enumerate() {
            if i as u16 >= area.height {
                break;
            }
            let row = Rect {
                x: area.x,
                y: area.y + i as u16,
                width: area.width,
                height: 1,
            };
            let para = Paragraph::new(Line::from(Span::styled(
                format!(" {} ", banner.message),
                banner.style,
            )));
            f.render_widget(para, row);
        }
    }

    fn render_peek_popup(&self, f: &mut Frame, area: Rect, worker: &crate::model::swarm::AgentInfo) {
        // Center a popup showing the last 15 lines of the worker's pane
        let popup_width = (area.width * 80 / 100).min(100);
        let popup_height = 18u16; // 15 lines + borders + title

        let popup_area = Rect {
            x: area.x + (area.width.saturating_sub(popup_width)) / 2,
            y: area.y + (area.height.saturating_sub(popup_height)) / 2,
            width: popup_width.min(area.width),
            height: popup_height.min(area.height),
        };

        f.render_widget(Clear, popup_area);

        let lines: Vec<Line> = worker
            .pane_content
            .lines()
            .rev()
            .take(15)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|l| Line::from(l.to_string()))
            .collect();

        let title = format!(" {} — {} ", worker.id, worker.status.state);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(theme::title_style());

        let para = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });
        f.render_widget(para, popup_area);
    }

    fn render_title_bar(&self, f: &mut Frame, area: Rect, swarm: &Swarm) {
        let busy = swarm.busy_count();
        let idle = swarm.attention_count();
        let total = swarm.workers.len();
        let stopped = total - busy - idle;
        let waiting = swarm.waiting_count();

        let (p0, p1, p2, p3) = swarm.issue_cache.priority_counts();

        let project_label = format!("  {} ", swarm.project_name);
        let workflow_label = format!(
            "[{}] ",
            swarm
                .workflow
                .as_ref()
                .map(|w| w.to_string())
                .unwrap_or_else(|| "—".to_string())
        );
        let workers_label = format!(" {busy}/{total} busy ");
        let waiting_label = if waiting > 0 {
            format!("{waiting} NEED INPUT ")
        } else {
            String::new()
        };
        let idle_label = if idle > 0 {
            format!("{idle} idle ")
        } else {
            String::new()
        };
        let stopped_label = if stopped > 0 {
            format!("{stopped} stopped ")
        } else {
            String::new()
        };
        let issues_label = format!(" P0:{p0} P1:{p1} P2:{p2} P3:{p3}");

        let title = Paragraph::new(Line::from(vec![
            Span::styled(project_label, theme::title_style()),
            Span::styled(workflow_label, theme::help_style()),
            Span::styled(workers_label, theme::status_style(
                &crate::model::status::AgentState::Working { issue: None },
            )),
            Span::styled(waiting_label, theme::waiting_style()),
            Span::styled(idle_label, theme::status_style(
                &crate::model::status::AgentState::Idle,
            )),
            Span::styled(stopped_label, theme::status_style(
                &crate::model::status::AgentState::Stopped,
            )),
            Span::styled(issues_label, theme::help_style()),
        ]));
        f.render_widget(title, area);
    }

    fn render_columns(&mut self, f: &mut Frame, area: Rect, swarm: &Swarm) {
        let cols = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(60),
        ])
        .split(area);

        self.render_workers_column(f, cols[0], swarm);
        self.render_issues_column(f, cols[1], swarm);
    }

    fn render_workers_column(&mut self, f: &mut Frame, area: Rect, swarm: &Swarm) {
        let items: Vec<ListItem> = swarm
            .workers
            .iter()
            .map(|w| {
                let (dot, dot_style) = if w.waiting_for_input {
                    ("⚠ ", theme::waiting_style())
                } else {
                    match &w.status.state {
                        crate::model::status::AgentState::Working { .. } => {
                            ("● ", theme::status_style(&w.status.state))
                        }
                        crate::model::status::AgentState::Starting => {
                            ("◐ ", theme::status_style(&w.status.state))
                        }
                        crate::model::status::AgentState::Idle => {
                            ("○ ", theme::status_style(&w.status.state))
                        }
                        crate::model::status::AgentState::Stopped => {
                            ("◌ ", theme::status_style(&w.status.state))
                        }
                        _ => ("  ", theme::help_style()),
                    }
                };

                let status_text = if w.waiting_for_input {
                    "NEEDS INPUT".to_string()
                } else {
                    w.status.state.to_string()
                };
                let status_style = if w.waiting_for_input {
                    theme::waiting_style()
                } else {
                    theme::status_style(&w.status.state)
                };

                let elapsed = w
                    .status
                    .timestamp
                    .map(|ts| {
                        let now = chrono::Local::now().naive_local();
                        let dur = now - ts;
                        if dur.num_hours() > 0 {
                            format!("{}h ago", dur.num_hours())
                        } else if dur.num_minutes() > 0 {
                            format!("{}m ago", dur.num_minutes())
                        } else {
                            "just now".to_string()
                        }
                    })
                    .unwrap_or_default();

                let line1 = Line::from(vec![
                    Span::styled(dot, dot_style),
                    Span::styled(&w.id, theme::title_style()),
                ]);
                let line2 = Line::from(vec![
                    Span::raw("  "),
                    Span::styled(status_text, status_style),
                ]);
                let mut lines = vec![line1, line2];
                if !elapsed.is_empty() {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(elapsed, theme::help_style()),
                    ]));
                }
                ListItem::new(lines)
            })
            .collect();

        let workers_title = format!(" Workers ({}) ", swarm.workers.len());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(workers_title)
            .border_style(if self.focus == RepoViewFocus::Workers {
                theme::title_style()
            } else {
                ratatui::style::Style::default()
            });

        let list = List::new(items)
            .block(block)
            .highlight_style(theme::selected_style());

        f.render_stateful_widget(list, area, &mut self.worker_list_state);
    }

    fn render_issues_column(&mut self, f: &mut Frame, area: Rect, swarm: &Swarm) {
        let (p0, p1, p2, p3) = swarm.issue_cache.priority_counts();
        let total = swarm.issue_cache.issues.len();

        // Build filter header
        let filter_spans: Vec<Span> = vec![
            if self.priority_filter.is_none() {
                Span::styled(format!("All({total}) "), theme::active_filter_style())
            } else {
                Span::styled(format!("All({total}) "), theme::help_style())
            },
            if self.priority_filter == Some(IssuePriority::P0) {
                Span::styled(format!("P0({p0}) "), theme::active_filter_style())
            } else {
                Span::styled(format!("P0({p0}) "), theme::priority_style(&IssuePriority::P0))
            },
            if self.priority_filter == Some(IssuePriority::P1) {
                Span::styled(format!("P1({p1}) "), theme::active_filter_style())
            } else {
                Span::styled(format!("P1({p1}) "), theme::priority_style(&IssuePriority::P1))
            },
            if self.priority_filter == Some(IssuePriority::P2) {
                Span::styled(format!("P2({p2}) "), theme::active_filter_style())
            } else {
                Span::styled(format!("P2({p2}) "), theme::priority_style(&IssuePriority::P2))
            },
            if self.priority_filter == Some(IssuePriority::P3) {
                Span::styled(format!("P3({p3}) "), theme::active_filter_style())
            } else {
                Span::styled(format!("P3({p3}) "), theme::priority_style(&IssuePriority::P3))
            },
        ];

        let filtered = swarm.issue_cache.filtered(self.priority_filter.as_ref());

        let mut items: Vec<ListItem> = Vec::new();
        // Filter header as first item
        items.push(ListItem::new(Line::from(filter_spans)));

        for issue in &filtered {
            let working_label = if issue.is_working { " [working]" } else { "" };
            let type_label = format!("{}", issue.issue_type);

            let line = Line::from(vec![
                Span::styled(
                    format!("{} ", issue.priority),
                    theme::priority_style(&issue.priority),
                ),
                Span::styled(format!("#{} ", issue.number), theme::title_style()),
                Span::raw(truncate_str(&issue.title, 30)),
                Span::styled(
                    format!(" {type_label}"),
                    theme::help_style(),
                ),
                Span::styled(
                    working_label.to_string(),
                    theme::status_style(&crate::model::status::AgentState::Working { issue: None }),
                ),
            ]);
            items.push(ListItem::new(line));
        }

        if swarm.issue_cache.is_loading {
            items.push(ListItem::new(Line::from(Span::styled(
                "  Loading...",
                theme::help_style(),
            ))));
        } else if filtered.is_empty() {
            items.push(ListItem::new(Line::from(Span::styled(
                "  No issues",
                theme::help_style(),
            ))));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Issues ")
            .border_style(if self.focus == RepoViewFocus::Issues {
                theme::title_style()
            } else {
                ratatui::style::Style::default()
            });

        let list = List::new(items)
            .block(block)
            .highlight_style(if self.focus == RepoViewFocus::Issues {
                theme::selected_style()
            } else {
                ratatui::style::Style::default()
            });

        f.render_stateful_widget(list, area, &mut self.issue_list_state);
    }

    fn render_help(&self, f: &mut Frame, area: Rect) {
        let help = match self.focus {
            RepoViewFocus::Workers => Paragraph::new(Line::from(vec![
                Span::styled(" Enter", theme::title_style()),
                Span::styled(" drill in  ", theme::help_style()),
                Span::styled("Space", theme::title_style()),
                Span::styled(" peek  ", theme::help_style()),
                Span::styled("Tab", theme::title_style()),
                Span::styled(" issues  ", theme::help_style()),
                Span::styled("n", theme::waiting_style()),
                Span::styled(" next waiting  ", theme::help_style()),
                Span::styled("m", theme::title_style()),
                Span::styled(" manager  ", theme::help_style()),
                Span::styled("Esc", theme::title_style()),
                Span::styled(" back", theme::help_style()),
            ])),
            RepoViewFocus::Issues => Paragraph::new(Line::from(vec![
                Span::styled(" Enter", theme::title_style()),
                Span::styled(" assign  ", theme::help_style()),
                Span::styled("Tab", theme::title_style()),
                Span::styled(" workers  ", theme::help_style()),
                Span::styled("0-4", theme::title_style()),
                Span::styled(" filter  ", theme::help_style()),
                Span::styled("↑/↓", theme::title_style()),
                Span::styled(" select  ", theme::help_style()),
                Span::styled("r", theme::title_style()),
                Span::styled(" refresh  ", theme::help_style()),
                Span::styled("Esc", theme::title_style()),
                Span::styled(" back", theme::help_style()),
            ])),
            RepoViewFocus::ManagerInput => Paragraph::new(Line::from(vec![
                Span::styled(" Typing goes to manager  ", theme::help_style()),
                Span::styled("Ctrl+]", theme::title_style()),
                Span::styled(" workers", theme::help_style()),
            ])),
        };
        f.render_widget(help, area);
    }

    // Navigation helpers

    pub fn next_worker(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let i = self.worker_list_state.selected().unwrap_or(0);
        self.worker_list_state.select(Some((i + 1) % len));
    }

    pub fn previous_worker(&mut self, len: usize) -> bool {
        if len == 0 {
            return true;
        }
        let i = self.worker_list_state.selected().unwrap_or(0);
        if i == 0 {
            return true; // Signal to focus manager input
        }
        self.worker_list_state.select(Some(i - 1));
        false
    }

    pub fn selected_worker(&self) -> Option<usize> {
        self.worker_list_state.selected()
    }

    pub fn next_issue(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        // +1 because first item is the filter header
        let max = len; // filtered issues count (header is index 0)
        let i = self.issue_list_state.selected().unwrap_or(0);
        if i < max {
            self.issue_list_state.select(Some(i + 1));
        }
    }

    pub fn previous_issue(&mut self) -> bool {
        let i = self.issue_list_state.selected().unwrap_or(0);
        if i <= 1 {
            // At top of issue list (0 is header, 1 is first issue)
            return true; // Signal to focus manager input
        }
        self.issue_list_state.select(Some(i - 1));
        false
    }

    /// Returns the index into the filtered issues list (0-based), accounting for the header row.
    pub fn selected_issue_idx(&self) -> Option<usize> {
        self.issue_list_state
            .selected()
            .and_then(|i| if i >= 1 { Some(i - 1) } else { None })
    }
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len - 1])
    }
}
