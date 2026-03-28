/// A GitHub issue with priority derived from labels.
#[derive(Debug, Clone)]
pub struct GithubIssue {
    pub number: u32,
    pub title: String,
    /// Numeric priority: 0=P0, 1=P1, 2=P2, 3=P3, 9=none
    pub priority: u8,
    pub labels: Vec<String>,
}

impl GithubIssue {
    /// True if any label matches blocking categories.
    pub fn is_blocked(&self) -> bool {
        self.labels.iter().any(|l| {
            matches!(
                l.as_str(),
                "needs-approval"
                    | "needs-design"
                    | "needs-clarification"
                    | "too-complex"
                    | "future"
            )
        })
    }

    pub fn priority_label(&self) -> &str {
        match self.priority {
            0 => "P0",
            1 => "P1",
            2 => "P2",
            3 => "P3",
            _ => "  ",
        }
    }
}

/// Parse and sort a list of issues from `gh issue list --json number,title,labels` output.
pub fn parse_issues(json: &[u8]) -> Vec<GithubIssue> {
    let Ok(arr) = serde_json::from_slice::<Vec<serde_json::Value>>(json) else {
        return vec![];
    };

    let mut issues: Vec<GithubIssue> = arr
        .iter()
        .filter_map(|v| {
            let number = v["number"].as_u64()? as u32;
            let title = v["title"].as_str()?.to_string();
            let labels: Vec<String> = v["labels"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|l| l["name"].as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let priority = if labels.iter().any(|l| l == "P0") {
                0
            } else if labels.iter().any(|l| l == "P1") {
                1
            } else if labels.iter().any(|l| l == "P2") {
                2
            } else if labels.iter().any(|l| l == "P3") {
                3
            } else {
                9
            };
            Some(GithubIssue { number, title, priority, labels })
        })
        .collect();

    // Sort: priority ascending, then issue number descending
    issues.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| b.number.cmp(&a.number))
    });
    issues
}
