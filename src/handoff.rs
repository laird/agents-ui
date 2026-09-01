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

/// Filename for the worktree copy. Timestamped so stopping twice does not
/// overwrite the earlier record.
pub fn file_name(role: &str, timestamp: &str) -> String {
    let safe_role: String = role
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' })
        .collect();
    let safe_ts: String = timestamp
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    format!("handoff-{safe_role}-{safe_ts}.md")
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
    let dir = h.worktree.join(".agents-ui").join("handoffs");
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

    /// Unique temp dir that removes itself, so a failing assertion cannot
    /// leave a tree behind in /tmp. `tempfile` is not a dependency of this
    /// crate, and one test module is not a reason to add one.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("agents-ui-handoff-{}-{}-{}", std::process::id(), tag, n));
            std::fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn parse_worktree_ordinals_finds_only_this_projects_numbered_worktrees() {
        let out = "\
worktree /src/kink-party
branch refs/heads/main

worktree /src/kink-party-wt-2
branch refs/heads/worker-2

worktree /src/kink-party-wt-10
branch refs/heads/worker-10

worktree /src/kink-party-wt-1
branch refs/heads/worker-1

worktree /src/other-project-wt-1
branch refs/heads/worker-1

worktree /src/kink-party-wt-2-old
branch refs/heads/scratch
";
        let got = parse_worktree_ordinals(out, "kink-party");

        // Sorted by ordinal, numerically -- wt-10 must not sort before wt-2.
        assert_eq!(
            got,
            vec![
                (1, PathBuf::from("/src/kink-party-wt-1")),
                (2, PathBuf::from("/src/kink-party-wt-2")),
                (10, PathBuf::from("/src/kink-party-wt-10")),
            ]
        );
    }

    #[test]
    fn parse_worktree_ordinals_ignores_the_base_repo_and_junk() {
        assert!(parse_worktree_ordinals("", "p").is_empty());
        assert!(parse_worktree_ordinals("worktree /src/p\n", "p").is_empty());
        // A suffix that is not a bare number is somebody's copy, not a worker.
        assert!(parse_worktree_ordinals("worktree /src/p-wt-abc\n", "p").is_empty());
        assert!(parse_worktree_ordinals("garbage\n", "p").is_empty());
    }

    #[test]
    fn latest_for_role_picks_the_newest_and_ignores_other_roles() {
        let tmp = TempDir::new("t");
        let dir = tmp.path().join(".agents-ui").join("handoffs");
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(dir.join("handoff-worker-1-2026-09-01T10-00-00Z.md"), "older").unwrap();
        std::fs::write(dir.join("handoff-worker-1-2026-09-01T17-00-00Z.md"), "newest").unwrap();
        std::fs::write(dir.join("handoff-worker-2-2026-09-01T18-00-00Z.md"), "other role").unwrap();
        std::fs::write(dir.join("notes.md"), "not a handoff").unwrap();

        let (path, body) = latest_for_role(tmp.path(), "worker-1").unwrap();
        assert_eq!(body, "newest");
        assert!(path.to_string_lossy().contains("17-00-00"));
    }

    #[test]
    fn latest_for_role_is_none_when_there_is_nothing_to_resume_from() {
        let tmp = TempDir::new("t");
        assert!(latest_for_role(tmp.path(), "worker-1").is_none());

        let dir = tmp.path().join(".agents-ui").join("handoffs");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("handoff-manager-t.md"), "x").unwrap();
        assert!(latest_for_role(tmp.path(), "worker-1").is_none());
    }

    #[test]
    fn latest_for_role_matches_the_sanitising_file_name_applies() {
        let tmp = TempDir::new("t");
        let dir = tmp.path().join(".agents-ui").join("handoffs");
        std::fs::create_dir_all(&dir).unwrap();
        // file_name("worker/1", ..) writes "worker-1"; the reader must agree.
        std::fs::write(dir.join(file_name("worker/1", "t")), "body").unwrap();

        assert!(latest_for_role(tmp.path(), "worker/1").is_some());
    }

    #[test]
    fn resume_prompt_is_one_line_that_points_at_the_file() {
        let prompt = resume_prompt(Path::new("/wt/.agents-ui/handoffs/handoff-worker-3-t.md"));

        assert!(prompt.contains("handoff-worker-3-t.md"));
        assert!(prompt.to_lowercase().contains("resuming"));
        // Newlines are Enter presses to tmux send-keys. A multi-line prompt
        // types itself into whatever menu the agent happens to be showing.
        assert!(
            !prompt.contains('\n'),
            "resume prompt must never contain a newline: {prompt:?}"
        );
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

/// The newest handoff written for `role` in this worktree, if any.
///
/// `file_name` stamps an ISO-8601 timestamp with non-alphanumerics replaced,
/// so the names sort chronologically as plain strings and the last one is the
/// most recent. Returns the path alongside the body so a caller can say where
/// the context came from.
pub fn latest_for_role(worktree: &Path, role: &str) -> Option<(PathBuf, String)> {
    let dir = worktree.join(".agents-ui").join("handoffs");
    let prefix = {
        // Same sanitising as file_name, so a role like "worker/1" matches the
        // file it actually wrote.
        let safe: String = role
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' })
            .collect();
        format!("handoff-{safe}-")
    };

    let mut candidates: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with(&prefix) && n.ends_with(".md"))
                .unwrap_or(false)
        })
        .collect();

    candidates.sort();
    let newest = candidates.pop()?;
    let body = std::fs::read_to_string(&newest).ok()?;
    Some((newest, body))
}

