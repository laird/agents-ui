use std::collections::HashMap;
use std::path::PathBuf;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::model::issue::IssueCache;
use crate::model::swarm::Swarm;
use crate::ui::swarm_view::count_attention;
use super::theme;

pub struct ReposListView {
    pub table_state: TableState,
}

impl ReposListView {
    pub fn new() -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self { table_state }
    }

    pub fn render(
        &mut self,
        f: &mut Frame,
        area: Rect,
        swarms: &[Swarm],
        available: &[PathBuf],
        status_msg: Option<&str>,
        issue_caches: &HashMap<String, IssueCache>,
    ) {
        let total_items = swarms.len() + available.len();

        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(area);

        // Title
        let active_count = swarms.len();
        let avail_count = available.len();
        let title_info = if active_count > 0 && avail_count > 0 {
            format!("  ({active_count} active, {avail_count} available)")
        } else if active_count > 0 {
            format!("  ({active_count} active)")
        } else if avail_count > 0 {
            format!("  ({avail_count} repos found)")
        } else {
            String::new()
        };
        let version = env!("CARGO_PKG_VERSION");
        let left_text = format!("  Agents UI{title_info}");
        let right_text = format!("v{version} ");
        // Pad between left and right to fill the width
        let width = chunks[0].width as usize;
        let left_len = left_text.len();
        let right_len = right_text.len();
        let padding = if width > left_len + right_len {
            " ".repeat(width - left_len - right_len)
        } else {
            " ".to_string()
        };
        let title = Paragraph::new(Line::from(vec![
            Span::styled("  Agents UI", theme::title_style()),
            Span::styled(title_info, theme::help_style()),
            Span::raw(padding),
            Span::styled(right_text, theme::help_style()),
        ]))
        .block(Block::default().borders(Borders::BOTTOM));
        f.render_widget(title, chunks[0]);

        // Table
        if total_items == 0 {
            let empty = Paragraph::new(Line::from(vec![
                Span::styled("  No repos found. Press ", theme::help_style()),
                Span::styled("n", theme::title_style()),
                Span::styled(" to launch a new swarm.", theme::help_style()),
            ]));
            f.render_widget(empty, chunks[1]);
        } else {
            let header = Row::new(vec![
                Cell::from("#"),
                Cell::from("Repo"),
                Cell::from("Status"),
                Cell::from("Workflow"),
                Cell::from("Runtime"),
                Cell::from("Sessions"),
                Cell::from("Waiting"),
                Cell::from("Issues"),
            ])
            .style(theme::header_style());

            let mut rows: Vec<Row> = Vec::new();
            let mut row_num = 1;

            // Active swarms first
            for s in swarms {
                let busy = s.busy_count();
                let total = s.workers.len();
                let waiting = count_attention(s);
                let blocked_issues = s.attention_count();

                // Build issue priority summary from cache
                let issue_summary = if let Some(cache) = issue_caches.get(&s.project_name) {
                    let open_issues: Vec<_> = cache.issues.iter()
                        .filter(|i| i.state == crate::model::issue::IssueState::Open)
                        .collect();
                    if open_issues.is_empty() {
                        "—".to_string()
                    } else {
                        let mut counts = [0u32; 4]; // P0, P1, P2, P3
                        for issue in &open_issues {
                            if let Some(p) = issue.priority_num() {
                                if (p as usize) < 4 {
                                    counts[p as usize] += 1;
                                }
                            }
                        }
                        let parts: Vec<String> = counts.iter().enumerate()
                            .filter(|&(_, c)| *c > 0)
                            .map(|(i, c)| format!("P{i}:{c}"))
                            .collect();
                        if parts.is_empty() {
                            format!("{} open", open_issues.len())
                        } else {
                            parts.join(" ")
                        }
                    }
                } else {
                    "—".to_string()
                };

                rows.push(Row::new(vec![
                    Cell::from(format!("{row_num}")).style(theme::title_style()),
                    Cell::from(s.project_name.clone()),
                    Cell::from("Active").style(Style::default().fg(ratatui::style::Color::Green)),
                    Cell::from(
                        s.workflow
                            .as_ref()
                            .map(|w| w.to_string())
                            .unwrap_or_else(|| "—".to_string()),
                    ),
                    Cell::from(s.agent_type.to_string()),
                    Cell::from(format!("{busy}/{total} working")),
                    Cell::from({
                        let mut parts = Vec::new();
                        if waiting > 0 { parts.push(format!("{waiting} input")); }
                        if blocked_issues > 0 { parts.push(format!("{blocked_issues} blocked")); }
                        if parts.is_empty() { "—".to_string() } else { parts.join(", ") }
                    })
                    .style(if waiting > 0 || blocked_issues > 0 {
                        theme::attention_style()
                    } else {
                        theme::help_style()
                    }),
                    Cell::from(issue_summary),
                ]));
                row_num += 1;
            }

            // Available repos
            for repo in available {
                let name = repo
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| repo.to_string_lossy().to_string());
                rows.push(Row::new(vec![
                    Cell::from(format!("{row_num}")).style(theme::help_style()),
                    Cell::from(name),
                    Cell::from("—").style(theme::help_style()),
                    Cell::from("—").style(theme::help_style()),
                    Cell::from("—").style(theme::help_style()),
                    Cell::from("—").style(theme::help_style()),
                    Cell::from("—").style(theme::help_style()),
                    Cell::from("—").style(theme::help_style()),
                ]));
                row_num += 1;
            }

            let table = Table::new(
                rows,
                [
                    Constraint::Length(3),
                    Constraint::Percentage(18),
                    Constraint::Percentage(10),
                    Constraint::Percentage(12),
                    Constraint::Percentage(10),
                    Constraint::Percentage(14),
                    Constraint::Percentage(10),
                    Constraint::Percentage(16),
                ],
            )
            .header(header)
            .block(Block::default().borders(Borders::ALL).title(" Repos "))
            .row_highlight_style(theme::selected_style());

            f.render_stateful_widget(table, chunks[1], &mut self.table_state);
        }

        // Status line
        if let Some(msg) = status_msg {
            let status = Paragraph::new(Line::from(Span::styled(
                format!(" {msg}"),
                theme::help_style(),
            )));
            f.render_widget(status, chunks[2]);
        }

        // Help bar
        let help = Paragraph::new(Line::from(vec![
            Span::styled(" 1-9", theme::title_style()),
            Span::styled("/", theme::help_style()),
            Span::styled("Enter", theme::title_style()),
            Span::styled(" select  ", theme::help_style()),
            Span::styled("n", theme::title_style()),
            Span::styled(" new swarm  ", theme::help_style()),
            Span::styled("d", theme::title_style()),
            Span::styled(" teardown  ", theme::help_style()),
            Span::styled("r", theme::title_style()),
            Span::styled(" refresh  ", theme::help_style()),
            Span::styled("q", theme::title_style()),
            Span::styled(" quit", theme::help_style()),
        ]))
        .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, chunks[3]);
    }

    pub fn next(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let i = self.table_state.selected().unwrap_or(0);
        self.table_state.select(Some((i + 1) % len));
    }

    pub fn previous(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let i = self.table_state.selected().unwrap_or(0);
        self.table_state
            .select(Some(if i == 0 { len - 1 } else { i - 1 }));
    }

    pub fn selected(&self) -> Option<usize> {
        self.table_state.selected()
    }
}

