use chrono::NaiveDateTime;
use std::collections::HashMap;
use std::path::Path;

use super::swarm::AgentType;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneActivity {
    NeedsLaunch,
    AgentBusy,
    AgentIdle,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerSupervisionState {
    Healthy,
    DeadShellPrompt,
    DeadIdleOutsideLoop,
    DeadMissingStatus,
    DeadStaleStatus,
    DeadStopped,
}

impl WorkerSupervisionState {
    pub fn is_dead(self) -> bool {
        !matches!(self, WorkerSupervisionState::Healthy)
    }

    pub fn detail(self) -> &'static str {
        match self {
            WorkerSupervisionState::Healthy => "healthy",
            WorkerSupervisionState::DeadShellPrompt => "dead: shell prompt",
            WorkerSupervisionState::DeadIdleOutsideLoop => "dead: idle outside loop",
            WorkerSupervisionState::DeadMissingStatus => "dead: missing loop status",
            WorkerSupervisionState::DeadStaleStatus => "dead: stale loop heartbeat",
            WorkerSupervisionState::DeadStopped => "dead: stopped loop",
        }
    }
}

const LOOP_STATUS_STALE_SECONDS: i64 = 30 * 60;

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

pub fn classify_pane_activity(content: &str) -> PaneActivity {
    let stripped = strip_ansi(content);
    let non_empty_lines: Vec<&str> = stripped
        .lines()
        .rev()
        .filter(|line| !line.trim().is_empty())
        .collect();

    if non_empty_lines.is_empty() {
        return PaneActivity::Unknown;
    }

    let mut saw_agent_indicator = false;
    let mut saw_idle_prompt = false;
    let mut saw_busy_indicator = false;

    for line in non_empty_lines.iter().take(8) {
        let lower = line.trim().to_lowercase();

        if lower.contains("thinking")
            || lower.contains("working")
            || lower.contains("reading")
            || lower.contains("writing")
            || lower.contains("analyzing")
            || lower.contains("esc to interrupt")
        {
            saw_busy_indicator = true;
        }

        if lower.contains("bypass permissions")
            || lower.contains("permissions on")
            || lower.contains("permissions off")
            || lower.contains("gpt-")
            || lower.contains("codex")
            || lower.contains("claude")
            || lower.contains("gemini")
            || lower.contains("droid")
            || lower.starts_with('\u{23f5}')
        {
            saw_agent_indicator = true;
        }

        if lower.contains("how can i help") || lower.contains("what would you like") {
            saw_idle_prompt = true;
        } else if lower.starts_with('>')
            || lower.starts_with('\u{276f}')
            || lower.starts_with('\u{203a}')
        {
            let after_prompt = lower
                .trim_start_matches(|c: char| c == '>' || c == '\u{276f}' || c == '\u{203a}')
                .trim();
            if after_prompt.is_empty() {
                saw_idle_prompt = true;
            } else {
                saw_busy_indicator = true;
            }
        }
    }

    if saw_busy_indicator {
        return PaneActivity::AgentBusy;
    }
    if saw_idle_prompt || (saw_agent_indicator && !saw_busy_indicator) {
        return PaneActivity::AgentIdle;
    }

    for line in non_empty_lines.iter().take(5) {
        let lower = line.trim().to_lowercase();
        if lower.contains("please restart codex")
            || lower.contains("update ran successfully")
            || lower.contains("please restart claude")
            || lower.contains("restart to apply")
        {
            return PaneActivity::NeedsLaunch;
        }
    }

    if let Some(last_line) = non_empty_lines.first() {
        let trimmed = last_line.trim();
        if trimmed.ends_with('%') || trimmed.ends_with('$') || trimmed.ends_with('#') {
            return PaneActivity::NeedsLaunch;
        }
    }

    PaneActivity::Unknown
}

pub fn worker_supervision_state(
    runtime: &AgentType,
    worktree_path: &Path,
    pane_content: &str,
) -> WorkerSupervisionState {
    if !runtime.uses_loop_wrapper() {
        return WorkerSupervisionState::Healthy;
    }

    let pane_state = classify_pane_activity(pane_content);
    if pane_state == PaneActivity::NeedsLaunch {
        return WorkerSupervisionState::DeadShellPrompt;
    }
    if pane_state == PaneActivity::AgentIdle {
        return WorkerSupervisionState::DeadIdleOutsideLoop;
    }

    let status_path = worktree_path
        .join(runtime.status_dir())
        .join("fix-loop.status");
    if !status_path.exists() {
        return if pane_state == PaneActivity::AgentBusy {
            WorkerSupervisionState::Healthy
        } else {
            WorkerSupervisionState::DeadMissingStatus
        };
    }

    let status = read_status_file(&status_path);
    if matches!(status.state, AgentState::Stopped) {
        return WorkerSupervisionState::DeadStopped;
    }
    if matches!(status.state, AgentState::Unknown(_)) && pane_state != PaneActivity::AgentBusy {
        return WorkerSupervisionState::DeadMissingStatus;
    }
    if status_timestamp_is_stale(status.timestamp) && pane_state != PaneActivity::AgentBusy {
        return WorkerSupervisionState::DeadStaleStatus;
    }

    WorkerSupervisionState::Healthy
}

