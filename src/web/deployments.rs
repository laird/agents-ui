//! What each environment runs, from GitHub Deployments.
//!
//! An environment's newest deployment whose latest status is `success` names
//! the commit it runs. The pipeline prefers that over the environment's branch
//! ref, and falls back to the ref when the repository records no deployments
//! (or `gh` isn't there to ask).

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::transport::ServerTransport;

/// Deployments looked at per environment before giving up on finding a
/// successful one. Each costs one `gh api` call for its statuses.
const DEPLOYMENTS_CHECKED: usize = 10;

/// One entry of `GET /repos/{owner}/{repo}/deployments`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Deployment {
    pub id: u64,
    pub sha: String,
    #[serde(default)]
    pub created_at: String,
    /// `tools/release.sh` writes "Release vX.Y.Z" here.
    #[serde(default)]
    pub description: Option<String>,
}

/// One entry of `GET /repos/{owner}/{repo}/deployments/{id}/statuses`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct DeploymentStatus {
    /// `success`, `failure`, `error`, `pending`, `queued`, `in_progress` or `inactive`.
    pub state: String,
    #[serde(default)]
    pub created_at: String,
    /// Where the deployment's output is: `tools/release.sh` links the GitHub Release.
    #[serde(default)]
    pub log_url: Option<String>,
}

/// What an environment runs according to its newest successful deployment.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Deployed {
    pub sha: String,
    /// The release tag, when the deployment says "Release <tag>".
    pub tag: Option<String>,
    /// The page the successful status links to, such as the GitHub Release.
    pub url: Option<String>,
}

/// The tag in a deployment description of the form "Release <tag>".
pub fn release_tag(description: Option<&str>) -> Option<String> {
    let tag = description?.trim().strip_prefix("Release ")?.trim();
    (!tag.is_empty() && !tag.contains(char::is_whitespace)).then(|| tag.to_string())
}

/// Parse a deployments listing. Anything unreadable is no deployments.
pub fn parse_deployments(json: &str) -> Vec<Deployment> {
    serde_json::from_str(json).unwrap_or_default()
}

/// Parse a deployment's statuses. Anything unreadable is no statuses.
pub fn parse_statuses(json: &str) -> Vec<DeploymentStatus> {
    serde_json::from_str(json).unwrap_or_default()
}

/// Deployments newest first. GitHub already lists them that way; sorting
/// keeps a fixture or a proxy that doesn't from changing the answer.
fn newest_first(deployments: &[Deployment]) -> Vec<&Deployment> {
    let mut sorted: Vec<&Deployment> = deployments.iter().collect();
    sorted.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
    sorted
}

/// Whether a deployment's latest status is `success`. A deployment that was
/// later marked `inactive` (superseded), failed, or is still pending is not
/// what the environment runs.
fn succeeded(statuses: &[DeploymentStatus]) -> bool {
    latest(statuses).is_some_and(|s| s.state == "success")
}

fn latest(statuses: &[DeploymentStatus]) -> Option<&DeploymentStatus> {
    statuses.iter().max_by(|a, b| a.created_at.cmp(&b.created_at))
}

/// The newest deployment whose latest status is `success`, given each
/// deployment's statuses by id. Deployments without statuses are skipped.
pub fn newest_deployed(deployments: &[Deployment], statuses: &HashMap<u64, Vec<DeploymentStatus>>) -> Option<Deployed> {
    newest_first(deployments).into_iter().find_map(|d| {
        let s = statuses.get(&d.id).filter(|s| succeeded(s))?;
        Some(Deployed {
            sha: d.sha.clone(),
            tag: release_tag(d.description.as_deref()),
            url: latest(s).and_then(|s| s.log_url.clone()).filter(|u| u.starts_with("https://")),
        })
    })
}

/// The sha of [`newest_deployed`].
pub fn newest_successful(deployments: &[Deployment], statuses: &HashMap<u64, Vec<DeploymentStatus>>) -> Option<String> {
    newest_deployed(deployments, statuses).map(|d| d.sha)
}

/// Percent-encode an environment name for a query string.
fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

