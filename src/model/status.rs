use chrono::NaiveDateTime;
use std::path::Path;

/// The state of an individual agent.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentState {
    Starting,
    Working { issue: Option<u32> },
    Idle,
    Completed { detail: String },
    Stopped,
    Unknown(String),
}

impl std::fmt::Display for AgentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentState::Starting => write!(f, "Starting"),
            AgentState::Working { issue: Some(n) } => write!(f, "Working #{n}"),
            AgentState::Working { issue: None } => write!(f, "Working"),
            AgentState::Idle => write!(f, "Idle"),
            AgentState::Completed { detail } => write!(f, "Done: {detail}"),
            AgentState::Stopped => write!(f, "Stopped"),
            AgentState::Unknown(s) => write!(f, "{s}"),
        }
    }
}

/// Parsed status of an agent from its status file.
#[derive(Debug, Clone)]
pub struct AgentStatus {
    #[allow(dead_code)] // Parsed for future use in status age display
    pub timestamp: Option<NaiveDateTime>,
    pub state: AgentState,
}

impl Default for AgentStatus {
    fn default() -> Self {
        Self {
            timestamp: None,
            state: AgentState::Unknown("No status".into()),
        }
    }
}

/// Parse a status file line.
/// Format: "2024-01-15 10:30:00\tworking issue #42"
pub fn parse_status_line(line: &str) -> AgentStatus {
    let parts: Vec<&str> = line.splitn(2, '\t').collect();
    let (timestamp, status_text) = match parts.len() {
        2 => {
            let ts = NaiveDateTime::parse_from_str(parts[0].trim(), "%Y-%m-%d %H:%M:%S").ok();
            (ts, parts[1].trim())
        }
        1 => (None, parts[0].trim()),
        _ => return AgentStatus::default(),
    };

    let state = parse_state(status_text);
    AgentStatus { timestamp, state }
}

fn parse_state(text: &str) -> AgentState {
    let lower = text.to_lowercase();
    if lower.starts_with("starting") {
        AgentState::Starting
    } else if lower.starts_with("working") || lower.starts_with("fixing") {
        let issue = extract_issue_number(text);
        AgentState::Working { issue }
    } else if lower == "idle" || lower.contains("idle_no_work_available") {
        AgentState::Idle
    } else if lower.starts_with("completed") || lower.starts_with("done") {
        AgentState::Completed {
            detail: text.to_string(),
        }
    } else if lower.starts_with("stopped") {
        AgentState::Stopped
    } else {
        AgentState::Unknown(text.to_string())
    }
}

fn extract_issue_number(text: &str) -> Option<u32> {
    // Look for #N or "issue N" patterns
    for word in text.split_whitespace() {
        if let Some(stripped) = word.strip_prefix('#') {
            if let Ok(n) = stripped.parse::<u32>() {
                return Some(n);
            }
        }
    }
    // Try "issue N" pattern
    if let Some(pos) = text.to_lowercase().find("issue") {
        let after = &text[pos + 5..];
        for word in after.split_whitespace() {
            let cleaned = word.trim_start_matches('#');
            if let Ok(n) = cleaned.parse::<u32>() {
                return Some(n);
            }
        }
    }
    None
}

/// Read and parse the last line of a status file.
pub fn read_status_file(path: &Path) -> AgentStatus {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            if let Some(last_line) = content.lines().last() {
                parse_status_line(last_line)
            } else {
                AgentStatus::default()
            }
        }
        Err(_) => AgentStatus::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_state tests ---

    #[test]
    fn parse_state_starting() {
        assert_eq!(parse_state("Starting"), AgentState::Starting);
        assert_eq!(parse_state("starting up"), AgentState::Starting);
    }

    #[test]
    fn parse_state_working_no_issue() {
        assert_eq!(
            parse_state("Working"),
            AgentState::Working { issue: None }
        );
    }

    #[test]
    fn parse_state_working_with_issue() {
        assert_eq!(
            parse_state("Working on #42"),
            AgentState::Working { issue: Some(42) }
        );
    }

    #[test]
    fn parse_state_fixing() {
        assert_eq!(
            parse_state("Fixing issue #7"),
            AgentState::Working { issue: Some(7) }
        );
    }

    #[test]
    fn parse_state_idle() {
        assert_eq!(parse_state("idle"), AgentState::Idle);
        assert_eq!(parse_state("IDLE_NO_WORK_AVAILABLE"), AgentState::Idle);
    }

    #[test]
    fn parse_state_completed() {
        assert_eq!(
            parse_state("Completed all tasks"),
            AgentState::Completed {
                detail: "Completed all tasks".to_string()
            }
        );
        assert_eq!(
            parse_state("Done: merged PR"),
            AgentState::Completed {
                detail: "Done: merged PR".to_string()
            }
        );
    }

    #[test]
    fn parse_state_stopped() {
        assert_eq!(parse_state("Stopped"), AgentState::Stopped);
    }

    #[test]
    fn parse_state_unknown() {
        assert_eq!(
            parse_state("something else"),
            AgentState::Unknown("something else".to_string())
        );
    }

    // --- extract_issue_number tests ---

    #[test]
    fn extract_issue_hash_format() {
        assert_eq!(extract_issue_number("Working on #42"), Some(42));
        assert_eq!(extract_issue_number("#1"), Some(1));
    }

    #[test]
    fn extract_issue_word_format() {
        assert_eq!(extract_issue_number("fixing issue 99"), Some(99));
        assert_eq!(extract_issue_number("issue #5 in progress"), Some(5));
    }

    #[test]
    fn extract_issue_none() {
        assert_eq!(extract_issue_number("no issue here"), None);
        assert_eq!(extract_issue_number("working"), None);
        assert_eq!(extract_issue_number(""), None);
    }

    // --- parse_status_line tests ---

    #[test]
    fn parse_status_line_with_timestamp() {
        let status = parse_status_line("2024-01-15 10:30:00\tworking issue #42");
        assert!(status.timestamp.is_some());
        assert_eq!(
            status.state,
            AgentState::Working { issue: Some(42) }
        );
    }

    #[test]
    fn parse_status_line_without_timestamp() {
        let status = parse_status_line("idle");
        assert!(status.timestamp.is_none());
        assert_eq!(status.state, AgentState::Idle);
    }

    #[test]
    fn parse_status_line_bad_timestamp() {
        let status = parse_status_line("not-a-date\tStarting");
        assert!(status.timestamp.is_none());
        assert_eq!(status.state, AgentState::Starting);
    }

    #[test]
    fn parse_status_line_empty() {
        let status = parse_status_line("");
        // Empty string becomes Unknown("")
        matches!(status.state, AgentState::Unknown(_));
    }

    // --- AgentStatus default ---

    #[test]
    fn agent_status_default() {
        let d = AgentStatus::default();
        assert!(d.timestamp.is_none());
        assert!(matches!(d.state, AgentState::Unknown(_)));
    }

    // --- Display ---

    #[test]
    fn agent_state_display() {
        assert_eq!(AgentState::Starting.to_string(), "Starting");
        assert_eq!(
            AgentState::Working { issue: Some(5) }.to_string(),
            "Working #5"
        );
        assert_eq!(
            AgentState::Working { issue: None }.to_string(),
            "Working"
        );
        assert_eq!(AgentState::Idle.to_string(), "Idle");
        assert_eq!(
            AgentState::Completed {
                detail: "done".into()
            }
            .to_string(),
            "Done: done"
        );
        assert_eq!(AgentState::Stopped.to_string(), "Stopped");
    }
}
