use anyhow::{Context, Result};
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::model::issue::{GhIssueJson, GitHubIssue};

/// Fetch open issues for the repo at the given path using `gh`.
pub async fn fetch_issues(repo_path: &Path) -> Result<Vec<GitHubIssue>> {
    let output = Command::new("gh")
        .args([
            "issue",
            "list",
            "--state",
            "open",
            "--limit",
            "100",
            "--json",
            "number,title,state,labels",
        ])
        .current_dir(repo_path)
        .output()
        .await
        .context("Failed to run gh issue list")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gh issue list failed: {stderr}");
    }

    let raw: Vec<GhIssueJson> = serde_json::from_slice(&output.stdout)
        .context("Failed to parse gh issue list JSON")?;

    let issues: Vec<GitHubIssue> = raw.into_iter().map(GitHubIssue::from).collect();
    Ok(issues)
}

/// Create a new issue on the repo.
pub async fn create_issue(repo_path: &Path, title: &str, body: &str, labels: &[&str]) -> Result<u32> {
    let mut args = vec![
        "issue".to_string(),
        "create".to_string(),
        "--title".to_string(),
        title.to_string(),
        "--body".to_string(),
        body.to_string(),
    ];
    for label in labels {
        args.push("--label".to_string());
        args.push(label.to_string());
    }

    let output = Command::new("gh")
        .args(&args)
        .current_dir(repo_path)
        .output()
        .await
        .context("Failed to run gh issue create")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gh issue create failed: {stderr}");
    }

    // gh outputs the issue URL, extract the number from it
    let stdout = String::from_utf8_lossy(&output.stdout);
    let number = stdout
        .trim()
        .rsplit('/')
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    Ok(number)
}

/// Spawn a background task that periodically fetches issues.
pub fn spawn_issue_fetcher(
    repo_path: std::path::PathBuf,
    project_name: String,
    tx: mpsc::UnboundedSender<crate::event::Event>,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            match fetch_issues(&repo_path).await {
                Ok(issues) => {
                    if tx
                        .send(crate::event::Event::IssuesUpdated {
                            project_name: project_name.clone(),
                            issues,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch issues for {project_name}: {e}");
                }
            }
        }
    })
}