async fn gh_api(transport: &ServerTransport, repo: &Path, path: &str) -> Option<String> {
    let args = vec!["api".to_string(), path.to_string()];
    let out = transport.output("gh", &args, Some(repo)).await.ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// What `environment` runs according to GitHub Deployments, or `None` when it
/// has no successful deployment (or GitHub can't be asked).
pub async fn deployed(transport: &ServerTransport, repo: &Path, environment: &str) -> Option<Deployed> {
    let listing = gh_api(
        transport,
        repo,
        &format!("repos/{{owner}}/{{repo}}/deployments?environment={}&per_page={DEPLOYMENTS_CHECKED}", encode(environment)),
    )
    .await?;
    let deployments = parse_deployments(&listing);
    let mut statuses = HashMap::new();
    for d in newest_first(&deployments).into_iter().take(DEPLOYMENTS_CHECKED) {
        let Some(json) = gh_api(transport, repo, &format!("repos/{{owner}}/{{repo}}/deployments/{}/statuses", d.id)).await else {
            continue;
        };
        let these = parse_statuses(&json);
        let done = succeeded(&these);
        statuses.insert(d.id, these);
        if done {
            break;
        }
    }
    // The sha is handed to git; anything but a commit id is no answer.
    newest_deployed(&deployments, &statuses).filter(|d| !d.sha.is_empty() && d.sha.chars().all(|c| c.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEPLOYMENTS: &str = r#"[
        {"id": 30, "sha": "ccc333", "environment": "prod", "created_at": "2026-11-03T10:00:00Z"},
        {"id": 20, "sha": "bbb222", "environment": "prod", "created_at": "2026-11-02T10:00:00Z"},
        {"id": 10, "sha": "aaa111", "environment": "prod", "created_at": "2026-11-01T10:00:00Z"},
        {"id": 5,  "sha": "000000", "environment": "prod", "created_at": "2026-10-30T10:00:00Z"}
    ]"#;

    fn statuses(pairs: &[(u64, &str)]) -> HashMap<u64, Vec<DeploymentStatus>> {
        pairs.iter().map(|(id, json)| (*id, parse_statuses(json))).collect()
    }

    #[test]
    fn parses_deployments_and_statuses_from_the_api() {
        let d = parse_deployments(DEPLOYMENTS);
        assert_eq!(d.len(), 4);
        assert_eq!(d[0], Deployment { id: 30, sha: "ccc333".into(), created_at: "2026-11-03T10:00:00Z".into(), description: None });
        let s = parse_statuses(r#"[{"id": 1, "state": "success", "created_at": "2026-11-01T10:05:00Z", "environment": "prod"}]"#);
        assert_eq!(s, vec![DeploymentStatus { state: "success".into(), created_at: "2026-11-01T10:05:00Z".into(), log_url: None }]);
        assert!(parse_deployments("not json").is_empty());
        assert!(parse_deployments(r#"{"message": "Not Found"}"#).is_empty());
        assert!(parse_statuses("").is_empty());
    }

    #[test]
    fn newest_successful_deployment_wins() {
        let d = parse_deployments(DEPLOYMENTS);
        let s = statuses(&[
            (30, r#"[{"state": "success", "created_at": "2026-11-03T10:05:00Z"}, {"state": "in_progress", "created_at": "2026-11-03T10:01:00Z"}]"#),
            (20, r#"[{"state": "inactive", "created_at": "2026-11-03T10:05:00Z"}, {"state": "success", "created_at": "2026-11-02T10:05:00Z"}]"#),
        ]);
        assert_eq!(newest_successful(&d, &s), Some("ccc333".into()));
    }

    #[test]
    fn failed_pending_and_inactive_deployments_are_ignored() {
        let d = parse_deployments(DEPLOYMENTS);
        let s = statuses(&[
            (30, r#"[{"state": "failure", "created_at": "2026-11-03T10:05:00Z"}, {"state": "pending", "created_at": "2026-11-03T10:01:00Z"}]"#),
            (20, r#"[{"state": "pending", "created_at": "2026-11-02T10:01:00Z"}]"#),
            (10, r#"[{"state": "inactive", "created_at": "2026-11-02T10:05:00Z"}, {"state": "success", "created_at": "2026-11-01T10:05:00Z"}]"#),
            (5, r#"[{"state": "success", "created_at": "2026-10-30T10:05:00Z"}]"#),
        ]);
        assert_eq!(newest_successful(&d, &s), Some("000000".into()));
    }

    #[test]
    fn listing_order_does_not_matter() {
        let mut d = parse_deployments(DEPLOYMENTS);
        d.reverse();
        let s = statuses(&[
            (20, r#"[{"state": "success", "created_at": "2026-11-02T10:05:00Z"}]"#),
            (10, r#"[{"state": "success", "created_at": "2026-11-01T10:05:00Z"}]"#),
        ]);
        assert_eq!(newest_successful(&d, &s), Some("bbb222".into()));
    }

    #[test]
    fn no_successful_deployment_is_none() {
        let d = parse_deployments(DEPLOYMENTS);
        assert_eq!(newest_successful(&d, &HashMap::new()), None);
        assert_eq!(newest_successful(&[], &HashMap::new()), None);
        let s = statuses(&[(30, r#"[{"state": "error", "created_at": "2026-11-03T10:05:00Z"}]"#), (20, "[]")]);
        assert_eq!(newest_successful(&d, &s), None);
    }

    #[test]
    fn a_release_deployment_names_its_tag_and_page() {
        let d = parse_deployments(
            r#"[{"id": 40, "sha": "ddd444", "created_at": "2026-11-04T10:00:00Z", "description": "Release v0.5.0"},
                {"id": 30, "sha": "ccc333", "created_at": "2026-11-03T10:00:00Z", "description": "agents-tui installed on nuc-1"}]"#,
        );
        let s = statuses(&[
            (40, r#"[{"state": "success", "created_at": "2026-11-04T10:05:00Z", "log_url": "https://github.com/o/r/releases/tag/v0.5.0"}]"#),
            (30, r#"[{"state": "success", "created_at": "2026-11-03T10:05:00Z"}]"#),
        ]);
        assert_eq!(
            newest_deployed(&d, &s),
            Some(Deployed { sha: "ddd444".into(), tag: Some("v0.5.0".into()), url: Some("https://github.com/o/r/releases/tag/v0.5.0".into()) })
        );
        // Not a release: no tag, and no page to link.
        assert_eq!(newest_deployed(&d[1..], &s), Some(Deployed { sha: "ccc333".into(), tag: None, url: None }));
    }

    #[test]
    fn release_tags_come_only_from_release_descriptions() {
        assert_eq!(release_tag(Some("Release v1.2.3")), Some("v1.2.3".into()));
        assert_eq!(release_tag(Some("  Release v1.2.3 ")), Some("v1.2.3".into()));
        assert_eq!(release_tag(Some("Release ")), None);
        assert_eq!(release_tag(Some("Release two words")), None);
        assert_eq!(release_tag(Some("agents-tui installed on nuc-1")), None);
        assert_eq!(release_tag(None), None);
    }

    #[test]
    fn environment_names_are_query_encoded() {
        assert_eq!(encode("prod"), "prod");
        assert_eq!(encode("github-pages"), "github-pages");
        assert_eq!(encode("Production EU/1"), "Production%20EU%2F1");
    }

    /// tools/deploy.sh makes the Deployments read here. Its shell tests run it
    /// against stub systemctl/gh/cargo/curl: a missing unit refuses before
    /// recording anything, and success waits for the village to answer.
    #[test]
    fn deploy_script_tests_pass() {
        let root = env!("CARGO_MANIFEST_DIR");
        let out = std::process::Command::new("bash")
            .arg(format!("{root}/tools/test_deploy.sh"))
            .output()
            .expect("run tools/test_deploy.sh");
        assert!(
            out.status.success(),
            "tools/test_deploy.sh failed:\n{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// The served instance is the systemd unit only: no detached-server fallback.
    #[test]
    fn deploy_script_never_starts_a_server_of_its_own() {
        let script = include_str!("../../tools/deploy.sh");
        for gone in ["setsid", "nohup", "serve.pid", "serve.log", "7877", "AGENTS_TUI_SERVE_ARGS", "AGENTS_TUI_PORT"] {
            assert!(!script.contains(gone), "deploy.sh still has {gone}");
        }
        assert!(script.contains("systemctl --user cat \"$UNIT\""));
        assert!(script.contains("cp docs/agents-tui.service ~/.config/systemd/user/"));
        let unit = include_str!("../../docs/agents-tui.service");
        assert!(unit.contains("ExecStart=%h/.cargo/bin/agents-tui --headless --web-port 7890 --web-bind 127.0.0.1,tailscale"));
    }
}
