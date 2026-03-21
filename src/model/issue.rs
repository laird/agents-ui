use anyhow::Result;
use std::path::Path;
use tokio::process::Command;

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

/// A cached GitHub issue.
#[derive(Debug, Clone)]
pub struct GitHubIssue {
    pub number: u32,
    pub title: String,
    pub priority: IssuePriority,
    pub issue_type: IssueType,
    pub labels: Vec<String>,
    pub is_working: bool,
}

/// Cache of GitHub issues for a swarm.
#[derive(Debug, Clone)]
pub struct IssueCache {
    pub issues: Vec<GitHubIssue>,
    pub is_loading: bool,
}

impl Default for IssueCache {
    fn default() -> Self {
        Self {
            issues: Vec::new(),
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

/// Fetch open issues from GitHub using `gh` CLI.
pub async fn fetch_issues(repo_path: &Path) -> Result<Vec<GitHubIssue>> {
    let output = Command::new("gh")
        .args([
            "issue",
            "list",
            "--state",
            "open",
            "--json",
            "number,title,labels",
            "--limit",
            "100",
        ])
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

            let priority = if labels.iter().any(|l| l == "P0") {
                IssuePriority::P0
            } else if labels.iter().any(|l| l == "P1") {
                IssuePriority::P1
            } else if labels.iter().any(|l| l == "P2") {
                IssuePriority::P2
            } else if labels.iter().any(|l| l == "P3") {
                IssuePriority::P3
            } else {
                IssuePriority::None
            };

            let issue_type = if labels.iter().any(|l| l == "proposal") {
                IssueType::Proposal
            } else if labels.iter().any(|l| l == "enhancement") {
                IssueType::Enhancement
            } else if labels.iter().any(|l| l == "bug") {
                IssueType::Bug
            } else {
                IssueType::Other
            };

            let is_working = labels.iter().any(|l| l == "working");

            issues.push(GitHubIssue {
                number,
                title,
                priority,
                issue_type,
                labels,
                is_working,
            });
        }
    }

    // Sort by priority then number
    issues.sort_by(|a, b| a.priority.cmp(&b.priority).then(a.number.cmp(&b.number)));

    Ok(issues)
}
