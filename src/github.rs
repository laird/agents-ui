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
        .output("gh", &["auth".to_string(), "status".to_string()], None)
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

fn repo_owner_from_remote(remote: &str) -> Option<String> {
    if let Some(rest) = remote.trim().strip_prefix("https://github.com/") {
        return rest
            .split('/')
            .next()
            .filter(|s| !s.is_empty())
            .map(ToString::to_string);
    }
    if let Some(rest) = remote.trim().strip_prefix("git@github.com:") {
        return rest
            .split('/')
            .next()
            .filter(|s| !s.is_empty())
            .map(ToString::to_string);
    }
    None
}

/// Return the list of logged-in gh usernames (one per line from `gh auth status`).
async fn list_gh_users(transport: &ServerTransport) -> Vec<String> {
    let Ok(o) = transport
        .output(
            "gh",
            &["auth".to_string(), "status".to_string()],
            None,
        )
        .await
    else {
        return vec![];
    };
    // `gh auth status` prints to stderr; lines like "✓ Logged in to github.com account foo (keyring)"
    let text = String::from_utf8_lossy(&o.stderr);
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            // Match "Logged in to github.com account <username>"
            let marker = "account ";
            let pos = line.find(marker)?;
            let rest = &line[pos + marker.len()..];
            let user = rest.split_whitespace().next()?;
            if user.is_empty() { None } else { Some(user.to_string()) }
        })
        .collect()
}

/// Return the currently active gh username, or None on failure.
async fn current_gh_user(transport: &ServerTransport, repo_path: &Path) -> Option<String> {
    let o = transport
        .output(
            "gh",
            &[
                "api".to_string(),
                "user".to_string(),
                "--jq".to_string(),
                ".login".to_string(),
            ],
            Some(repo_path),
        )
        .await
        .ok()?;
    if o.status.success() {
        Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
    } else {
        None
    }
}

/// Switch gh to `user` and return true on success.
async fn switch_gh_user(transport: &ServerTransport, user: &str) -> bool {
    transport
        .output(
            "gh",
            &[
                "auth".to_string(),
                "switch".to_string(),
                "--user".to_string(),
                user.to_string(),
            ],
            None,
        )
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Return true if the current active gh account can reach `repo_path`'s origin repo.
async fn gh_can_access_repo(transport: &ServerTransport, repo_path: &Path) -> bool {
    transport
        .output(
            "gh",
            &[
                "repo".to_string(),
                "view".to_string(),
                "--json".to_string(),
                "name".to_string(),
            ],
            Some(repo_path),
        )
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Ensure `gh` is using a profile that can access the repo.
///
/// Strategy:
/// 1. If the current profile already works, do nothing.
/// 2. Try the profile whose username matches the repo owner.
/// 3. If that fails (org repo, member account, etc.), try every other configured profile.
/// 4. Log a warning only if no profile works.
pub async fn ensure_gh_auth_for_repo(transport: &ServerTransport, repo_path: &Path) {
    // Fast path: current profile already has access.
    if gh_can_access_repo(transport, repo_path).await {
        return;
    }

    let remote = match transport
        .output(
            "git",
            &[
                "remote".to_string(),
                "get-url".to_string(),
                "origin".to_string(),
            ],
            Some(repo_path),
        )
        .await
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => return,
    };

    let owner = repo_owner_from_remote(&remote);
    let current = current_gh_user(transport, repo_path).await;
    let all_users = list_gh_users(transport).await;

    // Build candidate list: owner-matching account first, then all others.
    let mut candidates: Vec<String> = Vec::new();
    if let Some(ref o) = owner {
        if all_users.iter().any(|u| u == o) {
            candidates.push(o.clone());
        }
    }
    for u in &all_users {
        if Some(u) != current.as_ref() && !candidates.contains(u) {
            candidates.push(u.clone());
        }
    }

    for user in &candidates {
        tracing::info!(
            "Trying gh profile '{user}' for repo at {}",
            repo_path.display()
        );
        if switch_gh_user(transport, user).await && gh_can_access_repo(transport, repo_path).await {
            tracing::info!("Switched gh auth to '{user}' for repo at {}", repo_path.display());
            return;
        }
    }

    tracing::warn!(
        "No configured gh profile can access repo at {} (tried: {})",
        repo_path.display(),
        candidates.join(", ")
    );
}

/// Run a repo-scoped `gh` command after ensuring the matching GitHub profile is active.
pub async fn gh_repo_output(
    transport: &ServerTransport,
    repo_path: &Path,
    args: &[String],
) -> Result<std::process::Output> {
    ensure_gh_auth_for_repo(transport, repo_path).await;
    transport.output("gh", args, Some(repo_path)).await
}

/// Fetch open issues for the repo at the given path using `gh`.
pub async fn fetch_issues(
    transport: &ServerTransport,
    repo_path: &Path,
) -> std::result::Result<Vec<GitHubIssue>, GhError> {
    let output = gh_repo_output(
        transport,
        repo_path,
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
                        tracing::debug!(
                            "Issue fetcher channel closed for {}, stopping watcher",
                            project_name
                        );
                        break;
                    }
                }
                Err(
                    ref e @ (GhError::AuthRequired(_)
                    | GhError::RepoNotFound(_)
                    | GhError::NotInstalled),
                ) => {
                    let message = e.to_string();
                    tracing::warn!("Stopping issue fetch for {project_name}: {message}");
                    if let Err(send_err) = tx.send(crate::event::Event::GhWarning {
                        project_name: project_name.clone(),
                        message,
                    }) {
                        tracing::debug!(
                            "Failed to send gh warning event for {}: {}",
                            project_name,
                            send_err
                        );
                    }
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
    let raw: Vec<GhIssueJson> =
        serde_json::from_slice(bytes).context("Failed to parse gh issue list JSON")?;

    Ok(raw.into_iter().map(GitHubIssue::from).collect())
}

#[cfg(test)]
mod tests {
    use super::{GhError, classify_gh_error, parse_issues_json, repo_owner_from_remote};
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
    fn parses_updated_at_from_gh_json() {
        let issues = parse_issues_json(
            br#"[{
                "number": 5,
                "title": "Recent issue",
                "state": "OPEN",
                "labels": [],
                "updatedAt": "2024-01-15T10:30:00Z"
            }]"#,
        )
        .unwrap();
        assert!(issues[0].updated_at.is_some());
    }

    #[test]
    fn parses_missing_updated_at_as_none() {
        let issues = parse_issues_json(
            br#"[{
                "number": 6,
                "title": "Old format issue",
                "state": "OPEN",
                "labels": []
            }]"#,
        )
        .unwrap();
        assert!(issues[0].updated_at.is_none());
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
            classify_gh_error(
                "GraphQL: Could not resolve to a Repository with the name 'org/repo'. (repository)"
            ),
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

    #[test]
    fn extracts_repo_owner_from_supported_remote_urls() {
        assert_eq!(
            repo_owner_from_remote("https://github.com/acme/widgets.git"),
            Some("acme".to_string())
        );
        assert_eq!(
            repo_owner_from_remote("git@github.com:acme/widgets.git"),
            Some("acme".to_string())
        );
        assert_eq!(
            repo_owner_from_remote("ssh://git.example.com/acme/widgets"),
            None
        );
    }
}
