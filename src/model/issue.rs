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
    Working,
}

impl IssueFilter {
    pub fn next(self) -> Self {
        match self {
            IssueFilter::All => IssueFilter::Open,
            IssueFilter::Open => IssueFilter::Blocked,
            IssueFilter::Blocked => IssueFilter::Working,
            IssueFilter::Working => IssueFilter::All,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            IssueFilter::All => "all",
            IssueFilter::Open => "open",
            IssueFilter::Blocked => "blocked",
            IssueFilter::Working => "working",
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
    /// When this issue was last updated on GitHub.
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
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

    /// Returns a single-character type indicator for display in the issues table.
    pub fn type_char(&self) -> &'static str {
        match self.issue_type {
            IssueType::Bug => "B",
            IssueType::Enhancement => "E",
            IssueType::Proposal => "P",
            IssueType::Other => "·",
        }
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

    pub fn matches_filter(&self, filter: IssueFilter) -> bool {
        match filter {
            IssueFilter::All => true,
            IssueFilter::Open => self.state == IssueState::Open && !self.is_blocked(),
            IssueFilter::Blocked => self.is_blocked(),
            IssueFilter::Working => self.is_being_worked(),
        }
    }

    /// Returns true if the issue was updated within the last 24 hours.
    pub fn is_recently_updated(&self) -> bool {
        self.updated_at.map_or(false, |t| {
            chrono::Utc::now().signed_duration_since(t).num_hours() < 24
        })
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
    #[serde(rename = "updatedAt", default)]
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
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
            updated_at: raw.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

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
            updated_at: None,
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
    fn recently_updated_within_24h_returns_true() {
        let mut issue = make_issue(1, &["bug"]);
        issue.updated_at = Some(Utc::now() - chrono::Duration::hours(1));
        assert!(issue.is_recently_updated());
    }

    #[test]
    fn recently_updated_older_than_24h_returns_false() {
        let mut issue = make_issue(1, &["bug"]);
        issue.updated_at = Some(Utc::now() - chrono::Duration::hours(25));
        assert!(!issue.is_recently_updated());
    }

    #[test]
    fn recently_updated_none_returns_false() {
        assert!(!make_issue(1, &["bug"]).is_recently_updated());
    }

    #[test]
    fn recently_updated_does_not_change_type_char() {
        // The ★ indicator is rendered by swarm_view, not type_char()
        let mut issue = make_issue(1, &["bug"]);
        issue.updated_at = Some(Utc::now() - chrono::Duration::hours(1));
        assert!(issue.is_recently_updated());
        assert_eq!(issue.type_char(), "B");
    }

    #[test]
    fn filter_cycle() {
        assert_eq!(IssueFilter::All.next(), IssueFilter::Open);
        assert_eq!(IssueFilter::Open.next(), IssueFilter::Blocked);
        assert_eq!(IssueFilter::Blocked.next(), IssueFilter::Working);
        assert_eq!(IssueFilter::Working.next(), IssueFilter::All);
    }

    #[test]
    fn filter_working_matches_only_working_issues() {
        let working = make_issue(1, &["working"]);
        let open = make_issue(2, &["bug"]);
        let blocked = make_issue(3, &["needs-design"]);

        assert!(working.matches_filter(IssueFilter::Working));
        assert!(!open.matches_filter(IssueFilter::Working));
        assert!(!blocked.matches_filter(IssueFilter::Working));

        assert_eq!(IssueFilter::Working.label(), "working");
    }

    #[test]
    fn type_char_returns_correct_indicator() {
        assert_eq!(make_issue(1, &["bug"]).type_char(), "B");
        assert_eq!(make_issue(2, &["enhancement"]).type_char(), "E");
        assert_eq!(make_issue(3, &["proposal"]).type_char(), "P");
        assert_eq!(make_issue(4, &[]).type_char(), "·");
    }

    #[test]
    fn status_label_display() {
        assert_eq!(make_issue(1, &["bug"]).status_label(), "available");
        assert!(
            make_issue(2, &["working"])
                .status_label()
                .contains("working")
        );
        assert!(
            make_issue(3, &["needs-design"])
                .status_label()
                .contains("needs-design")
        );
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

    #[test]
    fn status_label_shows_assigned_worker_name() {
        let mut issue = make_issue(1, &["working"]);
        issue.assigned_worker = Some("worker-1".to_string());
        assert_eq!(issue.status_label(), "🔨 worker-1");
    }

    #[test]
    fn issue_priority_display_formats_correctly() {
        assert_eq!(IssuePriority::P0.to_string(), "P0");
        assert_eq!(IssuePriority::P1.to_string(), "P1");
        assert_eq!(IssuePriority::P2.to_string(), "P2");
        assert_eq!(IssuePriority::P3.to_string(), "P3");
        assert_eq!(IssuePriority::None.to_string(), "—");
    }

    #[test]
    fn issue_type_display_formats_correctly() {
        assert_eq!(IssueType::Bug.to_string(), "bug");
        assert_eq!(IssueType::Enhancement.to_string(), "enhancement");
        assert_eq!(IssueType::Proposal.to_string(), "proposal");
        assert_eq!(IssueType::Other.to_string(), "");
    }

    #[test]
    fn gh_issue_json_converts_closed_state_correctly() {
        let raw = GhIssueJson {
            number: 42,
            title: "A closed bug".to_string(),
            state: "CLOSED".to_string(),
            labels: vec![
                GhLabelJson { name: "bug".to_string() },
                GhLabelJson { name: "P1".to_string() },
            ],
            updated_at: None,
        };
        let issue = GitHubIssue::from(raw);
        assert_eq!(issue.state, IssueState::Closed);
        assert_eq!(issue.issue_type, IssueType::Bug);
        assert_eq!(issue.priority, IssuePriority::P1);
        assert_eq!(issue.status_label(), "closed");
    }

    // ── priority_num ─────────────────────────────────────────────────────────

    #[test]
    fn priority_num_p0_label_returns_some_0() {
        assert_eq!(make_issue(1, &["P0", "bug"]).priority_num(), Some(0));
    }

    #[test]
    fn priority_num_p3_label_returns_some_3() {
        assert_eq!(make_issue(1, &["P3"]).priority_num(), Some(3));
    }

    #[test]
    fn priority_num_no_priority_label_returns_none() {
        assert_eq!(make_issue(1, &["bug", "enhancement"]).priority_num(), None);
    }

    #[test]
    fn priority_num_non_priority_p_label_ignored() {
        // "Pxyz" doesn't parse as u8, so should return None
        assert_eq!(make_issue(1, &["Pxyz"]).priority_num(), None);
    }

    // ── priority_label ────────────────────────────────────────────────────────

    #[test]
    fn priority_label_with_p2_returns_p2_string() {
        assert_eq!(make_issue(1, &["P2"]).priority_label(), "P2");
    }

    #[test]
    fn priority_label_no_priority_returns_dash() {
        assert_eq!(make_issue(1, &["bug"]).priority_label(), "—");
    }

    // ── IssueCache::priority_counts ───────────────────────────────────────────

    #[test]
    fn priority_counts_empty_cache_returns_zeros() {
        let cache = IssueCache::default();
        assert_eq!(cache.priority_counts(), (0, 0, 0, 0));
    }

    // ── IssueCache::filtered ──────────────────────────────────────────────────

    #[test]
    fn filtered_none_returns_all_issues() {
        let mut cache = IssueCache::default();
        cache.issues.push(make_issue(1, &["P0"]));
        cache.issues.push(make_issue(2, &["P1"]));
        cache.issues.push(make_issue(3, &["P3"]));
        assert_eq!(cache.filtered(None).len(), 3);
    }

    #[test]
    fn filtered_by_p1_returns_only_p1_issues() {
        let mut cache = IssueCache::default();
        cache.issues.push(make_issue(1, &["P0"]));
        cache.issues.push(make_issue(2, &["P1"]));
        cache.issues.push(make_issue(3, &["P1"]));
        cache.issues.push(make_issue(4, &["P2"]));
        let result = cache.filtered(Some(&IssuePriority::P1));
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|i| i.priority == IssuePriority::P1));
    }

    // ── blocking_guidance ─────────────────────────────────────────────────────

    #[test]
    fn blocking_guidance_needs_design() {
        assert_eq!(blocking_guidance("needs-design"), "Add design doc or spec to issue");
    }

    #[test]
    fn blocking_guidance_needs_approval() {
        assert_eq!(blocking_guidance("needs-approval"), "Request stakeholder sign-off");
    }

    #[test]
    fn blocking_guidance_needs_clarification() {
        assert_eq!(blocking_guidance("needs-clarification"), "Reply with additional context");
    }

    #[test]
    fn blocking_guidance_too_complex() {
        assert_eq!(blocking_guidance("too-complex"), "Break into sub-tasks manually");
    }

    #[test]
    fn blocking_guidance_future() {
        assert_eq!(blocking_guidance("future"), "Defer — remove label to unblock");
    }

    #[test]
    fn blocking_guidance_proposal() {
        assert_eq!(blocking_guidance("proposal"), "Remove proposal label to approve");
    }

    #[test]
    fn blocking_guidance_unknown_label_returns_default() {
        assert_eq!(
            blocking_guidance("some-unknown-label"),
            "Review and remove blocking label to unblock"
        );
    }

    // ── IssueFilter::label ────────────────────────────────────────────────────

    #[test]
    fn issue_filter_label_all_variants() {
        assert_eq!(IssueFilter::All.label(), "all");
        assert_eq!(IssueFilter::Open.label(), "open");
        assert_eq!(IssueFilter::Blocked.label(), "blocked");
        assert_eq!(IssueFilter::Working.label(), "working");
    }

    // ── IssuePriority ordering ────────────────────────────────────────────────

    #[test]
    fn issue_priority_ordering_p0_less_than_p1() {
        assert!(IssuePriority::P0 < IssuePriority::P1);
    }

    #[test]
    fn issue_priority_ordering_p1_less_than_p2() {
        assert!(IssuePriority::P1 < IssuePriority::P2);
    }

    #[test]
    fn issue_priority_ordering_p2_less_than_p3() {
        assert!(IssuePriority::P2 < IssuePriority::P3);
    }

    #[test]
    fn issue_priority_ordering_p3_less_than_none() {
        assert!(IssuePriority::P3 < IssuePriority::None);
    }

    #[test]
    fn issue_priority_sorted_vec_is_in_order() {
        let mut priorities = vec![
            IssuePriority::None,
            IssuePriority::P2,
            IssuePriority::P0,
            IssuePriority::P3,
            IssuePriority::P1,
        ];
        priorities.sort();
        assert_eq!(
            priorities,
            vec![
                IssuePriority::P0,
                IssuePriority::P1,
                IssuePriority::P2,
                IssuePriority::P3,
                IssuePriority::None,
            ]
        );
    }
}
