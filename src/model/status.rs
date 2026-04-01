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
    #[allow(dead_code)] // Parsed from status files, available for future display
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

/// Mark a worker as stalled when it has remained in a "working" or "starting"
/// state without status updates for too long.
pub fn mark_stalled_if_needed(
    mut status: AgentStatus,
    now: NaiveDateTime,
    stale_after_minutes: i64,
) -> AgentStatus {
    if stale_after_minutes <= 0 {
        return status;
    }

    let Some(timestamp) = status.timestamp else {
        return status;
    };

    let age = now.signed_duration_since(timestamp);
    if age.num_seconds() < 0 {
        return status;
    }

    let age_minutes = age.num_minutes();
    if age_minutes < stale_after_minutes {
        return status;
    }

    let stalled_message = match &status.state {
        AgentState::Working { issue: Some(n) } => {
            format!("Stalled #{n} ({age_minutes}m no status updates)")
        }
        AgentState::Working { issue: None } => {
            format!("Stalled ({age_minutes}m no status updates)")
        }
        AgentState::Starting => format!("Starting stalled ({age_minutes}m no status updates)"),
        _ => return status,
    };

    status.state = AgentState::Unknown(stalled_message);
    status
}

#[cfg(test)]
mod tests {
    use super::{mark_stalled_if_needed, parse_status_line, AgentState};
    use chrono::{Duration, NaiveDateTime};

    fn parse_ts(ts: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S").expect("valid test timestamp")
    }

    #[test]
    fn parses_working_issue_number() {
        let status = parse_status_line("2026-04-01 11:00:00\tworking issue #42");
        assert!(matches!(
            status.state,
            AgentState::Working { issue: Some(42) }
        ));
    }

    #[test]
    fn marks_old_working_status_as_stalled() {
        let status = parse_status_line("2026-04-01 11:00:00\tworking issue #42");
        let now = parse_ts("2026-04-01 11:20:00");

        let stalled = mark_stalled_if_needed(status, now, 15);
        assert!(matches!(
            stalled.state,
            AgentState::Unknown(ref msg) if msg.contains("Stalled #42")
        ));
    }

    #[test]
    fn keeps_recent_working_status_active() {
        let status = parse_status_line("2026-04-01 11:10:00\tworking issue #42");
        let now = parse_ts("2026-04-01 11:20:00");

        let refreshed = mark_stalled_if_needed(status, now, 15);
        assert!(matches!(
            refreshed.state,
            AgentState::Working { issue: Some(42) }
        ));
    }

    #[test]
    fn does_not_mark_idle_status_as_stalled() {
        let status = parse_status_line("2026-04-01 11:00:00\tidle");
        let now = parse_ts("2026-04-01 12:00:00") + Duration::minutes(1);

        let refreshed = mark_stalled_if_needed(status, now, 15);
        assert!(matches!(refreshed.state, AgentState::Idle));
    }
}
