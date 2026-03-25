use anyhow::{Context, Result};
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::model::issue::{GhIssueJson, GitHubIssue};
use crate::transport::ServerTransport;

/// Classified GitHub CLI errors.
#[derive(Debug, Clone)]
pub enum GhError {
    /// gh binary not installed
    NotInstalled,
    /// Authentication required (expired token, not logged in)
    AuthRequired(String),
    /// Repository not found on GitHub
    RepoNotFound(String),
    /// Transient error (network, timeout, etc.)
    Transient(String),
}

impl std::fmt::Display for GhError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GhError::NotInstalled => {
                let hint = if cfg!(target_os = "macos") {
                    "Install with: brew install gh"
                } else {
                    "Install from: https://cli.github.com/"
                };
                write!(f, "gh CLI not installed. {hint}")
            }
            GhError::AuthRequired(msg) => {
                write!(f, "GitHub auth required. Run: gh auth login ({msg})")
            }
            GhError::RepoNotFound(msg) => write!(f, "GitHub repo not found: {msg}"),
            GhError::Transient(msg) => write!(f, "GitHub error: {msg}"),
        }
    }
}

/// Classify a gh CLI stderr message into a GhError variant.
fn classify_gh_error(stderr: &str) -> GhError {
    let lower = stderr.to_lowercase();
    if lower.contains("not logged in")
        || lower.contains("token expired")
        || lower.contains("authentication")
        || lower.contains("auth login")
        || lower.contains("401")
    {
        GhError::AuthRequired(stderr.trim().to_string())
    } else if lower.contains("could not resolve to a repository")
        || lower.contains("repository not found")
    {
        GhError::RepoNotFound(stderr.trim().to_string())
    } else {
        GhError::Transient(stderr.trim().to_string())
    }
}

/// Check if gh is installed and authenticated.
/// Returns `None` if everything is OK, or `Some(GhError)` describing the problem.
pub async fn check_gh_auth(transport: &ServerTransport) -> Option<GhError> {
    if !transport.command_exists("gh").await {
        return Some(GhError::NotInstalled);
    }

    let output = transport
        .output(
            "gh",
            &["auth".to_string(), "status".to_string()],
            None,
        )
        .await;

    match output {
        Ok(o) if o.status.success() => None,
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            Some(GhError::AuthRequired(stderr.trim().to_string()))
        }
        Err(e) => Some(GhError::Transient(e.to_string())),
    }
}

/// Fetch open issues for the repo at the given path using `gh`.
pub async fn fetch_issues(
    transport: &ServerTransport,
    repo_path: &Path,
) -> std::result::Result<Vec<GitHubIssue>, GhError> {
    let output = transport
        .output(
            "gh",
            &[
                "issue".to_string(),
                "list".to_string(),
                "--state".to_string(),
                "open".to_string(),
                "--limit".to_string(),
                "100".to_string(),
                "--json".to_string(),
                "number,title,state,labels".to_string(),
            ],
            Some(repo_path),
        )
        .await
        .map_err(|e| GhError::Transient(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(classify_gh_error(&stderr));
    }

    parse_issues_json(&output.stdout).map_err(|e| GhError::Transient(e.to_string()))
}

/// Spawn a background task that periodically fetches issues.
/// Stops retrying on permanent errors (auth, repo not found) and sends a warning event.
pub fn spawn_issue_fetcher(
    transport: ServerTransport,
    repo_path: std::path::PathBuf,
    project_name: String,
    tx: mpsc::UnboundedSender<crate::event::Event>,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            match fetch_issues(&transport, &repo_path).await {
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
                Err(ref e @ (GhError::AuthRequired(_) | GhError::RepoNotFound(_) | GhError::NotInstalled)) => {
                    let message = e.to_string();
                    tracing::warn!("Stopping issue fetch for {project_name}: {message}");
                    tx.send(crate::event::Event::GhWarning {
                        project_name: project_name.clone(),
                        message,
                    })
                    .ok();
                    break; // Don't retry permanent errors
                }
                Err(GhError::Transient(msg)) => {
                    tracing::warn!("Failed to fetch issues for {project_name}: {msg}");
                }
            }
        }
    })
}

fn parse_issues_json(bytes: &[u8]) -> Result<Vec<GitHubIssue>> {
    let raw: Vec<GhIssueJson> = serde_json::from_slice(bytes)
        .context("Failed to parse gh issue list JSON")?;

    Ok(raw.into_iter().map(GitHubIssue::from).collect())
}

#[cfg(test)]
mod tests {
    use super::{classify_gh_error, parse_issues_json, GhError};
    use crate::model::issue::IssueState;

    #[test]
    fn parses_gh_issue_json_into_issue_models() {
        let issues = parse_issues_json(
            br#"[{
                "number": 12,
                "title": "Fix reconnect bootstrap",
                "state": "OPEN",
                "labels": [{"name":"P1"},{"name":"working"}]
            }]"#,
        )
        .unwrap();

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].number, 12);
        assert_eq!(issues[0].state, IssueState::Open);
        assert!(issues[0].labels.contains(&"P1".to_string()));
    }

    #[test]
    fn rejects_invalid_issue_json() {
        assert!(parse_issues_json(br#"{"not":"an array"}"#).is_err());
    }

    #[test]
    fn classifies_auth_errors() {
        assert!(matches!(
            classify_gh_error("To get started with GitHub CLI, please run:  gh auth login"),
            GhError::AuthRequired(_)
        ));
        assert!(matches!(
            classify_gh_error("token expired"),
            GhError::AuthRequired(_)
        ));
        assert!(matches!(
            classify_gh_error("HTTP 401: Bad credentials"),
            GhError::AuthRequired(_)
        ));
    }

    #[test]
    fn classifies_repo_not_found() {
        assert!(matches!(
            classify_gh_error("GraphQL: Could not resolve to a Repository with the name 'org/repo'. (repository)"),
            GhError::RepoNotFound(_)
        ));
    }

    #[test]
    fn classifies_transient_errors() {
        assert!(matches!(
            classify_gh_error("error connecting to api.github.com"),
            GhError::Transient(_)
        ));
    }
}
