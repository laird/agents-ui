//! Handoffs written when a swarm is stopped.
//!
//! Stopping a swarm kills tmux panes, and with them everything the agent knew:
//! what it was working on, what it had already changed on disk, and what it was
//! about to do next. The worktree is often reaped afterwards, so uncommitted
//! work and unpushed commits vanish with no record that they existed.
//!
//! A handoff is written for every agent before anything is killed. It goes to
//! two places on purpose:
//!
//! - a comment on the GitHub issue the agent was working, so the next agent to
//!   claim that issue finds it without knowing anything about worktrees, and
//!   it survives the worktree being deleted;
//! - a file inside the worktree, so the handoff is still there when GitHub is
//!   unreachable or the agent had no issue at all (the manager usually does
//!   not).
//!
//! Failing to write a handoff never blocks the shutdown. A swarm the user
//! asked to stop must stop, so every step here is best-effort and logged.

use std::path::{Path, PathBuf};

use crate::transport::ServerTransport;

/// How many lines of the agent's terminal to keep. Enough to show what it had
/// just run and what it was about to do, without pasting a whole session into
/// a GitHub comment.
const PANE_TAIL_LINES: usize = 50;

/// Everything worth recording about one agent at shutdown.
#[derive(Debug, Clone, Default)]
pub struct Handoff {
    pub project: String,
    pub role: String,
    pub is_manager: bool,
    pub worktree: PathBuf,
    pub branch: Option<String>,
    pub issue: Option<u32>,
    pub issue_title: Option<String>,
    pub tmux_target: String,
    pub state: String,
    pub health: String,
    pub completed_issue_count: u32,
    /// Tail of the agent's pane.
    pub pane_tail: String,
    /// `git status --porcelain` lines: work that exists only in the worktree.
    pub dirty_files: Vec<String>,
    /// Commits on this branch that are not on the integration branch: work
    /// that is committed but would still be stranded if the worktree is reaped.
    pub unpushed: Vec<String>,
}

/// Keep the last `PANE_TAIL_LINES` non-blank lines of a pane capture.
///
/// Agent TUIs pad their output with blank lines to hold the viewport, so a
/// naive tail is mostly whitespace and shows nothing of what happened.
pub fn pane_tail(content: &str) -> String {
    let lines: Vec<&str> = content
        .lines()
        .map(|l| l.trim_end())
        .filter(|l| !l.is_empty())
        .collect();

    let start = lines.len().saturating_sub(PANE_TAIL_LINES);
    lines[start..].join("\n")
}

/// Render the handoff as Markdown. Pure, so it is testable without a swarm.
pub fn render(h: &Handoff, timestamp: &str) -> String {
    let mut out = String::new();

    out.push_str(&format!("## Handoff — `{}` ({})\n\n", h.role, h.project));
    out.push_str(&format!("Swarm stopped at {timestamp}.\n\n"));

    out.push_str("| | |\n|---|---|\n");
    out.push_str(&format!("| Role | `{}` |\n", h.role));
    if let Some(n) = h.issue {
        let title = h.issue_title.as_deref().unwrap_or("");
        out.push_str(&format!("| Issue | #{n} {title} |\n"));
    } else {
        out.push_str("| Issue | none assigned |\n");
    }
    out.push_str(&format!(
        "| Worktree | `{}` |\n",
        h.worktree.to_string_lossy()
    ));
    out.push_str(&format!(
        "| Branch | {} |\n",
        h.branch
            .as_deref()
            .map(|b| format!("`{b}`"))
            .unwrap_or_else(|| "detached".to_string())
    ));
    out.push_str(&format!("| State at stop | {} |\n", h.state));
    out.push_str(&format!("| Health | {} |\n", h.health));
    if !h.is_manager {
        out.push_str(&format!(
            "| Issues completed this session | {} |\n",
            h.completed_issue_count
        ));
    }
    out.push('\n');

    // Uncommitted work first: it is the most easily lost.
    out.push_str("### Uncommitted changes\n\n");
    if h.dirty_files.is_empty() {
        out.push_str("Worktree clean.\n\n");
    } else {
        out.push_str("These exist only in the worktree — they are gone if it is removed.\n\n");
        out.push_str("```\n");
        for f in &h.dirty_files {
            out.push_str(f);
            out.push('\n');
        }
        out.push_str("```\n\n");
    }

    out.push_str("### Commits not on the integration branch\n\n");
    if h.unpushed.is_empty() {
        out.push_str("None.\n\n");
    } else {
        out.push_str("```\n");
        for c in &h.unpushed {
            out.push_str(c);
            out.push('\n');
        }
        out.push_str("```\n\n");
    }

    out.push_str("### What it was doing\n\n");
    if h.pane_tail.is_empty() {
        out.push_str("_No pane output captured._\n");
    } else {
        out.push_str("```\n");
        out.push_str(&h.pane_tail);
        out.push_str("\n```\n");
    }

    out
}

