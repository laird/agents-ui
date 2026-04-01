use anyhow::{Context, Result};
use std::path::Path;

use crate::transport::ServerTransport;

/// Project-management provider used by the command wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectManagementBackend {
    GitHub,
    Linear,
    Jira,
}

impl ProjectManagementBackend {
    fn binary(self) -> &'static str {
        match self {
            ProjectManagementBackend::GitHub => "gh",
            ProjectManagementBackend::Linear => "linear",
            ProjectManagementBackend::Jira => "jira",
        }
    }

    pub fn from_env_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "github" | "gh" => Some(ProjectManagementBackend::GitHub),
            "linear" => Some(ProjectManagementBackend::Linear),
            "jira" => Some(ProjectManagementBackend::Jira),
            _ => None,
        }
    }

    fn auth_status_args(self) -> Vec<String> {
        vec!["auth".to_string(), "status".to_string()]
    }

    fn issue_list_args(self, fields: &str, limit: u32) -> Vec<String> {
        match self {
            ProjectManagementBackend::GitHub => vec![
                "issue".to_string(),
                "list".to_string(),
                "--state".to_string(),
                "open".to_string(),
                "--limit".to_string(),
                limit.to_string(),
                "--json".to_string(),
                fields.to_string(),
            ],
            ProjectManagementBackend::Linear => {
                let mut args = vec![
                    "issue".to_string(),
                    "list".to_string(),
                    "--limit".to_string(),
                    limit.to_string(),
                    "--json".to_string(),
                ];
                if !fields.trim().is_empty() {
                    args.push("--fields".to_string());
                    args.push(fields.to_string());
                }
                args
            }
            ProjectManagementBackend::Jira => {
                let mut args = vec![
                    "issue".to_string(),
                    "list".to_string(),
                    "--status".to_string(),
                    "open".to_string(),
                    "--limit".to_string(),
                    limit.to_string(),
                    "--json".to_string(),
                ];
                if !fields.trim().is_empty() {
                    args.push("--fields".to_string());
                    args.push(fields.to_string());
                }
                args
            }
        }
    }

    fn issue_view_args(self, issue_ref: &str) -> Vec<String> {
        vec![
            "issue".to_string(),
            "view".to_string(),
            issue_ref.to_string(),
        ]
    }

    fn issue_view_web_args(self, issue_ref: &str) -> Vec<String> {
        match self {
            ProjectManagementBackend::GitHub => vec![
                "issue".to_string(),
                "view".to_string(),
                issue_ref.to_string(),
                "--web".to_string(),
            ],
            ProjectManagementBackend::Linear | ProjectManagementBackend::Jira => vec![
                "issue".to_string(),
                "open".to_string(),
                issue_ref.to_string(),
            ],
        }
    }

    fn issue_view_json_args(self, issue_ref: &str, fields: &str) -> Vec<String> {
        match self {
            ProjectManagementBackend::GitHub => vec![
                "issue".to_string(),
                "view".to_string(),
                issue_ref.to_string(),
                "--json".to_string(),
                fields.to_string(),
            ],
            ProjectManagementBackend::Linear | ProjectManagementBackend::Jira => {
                let mut args = vec![
                    "issue".to_string(),
                    "view".to_string(),
                    issue_ref.to_string(),
                    "--json".to_string(),
                ];
                if !fields.trim().is_empty() {
                    args.push("--fields".to_string());
                    args.push(fields.to_string());
                }
                args
            }
        }
    }

    fn issue_edit_remove_label_args(self, issue_ref: &str, label: &str) -> Vec<String> {
        match self {
            ProjectManagementBackend::GitHub => vec![
                "issue".to_string(),
                "edit".to_string(),
                issue_ref.to_string(),
                "--remove-label".to_string(),
                label.to_string(),
            ],
            ProjectManagementBackend::Linear | ProjectManagementBackend::Jira => vec![
                "issue".to_string(),
                "update".to_string(),
                issue_ref.to_string(),
                "--remove-label".to_string(),
                label.to_string(),
            ],
        }
    }

    fn issue_create_args(self, title: &str, body: &str, label: &str) -> Vec<String> {
        match self {
            ProjectManagementBackend::GitHub => vec![
                "issue".to_string(),
                "create".to_string(),
                "--title".to_string(),
                title.to_string(),
                "--body".to_string(),
                body.to_string(),
                "--label".to_string(),
                label.to_string(),
            ],
            ProjectManagementBackend::Linear => vec![
                "issue".to_string(),
                "create".to_string(),
                "--title".to_string(),
                title.to_string(),
                "--description".to_string(),
                body.to_string(),
                "--label".to_string(),
                label.to_string(),
            ],
            ProjectManagementBackend::Jira => vec![
                "issue".to_string(),
                "create".to_string(),
                "--summary".to_string(),
                title.to_string(),
                "--description".to_string(),
                body.to_string(),
                "--labels".to_string(),
                label.to_string(),
            ],
        }
    }
}

