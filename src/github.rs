use anyhow::{Context, Result};
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::model::issue::{
    FEATURE_LINK_QUERY, FEATURE_PARENTS_QUERY, FeatureLink, GhIssueJson, GitHubIssue,
    feature_link_from_json, feature_parents_from_json, subtask_search_query,
};
use crate::project_management::ProjectManagementClient;
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

/// The `https://github.com/<owner>/<repo>` page for a git remote URL, in any
/// of the forms git accepts for GitHub (`https://…(.git)`, `git@github.com:…`,
/// `ssh://git@github.com/…`). `None` for remotes that aren't on GitHub.
pub fn repo_web_url(remote: &str) -> Option<String> {
    let remote = remote.trim();
    let path = ["https://github.com/", "http://github.com/", "git@github.com:", "ssh://git@github.com/"]
        .iter()
        .find_map(|prefix| remote.strip_prefix(prefix))?;
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut parts = path.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(owner), Some(repo), None) if !owner.is_empty() && !repo.is_empty() => {
            Some(format!("https://github.com/{owner}/{repo}"))
        }
        _ => None,
    }
}

/// The GitHub page of the repo at `repo_path`, from its `origin` remote.
/// `None` when there is no origin or it isn't on GitHub.
pub async fn origin_web_url(transport: &ServerTransport, repo_path: &Path) -> Option<String> {
    let args = ["remote", "get-url", "origin"].map(String::from);
    let output = transport.output("git", &args, Some(repo_path)).await.ok()?;
    output
        .status
        .success()
        .then(|| repo_web_url(&String::from_utf8_lossy(&output.stdout)))
        .flatten()
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
            // `body` for the dependency lines (see `parse_dependencies`).
            "number,title,state,labels,url,body".to_string(),
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

/// Recently closed issues: what landed work was, once its issue closed.
pub async fn fetch_closed_issues(transport: &ServerTransport, repo_path: &Path) -> std::result::Result<Vec<GitHubIssue>, GhError> {
    let args: Vec<String> = ["issue", "list", "--state", "closed", "--limit", "200", "--json", "number,title,state,labels,url"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let output = gh_repo_output(transport, repo_path, &args).await.map_err(|e| GhError::Transient(e.to_string()))?;
    if !output.status.success() {
        return Err(classify_gh_error(&String::from_utf8_lossy(&output.stderr)));
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

/// The `gh` arguments that read one parent's feature link: a single GraphQL
/// round trip returning its sub-issues and the `Sub-task of #N` search hits.
/// `gh` fills in `{owner}` and `{repo}` from the repository it runs in.
fn feature_link_args(parent: u32) -> Vec<String> {
    let field = |k: &str, v: String| ["-F".to_string(), format!("{k}={v}")];
    let mut args = vec!["api".to_string(), "graphql".to_string()];
    args.extend(field("owner", "{owner}".into()));
    args.extend(field("name", "{repo}".into()));
    args.extend(field("number", parent.to_string()));
    args.extend(field("q", subtask_search_query("{owner}/{repo}", parent)));
    args.extend(["-f".to_string(), format!("query={FEATURE_LINK_QUERY}")]);
    args
}

/// Which subtasks make the feature whose parent issue is `parent`. `None` when
/// the parent has no subtasks, or when `gh` fails for any reason: a feature
/// that can't be read is left out, never an error.
pub async fn fetch_feature_link(
    transport: &ServerTransport,
    repo_path: &Path,
    parent: u32,
) -> Option<FeatureLink> {
    let output = gh_repo_output(transport, repo_path, &feature_link_args(parent))
        .await
        .map_err(|e| tracing::debug!("feature link #{parent}: {e}"))
        .ok()?;
    if !output.status.success() {
        tracing::debug!(
            "feature link #{parent}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        return None;
    }
    feature_link_from_json(&output.stdout)
}

/// The repository's open feature parents: issues with sub-issues, or labelled
/// `decomposed`. One GraphQL round trip; none when `gh` fails.
pub async fn fetch_feature_parents(transport: &ServerTransport, repo_path: &Path) -> Vec<u32> {
    let args: Vec<String> = [
        "api",
        "graphql",
        "-F",
        "owner={owner}",
        "-F",
        "name={repo}",
        "-f",
    ]
    .iter()
    .map(|s| s.to_string())
    .chain(std::iter::once(format!("query={FEATURE_PARENTS_QUERY}")))
    .collect();
    match gh_repo_output(transport, repo_path, &args).await {
        Ok(o) if o.status.success() => feature_parents_from_json(&o.stdout),
        Ok(o) => {
            tracing::debug!("feature parents: {}", String::from_utf8_lossy(&o.stderr).trim());
            Vec::new()
        }
        Err(e) => {
            tracing::debug!("feature parents: {e}");
            Vec::new()
        }
    }
}

/// Feature links reused for a TTL, so each parent costs at most one `gh`
/// round trip per TTL however often the village asks.
pub struct FeatureLinks {
    cache: crate::web::source::TtlCache<(std::path::PathBuf, u32), Option<FeatureLink>>,
}

impl FeatureLinks {
    pub fn new(ttl: Duration) -> Self {
        Self { cache: crate::web::source::TtlCache::new(ttl) }
    }

    /// Feature links for these parents, skipping any that aren't features or
    /// couldn't be read.
    pub async fn get(
        &self,
        transport: &ServerTransport,
        repo_path: &Path,
        parents: &[u32],
    ) -> Vec<FeatureLink> {
        let mut out = Vec::new();
        for &parent in parents {
            let key = (repo_path.to_path_buf(), parent);
            if let Some(link) = self
                .cache
                .get_or(key, || fetch_feature_link(transport, repo_path, parent))
                .await
            {
                out.push(link);
            }
        }
        out
    }
}

fn parse_issues_json(bytes: &[u8]) -> Result<Vec<GitHubIssue>> {
    let raw: Vec<GhIssueJson> =
        serde_json::from_slice(bytes).context("Failed to parse gh issue list JSON")?;

    Ok(raw.into_iter().map(GitHubIssue::from).collect())
}

#[cfg(test)]
mod tests {
    use super::{
        GhError, classify_gh_error, feature_link_args, parse_issues_json, repo_owner_from_remote,
        repo_web_url,
    };
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

    #[test]
    fn repo_web_url_handles_https_and_ssh_remotes() {
        let want = Some("https://github.com/laird/agents-ui".to_string());
        assert_eq!(repo_web_url("https://github.com/laird/agents-ui.git\n"), want);
        assert_eq!(repo_web_url("https://github.com/laird/agents-ui"), want);
        assert_eq!(repo_web_url("https://github.com/laird/agents-ui/"), want);
        assert_eq!(repo_web_url("git@github.com:laird/agents-ui.git"), want);
        assert_eq!(repo_web_url("git@github.com:laird/agents-ui"), want);
        assert_eq!(repo_web_url("ssh://git@github.com/laird/agents-ui.git"), want);
    }

    #[test]
    fn repo_web_url_is_none_off_github_or_without_a_repo() {
        assert_eq!(repo_web_url(""), None);
        assert_eq!(repo_web_url("/home/me/src/shop"), None);
        assert_eq!(repo_web_url("git@gitlab.com:acme/shop.git"), None);
        assert_eq!(repo_web_url("https://github.com/acme"), None);
        assert_eq!(repo_web_url("https://github.com/acme/shop/issues/3"), None);
    }

    #[test]
    fn parsed_issues_keep_the_url_gh_reports() {
        let issues = parse_issues_json(
            br#"[{"number": 358, "title": "t", "state": "OPEN", "labels": [],
                  "url": "https://github.com/laird/agents-ui/issues/358"},
                 {"number": 2, "title": "u", "state": "OPEN", "labels": []}]"#,
        )
        .unwrap();
        assert_eq!(issues[0].url.as_deref(), Some("https://github.com/laird/agents-ui/issues/358"));
        assert_eq!(issues[1].url, None);
    }

    #[test]
    fn feature_link_args_are_one_graphql_call_scoped_to_the_repo() {
        let args = feature_link_args(346);
        assert_eq!(&args[..2], ["api", "graphql"]);
        assert!(args.contains(&"owner={owner}".to_string()));
        assert!(args.contains(&"name={repo}".to_string()));
        assert!(args.contains(&"number=346".to_string()));
        assert!(args.contains(&"q=repo:{owner}/{repo} is:issue \"Sub-task of #346\"".to_string()));
        assert!(args.iter().any(|a| a.starts_with("query=query(") && a.contains("subIssues")));
    }
}
