use serde::Deserialize;
use serde_json::json;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::github::GhError;
use crate::model::issue::{GitHubIssue, IssuePriority, IssueState, IssueType};
use crate::transport::ServerTransport;

const BACKEND_ENV_VAR: &str = "AGENTS_PROJECT_BACKEND";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectManagementBackend {
    GitHub,
    Linear,
    Jira,
}

impl ProjectManagementBackend {
    fn as_str(self) -> &'static str {
        match self {
            Self::GitHub => "GitHub",
            Self::Linear => "Linear",
            Self::Jira => "Jira",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "github" | "gh" => Some(Self::GitHub),
            "linear" => Some(Self::Linear),
            "jira" => Some(Self::Jira),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ProjectManagementError {
    NotInstalled {
        backend: ProjectManagementBackend,
        message: String,
    },
    AuthRequired {
        backend: ProjectManagementBackend,
        message: String,
    },
    RepoNotFound {
        backend: ProjectManagementBackend,
        message: String,
    },
    Config {
        backend: ProjectManagementBackend,
        message: String,
    },
    Transient {
        backend: ProjectManagementBackend,
        message: String,
    },
}

impl ProjectManagementError {
    fn backend(&self) -> ProjectManagementBackend {
        match self {
            Self::NotInstalled { backend, .. }
            | Self::AuthRequired { backend, .. }
            | Self::RepoNotFound { backend, .. }
            | Self::Config { backend, .. }
            | Self::Transient { backend, .. } => *backend,
        }
    }

    fn message(&self) -> &str {
        match self {
            Self::NotInstalled { message, .. }
            | Self::AuthRequired { message, .. }
            | Self::RepoNotFound { message, .. }
            | Self::Config { message, .. }
            | Self::Transient { message, .. } => message,
        }
    }
}

impl std::fmt::Display for ProjectManagementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let backend = self.backend().as_str();
        match self {
            Self::NotInstalled { .. } => {
                write!(
                    f,
                    "{backend} integration is not installed: {}",
                    self.message()
                )
            }
            Self::AuthRequired { .. } => {
                write!(f, "{backend} authentication required: {}", self.message())
            }
            Self::RepoNotFound { .. } => {
                write!(f, "{backend} project not found: {}", self.message())
            }
            Self::Config { .. } => {
                write!(f, "{backend} configuration required: {}", self.message())
            }
            Self::Transient { .. } => write!(f, "{backend} error: {}", self.message()),
        }
    }
}

impl From<GhError> for ProjectManagementError {
    fn from(value: GhError) -> Self {
        match value {
            GhError::NotInstalled => Self::NotInstalled {
                backend: ProjectManagementBackend::GitHub,
                message: "Install `gh` and run `gh auth login`".to_string(),
            },
            GhError::AuthRequired(msg) => Self::AuthRequired {
                backend: ProjectManagementBackend::GitHub,
                message: msg,
            },
            GhError::RepoNotFound(msg) => Self::RepoNotFound {
                backend: ProjectManagementBackend::GitHub,
                message: msg,
            },
            GhError::Transient(msg) => Self::Transient {
                backend: ProjectManagementBackend::GitHub,
                message: msg,
            },
        }
    }
}

#[derive(Debug, Deserialize)]
struct AgentsUiConfig {
    project_management_backend: Option<String>,
}

fn backend_from_repo_config(repo_path: &Path) -> Option<ProjectManagementBackend> {
    let config_path = repo_path.join(".agents-ui.toml");
    let raw = std::fs::read_to_string(config_path).ok()?;
    let parsed: AgentsUiConfig = toml::from_str(&raw).ok()?;
    parsed
        .project_management_backend
        .as_deref()
        .and_then(ProjectManagementBackend::parse)
}

pub fn resolve_backend(repo_path: &Path) -> ProjectManagementBackend {
    if let Ok(value) = std::env::var(BACKEND_ENV_VAR) {
        if let Some(backend) = ProjectManagementBackend::parse(&value) {
            return backend;
        }
    }

    backend_from_repo_config(repo_path).unwrap_or(ProjectManagementBackend::GitHub)
}

