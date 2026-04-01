use anyhow::{Context, Result};
use serde_json::Value;
use tokio::process::Command;

use crate::model::issue::{GithubIssue, parse_issues};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueDetail {
    pub number: u32,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
    pub state: String,
}

pub async fn list_open_issues(limit: u32) -> Result<Vec<GithubIssue>> {
    let limit = limit.to_string();
    let output = run_gh(&[
        "issue",
        "list",
        "--state",
        "open",
        "--json",
        "number,title,labels",
        "--limit",
        &limit,
    ])
    .await?;

    Ok(parse_issues(&output.stdout))
}

pub async fn fetch_issue_detail(issue_number: u32) -> Result<IssueDetail> {
    let issue_number_arg = issue_number.to_string();
    let output = run_gh(&[
        "issue",
        "view",
        &issue_number_arg,
        "--json",
        "number,title,body,labels,state",
    ])
    .await?;

    parse_issue_detail(&output.stdout, issue_number)
}

pub async fn remove_issue_label(issue_number: u32, label: &str) -> Result<()> {
    let issue_number_arg = issue_number.to_string();
    run_gh(&["issue", "edit", &issue_number_arg, "--remove-label", label]).await?;
    Ok(())
}

pub fn open_issue_in_browser(issue_number: u32) -> Result<()> {
    let issue_number_arg = issue_number.to_string();
    Command::new("gh")
        .args(["issue", "view", "--web", &issue_number_arg])
        .spawn()
        .with_context(|| format!("Failed to run gh issue view --web for #{issue_number}"))?;
    Ok(())
}

fn parse_issue_detail(stdout: &[u8], fallback_number: u32) -> Result<IssueDetail> {
    let json: Value = serde_json::from_slice(stdout)
        .with_context(|| format!("Failed to parse gh issue JSON for #{fallback_number}"))?;

    let number = json["number"]
        .as_u64()
        .map(|n| n as u32)
        .unwrap_or(fallback_number);
    let title = json["title"].as_str().unwrap_or("").to_string();
    let body = json["body"].as_str().unwrap_or("").to_string();
    let state = json["state"].as_str().unwrap_or("OPEN").to_string();
    let labels = json["labels"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|l| l["name"].as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default();

    Ok(IssueDetail {
        number,
        title,
        body,
        labels,
        state,
    })
}

async fn run_gh(args: &[&str]) -> Result<std::process::Output> {
    let output = Command::new("gh")
        .args(args)
        .output()
        .await
        .with_context(|| format!("Failed to run gh {}", args.join(" ")))?;

    if output.status.success() {
        return Ok(output);
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        anyhow::bail!(stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        anyhow::bail!(stdout);
    }

    anyhow::bail!("gh {} failed with status {}", args.join(" "), output.status);
}

#[cfg(test)]
mod tests {
    use super::parse_issue_detail;

    #[test]
    fn parse_issue_detail_reads_expected_fields() {
        let raw = br#"{
            "number": 42,
            "title": "Example issue",
            "body": "Details",
            "state": "OPEN",
            "labels": [{"name":"enhancement"},{"name":"P3"}]
        }"#;

        let detail = parse_issue_detail(raw, 1).expect("detail should parse");

        assert_eq!(detail.number, 42);
        assert_eq!(detail.title, "Example issue");
        assert_eq!(detail.body, "Details");
        assert_eq!(detail.state, "OPEN");
        assert_eq!(detail.labels, vec!["enhancement", "P3"]);
    }

    #[test]
    fn parse_issue_detail_uses_fallback_defaults() {
        let raw = br#"{"labels":[{"id":"1"}]}"#;

        let detail = parse_issue_detail(raw, 99).expect("detail should parse");

        assert_eq!(detail.number, 99);
        assert_eq!(detail.title, "");
        assert_eq!(detail.body, "");
        assert_eq!(detail.state, "OPEN");
        assert!(detail.labels.is_empty());
    }
}
