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

pub fn cursor_style() -> Style {
    Style::default().bg(Color::White).fg(Color::Black)
}

pub fn attention_style() -> Style {
    Style::default()
        .fg(Color::Red)
        .add_modifier(Modifier::BOLD)
}

pub fn attention_blink_style(blink: bool) -> Style {
    if blink {
        Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    }
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

/// Inverted style for sessions waiting for input — high-visibility row highlight.
pub fn waiting_inverted_style() -> Style {
    Style::default()
        .bg(Color::Magenta)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

pub fn active_filter_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
}

/// Returns the system hostname, or empty string if unavailable.
pub fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Builds a right-aligned hostname span with padding to fill `width`,
/// given the visual width of the already-rendered left content.
pub fn hostname_right_span(left_len: usize, width: usize) -> ratatui::text::Span<'static> {
    let hn = hostname();
    let right_text = if hn.is_empty() {
        String::new()
    } else {
        format!("{hn}  ")
    };
    let right_len = right_text.len();
    let padded = if width > left_len + right_len {
        format!("{}{}", " ".repeat(width - left_len - right_len), right_text)
    } else if !right_text.is_empty() {
        format!(" {right_text}")
    } else {
        String::new()
    };
    ratatui::text::Span::styled(padded, help_style())
}
