//! What each environment runs: from GitHub Deployments, Releases, tags, a
//! version endpoint, or the environment's branch.
//!
//! Each environment has a chain of [`EnvSource`]s, tried in order; the first
//! that names a commit wins. Unconfigured, the chain is the newest successful
//! Deployment, then (for the last stop) the latest published Release, then
//! the branch ref. A source that can't answer (no `gh`, no Releases, an
//! unreachable endpoint) falls through to the next, never to an error.

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

/// Where an environment's "what runs here" comes from. In `.autocoder.json`
/// an environment's `source` is one of these or an ordered list of them:
/// `deployment`, `release`, `tag:<glob>`, `branch`, `url:<endpoint>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvSource {
    /// The newest Deployment to the environment whose latest status is `success`.
    Deployment,
    /// The latest published GitHub Release (not a draft or a prerelease).
    Release,
    /// The newest tag matching the glob (`v*`, `prod-*`), in version order.
    Tag(String),
    /// The environment's branch ref (`origin/<ref>`).
    Branch,
    /// An endpoint whose response names the running commit, e.g. `https://app/version`.
    Url(String),
}

impl EnvSource {
    /// Parse one `source` entry. Anything else is `None`, and is dropped.
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        match raw {
            "deployment" => Some(Self::Deployment),
            "release" => Some(Self::Release),
            "branch" => Some(Self::Branch),
            _ => {
                if let Some(glob) = raw.strip_prefix("tag:") {
                    let glob = glob.trim();
                    (!glob.is_empty()).then(|| Self::Tag(glob.to_string()))
                } else if let Some(url) = raw.strip_prefix("url:") {
                    let url = url.trim();
                    (url.starts_with("http://") || url.starts_with("https://")).then(|| Self::Url(url.to_string()))
                } else {
                    None
                }
            }
        }
    }
}

/// The chain for an environment whose `source` isn't set: its Deployments,
/// then for the last stop (the harbour) the latest Release, then its branch.
/// A repo that publishes Releases gets a real harbour with no config.
pub fn default_chain(last: bool) -> Vec<EnvSource> {
    let mut chain = vec![EnvSource::Deployment];
    if last {
        chain.push(EnvSource::Release);
    }
    chain.push(EnvSource::Branch);
    chain
}

/// Parse `source` from an environment entry: a string or a list of strings.
/// Unreadable entries are dropped; nothing readable leaves it unset (the
/// default chain).
pub fn parse_sources(value: Option<&serde_json::Value>) -> Vec<EnvSource> {
    match value {
        Some(serde_json::Value::String(s)) => EnvSource::parse(s).into_iter().collect(),
        Some(serde_json::Value::Array(a)) => a.iter().filter_map(|v| v.as_str()).filter_map(EnvSource::parse).collect(),
        _ => Vec::new(),
    }
}

/// Try each source in order; the first that `look` answers wins.
pub async fn first_hit<T, F, Fut>(chain: &[EnvSource], mut look: F) -> Option<(EnvSource, T)>
where
    F: FnMut(EnvSource) -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    for source in chain {
        if let Some(found) = look(source.clone()).await {
            return Some((source.clone(), found));
        }
    }
    None
}

/// One entry of `GET /repos/{owner}/{repo}/releases`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
}

/// The latest published release: drafts and prereleases don't count.
pub fn latest_release(json: &str) -> Option<Release> {
    let releases: Vec<Release> = serde_json::from_str(json).unwrap_or_default();
    releases
        .into_iter()
        .filter(|r| !r.draft && !r.prerelease && !r.tag_name.is_empty())
        .max_by(|a, b| a.published_at.cmp(&b.published_at))
}

/// One entry of `GET /repos/{owner}/{repo}/tags`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Tag {
    pub name: String,
    pub commit: TagCommit,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TagCommit {
    pub sha: String,
}

/// Parse a tags listing. Anything unreadable is no tags.
pub fn parse_tags(json: &str) -> Vec<Tag> {
    serde_json::from_str(json).unwrap_or_default()
}

/// Whether `name` matches a glob of `*` (any run) and `?` (any one character).
pub fn glob_match(glob: &str, name: &str) -> bool {
    fn go(g: &[char], n: &[char]) -> bool {
        match g.split_first() {
            None => n.is_empty(),
            Some(('*', rest)) => (0..=n.len()).any(|i| go(rest, &n[i..])),
            Some(('?', rest)) => !n.is_empty() && go(rest, &n[1..]),
            Some((c, rest)) => n.first() == Some(c) && go(rest, &n[1..]),
        }
    }
    go(&glob.chars().collect::<Vec<_>>(), &name.chars().collect::<Vec<_>>())
}