pub fn configured_backend() -> ProjectManagementBackend {
    std::env::var("AGENTS_PM_BACKEND")
        .ok()
        .or_else(|| std::env::var("AGENTS_PROJECT_MANAGEMENT_BACKEND").ok())
        .as_deref()
        .and_then(ProjectManagementBackend::from_env_value)
        .unwrap_or(ProjectManagementBackend::GitHub)
}

fn legacy_compatible_backend() -> ProjectManagementBackend {
    match configured_backend() {
        ProjectManagementBackend::GitHub => ProjectManagementBackend::GitHub,
        ProjectManagementBackend::Linear | ProjectManagementBackend::Jira => {
            tracing::warn!(
                "Configured non-GitHub backend while legacy GitHub issue model is active; falling back to GitHub wrapper"
            );
            ProjectManagementBackend::GitHub
        }
    }
}

async fn run_backend(
    transport: &ServerTransport,
    backend: ProjectManagementBackend,
    repo_path: Option<&Path>,
    args: &[String],
) -> Result<std::process::Output> {
    let program = backend.binary();
    let cwd_suffix = repo_path
        .map(|path| format!(" (cwd: {})", path.display()))
        .unwrap_or_default();
    tracing::debug!(
        "project-management command: {} {}{cwd_suffix}",
        program,
        args.join(" ")
    );

    transport
        .output(program, args, repo_path)
        .await
        .with_context(|| {
            let command = if args.is_empty() {
                program.to_string()
            } else {
                format!("{program} {}", args.join(" "))
            };
            match repo_path {
                Some(path) => format!("Failed to run `{command}` in {}", path.display()),
                None => format!("Failed to run `{command}`"),
            }
        })
}

pub fn command_error_detail(output: &std::process::Output, fallback: &str) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        fallback.to_string()
    }
}

pub async fn backend_auth_status(
    transport: &ServerTransport,
    backend: ProjectManagementBackend,
) -> Result<std::process::Output> {
    run_backend(transport, backend, None, &backend.auth_status_args()).await
}

pub async fn backend_issue_list(
    transport: &ServerTransport,
    backend: ProjectManagementBackend,
    repo_path: &Path,
    fields: &str,
    limit: u32,
) -> Result<std::process::Output> {
    run_backend(
        transport,
        backend,
        Some(repo_path),
        &backend.issue_list_args(fields, limit),
    )
    .await
}

pub async fn backend_issue_view(
    transport: &ServerTransport,
    backend: ProjectManagementBackend,
    repo_path: &Path,
    issue_ref: &str,
) -> Result<std::process::Output> {
    run_backend(
        transport,
        backend,
        Some(repo_path),
        &backend.issue_view_args(issue_ref),
    )
    .await
}

pub async fn backend_issue_view_web(
    transport: &ServerTransport,
    backend: ProjectManagementBackend,
    repo_path: &Path,
    issue_ref: &str,
) -> Result<std::process::Output> {
    run_backend(
        transport,
        backend,
        Some(repo_path),
        &backend.issue_view_web_args(issue_ref),
    )
    .await
}

pub async fn backend_issue_view_json(
    transport: &ServerTransport,
    backend: ProjectManagementBackend,
    repo_path: &Path,
    issue_ref: &str,
    fields: &str,
) -> Result<std::process::Output> {
    run_backend(
        transport,
        backend,
        Some(repo_path),
        &backend.issue_view_json_args(issue_ref, fields),
    )
    .await
}

pub async fn backend_issue_edit_remove_label(
    transport: &ServerTransport,
    backend: ProjectManagementBackend,
    repo_path: &Path,
    issue_ref: &str,
    label: &str,
) -> Result<std::process::Output> {
    run_backend(
        transport,
        backend,
        Some(repo_path),
        &backend.issue_edit_remove_label_args(issue_ref, label),
    )
    .await
}