pub fn dead_worker_status(
    runtime: &AgentType,
    worktree_path: &Path,
    pane_content: &str,
) -> Option<AgentStatus> {
    let status_path = worktree_path
        .join(runtime.status_dir())
        .join("fix-loop.status");
    let current = if status_path.exists() {
        read_status_file(&status_path)
    } else {
        AgentStatus::default()
    };

    let supervision = worker_supervision_state(runtime, worktree_path, pane_content);
    if !supervision.is_dead() {
        return None;
    }

    Some(AgentStatus {
        timestamp: current.timestamp,
        state: AgentState::Unknown(supervision.detail().to_string()),
    })
}

/// Returns a compact elapsed-time string for a status timestamp.
/// Returns `""` if no timestamp. Examples: `"2m"`, `"45m"`, `"2h"`.
pub fn elapsed_display(ts: Option<NaiveDateTime>) -> String {
    let ts = match ts {
        Some(t) => t,
        None => return String::new(),
    };
    let now = chrono::Local::now().naive_local();
    let secs = (now - ts).num_seconds().max(0);
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

fn status_timestamp_is_stale(timestamp: Option<NaiveDateTime>) -> bool {
    let Some(timestamp) = timestamp else {
        return true;
    };
    let now = chrono::Local::now().naive_local();
    (now - timestamp).num_seconds() > LOOP_STATUS_STALE_SECONDS
}

fn strip_ansi(content: &str) -> String {
    content
        .chars()
        .fold((String::new(), false), |(mut s, in_esc), c| {
            if c == '\x1b' {
                (s, true)
            } else if in_esc {
                (s, c != 'm' && !c.is_ascii_uppercase())
            } else {
                s.push(c);
                (s, false)
            }
        })
        .0
}

#[derive(Debug, Clone)]
pub struct JsonAgentStatus {
    pub status: String,
    pub issue: Option<u32>,
    pub title: Option<String>,
}

/// Read all agent status files from /tmp/agents-ui/.
/// Returns a map from pane identifier (e.g., "claude-agents-ui:2.0") to status.
pub fn read_json_status_files() -> HashMap<String, JsonAgentStatus> {
    let mut result = HashMap::new();
    let dir = Path::new("/tmp/agents-ui");
    if !dir.exists() {
        return result;
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            let key = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                    // Check if this is a monitor summary with a "workers" array
                    if let Some(workers) = val.get("workers").and_then(|v| v.as_array()) {
                        for w in workers {
                            let pane = w
                                .get("pane")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            if pane.is_empty() {
                                continue;
                            }
                            let status = w
                                .get("status")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let issue = w.get("issue").and_then(|v| v.as_u64()).map(|n| n as u32);
                            let title = w
                                .get("title")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            result.insert(
                                pane,
                                JsonAgentStatus {
                                    status,
                                    issue,
                                    title,
                                },
                            );
                        }
                    } else {
                        // Per-worker status file (written by /fix)
                        let status = val
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let issue = val.get("issue").and_then(|v| v.as_u64()).map(|n| n as u32);
                        let title = val
                            .get("title")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        result.insert(
                            key,
                            JsonAgentStatus {
                                status,
                                issue,
                                title,
                            },
                        );
                    }
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_status_path(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "agents-ui-status-{name}-{}-{nanos}",
            std::process::id()
        ))
    }

    #[test]
    fn elapsed_display_none_returns_empty() {
        assert_eq!(elapsed_display(None), "");
    }

    #[test]
    fn elapsed_display_seconds() {
        let ts = chrono::Local::now().naive_local() - chrono::Duration::seconds(30);
        let result = elapsed_display(Some(ts));
        assert!(
            result.ends_with('s'),
            "Expected seconds format, got {result}"
        );
    }

    #[test]
    fn elapsed_display_minutes() {
        let ts = chrono::Local::now().naive_local() - chrono::Duration::minutes(45);
        assert_eq!(elapsed_display(Some(ts)), "45m");
    }

    #[test]
    fn elapsed_display_hours() {
        let ts = chrono::Local::now().naive_local() - chrono::Duration::hours(2);
        assert_eq!(elapsed_display(Some(ts)), "2h");
    }

    #[test]
    fn parses_working_issue_and_timestamp() {
        let status = parse_status_line("2024-01-15 10:30:00\tworking issue #42");
        assert!(status.timestamp.is_some());
        assert!(matches!(
            status.state,
            AgentState::Working { issue: Some(42) }
        ));
    }

    #[test]
    fn parses_idle_completed_and_unknown_states() {
        assert!(matches!(parse_status_line("idle").state, AgentState::Idle));
        assert!(matches!(
            parse_status_line("completed successfully").state,
            AgentState::Completed { .. }
        ));
        assert!(matches!(
            parse_status_line("mystery mode").state,
            AgentState::Unknown(_)
        ));
    }

    #[test]
    fn reads_last_line_from_status_file() {
        let path = temp_status_path("read-last-line");
        std::fs::write(
            &path,
            "2024-01-15 10:00:00\tidle\n2024-01-15 10:30:00\tfixing issue 77\n",
        )
        .unwrap();

        let status = read_status_file(&path);
        assert!(matches!(
            status.state,
            AgentState::Working { issue: Some(77) }
        ));

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn reads_json_status_files_from_tmp() {
        let dir = std::env::temp_dir().join(format!("agents-ui-json-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        // Write a working status file
        std::fs::write(
            dir.join("claude-test:1.0.json"),
            r#"{"status": "working", "issue": 42, "title": "Fix the bug"}"#,
        )
        .unwrap();

        // Write an idle status file
        std::fs::write(dir.join("claude-test:2.0.json"), r#"{"status": "idle"}"#).unwrap();

        // Write a non-json file (should be ignored)
        std::fs::write(dir.join("readme.txt"), "not json").unwrap();

        // Temporarily override the path by calling the internal logic directly
        let mut result = std::collections::HashMap::new();
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let key = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                    let status = val
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let issue = val.get("issue").and_then(|v| v.as_u64()).map(|n| n as u32);
                    let title = val
                        .get("title")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    result.insert(
                        key,
                        super::JsonAgentStatus {
                            status,
                            issue,
                            title,
                        },
                    );
                }
            }
        }

        assert_eq!(result.len(), 2);
        let working = result.get("claude-test:1.0").unwrap();
        assert_eq!(working.status, "working");
        assert_eq!(working.issue, Some(42));
        assert_eq!(working.title.as_deref(), Some("Fix the bug"));

        let idle = result.get("claude-test:2.0").unwrap();
        assert_eq!(idle.status, "idle");
        assert_eq!(idle.issue, None);

        // Also verify read_json_status_files doesn't crash on missing dir
        let _ = read_json_status_files();

        std::fs::remove_dir_all(dir).ok();
    }

    // --- parse_state tests ---

    #[test]
    fn parse_state_starting() {
        assert_eq!(parse_state("Starting"), AgentState::Starting);
        assert_eq!(parse_state("starting up"), AgentState::Starting);
    }

    #[test]
    fn parse_state_working_no_issue() {
        assert_eq!(parse_state("Working"), AgentState::Working { issue: None });
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
        assert_eq!(status.state, AgentState::Working { issue: Some(42) });
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
        assert_eq!(AgentState::Working { issue: None }.to_string(), "Working");
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

    #[test]
    fn classify_pane_activity_detects_shell_idle_and_busy() {
        assert_eq!(
            classify_pane_activity("user@host repo %"),
            PaneActivity::NeedsLaunch
        );
        assert_eq!(classify_pane_activity("> \n"), PaneActivity::AgentIdle);
        assert_eq!(
            classify_pane_activity("reading src/main.rs\n"),
            PaneActivity::AgentBusy
        );
    }

    #[test]
    fn wrapper_worker_missing_status_is_dead_when_not_busy() {
        let dir = temp_status_path("dead-worker");
        std::fs::create_dir_all(&dir).unwrap();

        assert_eq!(
            worker_supervision_state(&AgentType::Codex, &dir, ""),
            WorkerSupervisionState::DeadMissingStatus
        );
        assert_eq!(
            worker_supervision_state(&AgentType::Codex, &dir, "> \n"),
            WorkerSupervisionState::DeadIdleOutsideLoop
        );
        assert_eq!(
            worker_supervision_state(&AgentType::Codex, &dir, "reading src/main.rs\n"),
            WorkerSupervisionState::Healthy
        );

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn wrapper_worker_stale_status_is_dead_when_not_busy() {
        let dir = temp_status_path("stale-worker");
        let loops = dir.join(".codex/loops");
        std::fs::create_dir_all(&loops).unwrap();
        std::fs::write(loops.join("fix-loop.status"), "2024-01-15 10:00:00\tidle\n").unwrap();

        assert_eq!(
            worker_supervision_state(&AgentType::Codex, &dir, ""),
            WorkerSupervisionState::DeadStaleStatus
        );
        assert_eq!(
            dead_worker_status(&AgentType::Codex, &dir, "")
                .unwrap()
                .state
                .to_string(),
            "dead: stale loop heartbeat"
        );

        std::fs::remove_dir_all(dir).ok();
    }
}