pub async fn check_auth(
    transport: &ServerTransport,
    repo_path: &Path,
) -> Option<ProjectManagementError> {
    match resolve_backend(repo_path) {
        ProjectManagementBackend::GitHub => crate::github::check_gh_auth(transport)
            .await
            .map(ProjectManagementError::from),
        ProjectManagementBackend::Linear => match linear_credentials(transport, repo_path).await {
            Ok(_) => None,
            Err(e) => Some(e),
        },
        ProjectManagementBackend::Jira => match jira_credentials(transport, repo_path).await {
            Ok(_) => None,
            Err(e) => Some(e),
        },
    }
}

pub async fn fetch_issues(
    transport: &ServerTransport,
    repo_path: &Path,
) -> std::result::Result<Vec<GitHubIssue>, ProjectManagementError> {
    match resolve_backend(repo_path) {
        ProjectManagementBackend::GitHub => crate::github::fetch_issues(transport, repo_path)
            .await
            .map_err(ProjectManagementError::from),
        ProjectManagementBackend::Linear => fetch_linear_issues(transport, repo_path).await,
        ProjectManagementBackend::Jira => fetch_jira_issues(transport, repo_path).await,
    }
}

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
                Err(
                    ref e @ (ProjectManagementError::AuthRequired { .. }
                    | ProjectManagementError::RepoNotFound { .. }
                    | ProjectManagementError::NotInstalled { .. }
                    | ProjectManagementError::Config { .. }),
                ) => {
                    let message = e.to_string();
                    tracing::warn!("Stopping issue fetch for {project_name}: {message}");
                    tx.send(crate::event::Event::GhWarning {
                        project_name: project_name.clone(),
                        message,
                    })
                    .ok();
                    break;
                }
                Err(ProjectManagementError::Transient { message, .. }) => {
                    tracing::warn!("Failed to fetch issues for {project_name}: {message}");
                }
            }
        }
    })
}

