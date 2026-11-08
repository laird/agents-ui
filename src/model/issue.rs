use serde::Deserialize;
use std::time::Instant;
use std::path::Path;
use tokio::process::Command;
use crate::project_management::ProjectManagementClient;

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
    "needs-feedback",
    "blocked",
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
        "needs-feedback" => "Reply with feedback on the work so far",
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

/// Fetch open issues from GitHub using `gh` CLI.
pub async fn fetch_issues(repo_path: &Path) -> anyhow::Result<Vec<GitHubIssue>> {
    let pm = ProjectManagementClient::from_env();
    let cmd = pm.list_open_issues("number,title,labels", 100);
    let output = Command::new(cmd.program)
        .args(cmd.args)
        .current_dir(repo_path)
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!(
            "gh issue list failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let mut issues = Vec::new();

    if let Some(arr) = json.as_array() {
        for item in arr {
            let number = item["number"].as_u64().unwrap_or(0) as u32;
            let title = item["title"].as_str().unwrap_or("").to_string();

            let labels: Vec<String> = item["labels"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|l| l["name"].as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            let priority = labels_to_priority(&labels);
            let issue_type = labels_to_type(&labels);
            let is_working = labels.iter().any(|l| l == "working");

            issues.push(GitHubIssue {
                number,
                title,
                state: IssueState::Open,
                priority,
                issue_type,
                labels,
                is_working,
                assigned_worker: None,
                updated_at: None,
            });
        }
    }

    // Sort by priority then number
    issues.sort_by(|a, b| a.priority.cmp(&b.priority).then(a.number.cmp(&b.number)));

    Ok(issues)
}
// ---------------------------------------------------------------------------
// Feature links: which subtasks make a feature (#346, #347)
// ---------------------------------------------------------------------------

/// How a feature's subtasks were found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkedBy {
    /// GitHub sub-issues of the parent.
    SubIssues,
    /// Issues whose body says `Sub-task of #N` (autocoder's convention).
    Body,
}

/// One subtask of a feature.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Child {
    pub number: u32,
    pub title: String,
    pub url: String,
    pub closed: bool,
    /// Closed as not planned (or as a duplicate): out of the feature's total.
    pub not_planned: bool,
}

/// A parent issue and the subtasks that make it up.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FeatureLink {
    pub parent: u32,
    pub title: String,
    pub url: String,
    /// The repository document the parent's body links to, if any. When
    /// there is none the issue itself is the spec.
    pub spec_url: Option<String>,
    pub linked_by: LinkedBy,
    /// Sorted by number.
    pub children: Vec<Child>,
}

/// The GraphQL query `crate::github::fetch_feature_link` sends: the parent
/// with its sub-issues, plus a search for `Sub-task of #N` bodies, so both
/// sources cost one round trip.
pub const FEATURE_LINK_QUERY: &str = "query($owner:String!,$name:String!,$number:Int!,$q:String!){\
repository(owner:$owner,name:$name){issue(number:$number){number title url body \
subIssues(first:100){nodes{number title url state stateReason}}}}\
search(query:$q,type:ISSUE,first:100){nodes{... on Issue{number title url state stateReason body}}}}";

/// The GraphQL query `crate::github::fetch_feature_parents` sends: the
/// repository's open issues with their sub-issue counts and labels.
pub const FEATURE_PARENTS_QUERY: &str = "query($owner:String!,$name:String!){\
repository(owner:$owner,name:$name){issues(states:OPEN,first:100,orderBy:{field:UPDATED_AT,direction:DESC}){\
nodes{number subIssuesSummary{total} labels(first:30){nodes{name}}}}}}";