/// Replace anything that is not alphanumeric (plus `-`, for `role`) with `-`,
/// so the result is always a safe single path component.
fn sanitize_component(value: &str, keep_dash: bool) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || (keep_dash && c == '-') {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Filename for the worktree copy. Timestamped so stopping twice does not
/// overwrite the earlier record.
pub fn file_name(role: &str, timestamp: &str) -> String {
    let safe_role = sanitize_component(role, true);
    let safe_ts = sanitize_component(timestamp, false);
    format!("handoff-{safe_role}-{safe_ts}.md")
}

/// Directory a role's handoffs are written to, inside its worktree.
fn handoffs_dir(worktree: &Path) -> PathBuf {
    worktree.join(".agents-ui").join("handoffs")
}

/// Newest handoff written for `role` in this worktree, if any.
///
/// Files are named `handoff-<role>-<timestamp>.md` (see [`file_name`]) with an
/// ISO-8601 timestamp whose non-alphanumeric characters have been replaced, so
/// lexical order of matching filenames is chronological order — no need to
/// parse the timestamp back out. Returns the file's path and its raw contents;
/// the body is handed to the resuming agent verbatim, not parsed back into a
/// [`Handoff`].
pub fn latest_for_role(worktree: &Path, role: &str) -> Option<(PathBuf, String)> {
    let dir = handoffs_dir(worktree);
    let prefix = format!("handoff-{}-", sanitize_component(role, true));

    let newest_name = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| name.starts_with(&prefix) && name.ends_with(".md"))
        .max()?;

    let path = dir.join(newest_name);
    let body = std::fs::read_to_string(&path).ok()?;
    Some((path, body))
}

/// Collect the git half of a handoff. Best-effort: a worktree that is gone, or
/// not a git repo, yields empty lists rather than an error.
pub async fn collect_git_state(
    transport: &ServerTransport,
    worktree: &Path,
    integration_branch: &str,
) -> (Vec<String>, Vec<String>) {
    if !worktree.exists() {
        return (Vec::new(), Vec::new());
    }

    let dirty = transport
        .output(
            "git",
            &["status".to_string(), "--porcelain".to_string()],
            Some(worktree),
        )
        .await
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.to_string())
                .filter(|l| !l.trim().is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // `--not <branch>` rather than `branch..HEAD` so this still reports
    // something useful when the integration branch is not an ancestor.
    let unpushed = transport
        .output(
            "git",
            &[
                "log".to_string(),
                "--oneline".to_string(),
                "--no-decorate".to_string(),
                "HEAD".to_string(),
                "--not".to_string(),
                integration_branch.to_string(),
            ],
            Some(worktree),
        )
        .await
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.to_string())
                .filter(|l| !l.trim().is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    (dirty, unpushed)
}

/// Write the handoff into the agent's worktree. Returns the path written.
pub fn write_to_worktree(h: &Handoff, body: &str, timestamp: &str) -> anyhow::Result<PathBuf> {
    let dir = handoffs_dir(&h.worktree);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(file_name(&h.role, timestamp));
    std::fs::write(&path, body)?;
    Ok(path)
}

