use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use super::theme;

/// Which field is active in the feedback dialog.
#[derive(Debug, Clone)]
pub enum FeedbackField {
    /// Choosing feedback type (bug, enhancement, feature request)
    FeedbackType,
    /// Typing the issue title
    Title,
    /// Typing the issue body
    Body,
    /// Submitting
    Submitting,
    /// Done (with result message)
    Done(String),
}

/// Feedback type options.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FeedbackType {
    Bug,
    Enhancement,
    FeatureRequest,
}

impl FeedbackType {
    pub fn label(&self) -> &str {
        match self {
            FeedbackType::Bug => "bug",
            FeedbackType::Enhancement => "enhancement",
            FeedbackType::FeatureRequest => "feature request",
        }
    }

    pub fn github_label(&self) -> &str {
        match self {
            FeedbackType::Bug => "bug",
            FeedbackType::Enhancement => "enhancement",
            FeedbackType::FeatureRequest => "enhancement",
        }
    }

    pub fn all() -> &'static [FeedbackType] {
        &[
            FeedbackType::Bug,
            FeedbackType::Enhancement,
            FeedbackType::FeatureRequest,
        ]
    }
}

/// State for the feedback dialog.
#[derive(Debug, Clone)]
pub struct FeedbackState {
    pub field: FeedbackField,
    pub feedback_type: FeedbackType,
    pub type_index: usize,
    pub title: String,
    pub body: String,
}

impl FeedbackState {
    pub fn new() -> Self {
        Self {
            field: FeedbackField::FeedbackType,
            feedback_type: FeedbackType::Bug,
            type_index: 0,
            title: String::new(),
            body: String::new(),
        }
    }
}

pub fn render_feedback_dialog(
    f: &mut Frame,
    area: Rect,
    state: &FeedbackState,
) {
    let dialog_area = centered_rect(70, 16, area);
    f.render_widget(Clear, dialog_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" File Feedback on laird/agents-ui ")
        .border_style(theme::title_style());

    let inner = block.inner(dialog_area);
    f.render_widget(block, dialog_area);

    let chunks = Layout::vertical([
        Constraint::Length(2), // Type selection
        Constraint::Length(2), // Title field
        Constraint::Length(4), // Body field
        Constraint::Length(1), // Status / help
        Constraint::Length(2), // Help bar
    ])
    .split(inner);

    // Type selection
    let types = FeedbackType::all();
    let type_spans: Vec<Span> = types
        .iter()
        .enumerate()
        .flat_map(|(i, t)| {
            let selected = i == state.type_index;
            let style = if selected {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                theme::help_style()
            };
            let prefix = if selected { " [" } else { "  " };
            let suffix = if selected { "] " } else { "  " };
            vec![
                Span::raw(prefix),
                Span::styled(t.label(), style),
                Span::raw(suffix),
            ]
        })
        .collect();

    let type_label = if matches!(state.field, FeedbackField::FeedbackType) {
        " Type: "
    } else {
        " Type: "
    };
    let mut line_spans = vec![Span::styled(type_label, theme::help_style())];
    line_spans.extend(type_spans);
    f.render_widget(Paragraph::new(Line::from(line_spans)), chunks[0]);

    // Title
    let title_style = if matches!(state.field, FeedbackField::Title) {
        theme::input_style()
    } else {
        theme::help_style()
    };
    let cursor = if matches!(state.field, FeedbackField::Title) {
        "█"
    } else {
        ""
    };
    let title_display = format!(" Title: {}{}", state.title, cursor);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(title_display, title_style))),
        chunks[1],
    );

    // Body
    let body_style = if matches!(state.field, FeedbackField::Body) {
        theme::input_style()
    } else {
        theme::help_style()
    };
    let cursor = if matches!(state.field, FeedbackField::Body) {
        "█"
    } else {
        ""
    };
    let body_display = format!(" Body: {}{}", state.body, cursor);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(body_display, body_style))),
        chunks[2],
    );

    // Status or help
    match &state.field {
        FeedbackField::Submitting => {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    " Submitting...",
                    theme::title_style(),
                ))),
                chunks[3],
            );
        }
        FeedbackField::Done(msg) => {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!(" {msg}"),
                    Style::default().fg(Color::Green),
                ))),
                chunks[3],
            );
        }
        _ => {}
    }

    // Help bar
    let help = match &state.field {
        FeedbackField::FeedbackType => Paragraph::new(Line::from(vec![
            Span::styled(" ←/→", theme::title_style()),
            Span::styled(" select type  ", theme::help_style()),
            Span::styled("Enter", theme::title_style()),
            Span::styled(" next  ", theme::help_style()),
            Span::styled("Esc", theme::title_style()),
            Span::styled(" cancel", theme::help_style()),
        ])),
        FeedbackField::Title => Paragraph::new(Line::from(vec![
            Span::styled(" Enter", theme::title_style()),
            Span::styled(" next  ", theme::help_style()),
            Span::styled("Esc", theme::title_style()),
            Span::styled(" back", theme::help_style()),
        ])),
        FeedbackField::Body => Paragraph::new(Line::from(vec![
            Span::styled(" Ctrl+Enter", theme::title_style()),
            Span::styled(" submit  ", theme::help_style()),
            Span::styled("Esc", theme::title_style()),
            Span::styled(" back", theme::help_style()),
        ])),
        FeedbackField::Done(_) => Paragraph::new(Line::from(vec![
            Span::styled(" Enter/Esc", theme::title_style()),
            Span::styled(" close", theme::help_style()),
        ])),
        _ => Paragraph::new(Line::from(Span::raw(""))),
    };
    f.render_widget(help, chunks[4]);
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