/// A tag name as runs of digits (compared as numbers) and other text, so
/// `v1.10.0` sorts after `v1.9.2`.
fn version_key(name: &str) -> Vec<(u64, String)> {
    let mut key = Vec::new();
    let mut chars = name.chars().peekable();
    while let Some(&c) = chars.peek() {
        let digit = c.is_ascii_digit();
        let mut run = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() != digit {
                break;
            }
            run.push(c);
            chars.next();
        }
        key.push(if digit { (run.parse().unwrap_or(u64::MAX), String::new()) } else { (0, run) });
    }
    key
}

/// The newest tag matching `glob`, in version order.
pub fn newest_matching<'a>(tags: &'a [Tag], glob: &str) -> Option<&'a Tag> {
    tags.iter().filter(|t| glob_match(glob, &t.name)).max_by(|a, b| version_key(&a.name).cmp(&version_key(&b.name)))
}

/// The `sha` of a `GET /repos/{owner}/{repo}/commits/{ref}` response.
pub fn commit_sha(json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v.get("sha")?.as_str().map(str::to_string)
}

fn is_sha(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// The commit a version endpoint names: a bare sha, or JSON with a `sha`,
/// `commit`, `git_sha`, `gitSha`, `revision` or `version` field holding one
/// (at the top level or one object down, as in `{"build": {"commit": ...}}`).
pub fn version_sha(body: &str) -> Option<String> {
    const KEYS: [&str; 6] = ["sha", "commit", "git_sha", "gitSha", "revision", "version"];
    let body = body.trim();
    if let Ok(serde_json::Value::Object(top)) = serde_json::from_str::<serde_json::Value>(body) {
        let pick = |o: &serde_json::Map<String, serde_json::Value>| {
            KEYS.iter().find_map(|k| o.get(*k).and_then(|v| v.as_str()).filter(|s| is_sha(s)).map(str::to_string))
        };
        return pick(&top).or_else(|| top.values().filter_map(|v| v.as_object()).find_map(pick));
    }
    body.split_whitespace().next().filter(|s| is_sha(s)).map(str::to_string)
}

/// What an environment runs according to one non-branch source.
pub async fn locate(transport: &ServerTransport, repo: &Path, environment: &str, source: &EnvSource) -> Option<Deployed> {
    let found = match source {
        EnvSource::Deployment => deployed(transport, repo, environment).await,
        EnvSource::Release => {
            let release = latest_release(&gh_api(transport, repo, "repos/{owner}/{repo}/releases?per_page=20").await?)?;
            let sha = commit_sha(&gh_api(transport, repo, &format!("repos/{{owner}}/{{repo}}/commits/{}", encode(&release.tag_name))).await?)?;
            Some(Deployed { sha, url: release.html_url.filter(|u| u.starts_with("https://")), tag: Some(release.tag_name) })
        }
        EnvSource::Tag(glob) => {
            let tags = parse_tags(&gh_api(transport, repo, "repos/{owner}/{repo}/tags?per_page=100").await?);
            let tag = newest_matching(&tags, glob)?;
            Some(Deployed { sha: tag.commit.sha.clone(), tag: Some(tag.name.clone()), url: None })
        }
        EnvSource::Url(url) => {
            let args: Vec<String> = ["-fsS", "--max-time", "5", url.as_str()].iter().map(|s| s.to_string()).collect();
            let out = transport.output("curl", &args, Some(repo)).await.ok()?;
            let sha = out.status.success().then(|| version_sha(&String::from_utf8_lossy(&out.stdout)))??;
            Some(Deployed { sha, tag: None, url: None })
        }
        EnvSource::Branch => None,
    };
    // The sha is handed to git; anything but a commit id is no answer.
    found.filter(|d| is_sha(&d.sha))
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

    /// tools/release.sh makes the `release` Deployments read here. Its shell
    /// tests cover date tags (first and next of a day), not releasing a commit
    /// twice, and the pre-push hook releasing only pushes from the main checkout.
    #[test]
    fn release_script_tests_pass() {
        let root = env!("CARGO_MANIFEST_DIR");
        let out = std::process::Command::new("bash")
            .arg(format!("{root}/tools/test_release.sh"))
            .output()
            .expect("run tools/test_release.sh");
        assert!(
            out.status.success(),
            "tools/test_release.sh failed:\n{}{}",
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

    #[test]
    fn the_latest_release_skips_drafts_and_prereleases() {
        let json = r#"[
            {"tag_name": "v1.3.0-rc1", "prerelease": true, "published_at": "2026-11-09T12:00:00Z"},
            {"tag_name": "v1.4.0", "draft": true, "published_at": null},
            {"tag_name": "v1.2.0", "html_url": "https://github.com/o/r/releases/tag/v1.2.0", "published_at": "2026-11-08T12:00:00Z"},
            {"tag_name": "v1.1.0", "published_at": "2026-11-01T12:00:00Z"}
        ]"#;
        let r = latest_release(json).unwrap();
        assert_eq!(r.tag_name, "v1.2.0");
        assert_eq!(r.html_url.as_deref(), Some("https://github.com/o/r/releases/tag/v1.2.0"));
        assert_eq!(latest_release("[]"), None, "no Releases: fall through");
        assert_eq!(latest_release(r#"{"message": "Not Found"}"#), None);
        assert_eq!(commit_sha(r#"{"sha": "a1b2c3d4e5", "commit": {}}"#).as_deref(), Some("a1b2c3d4e5"));
        assert_eq!(commit_sha("not json"), None);
    }

    #[test]
    fn the_newest_matching_tag_is_in_version_order() {
        let json = r#"[
            {"name": "v1.9.2", "commit": {"sha": "9999999"}},
            {"name": "v1.10.0", "commit": {"sha": "1010101"}},
            {"name": "prod-2026-11-01", "commit": {"sha": "abcabca"}},
            {"name": "prod-2026-11-09", "commit": {"sha": "defdefd"}},
            {"name": "nightly", "commit": {"sha": "0000000"}}
        ]"#;
        let tags = parse_tags(json);
        assert_eq!(newest_matching(&tags, "v*").map(|t| t.name.as_str()), Some("v1.10.0"), "1.10 after 1.9");
        assert_eq!(newest_matching(&tags, "prod-*").map(|t| t.commit.sha.as_str()), Some("defdefd"));
        assert_eq!(newest_matching(&tags, "release-*"), None);
        assert!(glob_match("v?.*", "v1.2"));
        assert!(!glob_match("v?.*", "v10.2"));
        assert!(glob_match("*", ""));
        assert!(!glob_match("v*", "nightly"));
        assert!(parse_tags("garbage").is_empty());
    }

    #[test]
    fn a_version_endpoint_names_its_commit() {
        assert_eq!(version_sha("9b1e440c3d\n").as_deref(), Some("9b1e440c3d"));
        assert_eq!(version_sha(r#"{"sha": "9b1e440"}"#).as_deref(), Some("9b1e440"));
        assert_eq!(version_sha(r#"{"status": "ok", "build": {"commit": "abcdef12"}}"#).as_deref(), Some("abcdef12"));
        assert_eq!(version_sha(r#"{"version": "1.2.0", "git_sha": "abcdef1"}"#).as_deref(), Some("abcdef1"), "a semver is not a commit");
        assert_eq!(version_sha("OK"), None);
        assert_eq!(version_sha(r#"{"status": "ok"}"#), None);
        assert_eq!(version_sha(""), None);
    }

    #[test]
    fn sources_parse_and_reject_what_they_cannot_use() {
        assert_eq!(EnvSource::parse("deployment"), Some(EnvSource::Deployment));
        assert_eq!(EnvSource::parse(" release "), Some(EnvSource::Release));
        assert_eq!(EnvSource::parse("branch"), Some(EnvSource::Branch));
        assert_eq!(EnvSource::parse("tag:prod-*"), Some(EnvSource::Tag("prod-*".into())));
        assert_eq!(EnvSource::parse("url:https://app/version"), Some(EnvSource::Url("https://app/version".into())));
        assert_eq!(EnvSource::parse("tag:"), None);
        assert_eq!(EnvSource::parse("url:/version"), None);
        assert_eq!(EnvSource::parse("deployments"), None);
        assert_eq!(parse_sources(Some(&serde_json::json!(["release", 3, "branch"]))), vec![EnvSource::Release, EnvSource::Branch]);
        assert!(parse_sources(None).is_empty());
        assert!(parse_sources(Some(&serde_json::json!({"x": 1}))).is_empty());
    }
}
