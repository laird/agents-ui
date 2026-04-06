use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
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
    pub repo_name: String,
    pub repo_path: std::path::PathBuf,
}

impl FeedbackState {
    pub fn new(repo_name: String, repo_path: std::path::PathBuf) -> Self {
        Self {
            field: FeedbackField::FeedbackType,
            feedback_type: FeedbackType::Bug,
            type_index: 0,
            title: String::new(),
            body: String::new(),
            repo_name,
            repo_path,
        }
    }
}

pub fn render_feedback_dialog(f: &mut Frame, area: Rect, state: &FeedbackState) {
    let dialog_area = centered_rect(70, 16, area);
    f.render_widget(Clear, dialog_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" File Feedback: {} ", state.repo_name))
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
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn feedback_type_label_returns_human_readable_string() {
        assert_eq!(FeedbackType::Bug.label(), "bug");
        assert_eq!(FeedbackType::Enhancement.label(), "enhancement");
        assert_eq!(FeedbackType::FeatureRequest.label(), "feature request");
    }

    #[test]
    fn feedback_type_github_label_maps_feature_request_to_enhancement() {
        assert_eq!(FeedbackType::Bug.github_label(), "bug");
        assert_eq!(FeedbackType::Enhancement.github_label(), "enhancement");
        assert_eq!(FeedbackType::FeatureRequest.github_label(), "enhancement");
    }

    #[test]
    fn feedback_type_all_returns_three_variants() {
        let all = FeedbackType::all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0], FeedbackType::Bug);
        assert_eq!(all[1], FeedbackType::Enhancement);
        assert_eq!(all[2], FeedbackType::FeatureRequest);
    }

    #[test]
    fn feedback_state_new_sets_defaults() {
        let state =
            FeedbackState::new("myrepo".to_string(), std::path::PathBuf::from("/repo/path"));
        assert_eq!(state.repo_name, "myrepo");
        assert_eq!(state.repo_path, std::path::PathBuf::from("/repo/path"));
        assert!(state.title.is_empty());
        assert!(state.body.is_empty());
        assert_eq!(state.type_index, 0);
        assert_eq!(state.feedback_type, FeedbackType::Bug);
    }

    #[test]
    fn label_covers_all_variants() {
        for ft in FeedbackType::all() {
            assert!(!ft.label().is_empty(), "{ft:?}.label() should be non-empty");
        }
    }

    #[test]
    fn github_label_covers_all_variants() {
        for ft in FeedbackType::all() {
            assert!(
                !ft.github_label().is_empty(),
                "{ft:?}.github_label() should be non-empty"
            );
        }
    }

    #[test]
    fn github_label_is_lowercase_slug() {
        for ft in FeedbackType::all() {
            let label = ft.github_label();
            assert_eq!(
                label,
                label.to_lowercase(),
                "{ft:?}.github_label() should be lowercase"
            );
            assert!(
                !label.contains(' '),
                "{ft:?}.github_label() should not contain spaces"
            );
        }
    }

    #[test]
    fn all_includes_every_variant() {
        assert_eq!(
            FeedbackType::all().len(),
            3,
            "FeedbackType::all() should include every variant (Bug, Enhancement, FeatureRequest)"
        );
    }

    #[test]
    fn all_contains_bug_enhancement_feature_request() {
        let all = FeedbackType::all();
        assert!(all.contains(&FeedbackType::Bug));
        assert!(all.contains(&FeedbackType::Enhancement));
        assert!(all.contains(&FeedbackType::FeatureRequest));
    }

    #[test]
    fn new_sets_first_selection() {
        let state = FeedbackState::new("myrepo".to_string(), PathBuf::from("/tmp/myrepo"));
        assert_eq!(state.type_index, 0, "type_index should default to 0");
        assert_eq!(state.feedback_type, FeedbackType::Bug, "default type should be Bug");
    }

    #[test]
    fn new_input_is_empty() {
        let state = FeedbackState::new("myrepo".to_string(), PathBuf::from("/tmp/myrepo"));
        assert!(state.title.is_empty(), "title should start empty");
        assert!(state.body.is_empty(), "body should start empty");
    }

    #[test]
    fn new_preserves_repo_fields() {
        let name = "agents-ui".to_string();
        let path = PathBuf::from("/home/user/agents-ui");
        let state = FeedbackState::new(name.clone(), path.clone());
        assert_eq!(state.repo_name, name);
        assert_eq!(state.repo_path, path);
    }
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
