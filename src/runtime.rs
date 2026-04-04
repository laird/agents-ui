use anyhow::{Context, Result, anyhow, bail};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::timeout;

use crate::model::swarm::AgentType;
use crate::transport::ServerTransport;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationOutcome {
    pub gh_warning: Option<String>,
}

pub async fn validate_environment(
    transport: &ServerTransport,
    agent_type: Option<&AgentType>,
    repo_root: Option<&Path>,
) -> Result<ValidationOutcome> {
    let location = transport.server().unwrap_or("this machine");
    let tmux_hint = if cfg!(target_os = "macos") {
        "brew install tmux"
    } else {
        "sudo apt install tmux"
    };

    if !transport.command_exists("tmux").await {
        bail!("tmux is not installed on {location}. Install with: {tmux_hint}");
    }

    if let Some(agent_type) = agent_type {
        validate_runtime(transport, agent_type, repo_root).await?;
    }

    let gh_warning = crate::github::check_gh_auth(transport)
        .await
        .map(|err| err.to_string());

    Ok(ValidationOutcome { gh_warning })
}

pub async fn validate_runtime(
    transport: &ServerTransport,
    agent_type: &AgentType,
    repo_root: Option<&Path>,
) -> Result<()> {
    let location = transport.server().unwrap_or("this machine");
    let (binary, hint) = runtime_binary_hint(agent_type);

    if !transport.command_exists(binary).await {
        bail!("{binary} is not installed on {location}. {hint}");
    }

    run_probe(
        transport,
        RuntimeProbe {
            label: "startup check",
            program: binary,
            args: &["--help"],
            current_dir: None,
            timeout: Duration::from_secs(8),
        },
        agent_type,
    )
    .await?;

    for probe in runtime_live_probes(agent_type) {
        run_probe(transport, probe, agent_type).await?;
    }

    if matches!(agent_type, AgentType::Gemini)
        && let Some(repo_root) = repo_root
        && !gemini_agents_assets_present(transport, repo_root).await
    {
        bail!(
            "Gemini runtime assets are missing for {}. Expected the sibling laird/agents repo with skills, .agent scripts, and agents content near {}",
            location,
            repo_root.display()
        );
    }

    Ok(())
}

fn agents_repo_root_candidates(repo_root: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(parent) = repo_root.parent() {
        candidates.push(parent.join("agents"));
    }

    candidates
}

async fn gemini_agents_assets_present(transport: &ServerTransport, repo_root: &Path) -> bool {
    for root in agents_repo_root_candidates(repo_root) {
        if transport
            .path_exists(&root.join("skills").join("autocoder").join("SKILL.md"))
            .await
            && transport
                .path_exists(
                    &root
                        .join(".agent")
                        .join("scripts")
                        .join("start-parallel-agents.sh"),
                )
                .await
            && transport
                .path_exists(&root.join("agents").join("autocoder").join("agent.md"))
                .await
        {
            return true;
        }
    }

    false
}

pub fn detect_runtime_alert(content: &str) -> Option<String> {
    let lower = content.to_lowercase();

    if (lower.contains("quota") && (lower.contains("exceeded") || lower.contains("reached")))
        || lower.contains("rate limit")
        || lower.contains("usage limit")
        || lower.contains("too many requests")
        || lower.contains("insufficient credits")
        || lower.contains("credit balance")
        || lower.contains("billing")
    {
        return Some("runtime quota or rate limit detected".to_string());
    }

    if lower.contains("not logged in")
        || lower.contains("auth login")
        || lower.contains("authentication required")
        || lower.contains("authentication failed")
        || lower.contains("api key")
        || lower.contains("unauthorized")
        || lower.contains("401")
    {
        return Some("runtime authentication required".to_string());
    }

    None
}

struct RuntimeProbe<'a> {
    label: &'static str,
    program: &'static str,
    args: &'a [&'a str],
    current_dir: Option<&'a std::path::Path>,
    timeout: Duration,
}

fn runtime_binary_hint(agent_type: &AgentType) -> (&'static str, &'static str) {
    match agent_type {
        AgentType::Claude => (
            "claude",
            "See https://docs.anthropic.com/en/docs/claude-code",
        ),
        AgentType::Codex => ("codex", "npm install -g @openai/codex"),
        AgentType::Droid => ("droid", "See https://droid.dev"),
        AgentType::Gemini => ("gemini", "See https://ai.google.dev"),
    }
}