/// Open issues that are feature parents: they have sub-issues, or carry
/// autocoder's `decomposed` label. Sorted; empty when the JSON can't be read.
pub fn feature_parents_from_json(bytes: &[u8]) -> Vec<u32> {
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return Vec::new();
    };
    let nodes = v.pointer("/data/repository/issues/nodes").and_then(|n| n.as_array());
    let mut parents: Vec<u32> = nodes
        .into_iter()
        .flatten()
        .filter(|n| {
            let subs = n.pointer("/subIssuesSummary/total").and_then(|t| t.as_u64()).unwrap_or(0);
            let decomposed = n
                .pointer("/labels/nodes")
                .and_then(|l| l.as_array())
                .is_some_and(|l| l.iter().any(|l| l["name"] == "decomposed"));
            subs > 0 || decomposed
        })
        .filter_map(|n| n["number"].as_u64().and_then(|x| u32::try_from(x).ok()))
        .collect();
    parents.sort_unstable();
    parents.dedup();
    parents
}

/// The search string that finds candidate `Sub-task of #N` children. GitHub's
/// search is loose, so results are re-checked with [`body_names_parent`].
pub fn subtask_search_query(owner_repo: &str, parent: u32) -> String {
    format!("repo:{owner_repo} is:issue \"Sub-task of #{parent}\"")
}

/// Whether an issue body carries autocoder's `Sub-task of #N` line for this
/// parent (`#34` does not match `#346`).
pub fn body_names_parent(body: &str, parent: u32) -> bool {
    let needle = format!("sub-task of #{parent}");
    let lower = body.to_lowercase();
    lower.match_indices(&needle).any(|(i, m)| {
        !lower[i + m.len()..].starts_with(|c: char| c.is_ascii_digit())
    })
}

/// The first link in a parent's body to a document in the same repository:
/// a `…/blob/…/*.md` URL, or a markdown link to a relative `docs/…` path
/// (made absolute against the repository). `None` means the issue is the spec.
pub fn spec_link(body: &str, issue_url: &str) -> Option<String> {
    let repo = issue_url.split("/issues/").next().filter(|r| *r != issue_url)?;
    let blob_prefix = format!("{repo}/blob/");
    let mut best: Option<(usize, String)> = None;
    let mut consider = |at: usize, link: String| {
        if best.as_ref().is_none_or(|(b, _)| at < *b) {
            best = Some((at, link));
        }
    };

    // Absolute blob URLs, bare or inside a markdown link.
    if let Some((at, url)) = body.match_indices(&blob_prefix).find_map(|(at, _)| {
        let url = take_link(&body[at..]);
        is_markdown_doc(url).then(|| (at, url.to_string()))
    }) {
        consider(at, url);
    }

    // Relative markdown link targets: `[spec](docs/specs/x.md)`.
    if let Some((at, path)) = body.match_indices("](").find_map(|(at, m)| {
        let target = take_link(&body[at + m.len()..]);
        let path = target.trim_start_matches("./").trim_start_matches('/');
        path.starts_with("docs/").then(|| (at, path.to_string()))
    }) {
        consider(at, format!("{repo}/blob/HEAD/{path}"));
    }

    best.map(|(_, link)| link)
}

/// The link at the start of `s`, up to whitespace or a closing delimiter.
fn take_link(s: &str) -> &str {
    let end = s
        .find(|c: char| c.is_whitespace() || matches!(c, ')' | '>' | '"' | '\'' | '`' | ']'))
        .unwrap_or(s.len());
    s[..end].trim_end_matches(['.', ',', ';', ':'])
}

fn is_markdown_doc(url: &str) -> bool {
    let path = url.split(['#', '?']).next().unwrap_or(url);
    path.to_ascii_lowercase().ends_with(".md")
}

#[derive(Deserialize)]
struct GqlIssue {
    #[serde(default)]
    number: u32,
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    state: String,
    #[serde(default, rename = "stateReason")]
    state_reason: Option<String>,
    #[serde(default)]
    body: Option<String>,
}

impl GqlIssue {
    fn child(self) -> Child {
        let closed = self.state.eq_ignore_ascii_case("closed");
        let reason = self.state_reason.unwrap_or_default().to_ascii_uppercase();
        Child {
            number: self.number,
            title: self.title,
            url: self.url,
            closed,
            not_planned: closed && matches!(reason.as_str(), "NOT_PLANNED" | "DUPLICATE"),
        }
    }
}

