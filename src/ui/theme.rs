use ratatui::style::{Color, Modifier, Style};

use crate::model::issue::IssuePriority;
use crate::model::status::AgentState;

pub fn title_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

pub fn selected_style() -> Style {
    Style::default()
        .bg(Color::DarkGray)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

pub fn header_style() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

pub fn status_style(state: &AgentState) -> Style {
    match state {
        AgentState::Working { .. } => Style::default().fg(Color::Green),
        AgentState::Starting => Style::default().fg(Color::Yellow),
        AgentState::Idle => Style::default().fg(Color::Gray),
        AgentState::Completed { .. } => Style::default().fg(Color::Blue),
        AgentState::Stopped => Style::default().fg(Color::Red),
        AgentState::Unknown(_) => Style::default().fg(Color::DarkGray),
    }
}

pub fn help_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub fn input_style() -> Style {
    Style::default().fg(Color::White)
}

pub fn attention_style() -> Style {
    Style::default()
        .fg(Color::Red)
        .add_modifier(Modifier::BOLD)
}

pub fn priority_style(priority: &IssuePriority) -> Style {
    match priority {
        IssuePriority::P0 => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        IssuePriority::P1 => Style::default().fg(Color::Yellow),
        IssuePriority::P2 => Style::default().fg(Color::Blue),
        IssuePriority::P3 => Style::default().fg(Color::DarkGray),
        IssuePriority::None => Style::default().fg(Color::DarkGray),
    }
}

pub fn waiting_style() -> Style {
    Style::default()
        .fg(Color::Magenta)
        .add_modifier(Modifier::BOLD)
}

pub fn active_filter_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
}
