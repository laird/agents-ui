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
    let padded = if right_text.is_empty() {
        String::new()
    } else if width > left_len + right_len {
        format!("{}{}", " ".repeat(width - left_len - right_len), right_text)
    } else {
        format!(" {right_text}")
    };
    ratatui::text::Span::styled(padded, help_style())
}

/// Build a right-aligned span using an explicit hostname (for testing without /proc).
fn hostname_right_span_with(hn: &str, left_len: usize, width: usize) -> ratatui::text::Span<'static> {
    let right_text = if hn.is_empty() {
        String::new()
    } else {
        format!("{hn}  ")
    };
    let right_len = right_text.len();
    let padded = if right_text.is_empty() {
        String::new()
    } else if width > left_len + right_len {
        format!("{}{}", " ".repeat(width - left_len - right_len), right_text)
    } else {
        format!(" {right_text}")
    };
    ratatui::text::Span::styled(padded, help_style())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn status_style_covers_all_variants() {
        let cases = [
            (AgentState::Working { issue: None }, Color::Green),
            (AgentState::Working { issue: Some(1) }, Color::Green),
            (AgentState::Starting, Color::Yellow),
            (AgentState::Idle, Color::Gray),
            (AgentState::Completed { detail: "done".into() }, Color::Blue),
            (AgentState::Stopped, Color::Red),
            (AgentState::Unknown("?".into()), Color::DarkGray),
        ];
        for (state, expected_fg) in cases {
            let style = status_style(&state);
            assert_eq!(
                style.fg,
                Some(expected_fg),
                "status_style({state:?}) should be {expected_fg:?}"
            );
        }
    }

    #[test]
    fn priority_style_covers_all_variants() {
        let cases = [
            (IssuePriority::P0, Color::Red),
            (IssuePriority::P1, Color::Yellow),
            (IssuePriority::P2, Color::Blue),
            (IssuePriority::P3, Color::DarkGray),
            (IssuePriority::None, Color::DarkGray),
        ];
        for (priority, expected_fg) in cases {
            let style = priority_style(&priority);
            assert_eq!(
                style.fg,
                Some(expected_fg),
                "priority_style({priority:?}) should be {expected_fg:?}"
            );
        }
    }

    #[test]
    fn attention_blink_style_toggles() {
        let on = attention_blink_style(true);
        let off = attention_blink_style(false);
        assert_ne!(on, off, "blink=true and blink=false should produce different styles");
        assert_eq!(on.fg, Some(Color::Red));
        assert_eq!(off.fg, Some(Color::Yellow));
    }

    #[test]
    fn hostname_right_span_fits_when_room_available() {
        // hostname="host", right_text="host  " (6 chars), left_len=10, width=40
        // padding = 40 - 10 - 6 = 24 spaces
        let span = hostname_right_span_with("host", 10, 40);
        let content = span.content.as_ref();
        assert!(content.ends_with("host  "), "should end with hostname + 2 spaces");
        assert_eq!(content.len(), 30, "padding(24) + 'host  '(6) = 30");
    }

    #[test]
    fn hostname_right_span_empty_when_no_hostname() {
        let span = hostname_right_span_with("", 10, 40);
        assert_eq!(span.content.as_ref(), "", "empty hostname → empty span");
    }

    #[test]
    fn hostname_right_span_handles_overflow() {
        // left_len(30) + right_len(8) = 38 > width(20) → fallback to " hostname  "
        let span = hostname_right_span_with("hello", 30, 20);
        let content = span.content.as_ref();
        assert!(
            content.starts_with(' '),
            "overflow fallback should start with a space, got: {content:?}"
        );
    }
}