/// Build a parent's [`FeatureLink`] from the [`FEATURE_LINK_QUERY`]
/// response. Sub-issues win when the parent has any; otherwise children are
/// the search hits whose body names the parent. `None` when the JSON can't be
/// read, the parent is missing, or it has no subtasks (it isn't a feature).
pub fn feature_link_from_json(bytes: &[u8]) -> Option<FeatureLink> {
    let v: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let data = v.get("data")?;
    let issue = data.pointer("/repository/issue").filter(|i| !i.is_null())?;
    let nodes = |v: Option<&serde_json::Value>| -> Vec<GqlIssue> {
        v.and_then(|n| n.as_array())
            .map(|arr| {
                arr.iter()
                    // Search nodes that aren't issues come back as `{}`.
                    .filter(|n| n.get("number").is_some())
                    .filter_map(|n| serde_json::from_value(n.clone()).ok())
                    .collect()
            })
            .unwrap_or_default()
    };

    let parent: GqlIssue = serde_json::from_value(issue.clone()).ok()?;
    let subs = nodes(issue.pointer("/subIssues/nodes"));
    let (linked_by, mut children): (LinkedBy, Vec<Child>) = if !subs.is_empty() {
        (LinkedBy::SubIssues, subs.into_iter().map(GqlIssue::child).collect())
    } else {
        let hits = nodes(data.pointer("/search/nodes"));
        let kids = hits
            .into_iter()
            .filter(|h| h.number != parent.number)
            .filter(|h| h.body.as_deref().is_some_and(|b| body_names_parent(b, parent.number)))
            .map(GqlIssue::child)
            .collect();
        (LinkedBy::Body, kids)
    };
    if children.is_empty() {
        return None;
    }
    children.sort_by_key(|c| c.number);
    children.dedup_by_key(|c| c.number);

    let spec_url = spec_link(parent.body.as_deref().unwrap_or(""), &parent.url);
    Some(FeatureLink {
        parent: parent.number,
        title: parent.title,
        url: parent.url,
        spec_url,
        linked_by,
        children,
    })
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
    fn priority_label_returns_correct_strings() {
        assert_eq!(make_issue(1, &["P0"]).priority_label(), "P0");
        assert_eq!(make_issue(2, &["P1"]).priority_label(), "P1");
        assert_eq!(make_issue(3, &["P2"]).priority_label(), "P2");
        assert_eq!(make_issue(4, &["P3"]).priority_label(), "P3");
        assert_eq!(make_issue(5, &["bug"]).priority_label(), "\u{2014}");
    }

    #[test]
    fn priority_num_returns_correct_values() {
        assert_eq!(make_issue(1, &["P0"]).priority_num(), Some(0));
        assert_eq!(make_issue(2, &["P1"]).priority_num(), Some(1));
        assert_eq!(make_issue(3, &["P2"]).priority_num(), Some(2));
        assert_eq!(make_issue(4, &["P3"]).priority_num(), Some(3));
        assert_eq!(make_issue(5, &["bug"]).priority_num(), None);
    }

    #[test]
    fn issue_filter_label_covers_all_variants() {
        assert!(!IssueFilter::All.label().is_empty());
        assert!(!IssueFilter::Open.label().is_empty());
        assert!(!IssueFilter::Blocked.label().is_empty());
    }

    #[test]
    fn labels_to_type_maps_correctly() {
        assert_eq!(make_issue(1, &["bug"]).issue_type, IssueType::Bug);
        assert_eq!(make_issue(2, &["enhancement"]).issue_type, IssueType::Enhancement);
        assert_eq!(make_issue(3, &["proposal"]).issue_type, IssueType::Proposal);
        assert_eq!(make_issue(4, &[]).issue_type, IssueType::Other);
    }

    #[test]
    fn issue_cache_filtered_by_priority() {
        let mut cache = IssueCache::default();
        cache.issues.push(make_issue(1, &["P1", "bug"]));
        cache.issues.push(make_issue(2, &["P2", "bug"]));
        cache.issues.push(make_issue(3, &["P1", "enhancement"]));

        let p1_issues = cache.filtered(Some(&IssuePriority::P1));
        assert_eq!(p1_issues.len(), 2);
        assert!(p1_issues.iter().all(|i| i.priority == IssuePriority::P1));

        let all_issues = cache.filtered(None);
        assert_eq!(all_issues.len(), 3);
    }

    // -- Feature links (#347) ----------------------------------------------

    const REPO: &str = "https://github.com/acme/app";

    fn gql(parent_body: &str, subs: &str, search: &str) -> Vec<u8> {
        serde_json::json!({"data": {
            "repository": {"issue": {
                "number": 10, "title": "Checkout v2", "url": format!("{REPO}/issues/10"),
                "body": parent_body,
                "subIssues": {"nodes": serde_json::from_str::<serde_json::Value>(subs).unwrap()}
            }},
            "search": {"nodes": serde_json::from_str::<serde_json::Value>(search).unwrap()}
        }})
        .to_string()
        .into_bytes()
    }

    #[test]
    fn feature_link_from_sub_issues() {
        let f = feature_link_from_json(&gql(
            "The feature.",
            r##"[{"number":12,"title":"B","url":"u12","state":"OPEN","stateReason":null},
                {"number":11,"title":"A","url":"u11","state":"CLOSED","stateReason":"COMPLETED"}]"##,
            "[]",
        ))
        .unwrap();
        assert_eq!(f.parent, 10);
        assert_eq!(f.title, "Checkout v2");
        assert_eq!(f.linked_by, LinkedBy::SubIssues);
        assert_eq!(f.children.iter().map(|c| c.number).collect::<Vec<_>>(), vec![11, 12]);
        assert!(f.children[0].closed && !f.children[0].not_planned);
        assert!(!f.children[1].closed);
        assert_eq!(f.spec_url, None);
    }

    #[test]
    fn feature_link_from_subtask_bodies_only() {
        let f = feature_link_from_json(&gql(
            "",
            "[]",
            r##"[{"number":10,"title":"parent","url":"u10","state":"OPEN","body":"Sub-task of #10 appears in my own text"},
                {"number":21,"title":"A","url":"u21","state":"OPEN","body":"Dependencies:\n- Sub-task of #10\n"},
                {"number":22,"title":"other parent","url":"u22","state":"OPEN","body":"Sub-task of #100"},
                {"number":23,"title":"loose hit","url":"u23","state":"OPEN","body":"mentions #10 only"},
                {},
                {"number":20,"title":"B","url":"u20","state":"CLOSED","stateReason":"COMPLETED","body":"sub-task of #10."}]"##,
        ))
        .unwrap();
        assert_eq!(f.linked_by, LinkedBy::Body);
        assert_eq!(f.children.iter().map(|c| c.number).collect::<Vec<_>>(), vec![20, 21]);
        assert!(f.children[0].closed);
    }

    #[test]
    fn not_planned_child_is_flagged() {
        let f = feature_link_from_json(&gql(
            "",
            r##"[{"number":11,"title":"dropped","url":"u","state":"CLOSED","stateReason":"NOT_PLANNED"},
                {"number":12,"title":"dupe","url":"u","state":"CLOSED","stateReason":"DUPLICATE"},
                {"number":13,"title":"done","url":"u","state":"CLOSED","stateReason":"COMPLETED"}]"##,
            "[]",
        ))
        .unwrap();
        let flags: Vec<_> = f.children.iter().map(|c| (c.closed, c.not_planned)).collect();
        assert_eq!(flags, vec![(true, true), (true, true), (true, false)]);
    }

    #[test]
    fn sub_issues_win_over_subtask_bodies() {
        let f = feature_link_from_json(&gql(
            "",
            r##"[{"number":11,"title":"A","url":"u","state":"OPEN"}]"##,
            r##"[{"number":30,"title":"body child","url":"u","state":"OPEN","body":"Sub-task of #10"}]"##,
        ))
        .unwrap();
        assert_eq!(f.linked_by, LinkedBy::SubIssues);
        assert_eq!(f.children.iter().map(|c| c.number).collect::<Vec<_>>(), vec![11]);
    }

    #[test]
    fn parent_without_subtasks_or_bad_json_is_not_a_feature() {
        assert_eq!(feature_link_from_json(&gql("", "[]", "[]")), None);
        assert_eq!(feature_link_from_json(b"not json"), None);
        assert_eq!(feature_link_from_json(br#"{"data":{"repository":{"issue":null}}}"#), None);
        assert_eq!(feature_link_from_json(br#"{"errors":[{"message":"boom"}]}"#), None);
    }

    #[test]
    fn spec_link_from_parent_body() {
        let url = format!("{REPO}/issues/10");
        let f = feature_link_from_json(&gql(
            &format!("Spec: {REPO}/blob/main/docs/specs/checkout.md#goals.\nAlso {REPO}/blob/main/README.md"),
            r##"[{"number":11,"title":"A","url":"u","state":"OPEN"}]"##,
            "[]",
        ))
        .unwrap();
        assert_eq!(f.spec_url.as_deref(), Some(&*format!("{REPO}/blob/main/docs/specs/checkout.md#goals")));

        // A relative markdown link, made absolute; earlier in the body wins.
        assert_eq!(
            spec_link(&format!("See [the spec](./docs/specs/c.md) and {REPO}/blob/main/x.md"), &url),
            Some(format!("{REPO}/blob/HEAD/docs/specs/c.md"))
        );
        assert_eq!(
            spec_link(&format!("{REPO}/blob/main/x.md then [spec](docs/c.md)"), &url),
            Some(format!("{REPO}/blob/main/x.md"))
        );
        // Not a repo document: other repos, source files, issue links, code spans.
        assert_eq!(
            spec_link(
                &format!(
                    "https://github.com/other/repo/blob/main/a.md {REPO}/blob/main/src/x.rs \
                     {REPO}/issues/3 edit `docs/village.md`"
                ),
                &url
            ),
            None
        );
        assert_eq!(spec_link("", &url), None);
    }

    #[test]
    fn feature_parents_have_sub_issues_or_are_decomposed() {
        let json = br#"{"data":{"repository":{"issues":{"nodes":[
            {"number":346,"subIssuesSummary":{"total":6},"labels":{"nodes":[{"name":"enhancement"}]}},
            {"number":12,"subIssuesSummary":{"total":0},"labels":{"nodes":[{"name":"decomposed"}]}},
            {"number":9,"subIssuesSummary":{"total":0},"labels":{"nodes":[{"name":"bug"}]}},
            {"number":5}
        ]}}}}"#;
        assert_eq!(feature_parents_from_json(json), vec![12, 346]);
        assert!(feature_parents_from_json(b"oops").is_empty());
        assert!(feature_parents_from_json(br#"{"errors":[]}"#).is_empty());
    }

    #[test]
    fn linked_by_serializes_as_the_contract_says() {
        assert_eq!(serde_json::to_string(&LinkedBy::SubIssues).unwrap(), r#""sub-issues""#);
        assert_eq!(serde_json::to_string(&LinkedBy::Body).unwrap(), r#""body""#);
    }

    #[test]
    fn body_names_parent_matches_whole_numbers() {
        assert!(body_names_parent("- Sub-task of #34\n", 34));
        assert!(body_names_parent("## Sub-task of #34", 34));
        assert!(!body_names_parent("Sub-task of #346", 34));
        assert!(body_names_parent("Sub-task of #346 and Sub-task of #34.", 34));
        assert_eq!(subtask_search_query("o/r", 7), "repo:o/r is:issue \"Sub-task of #7\"");
    }
}