/// The prompt a resumed agent receives.
///
/// A single line that points at the handoff file, never the handoff itself.
///
/// The body is Markdown with many newlines, and tmux `send-keys` turns every
/// newline into Enter. Sending it verbatim types the document into whatever
/// the agent is showing -- during live testing that was Claude Code's
/// trust-this-folder prompt, where the stray Enters drove the menu and
/// selected "No, exit", killing the agent it was meant to resume. In a real
/// repo those keystrokes would have hit whatever menu happened to be open.
///
/// One line has no newlines to misfire, and the agent can read the file
/// itself -- which it is better at than having a document typed at it.
pub fn resume_prompt(handoff_path: &Path) -> String {
    format!(
        "You are resuming after this swarm was stopped. Read {} for your handoff: \
it records the issue you were on, your branch, your uncommitted changes and \
what you were doing. Your worktree is unchanged. Verify the current state on \
disk before trusting it, then continue.",
        handoff_path.display()
    )
}

/// Worktrees belonging to a project, found by the naming `create_worktrees`
/// uses, returned as `(ordinal, path)` sorted by ordinal.
///
/// Resume cannot ask the running state for these: a stopped swarm has no tmux
/// session, so discovery never sees it, and after a daemon restart there is
/// nothing in memory to resume from. The worktrees on disk are the durable
/// record.
///
/// `git worktree list` rather than a directory glob, because a glob happily
/// matches a stale directory that git no longer tracks as a worktree.
pub async fn discover_worktrees(
    transport: &ServerTransport,
    repo_path: &Path,
    project_name: &str,
) -> Vec<(u32, PathBuf)> {
    let Ok(out) = transport
        .output(
            "git",
            &[
                "worktree".to_string(),
                "list".to_string(),
                "--porcelain".to_string(),
            ],
            Some(repo_path),
        )
        .await
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }

    parse_worktree_ordinals(&String::from_utf8_lossy(&out.stdout), project_name)
}

/// Pull `(ordinal, path)` out of `git worktree list --porcelain` for the
/// worktrees named `<project>-wt-<N>`. Split out so it is testable without git.
pub fn parse_worktree_ordinals(stdout: &str, project_name: &str) -> Vec<(u32, PathBuf)> {
    let prefix = format!("{project_name}-wt-");
    let mut found: Vec<(u32, PathBuf)> = Vec::new();

    for line in stdout.lines() {
        let Some(path) = line.strip_prefix("worktree ") else {
            continue;
        };
        let path = PathBuf::from(path.trim());
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(rest) = name.strip_prefix(&prefix) else {
            continue;
        };
        // Only a bare number: "<project>-wt-2" counts, "<project>-wt-2-old"
        // is somebody's copy and must not be resumed into.
        if let Ok(n) = rest.parse::<u32>() {
            found.push((n, path));
        }
    }

    found.sort_by_key(|(n, _)| *n);
    found.dedup_by_key(|(n, _)| *n);
    found
}
