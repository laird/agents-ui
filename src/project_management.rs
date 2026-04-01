use anyhow::{Context, Result, bail};
use tokio::process::Command;

#[derive(Debug, Clone, Copy)]
pub struct GitHubIssueRequest<'a> {
    pub repo: &'a str,
    pub title: &'a str,
    pub body: &'a str,
    pub label: &'a str,
}

/// Wrapper for GitHub CLI operations used by project-management flows.
pub struct GitHubCli;

impl GitHubCli {
    pub async fn create_issue(request: GitHubIssueRequest<'_>) -> Result<String> {
        let args = build_issue_create_args(&request);
        tracing::info!(
            repo = request.repo,
            label = request.label,
            "Creating issue via project-management GitHub wrapper"
        );

        let output = Command::new("gh")
            .args(&args)
            .output()
            .await
            .with_context(|| "Failed to execute `gh issue create`")?;

        if output.status.success() {
            let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if url.is_empty() {
                bail!("`gh issue create` succeeded but returned no issue URL");
            }
            return Ok(url);
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let message = if stderr.is_empty() {
            "`gh issue create` failed with no stderr output".to_string()
        } else {
            stderr
        };

        if looks_like_auth_error(&message) {
            bail!("GitHub CLI authentication required (`gh auth login`): {message}");
        }

        bail!("{message}");
    }
}

fn build_issue_create_args(request: &GitHubIssueRequest<'_>) -> Vec<String> {
    vec![
        "issue".to_string(),
        "create".to_string(),
        "--repo".to_string(),
        request.repo.to_string(),
        "--label".to_string(),
        request.label.to_string(),
        "--title".to_string(),
        request.title.to_string(),
        "--body".to_string(),
        request.body.to_string(),
    ]
}

fn looks_like_auth_error(message: &str) -> bool {
    let normalized = message.to_lowercase();
    normalized.contains("not logged into any github hosts")
        || normalized.contains("authentication failed")
        || normalized.contains("gh auth login")
}

#[cfg(test)]
mod tests {
    use super::{GitHubIssueRequest, build_issue_create_args, looks_like_auth_error};

    #[test]
    fn builds_expected_issue_create_args() {
        let request = GitHubIssueRequest {
            repo: "laird/agents-ui",
            title: "Title",
            body: "Body",
            label: "bug",
        };

        let args = build_issue_create_args(&request);

        assert_eq!(
            args,
            vec![
                "issue",
                "create",
                "--repo",
                "laird/agents-ui",
                "--label",
                "bug",
                "--title",
                "Title",
                "--body",
                "Body",
            ]
        );
    }

    #[test]
    fn detects_auth_errors_from_gh_output() {
        assert!(looks_like_auth_error(
            "You are not logged into any GitHub hosts. Run gh auth login."
        ));
        assert!(looks_like_auth_error("Authentication failed"));
        assert!(!looks_like_auth_error("Validation Failed"));
    }
}
