use serde::Deserialize;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub enum IssueState {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Priority level parsed from GitHub issue labels.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum IssuePriority {
    P0,
    P1,
    P2,
    P3,
    None,
}

impl std::fmt::Display for IssuePriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IssuePriority::P0 => write!(f, "P0"),
            IssuePriority::P1 => write!(f, "P1"),
            IssuePriority::P2 => write!(f, "P2"),
            IssuePriority::P3 => write!(f, "P3"),
            IssuePriority::None => write!(f, "—"),
        }
    }
}

/// Type of issue derived from labels.
#[derive(Debug, Clone, PartialEq)]
pub enum IssueType {
    Bug,
    Enhancement,
    Proposal,
    Other,
}

impl std::fmt::Display for IssueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IssueType::Bug => write!(f, "bug"),
            IssueType::Enhancement => write!(f, "enhancement"),
            IssueType::Proposal => write!(f, "proposal"),
            IssueType::Other => write!(f, ""),
        }
    }
}

/// A GitHub issue.
#[derive(Debug, Clone)]
pub struct GitHubIssue {
    pub number: u32,
    pub title: String,
    pub state: IssueState,
    pub priority: IssuePriority,
    pub issue_type: IssueType,
    pub labels: Vec<String>,
    pub is_working: bool,
    /// Worker ID currently working on this issue, if any.
    pub assigned_worker: Option<String>,
}

pub const BLOCKING_LABELS: &[&str] = &[
    "needs-design",
    "needs-clarification",
    "needs-approval",
    "too-complex",
    "future",
    "proposal",
];

/// Returns a short action hint for a blocking label.
pub fn blocking_guidance(label: &str) -> &'static str {
    match label {
        "needs-design" => "Add design doc or spec to issue",
        "needs-approval" => "Request stakeholder sign-off",
        "needs-clarification" => "Reply with additional context",
        "too-complex" => "Break into sub-tasks manually",
        "future" => "Defer — remove label to unblock",
        "proposal" => "Remove proposal label to approve",
        _ => "Review and remove blocking label to unblock",
    }
}

impl GitHubIssue {
    pub fn is_blocked(&self) -> bool {
        self.labels
            .iter()
            .any(|l| BLOCKING_LABELS.contains(&l.as_str()))
    }

    pub fn is_being_worked(&self) -> bool {
        self.labels.iter().any(|l| l == "working")
    }

    pub fn priority_num(&self) -> Option<u8> {
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
        self.priority_num()
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

    pub fn type_char(&self) -> &str {
        match self.issue_type {
            IssueType::Bug => "B",
            IssueType::Enhancement => "E",
            IssueType::Proposal => "P",
            IssueType::Other => "·",
        }
    }
}

/// Cached issues for a project.
#[derive(Debug, Clone)]
pub struct IssueCache {
    pub issues: Vec<GitHubIssue>,
    pub last_fetched: Option<Instant>,
    pub is_loading: bool,
}

impl Default for IssueCache {
    fn default() -> Self {
        Self {
            issues: Vec::new(),
            last_fetched: None,
            is_loading: false,
        }
    }
}

impl IssueCache {
    /// Count issues by priority.
    pub fn priority_counts(&self) -> (usize, usize, usize, usize) {
        let mut p0 = 0;
        let mut p1 = 0;
        let mut p2 = 0;
        let mut p3 = 0;
        for issue in &self.issues {
            match issue.priority {
                IssuePriority::P0 => p0 += 1,
                IssuePriority::P1 => p1 += 1,
                IssuePriority::P2 => p2 += 1,
                IssuePriority::P3 => p3 += 1,
                IssuePriority::None => {}
            }
        }
        (p0, p1, p2, p3)
    }

    /// Filter issues by priority. `None` means show all.
    pub fn filtered(&self, filter: Option<&IssuePriority>) -> Vec<&GitHubIssue> {
        match filter {
            Some(p) => self.issues.iter().filter(|i| &i.priority == p).collect(),
            None => self.issues.iter().collect(),
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

fn labels_to_priority(labels: &[String]) -> IssuePriority {
    if labels.iter().any(|l| l == "P0") {
        IssuePriority::P0
    } else if labels.iter().any(|l| l == "P1") {
        IssuePriority::P1
    } else if labels.iter().any(|l| l == "P2") {
        IssuePriority::P2
    } else if labels.iter().any(|l| l == "P3") {
        IssuePriority::P3
    } else {
        IssuePriority::None
    }
}

fn labels_to_type(labels: &[String]) -> IssueType {
    if labels.iter().any(|l| l == "proposal") {
        IssueType::Proposal
    } else if labels.iter().any(|l| l == "enhancement") {
        IssueType::Enhancement
    } else if labels.iter().any(|l| l == "bug") {
        IssueType::Bug
    } else {
        IssueType::Other
    }
}

impl From<GhIssueJson> for GitHubIssue {
    fn from(raw: GhIssueJson) -> Self {
        let state = if raw.state == "OPEN" {
            IssueState::Open
        } else {
            IssueState::Closed
        };
        let labels: Vec<String> = raw.labels.into_iter().map(|l| l.name).collect();
        let priority = labels_to_priority(&labels);
        let issue_type = labels_to_type(&labels);
        let is_working = labels.iter().any(|l| l == "working");
        GitHubIssue {
            number: raw.number,
            title: raw.title,
            state,
            priority,
            issue_type,
            labels,
            is_working,
            assigned_worker: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_issue(number: u32, labels: &[&str]) -> GitHubIssue {
        let label_vec: Vec<String> = labels.iter().map(|s| s.to_string()).collect();
        let priority = labels_to_priority(&label_vec);
        let issue_type = labels_to_type(&label_vec);
        let is_working = label_vec.iter().any(|l| l == "working");
        GitHubIssue {
            number,
            title: format!("Issue #{number}"),
            state: IssueState::Open,
            priority,
            issue_type,
            labels: label_vec,
            is_working,
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
        assert_eq!(make_issue(1, &["P0", "bug"]).priority, IssuePriority::P0);
        assert_eq!(make_issue(2, &["P2"]).priority, IssuePriority::P2);
        assert_eq!(make_issue(3, &["bug"]).priority, IssuePriority::None);
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
    fn type_char_variants() {
        assert_eq!(make_issue(1, &["bug"]).type_char(), "B");
        assert_eq!(make_issue(2, &["enhancement"]).type_char(), "E");
        assert_eq!(make_issue(3, &["proposal"]).type_char(), "P");
        assert_eq!(make_issue(4, &[]).type_char(), "·");
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

    #[test]
    fn priority_counts_work() {
        let mut cache = IssueCache::default();
        cache.issues.push(make_issue(1, &["P0"]));
        cache.issues.push(make_issue(2, &["P1"]));
        cache.issues.push(make_issue(3, &["P1"]));
        cache.issues.push(make_issue(4, &["P2"]));
        let (p0, p1, p2, p3) = cache.priority_counts();
        assert_eq!(p0, 1);
        assert_eq!(p1, 2);
        assert_eq!(p2, 1);
        assert_eq!(p3, 0);
    }
}