#[cfg(test)]
mod tests {
    use super::ReposListView;
    use crate::model::status::{AgentState, AgentStatus};
    use crate::model::swarm::{AgentInfo, AgentType, Swarm};
    use ratatui::{backend::TestBackend, Terminal};
    use std::path::PathBuf;

    fn make_agent(id: &str, is_manager: bool) -> AgentInfo {
        AgentInfo {
            id: format!("test/{id}"),
            role: id.to_string(),
            worktree_path: PathBuf::new(),
            tmux_target: String::new(),
            status: AgentStatus {
                timestamp: None,
                state: AgentState::Idle,
            },
            is_manager,
            pane_content: String::new(),
            dispatched_issue: None,
            current_issue: None,
            current_issue_title: None,
            waiting_for_input: false,
        }
    }

    fn make_swarm() -> Swarm {
        Swarm {
            repo_path: PathBuf::from("/tmp/demo"),
            project_name: "demo".to_string(),
            agent_type: AgentType::Codex,
            workflow: None,
            tmux_session: "codex-demo".to_string(),
            manager: make_agent("manager", true),
            workers: vec![make_agent("worker-1", false)],
            issue_cache: crate::model::issue::IssueCache::default(),
        }
    }

    #[test]
    fn render_smoke_shows_active_and_available_repos() {
        use std::collections::HashMap;
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut view = ReposListView::new();
        let swarms = vec![make_swarm()];
        let available = vec![PathBuf::from("/tmp/other-repo")];
        let caches = HashMap::new();

        terminal
            .draw(|f| view.render(f, f.area(), &swarms, &available, Some("Ready"), &caches))
            .unwrap();

        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("Agents UI"));
        assert!(rendered.contains("demo"));
        assert!(rendered.contains("other-repo"));
        assert!(rendered.contains("Active"));
        assert!(rendered.contains("Ready"));
    }
}