/// Post the handoff as a comment on the agent's issue.
///
/// Does nothing when the agent had no issue — which is the normal case for a
/// manager, not an error.
pub async fn post_issue_comment(
    transport: &ServerTransport,
    repo_path: &Path,
    h: &Handoff,
    body: &str,
) -> anyhow::Result<bool> {
    let Some(issue) = h.issue else {
        return Ok(false);
    };

    let output = crate::github::gh_repo_output(
        transport,
        repo_path,
        &[
            "issue".to_string(),
            "comment".to_string(),
            issue.to_string(),
            "--body".to_string(),
            body.to_string(),
        ],
    )
    .await?;

    if !output.status.success() {
        anyhow::bail!(
            "gh issue comment failed for #{issue}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Handoff {
        Handoff {
            project: "agents-ui".to_string(),
            role: "worker-3".to_string(),
            is_manager: false,
            worktree: PathBuf::from("/src/agents-ui-wt-3"),
            branch: Some("worker-3".to_string()),
            issue: Some(2948),
            issue_title: Some("Fix the flaky pane test".to_string()),
            tmux_target: "claude-agents-ui:1.3".to_string(),
            state: "Working #2948".to_string(),
            health: "Healthy".to_string(),
            completed_issue_count: 2,
            pane_tail: "$ cargo test\ntest result: ok".to_string(),
            dirty_files: vec![" M src/web/server.rs".to_string()],
            unpushed: vec!["abc1234 Half-finished fix".to_string()],
        }
    }

    #[test]
    fn pane_tail_drops_the_padding_agent_tuis_emit() {
        let content = "first\n\n\n   \nsecond\n\n";
        assert_eq!(pane_tail(content), "first\nsecond");
    }

    #[test]
    fn pane_tail_keeps_only_the_last_lines() {
        let content: String = (0..200)
            .map(|i| format!("line {i}\n"))
            .collect::<Vec<_>>()
            .concat();
        let tail = pane_tail(&content);
        assert_eq!(tail.lines().count(), PANE_TAIL_LINES);
        assert!(tail.ends_with("line 199"));
        assert!(tail.starts_with("line 150"));
    }

    #[test]
    fn pane_tail_handles_empty_input() {
        assert_eq!(pane_tail(""), "");
        assert_eq!(pane_tail("\n\n\n"), "");
    }

    #[test]
    fn render_includes_the_facts_a_successor_needs() {
        let out = render(&sample(), "2026-09-01T17:00:00Z");

        assert!(out.contains("worker-3"));
        assert!(out.contains("#2948"));
        assert!(out.contains("Fix the flaky pane test"));
        assert!(out.contains("/src/agents-ui-wt-3"));
        assert!(out.contains("`worker-3`"));
        assert!(out.contains(" M src/web/server.rs"));
        assert!(out.contains("abc1234 Half-finished fix"));
        assert!(out.contains("test result: ok"));
        assert!(out.contains("2026-09-01T17:00:00Z"));
    }

    #[test]
    fn render_says_so_plainly_when_there_is_nothing_outstanding() {
        let mut h = sample();
        h.dirty_files.clear();
        h.unpushed.clear();
        let out = render(&h, "t");

        assert!(out.contains("Worktree clean."));
        assert!(out.contains("None."));
    }

    #[test]
    fn render_handles_a_manager_with_no_issue() {
        let mut h = sample();
        h.is_manager = true;
        h.role = "manager".to_string();
        h.issue = None;
        h.issue_title = None;
        let out = render(&h, "t");

        assert!(out.contains("none assigned"));
        // Completion count is a worker statistic; managers do not carry one.
        assert!(!out.contains("Issues completed this session"));
    }

    #[test]
    fn render_marks_a_detached_worktree_rather_than_inventing_a_branch() {
        let mut h = sample();
        h.branch = None;
        assert!(render(&h, "t").contains("detached"));
    }

    #[test]
    fn file_name_is_safe_for_odd_roles_and_timestamps() {
        let n = file_name("worker/3", "2026-09-01T17:00:00Z");
        assert!(!n.contains('/'), "must not create a subdirectory: {n}");
        assert!(!n.contains(':'), "must be portable: {n}");
        assert!(n.starts_with("handoff-worker-3-"));
        assert!(n.ends_with(".md"));
    }

    fn temp_worktree(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("agents-ui-handoff-{name}-{}-{nanos}", std::process::id()))
    }

    fn write_handoff(worktree: &Path, role: &str, timestamp: &str, body: &str) {
        std::fs::create_dir_all(handoffs_dir(worktree)).unwrap();
        std::fs::write(handoffs_dir(worktree).join(file_name(role, timestamp)), body).unwrap();
    }

    #[test]
    fn latest_for_role_picks_the_newest_of_several_timestamps() {
        let wt = temp_worktree("newest");
        write_handoff(&wt, "worker-3", "2026-09-01T10-00-00Z", "old");
        write_handoff(&wt, "worker-3", "2026-09-01T18-00-00Z", "newest");
        write_handoff(&wt, "worker-3", "2026-09-01T14-00-00Z", "middle");

        let (path, body) = latest_for_role(&wt, "worker-3").expect("expected a handoff");
        assert_eq!(body, "newest");
        assert!(path.to_string_lossy().contains("18-00-00Z"));

        std::fs::remove_dir_all(&wt).ok();
    }

    #[test]
    fn latest_for_role_ignores_other_roles() {
        let wt = temp_worktree("other-roles");
        write_handoff(&wt, "worker-1", "2026-09-01T10-00-00Z", "worker-1 body");
        write_handoff(&wt, "worker-10", "2026-09-01T12-00-00Z", "worker-10 body");
        write_handoff(&wt, "manager", "2026-09-01T11-00-00Z", "manager body");

        let (_, body) = latest_for_role(&wt, "worker-1").expect("expected a handoff");
        assert_eq!(body, "worker-1 body", "must not match worker-10's file");

        std::fs::remove_dir_all(&wt).ok();
    }

    #[test]
    fn latest_for_role_returns_none_when_no_files_exist() {
        let wt = temp_worktree("no-files");
        assert!(latest_for_role(&wt, "worker-1").is_none());

        std::fs::create_dir_all(handoffs_dir(&wt)).unwrap();
        assert!(
            latest_for_role(&wt, "worker-1").is_none(),
            "empty handoffs dir should also yield None"
        );

        std::fs::remove_dir_all(&wt).ok();
    }

    #[test]
    fn latest_for_role_does_not_panic_on_a_malformed_name() {
        let wt = temp_worktree("malformed");
        std::fs::create_dir_all(handoffs_dir(&wt)).unwrap();
        // Matches the prefix and suffix but has no usable timestamp — must not panic.
        std::fs::write(handoffs_dir(&wt).join("handoff-worker-1-.md"), "body").unwrap();

        let result = latest_for_role(&wt, "worker-1");
        assert!(result.is_some());

        std::fs::remove_dir_all(&wt).ok();
    }
}