pub async fn backend_issue_create(
    transport: &ServerTransport,
    backend: ProjectManagementBackend,
    repo_path: &Path,
    title: &str,
    body: &str,
    label: &str,
) -> Result<std::process::Output> {
    run_backend(
        transport,
        backend,
        Some(repo_path),
        &backend.issue_create_args(title, body, label),
    )
    .await
}

pub async fn auth_status(transport: &ServerTransport) -> Result<std::process::Output> {
    backend_auth_status(transport, legacy_compatible_backend()).await
}

pub async fn issue_list(
    transport: &ServerTransport,
    repo_path: &Path,
    fields: &str,
    limit: u32,
) -> Result<std::process::Output> {
    backend_issue_list(
        transport,
        legacy_compatible_backend(),
        repo_path,
        fields,
        limit,
    )
    .await
}

pub async fn issue_view(
    transport: &ServerTransport,
    repo_path: &Path,
    issue_number: u32,
) -> Result<std::process::Output> {
    backend_issue_view(
        transport,
        legacy_compatible_backend(),
        repo_path,
        &issue_number.to_string(),
    )
    .await
}

pub async fn issue_view_web(
    transport: &ServerTransport,
    repo_path: &Path,
    issue_number: u32,
) -> Result<std::process::Output> {
    backend_issue_view_web(
        transport,
        legacy_compatible_backend(),
        repo_path,
        &issue_number.to_string(),
    )
    .await
}

pub async fn issue_view_json(
    transport: &ServerTransport,
    repo_path: &Path,
    issue_number: u32,
    fields: &str,
) -> Result<std::process::Output> {
    backend_issue_view_json(
        transport,
        legacy_compatible_backend(),
        repo_path,
        &issue_number.to_string(),
        fields,
    )
    .await
}

pub async fn issue_edit_remove_label(
    transport: &ServerTransport,
    repo_path: &Path,
    issue_number: u32,
    label: &str,
) -> Result<std::process::Output> {
    backend_issue_edit_remove_label(
        transport,
        legacy_compatible_backend(),
        repo_path,
        &issue_number.to_string(),
        label,
    )
    .await
}

pub async fn issue_create(
    transport: &ServerTransport,
    repo_path: &Path,
    title: &str,
    body: &str,
    label: &str,
) -> Result<std::process::Output> {
    backend_issue_create(
        transport,
        legacy_compatible_backend(),
        repo_path,
        title,
        body,
        label,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::{configured_backend, ProjectManagementBackend};

    #[test]
    fn parses_backend_from_env_value() {
        assert_eq!(
            ProjectManagementBackend::from_env_value("github"),
            Some(ProjectManagementBackend::GitHub)
        );
        assert_eq!(
            ProjectManagementBackend::from_env_value("gh"),
            Some(ProjectManagementBackend::GitHub)
        );
        assert_eq!(
            ProjectManagementBackend::from_env_value("Linear"),
            Some(ProjectManagementBackend::Linear)
        );
        assert_eq!(
            ProjectManagementBackend::from_env_value("JIRA"),
            Some(ProjectManagementBackend::Jira)
        );
        assert_eq!(ProjectManagementBackend::from_env_value("unknown"), None);
    }

    #[test]
    fn github_issue_list_args_match_existing_contract() {
        let args = ProjectManagementBackend::GitHub.issue_list_args("number,title", 100);
        assert_eq!(
            args,
            vec![
                "issue",
                "list",
                "--state",
                "open",
                "--limit",
                "100",
                "--json",
                "number,title"
            ]
        );
    }

    #[test]
    fn linear_issue_list_args_include_backend_fields_flag() {
        let args = ProjectManagementBackend::Linear.issue_list_args("id,title,state", 25);
        assert_eq!(
            args,
            vec![
                "issue",
                "list",
                "--limit",
                "25",
                "--json",
                "--fields",
                "id,title,state"
            ]
        );
    }

    #[test]
    fn jira_issue_create_args_use_summary_and_labels() {
        let args = ProjectManagementBackend::Jira.issue_create_args("title", "body", "P3");
        assert_eq!(
            args,
            vec![
                "issue",
                "create",
                "--summary",
                "title",
                "--description",
                "body",
                "--labels",
                "P3"
            ]
        );
    }

    #[test]
    fn configured_backend_is_parseable() {
        let backend = configured_backend();
        assert!(matches!(
            backend,
            ProjectManagementBackend::GitHub
                | ProjectManagementBackend::Linear
                | ProjectManagementBackend::Jira
        ));
    }
}