fn runtime_live_probes(agent_type: &AgentType) -> Vec<RuntimeProbe<'static>> {
    match agent_type {
        AgentType::Codex => vec![
            RuntimeProbe {
                label: "login status",
                program: "codex",
                args: &["login", "status"],
                current_dir: None,
                timeout: Duration::from_secs(8),
            },
            RuntimeProbe {
                label: "quota probe",
                program: "codex",
                args: &[
                    "exec",
                    "--skip-git-repo-check",
                    "--sandbox",
                    "read-only",
                    "--color",
                    "never",
                    "--ephemeral",
                    "Reply with exactly OK and nothing else.",
                ],
                current_dir: None,
                timeout: Duration::from_secs(25),
            },
        ],
        AgentType::Droid => vec![RuntimeProbe {
            label: "quota probe",
            program: "droid",
            args: &[
                "exec",
                "--output-format",
                "text",
                "Reply with exactly OK and nothing else.",
            ],
            current_dir: None,
            timeout: Duration::from_secs(25),
        }],
        AgentType::Claude | AgentType::Gemini => Vec::new(),
    }
}

async fn run_probe(
    transport: &ServerTransport,
    probe: RuntimeProbe<'_>,
    agent_type: &AgentType,
) -> Result<()> {
    let args = probe
        .args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    let output = timeout(
        probe.timeout,
        transport.output(probe.program, &args, probe.current_dir),
    )
    .await
    .with_context(|| {
        format!(
            "{agent_type} {label} timed out after {:?}",
            probe.timeout,
            label = probe.label
        )
    })?
    .with_context(|| format!("Failed to run {} {}", probe.program, probe.label))?;

    let combined = command_output_text(&output);

    if output.status.success() {
        return Ok(());
    }

    if let Some(alert) = detect_runtime_alert(&combined) {
        let detail = summarize_output(&combined);
        if detail.is_empty() {
            bail!("{agent_type} failed {label}: {alert}", label = probe.label);
        }
        bail!(
            "{agent_type} failed {label}: {alert}. {detail}",
            label = probe.label
        );
    }

    let detail = summarize_output(&combined);
    if detail.is_empty() {
        bail!(
            "{agent_type} failed {} with exit {}",
            probe.label,
            output.status
        );
    }

    Err(anyhow!("{agent_type} failed {}: {}", probe.label, detail))
}

fn command_output_text(output: &std::process::Output) -> String {
    let mut text = String::new();
    if !output.stdout.is_empty() {
        text.push_str(&String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    text
}

fn summarize_output(output: &str) -> String {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{agents_repo_root_candidates, detect_runtime_alert, runtime_live_probes};
    use crate::model::swarm::AgentType;
    use std::path::{Path, PathBuf};

    #[test]
    fn detects_runtime_quota_alerts() {
        assert_eq!(
            detect_runtime_alert("Error: rate limit exceeded for this account"),
            Some("runtime quota or rate limit detected".to_string())
        );
        assert_eq!(
            detect_runtime_alert("billing issue: insufficient credits remaining"),
            Some("runtime quota or rate limit detected".to_string())
        );
    }

    #[test]
    fn detects_runtime_auth_alerts() {
        assert_eq!(
            detect_runtime_alert("authentication required; run auth login"),
            Some("runtime authentication required".to_string())
        );
        assert_eq!(
            detect_runtime_alert("Unauthorized (401)"),
            Some("runtime authentication required".to_string())
        );
    }

    #[test]
    fn codex_uses_live_probe_commands() {
        let probes = runtime_live_probes(&AgentType::Codex);
        assert_eq!(probes.len(), 2);
        assert_eq!(probes[0].program, "codex");
        assert_eq!(probes[1].args[0], "exec");
    }

    #[test]
    fn droid_uses_exec_probe() {
        let probes = runtime_live_probes(&AgentType::Droid);
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].program, "droid");
        assert_eq!(probes[0].args[0], "exec");
    }

    #[test]
    fn claude_has_no_live_probe_yet() {
        assert!(runtime_live_probes(&AgentType::Claude).is_empty());
    }

    #[test]
    fn gemini_asset_candidates_use_repo_sibling_agents_repo() {
        let repo = Path::new("/tmp/workspace/project");
        assert_eq!(
            agents_repo_root_candidates(repo),
            vec![PathBuf::from("/tmp/workspace/agents")]
        );
    }
}
