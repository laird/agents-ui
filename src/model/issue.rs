use serde::Deserialize;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub enum IssueState {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IssueFilter {
    All,
    Open,
    Blocked,
}

impl IssueFilter {
    pub fn next(self) -> Self {
        match self {
            IssueFilter::All => IssueFilter::Open,
            IssueFilter::Open => IssueFilter::Blocked,
            IssueFilter::Blocked => IssueFilter::All,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            IssueFilter::All => "all",
            IssueFilter::Open => "open",
            IssueFilter::Blocked => "blocked",
        }
    }
}

/// A GitHub issue.
#[derive(Debug, Clone)]
pub struct GitHubIssue {
    pub number: u32,
    pub title: String,
    pub state: IssueState,
    pub labels: Vec<String>,
    /// Worker ID currently working on this issue, if any.
    pub assigned_worker: Option<String>,
}

const BLOCKING_LABELS: &[&str] = &[
    "needs-design",
    "needs-clarification",
    "needs-approval",
    "too-complex",
    "future",
    "proposal",
];

impl GitHubIssue {
    pub fn is_blocked(&self) -> bool {
        self.labels
            .iter()
            .any(|l| BLOCKING_LABELS.contains(&l.as_str()))
    }

    pub fn is_being_worked(&self) -> bool {
        self.labels.iter().any(|l| l == "working")
    }

    pub fn priority(&self) -> Option<u8> {
        for label in &self.labels {
            if let Some(p) = label.strip_prefix('P') {
                if let Ok(n) = p.parse::<u8>() {
                    return Some(n);
                }
            }
        }
        None
    }

    pub fn priority_label(&self) -> String {
        self.priority()
            .map(|p| format!("P{p}"))
            .unwrap_or_else(|| "—".to_string())
    }

    pub fn status_label(&self) -> String {
        if self.is_being_worked() {
            if let Some(ref w) = self.assigned_worker {
                format!("🔨 {w}")
            } else {
                "🔨 working".to_string()
            }
        } else if self.is_blocked() {
            let blocking = self
                .labels
                .iter()
                .find(|l| BLOCKING_LABELS.contains(&l.as_str()))
                .cloned()
                .unwrap_or_default();
            format!("🚫 {blocking}")
        } else if self.state == IssueState::Open {
            "available".to_string()
        } else {
            "closed".to_string()
        }
    }

    /// Whether this issue matches the given filter.
    pub fn matches_filter(&self, filter: IssueFilter) -> bool {
        match filter {
            IssueFilter::All => true,
            IssueFilter::Open => self.state == IssueState::Open && !self.is_blocked(),
            IssueFilter::Blocked => self.is_blocked(),
        }
    }
}

/// Cached issues for a project.
pub struct IssueCache {
    pub issues: Vec<GitHubIssue>,
    pub last_fetched: Option<Instant>,
}

impl Default for IssueCache {
    fn default() -> Self {
        Self {
            issues: Vec::new(),
            last_fetched: None,
        }
    }
}

/// Raw JSON structure from `gh issue list --json`.
#[derive(Deserialize)]
pub struct GhIssueJson {
    pub number: u32,
    pub title: String,
    pub state: String,
    pub labels: Vec<GhLabelJson>,
}

#[derive(Deserialize)]
pub struct GhLabelJson {
    pub name: String,
}

impl From<GhIssueJson> for GitHubIssue {
    fn from(raw: GhIssueJson) -> Self {
        let state = if raw.state == "OPEN" {
            IssueState::Open
        } else {
            IssueState::Closed
        };
        let labels: Vec<String> = raw.labels.into_iter().map(|l| l.name).collect();
        GitHubIssue {
            number: raw.number,
            title: raw.title,
            state,
            labels,
            assigned_worker: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_issue(number: u32, labels: &[&str]) -> GitHubIssue {
        GitHubIssue {
            number,
            title: format!("Issue #{number}"),
            state: IssueState::Open,
            labels: labels.iter().map(|s| s.to_string()).collect(),
            assigned_worker: None,
        }
    }

    #[test]
    fn is_blocked_with_blocking_label() {
        assert!(make_issue(1, &["needs-design"]).is_blocked());
        assert!(make_issue(2, &["proposal"]).is_blocked());
        assert!(make_issue(3, &["too-complex"]).is_blocked());
    }

    #[test]
    fn is_not_blocked_without_blocking_label() {
        assert!(!make_issue(1, &["bug", "P1"]).is_blocked());
        assert!(!make_issue(2, &[]).is_blocked());
    }

    #[test]
    fn is_being_worked() {
        assert!(make_issue(1, &["working"]).is_being_worked());
        assert!(!make_issue(2, &["bug"]).is_being_worked());
    }

    #[test]
    fn priority_extraction() {
        assert_eq!(make_issue(1, &["P0", "bug"]).priority(), Some(0));
        assert_eq!(make_issue(2, &["P2"]).priority(), Some(2));
        assert_eq!(make_issue(3, &["bug"]).priority(), None);
    }

    #[test]
    fn filter_matching() {
        let open = make_issue(1, &["bug"]);
        let blocked = make_issue(2, &["needs-design"]);
        let working = make_issue(3, &["working"]);

        assert!(open.matches_filter(IssueFilter::All));
        assert!(open.matches_filter(IssueFilter::Open));
        assert!(!open.matches_filter(IssueFilter::Blocked));

        assert!(blocked.matches_filter(IssueFilter::All));
        assert!(!blocked.matches_filter(IssueFilter::Open));
        assert!(blocked.matches_filter(IssueFilter::Blocked));

        assert!(working.matches_filter(IssueFilter::All));
    }

    #[test]
    fn filter_cycle() {
        assert_eq!(IssueFilter::All.next(), IssueFilter::Open);
        assert_eq!(IssueFilter::Open.next(), IssueFilter::Blocked);
        assert_eq!(IssueFilter::Blocked.next(), IssueFilter::All);
    }

    #[test]
    fn status_label_display() {
        assert_eq!(make_issue(1, &["bug"]).status_label(), "available");
        assert!(make_issue(2, &["working"]).status_label().contains("working"));
        assert!(make_issue(3, &["needs-design"]).status_label().contains("needs-design"));
    }
}