async fn fetch_linear_issues(
    transport: &ServerTransport,
    repo_path: &Path,
) -> std::result::Result<Vec<GitHubIssue>, ProjectManagementError> {
    if !transport.command_exists("curl").await {
        return Err(ProjectManagementError::NotInstalled {
            backend: ProjectManagementBackend::Linear,
            message: "Install curl to use Linear integration".to_string(),
        });
    }

    let creds = linear_credentials(transport, repo_path).await?;
    let payload = json!({
        "query": "query { issues(first: 100) { nodes { identifier title priority updatedAt state { name type } labels { nodes { name } } } } }"
    })
    .to_string();

    let output = transport
        .output(
            "curl",
            &[
                "-sS".to_string(),
                "https://api.linear.app/graphql".to_string(),
                "-H".to_string(),
                "Content-Type: application/json".to_string(),
                "-H".to_string(),
                format!("Authorization: {}", creds.api_key),
                "-d".to_string(),
                payload,
            ],
            Some(repo_path),
        )
        .await
        .map_err(|e| ProjectManagementError::Transient {
            backend: ProjectManagementBackend::Linear,
            message: e.to_string(),
        })?;

    if !output.status.success() {
        return Err(ProjectManagementError::Transient {
            backend: ProjectManagementBackend::Linear,
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    parse_linear_issues(&output.stdout)
}

async fn fetch_jira_issues(
    transport: &ServerTransport,
    repo_path: &Path,
) -> std::result::Result<Vec<GitHubIssue>, ProjectManagementError> {
    if !transport.command_exists("curl").await {
        return Err(ProjectManagementError::NotInstalled {
            backend: ProjectManagementBackend::Jira,
            message: "Install curl to use Jira integration".to_string(),
        });
    }

    let creds = jira_credentials(transport, repo_path).await?;
    let base = creds.base_url.trim_end_matches('/');
    let jql = format!(
        "project={} AND statusCategory != Done ORDER BY priority DESC, updated DESC",
        creds.project_key
    );
    let output = transport
        .output(
            "curl",
            &[
                "-sS".to_string(),
                "--get".to_string(),
                format!("{base}/rest/api/3/search"),
                "--user".to_string(),
                format!("{}:{}", creds.email, creds.api_token),
                "--data-urlencode".to_string(),
                format!("jql={jql}"),
                "--data-urlencode".to_string(),
                "fields=summary,status,priority,labels,issuetype,updated".to_string(),
                "--data-urlencode".to_string(),
                "maxResults=100".to_string(),
            ],
            Some(repo_path),
        )
        .await
        .map_err(|e| ProjectManagementError::Transient {
            backend: ProjectManagementBackend::Jira,
            message: e.to_string(),
        })?;

    if !output.status.success() {
        return Err(ProjectManagementError::Transient {
            backend: ProjectManagementBackend::Jira,
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    parse_jira_issues(&output.stdout)
}

#[derive(Debug)]
struct LinearCredentials {
    api_key: String,
}

async fn linear_credentials(
    transport: &ServerTransport,
    repo_path: &Path,
) -> std::result::Result<LinearCredentials, ProjectManagementError> {
    let api_key = read_env_var(transport, repo_path, "LINEAR_API_KEY")
        .await?
        .ok_or(ProjectManagementError::AuthRequired {
            backend: ProjectManagementBackend::Linear,
            message: "Set LINEAR_API_KEY in your environment".to_string(),
        })?;
    Ok(LinearCredentials { api_key })
}

#[derive(Debug)]
struct JiraCredentials {
    base_url: String,
    email: String,
    api_token: String,
    project_key: String,
}

async fn jira_credentials(
    transport: &ServerTransport,
    repo_path: &Path,
) -> std::result::Result<JiraCredentials, ProjectManagementError> {
    let base_url = read_env_var(transport, repo_path, "JIRA_BASE_URL")
        .await?
        .ok_or(ProjectManagementError::Config {
            backend: ProjectManagementBackend::Jira,
            message: "Set JIRA_BASE_URL in your environment".to_string(),
        })?;
    let email = read_env_var(transport, repo_path, "JIRA_EMAIL")
        .await?
        .ok_or(ProjectManagementError::AuthRequired {
            backend: ProjectManagementBackend::Jira,
            message: "Set JIRA_EMAIL in your environment".to_string(),
        })?;
    let api_token = read_env_var(transport, repo_path, "JIRA_API_TOKEN")
        .await?
        .ok_or(ProjectManagementError::AuthRequired {
            backend: ProjectManagementBackend::Jira,
            message: "Set JIRA_API_TOKEN in your environment".to_string(),
        })?;
    let project_key = read_env_var(transport, repo_path, "JIRA_PROJECT_KEY")
        .await?
        .ok_or(ProjectManagementError::Config {
            backend: ProjectManagementBackend::Jira,
            message: "Set JIRA_PROJECT_KEY in your environment".to_string(),
        })?;
    Ok(JiraCredentials {
        base_url,
        email,
        api_token,
        project_key,
    })
}

async fn read_env_var(
    transport: &ServerTransport,
    repo_path: &Path,
    name: &str,
) -> std::result::Result<Option<String>, ProjectManagementError> {
    let output = transport
        .output(
            "sh",
            &["-lc".to_string(), format!("printf '%s' \"${{{name}:-}}\"")],
            Some(repo_path),
        )
        .await
        .map_err(|e| ProjectManagementError::Transient {
            backend: ProjectManagementBackend::GitHub,
            message: e.to_string(),
        })?;

    if !output.status.success() {
        return Ok(None);
    }

    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

fn parse_linear_issues(
    bytes: &[u8],
) -> std::result::Result<Vec<GitHubIssue>, ProjectManagementError> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| ProjectManagementError::Transient {
            backend: ProjectManagementBackend::Linear,
            message: format!("Failed to parse Linear response: {e}"),
        })?;

    if let Some(errors) = value.get("errors").and_then(|e| e.as_array()) {
        let msg = errors
            .iter()
            .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
            .collect::<Vec<_>>()
            .join("; ");
        let lower = msg.to_ascii_lowercase();
        if lower.contains("auth") || lower.contains("token") || lower.contains("permission") {
            return Err(ProjectManagementError::AuthRequired {
                backend: ProjectManagementBackend::Linear,
                message: msg,
            });
        }
        return Err(ProjectManagementError::Transient {
            backend: ProjectManagementBackend::Linear,
            message: msg,
        });
    }

    let nodes = value
        .pointer("/data/issues/nodes")
        .and_then(|n| n.as_array())
        .ok_or(ProjectManagementError::Transient {
            backend: ProjectManagementBackend::Linear,
            message: "Linear response missing data.issues.nodes".to_string(),
        })?;

    let mut issues = Vec::new();
    for node in nodes {
        let identifier = node
            .get("identifier")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if identifier.is_empty() {
            continue;
        }

        let state_type = node
            .pointer("/state/type")
            .and_then(|v| v.as_str())
            .unwrap_or("unstarted")
            .to_ascii_lowercase();
        if state_type == "completed" || state_type == "canceled" || state_type == "done" {
            continue;
        }

        let summary = node
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        let mut labels = vec!["linear".to_string(), format!("source-id:{identifier}")];

        let linear_labels = node
            .pointer("/labels/nodes")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|l| l.get("name").and_then(|n| n.as_str()))
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        labels.extend(linear_labels);

        if let Some(priority_label) =
            linear_priority_to_label(node.get("priority").and_then(|v| v.as_i64()))
        {
            labels.push(priority_label.to_string());
        }

        let updated_at = node
            .get("updatedAt")
            .and_then(|v| v.as_str())
            .and_then(parse_datetime_utc);

        issues.push(GitHubIssue {
            number: issue_number_from_external_id(&identifier),
            title: format!("{identifier}: {summary}"),
            state: IssueState::Open,
            priority: issue_priority_from_labels(&labels),
            issue_type: issue_type_from_labels(&labels),
            labels,
            is_working: false,
            assigned_worker: None,
            updated_at,
        });
    }

    issues.sort_by(|a, b| a.priority.cmp(&b.priority).then(a.number.cmp(&b.number)));
    Ok(issues)
}

fn parse_jira_issues(
    bytes: &[u8],
) -> std::result::Result<Vec<GitHubIssue>, ProjectManagementError> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| ProjectManagementError::Transient {
            backend: ProjectManagementBackend::Jira,
            message: format!("Failed to parse Jira response: {e}"),
        })?;

    if let Some(errors) = value.get("errorMessages").and_then(|e| e.as_array()) {
        let msg = errors
            .iter()
            .filter_map(|e| e.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        let lower = msg.to_ascii_lowercase();
        if lower.contains("auth") || lower.contains("permission") || lower.contains("unauthorized")
        {
            return Err(ProjectManagementError::AuthRequired {
                backend: ProjectManagementBackend::Jira,
                message: msg,
            });
        }
        return Err(ProjectManagementError::Transient {
            backend: ProjectManagementBackend::Jira,
            message: msg,
        });
    }

    let issues_json = value.get("issues").and_then(|v| v.as_array()).ok_or(
        ProjectManagementError::Transient {
            backend: ProjectManagementBackend::Jira,
            message: "Jira response missing issues array".to_string(),
        },
    )?;

    let mut issues = Vec::new();
    for issue in issues_json {
        let key = issue
            .get("key")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if key.is_empty() {
            continue;
        }

        let summary = issue
            .pointer("/fields/summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let status_key = issue
            .pointer("/fields/status/statusCategory/key")
            .and_then(|v| v.as_str())
            .unwrap_or("new")
            .to_ascii_lowercase();
        if status_key == "done" {
            continue;
        }

        let mut labels = vec!["jira".to_string(), format!("source-id:{key}")];
        labels.extend(
            issue
                .pointer("/fields/labels")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|l| l.as_str())
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        );

        if let Some(priority_name) = issue
            .pointer("/fields/priority/name")
            .and_then(|v| v.as_str())
        {
            if let Some(priority_label) = jira_priority_to_label(priority_name) {
                labels.push(priority_label.to_string());
            }
        }

        if let Some(issue_type) = issue
            .pointer("/fields/issuetype/name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_ascii_lowercase())
        {
            if issue_type.contains("bug") {
                labels.push("bug".to_string());
            } else if issue_type.contains("story")
                || issue_type.contains("task")
                || issue_type.contains("improvement")
            {
                labels.push("enhancement".to_string());
            }
        }

        let updated_at = issue
            .pointer("/fields/updated")
            .and_then(|v| v.as_str())
            .and_then(parse_datetime_utc);

        issues.push(GitHubIssue {
            number: issue_number_from_external_id(&key),
            title: format!("{key}: {summary}"),
            state: IssueState::Open,
            priority: issue_priority_from_labels(&labels),
            issue_type: issue_type_from_labels(&labels),
            labels,
            is_working: false,
            assigned_worker: None,
            updated_at,
        });
    }

    issues.sort_by(|a, b| a.priority.cmp(&b.priority).then(a.number.cmp(&b.number)));
    Ok(issues)
}

fn parse_datetime_utc(value: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .ok()
}

fn issue_number_from_external_id(value: &str) -> u32 {
    value
        .rsplit('-')
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|n| *n > 0)
        .unwrap_or_else(|| {
            let mut hasher = DefaultHasher::new();
            value.hash(&mut hasher);
            let value = (hasher.finish() % (u32::MAX as u64 - 1)) as u32;
            value.saturating_add(1)
        })
}

fn issue_priority_from_labels(labels: &[String]) -> IssuePriority {
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

fn issue_type_from_labels(labels: &[String]) -> IssueType {
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

fn linear_priority_to_label(priority: Option<i64>) -> Option<&'static str> {
    match priority {
        Some(1) => Some("P0"),
        Some(2) => Some("P1"),
        Some(3) => Some("P2"),
        Some(4) => Some("P3"),
        _ => None,
    }
}

fn jira_priority_to_label(priority_name: &str) -> Option<&'static str> {
    let normalized = priority_name.to_ascii_lowercase();
    if normalized.contains("highest")
        || normalized.contains("blocker")
        || normalized.contains("critical")
    {
        Some("P0")
    } else if normalized.contains("high") {
        Some("P1")
    } else if normalized.contains("medium") {
        Some("P2")
    } else if normalized.contains("low") || normalized.contains("lowest") {
        Some("P3")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProjectManagementBackend, backend_from_repo_config, parse_jira_issues, parse_linear_issues,
        resolve_backend,
    };
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "agents-ui-pm-{name}-{}-{nanos}",
            std::process::id()
        ))
    }

    #[test]
    fn config_file_backend_parses() {
        let root = temp_path("config");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(".agents-ui.toml"),
            "project_management_backend = \"linear\"\n",
        )
        .unwrap();

        assert_eq!(
            backend_from_repo_config(Path::new(&root)),
            Some(ProjectManagementBackend::Linear)
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn defaults_to_github_when_backend_not_set() {
        let root = temp_path("env-overrides");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(".agents-ui.toml"),
            "default_agent_type = \"droid\"\n",
        )
        .unwrap();
        assert_eq!(
            resolve_backend(Path::new(&root)),
            ProjectManagementBackend::GitHub
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn parses_linear_issues_response() {
        let json = br#"{
          "data": {
            "issues": {
              "nodes": [
                {
                  "identifier": "ENG-245",
                  "title": "Add Linear wrapper support",
                  "priority": 2,
                  "updatedAt": "2026-04-01T10:20:30Z",
                  "state": {"name":"In Progress","type":"started"},
                  "labels": {"nodes":[{"name":"enhancement"}]}
                },
                {
                  "identifier": "ENG-100",
                  "title": "Completed issue",
                  "priority": 1,
                  "updatedAt": "2026-03-01T10:20:30Z",
                  "state": {"name":"Done","type":"completed"},
                  "labels": {"nodes":[{"name":"bug"}]}
                }
              ]
            }
          }
        }"#;

        let issues = parse_linear_issues(json).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].number, 245);
        assert!(issues[0].title.starts_with("ENG-245:"));
        assert!(issues[0].labels.contains(&"P1".to_string()));
        assert!(issues[0].labels.contains(&"linear".to_string()));
    }

    #[test]
    fn parses_jira_issues_response() {
        let json = br#"{
          "issues": [
            {
              "key": "OPS-12",
              "fields": {
                "summary": "Fix jira integration",
                "updated": "2026-04-01T09:00:00.000+0000",
                "status": { "statusCategory": { "key": "indeterminate" } },
                "priority": { "name": "High" },
                "labels": ["bug"],
                "issuetype": { "name": "Bug" }
              }
            },
            {
              "key": "OPS-13",
              "fields": {
                "summary": "Already done",
                "updated": "2026-04-01T09:00:00.000+0000",
                "status": { "statusCategory": { "key": "done" } },
                "priority": { "name": "Low" },
                "labels": [],
                "issuetype": { "name": "Task" }
              }
            }
          ]
        }"#;

        let issues = parse_jira_issues(json).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].number, 12);
        assert!(issues[0].title.starts_with("OPS-12:"));
        assert!(issues[0].labels.contains(&"P1".to_string()));
        assert!(issues[0].labels.contains(&"jira".to_string()));
        assert!(issues[0].labels.contains(&"bug".to_string()));
    }
}
