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
            parse_state("working"),
            AgentState::Working { issue: None }
        );
    }

    #[test]
    fn parse_state_working_with_issue() {
        assert_eq!(
            parse_state("working on issue #42"),
            AgentState::Working { issue: Some(42) }
        );
    }

    #[test]
    fn parse_state_fixing() {
        assert_eq!(
            parse_state("fixing #7"),
            AgentState::Working { issue: Some(7) }
        );
    }

    #[test]
    fn parse_state_idle() {
        assert_eq!(parse_state("idle"), AgentState::Idle);
        assert_eq!(parse_state("Idle"), AgentState::Idle);
    }

    #[test]
    fn parse_state_idle_no_work() {
        assert_eq!(
            parse_state("IDLE_NO_WORK_AVAILABLE"),
            AgentState::Idle
        );
    }

    #[test]
    fn parse_state_completed() {
        assert_eq!(
            parse_state("completed all tasks"),
            AgentState::Completed {
                detail: "completed all tasks".to_string()
            }
        );
    }

    #[test]
    fn parse_state_done() {
        assert_eq!(
            parse_state("done: merged PR"),
            AgentState::Completed {
                detail: "done: merged PR".to_string()
            }
        );
    }

    #[test]
    fn parse_state_stopped() {
        assert_eq!(parse_state("stopped"), AgentState::Stopped);
        assert_eq!(parse_state("Stopped by user"), AgentState::Stopped);
    }

    #[test]
    fn parse_state_unknown() {
        assert_eq!(
            parse_state("something unexpected"),
            AgentState::Unknown("something unexpected".to_string())
        );
    }

    // --- extract_issue_number tests ---

    #[test]
    fn extract_issue_hash_format() {
        assert_eq!(extract_issue_number("working on #42"), Some(42));
    }

    #[test]
    fn extract_issue_word_format() {
        assert_eq!(extract_issue_number("fixing issue 99"), Some(99));
    }

    #[test]
    fn extract_issue_combined_format() {
        assert_eq!(extract_issue_number("working issue #123"), Some(123));
    }

    #[test]
    fn extract_issue_no_number() {
        assert_eq!(extract_issue_number("just working"), None);
    }

    #[test]
    fn extract_issue_empty() {
        assert_eq!(extract_issue_number(""), None);
    }

    // --- parse_status_line tests ---

    #[test]
    fn parse_status_line_with_timestamp() {
        let line = "2024-01-15 10:30:00\tworking issue #42";
        let status = parse_status_line(line);
        assert!(status.timestamp.is_some());
        assert_eq!(status.state, AgentState::Working { issue: Some(42) });
    }

    #[test]
    fn parse_status_line_without_timestamp() {
        let line = "idle";
        let status = parse_status_line(line);
        assert!(status.timestamp.is_none());
        assert_eq!(status.state, AgentState::Idle);
    }

    #[test]
    fn parse_status_line_bad_timestamp() {
        let line = "not-a-date\tworking";
        let status = parse_status_line(line);
        assert!(status.timestamp.is_none());
        assert_eq!(status.state, AgentState::Working { issue: None });
    }

    #[test]
    fn parse_status_line_empty() {
        let line = "";
        let status = parse_status_line(line);
        assert!(status.timestamp.is_none());
        // Empty string -> Unknown("")
    }

    // --- AgentState Display tests ---

    #[test]
    fn display_starting() {
        assert_eq!(AgentState::Starting.to_string(), "Starting");
    }

    #[test]
    fn display_working_with_issue() {
        assert_eq!(
            AgentState::Working { issue: Some(42) }.to_string(),
            "Working #42"
        );
    }

    #[test]
    fn display_working_no_issue() {
        assert_eq!(
            AgentState::Working { issue: None }.to_string(),
            "Working"
        );
    }

    #[test]
    fn display_idle() {
        assert_eq!(AgentState::Idle.to_string(), "Idle");
    }

    #[test]
    fn display_completed() {
        assert_eq!(
            AgentState::Completed {
                detail: "merged".to_string()
            }
            .to_string(),
            "Done: merged"
        );
    }

    #[test]
    fn display_stopped() {
        assert_eq!(AgentState::Stopped.to_string(), "Stopped");
    }

    // --- read_status_file tests ---

    #[test]
    fn read_status_file_nonexistent() {
        let status = read_status_file(Path::new("/nonexistent/path/status.txt"));
        assert!(status.timestamp.is_none());
        matches!(status.state, AgentState::Unknown(_));
    }
}
