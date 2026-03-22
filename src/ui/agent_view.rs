use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::model::swarm::AgentInfo;
use super::theme;

pub struct AgentView {
    pub scroll_offset: u16,
}

impl AgentView {
    pub fn new() -> Self {
        Self {
            scroll_offset: 0,
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, agent: &AgentInfo) {
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

        // Title
        let role = if agent.is_manager {
            "Manager"
        } else {
            "Worker"
        };
        let id_label = format!("  {} ", agent.id);
        let role_label = format!("[{role}] ");
        let status_label = agent.status.state.to_string();
        let path_label = format!("  {}", agent.worktree_path.display());

        let title = Paragraph::new(Line::from(vec![
            Span::styled(id_label, theme::title_style()),
            Span::styled(role_label, theme::help_style()),
            Span::styled(status_label, theme::status_style(&agent.status.state)),
            Span::styled(path_label, theme::help_style()),
        ]))
        .block(Block::default().borders(Borders::BOTTOM));
        f.render_widget(title, chunks[0]);

        // Pane output
        let content = &agent.pane_content;
        let lines: Vec<Line> = content.lines().map(|l| Line::from(l.to_string())).collect();
        let total_lines = lines.len() as u16;

        let visible_height = chunks[1].height.saturating_sub(2);
        let max_scroll = total_lines.saturating_sub(visible_height);
        if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }

        let session_title = format!(" Session — {} ", agent.tmux_target);
        let pane_output = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(session_title))
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset, 0));
        f.render_widget(pane_output, chunks[1]);

        // Help
        let help = Paragraph::new(Line::from(vec![
            Span::styled(" Typing goes to session  ", theme::help_style()),
            Span::styled("PgUp/PgDn", theme::title_style()),
            Span::styled(" scroll  ", theme::help_style()),
            Span::styled("Alt+0", theme::title_style()),
            Span::styled("/", theme::help_style()),
            Span::styled("Ctrl+]", theme::title_style()),
            Span::styled(" back  ", theme::help_style()),
            Span::styled("Alt+a", theme::waiting_style()),
            Span::styled(" next waiting  ", theme::help_style()),
        ]))
        .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, chunks[2]);
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = u16::MAX;
    }
}
