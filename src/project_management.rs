use anyhow::{Context, Result};
use std::path::Path;

use crate::transport::ServerTransport;

async fn run_gh(
    transport: &ServerTransport,
    repo_path: Option<&Path>,
    args: &[String],
) -> Result<std::process::Output> {
    let cwd_suffix = repo_path
        .map(|path| format!(" (cwd: {})", path.display()))
        .unwrap_or_default();
    tracing::debug!("project-management gh command: gh {}{cwd_suffix}", args.join(" "));

    transport
        .output("gh", args, repo_path)
        .await
        .with_context(|| {
            let command = format!("gh {}", args.join(" "));
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

pub async fn auth_status(transport: &ServerTransport) -> Result<std::process::Output> {
    run_gh(transport, None, &["auth".to_string(), "status".to_string()]).await
}

pub async fn issue_list(
    transport: &ServerTransport,
    repo_path: &Path,
    fields: &str,
    limit: u32,
) -> Result<std::process::Output> {
    run_gh(
        transport,
        Some(repo_path),
        &[
            "issue".to_string(),
            "list".to_string(),
            "--state".to_string(),
            "open".to_string(),
            "--limit".to_string(),
            limit.to_string(),
            "--json".to_string(),
            fields.to_string(),
        ],
    )
    .await
}

pub async fn issue_view(
    transport: &ServerTransport,
    repo_path: &Path,
    issue_number: u32,
) -> Result<std::process::Output> {
    run_gh(
        transport,
        Some(repo_path),
        &[
            "issue".to_string(),
            "view".to_string(),
            issue_number.to_string(),
        ],
    )
    .await
}

pub async fn issue_view_web(
    transport: &ServerTransport,
    repo_path: &Path,
    issue_number: u32,
) -> Result<std::process::Output> {
    run_gh(
        transport,
        Some(repo_path),
        &[
            "issue".to_string(),
            "view".to_string(),
            issue_number.to_string(),
            "--web".to_string(),
        ],
    )
    .await
}

pub async fn issue_view_json(
    transport: &ServerTransport,
    repo_path: &Path,
    issue_number: u32,
    fields: &str,
) -> Result<std::process::Output> {
    run_gh(
        transport,
        Some(repo_path),
        &[
            "issue".to_string(),
            "view".to_string(),
            issue_number.to_string(),
            "--json".to_string(),
            fields.to_string(),
        ],
    )
    .await
}

pub async fn issue_edit_remove_label(
    transport: &ServerTransport,
    repo_path: &Path,
    issue_number: u32,
    label: &str,
) -> Result<std::process::Output> {
    run_gh(
        transport,
        Some(repo_path),
        &[
            "issue".to_string(),
            "edit".to_string(),
            issue_number.to_string(),
            "--remove-label".to_string(),
            label.to_string(),
        ],
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
    run_gh(
        transport,
        Some(repo_path),
        &[
            "issue".to_string(),
            "create".to_string(),
            "--title".to_string(),
            title.to_string(),
            "--body".to_string(),
            body.to_string(),
            "--label".to_string(),
            label.to_string(),
        ],
    )
    .await
}