/// Branch checked out in a worktree, or `None` when detached or not a repo.
pub async fn current_branch(transport: &ServerTransport, worktree: &Path) -> Option<String> {
    if !worktree.exists() {
        return None;
    }
    let out = transport
        .output(
            "git",
            &[
                "rev-parse".to_string(),
                "--abbrev-ref".to_string(),
                "HEAD".to_string(),
            ],
            Some(worktree),
        )
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    // `rev-parse --abbrev-ref` says "HEAD" when detached.
    if name.is_empty() || name == "HEAD" {
        None
    } else {
        Some(name)
    }
}

/// Build a handoff for one agent.
pub async fn collect(
    transport: &ServerTransport,
    project: &str,
    info: &crate::model::swarm::AgentInfo,
    integration_branch: &str,
) -> Handoff {
    use crate::model::swarm::HealthStatus;

    let (dirty_files, unpushed) =
        collect_git_state(transport, &info.worktree_path, integration_branch).await;

    Handoff {
        project: project.to_string(),
        role: info.role.clone(),
        is_manager: info.is_manager,
        worktree: info.worktree_path.clone(),
        branch: current_branch(transport, &info.worktree_path).await,
        issue: info.current_issue,
        issue_title: info.current_issue_title.clone(),
        tmux_target: info.tmux_target.clone(),
        state: info.status.state.to_string(),
        health: match info.health.status() {
            HealthStatus::Healthy => "Healthy",
            HealthStatus::Stalled => "Stalled",
            HealthStatus::Restarting => "Restarting",
            HealthStatus::Dead => "Dead",
        }
        .to_string(),
        completed_issue_count: info.completed_issue_count,
        pane_tail: pane_tail(&info.pane_content),
        dirty_files,
        unpushed,
    }
}

/// Write a handoff for the manager and every worker.
///
/// Best-effort throughout: a swarm the user asked to stop has to stop, so a
/// failed write or an unreachable GitHub is logged and skipped rather than
/// aborting the shutdown. Returns one summary line per agent, for the caller
/// to log or show.
pub async fn write_all(
    transport: &ServerTransport,
    swarm: &crate::model::swarm::Swarm,
    integration_branch: &str,
) -> Vec<String> {
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let mut summaries = Vec::new();

    let agents: Vec<&crate::model::swarm::AgentInfo> = std::iter::once(&swarm.manager)
        .chain(swarm.workers.iter())
        .collect();

    for info in agents {
        let h = collect(transport, &swarm.project_name, info, integration_branch).await;
        let body = render(&h, &timestamp);

        let mut wrote = Vec::new();

        match write_to_worktree(&h, &body, &timestamp) {
            Ok(path) => wrote.push(format!("file {}", path.to_string_lossy())),
            Err(e) => {
                tracing::warn!("handoff: could not write file for {}: {e:#}", h.role);
            }
        }

        match post_issue_comment(transport, &swarm.repo_path, &h, &body).await {
            Ok(true) => wrote.push(format!("comment on #{}", h.issue.unwrap_or(0))),
            Ok(false) => {}
            Err(e) => {
                tracing::warn!("handoff: could not comment for {}: {e:#}", h.role);
            }
        }

        let where_to = if wrote.is_empty() {
            "nowhere (all targets failed)".to_string()
        } else {
            wrote.join(", ")
        };
        summaries.push(format!("{}: {}", h.role, where_to));
    }

    summaries
}

/// The branch completed work is merged into, for "what is not landed yet".
///
/// Read from the remote's default branch rather than assumed: this repo uses
/// `master`, but the same code runs against repos that use `main`, and naming
/// the wrong branch would report every commit as unpushed.
pub async fn integration_branch(transport: &ServerTransport, repo_path: &Path) -> String {
    let out = transport
        .output(
            "git",
            &[
                "symbolic-ref".to_string(),
                "--short".to_string(),
                "refs/remotes/origin/HEAD".to_string(),
            ],
            Some(repo_path),
        )
        .await;

    if let Ok(o) = out {
        if o.status.success() {
            let name = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if let Some(short) = name.strip_prefix("origin/") {
                if !short.is_empty() {
                    return short.to_string();
                }
            }
        }
    }

    "master".to_string()
}
