//! The TUI's swarm launch, add-worker and stop through `autocoder-ctl.sh`
//! (#544, `crate::adapter::ctl`), with the TUI's own launcher in `claude.rs`
//! as the fallback: on a remote host, when this autocoder lacks the verb
//! (no script, exit 3), when the swarm's session is already up (the TUI
//! reconnects to it, as before), and, for add-worker and stop, for a swarm
//! autocoder has no manifest for (one the TUI launched itself).

use std::path::PathBuf;

use anyhow::{anyhow, Result};

use super::App;
use crate::adapter::claude::ClaudeAdapter;
use crate::adapter::ctl::{self, CtlResult, CtlRun};
use crate::adapter::traits::{AgentRuntime, SwarmConfig};
use crate::event::Event;
use crate::model::swarm::Swarm;
use crate::transport::ServerTransport;

/// Start the swarm with `autocoder-ctl start`, reporting into the launch
/// dialog. `false` when the TUI should launch it itself; `true` when
/// autocoder answered, whether the start worked or not.
/// `plugin_installed`: the TUI already installed the plugin for the chosen
/// scope, so `start` must not install it again.
pub(super) async fn launch_via_ctl(
    config: &SwarmConfig,
    plugin_installed: bool,
    transport: &ServerTransport,
    progress: &impl Fn(&str),
    discovered: impl FnOnce(),
) -> bool {
    if transport.is_remote() {
        return false;
    }
    // `start` refuses a session that is already up; the TUI reconnects to it.
    let project = config.repo_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let session = format!("{}-{project}", config.agent_type.session_prefix());
    if crate::tmux::session::has_session(transport, &session).await {
        return false;
    }
    progress("⏳ Starting the swarm with autocoder-ctl start...\n");
    let args = ctl::start_args(&config.repo_path, config.num_workers, &config.agent_type, !plugin_installed);
    match ctl::run(ctl::script().as_deref(), &args, &config.repo_path).await {
        CtlRun::Unsupported => {
            progress("ℹ️  This autocoder has no `autocoder-ctl start`; using agents-tui's own launcher.\n");
            false
        }
        CtlRun::Done(result) => {
            progress(&ctl::report(&result));
            if result.ok {
                // A stop mark from an earlier stop would keep it from healing.
                crate::config::persistence::clear_swarm_stopped(&project);
                progress("✅ Triggering swarm discovery...\n");
                discovered();
            } else {
                progress("\nPress Esc to go back.\n");
            }
            true
        }
    }
}

impl App {
    /// The script to drive this swarm with, or `None` for the TUI's own path.
    fn ctl_script(&self, swarm: &Swarm) -> Option<PathBuf> {
        if self.transport.is_remote() || !ctl::has_manifest(&swarm.repo_path, &swarm.tmux_session) {
            return None;
        }
        ctl::script()
    }

    /// Stop a swarm: `autocoder-ctl stop-swarm`, else the TUI's teardown.
    pub(super) async fn stop_swarm(&self, swarm: &Swarm) -> Result<()> {
        // Before anything closes, so heal_workers does not respawn it.
        crate::config::persistence::mark_swarm_stopped(&swarm.project_name);
        if let Some(script) = self.ctl_script(swarm) {
            match ctl::run(Some(&script), &ctl::stop_swarm_args(&swarm.repo_path), &swarm.repo_path).await {
                CtlRun::Done(result) if result.ok => return Ok(()),
                CtlRun::Done(result) => return Err(anyhow!(ctl::status_line(&result, ""))),
                CtlRun::Unsupported => {}
            }
        }
        self.adapter.teardown(swarm).await
    }

    /// Add one worker to the swarm at `swarm_idx`. The status line to show
    /// now, or `None` when it runs in the background: `add-worker` waits a
    /// few seconds to read the new pane for prompts, and the TUI must not
    /// freeze meanwhile. Its outcome arrives as an [`Event::Status`].
    pub(super) async fn add_worker_to(&mut self, swarm_idx: usize) -> Option<String> {
        let swarm = self.swarms.get(swarm_idx).cloned()?;
        if let Some(script) = self.ctl_script(&swarm) {
            let (tx, transport) = (self.events.tx(), self.transport.clone());
            tokio::spawn(async move {
                let args = ctl::add_worker_args(&swarm.repo_path, &swarm.tmux_session);
                let line = match ctl::run(Some(&script), &args, &swarm.repo_path).await {
                    CtlRun::Done(result) => ctl::status_line(&result, &added_line(&result)),
                    // This autocoder lacks the verb: the TUI adds it itself.
                    CtlRun::Unsupported => match ClaudeAdapter::new(swarm.agent_type.clone(), transport).add_worker(&swarm).await {
                        Ok(worker) => format!("Added {} (starting worker loop)", worker.role),
                        Err(e) => format!("Failed to add worker: {e}"),
                    },
                };
                // Discovery picks up the new pane either way.
                let _ = tx.send(Event::SwarmDiscovered);
                let _ = tx.send(Event::Status(line));
            });
            return None;
        }
        Some(match self.adapter.add_worker(&swarm).await {
            Ok(worker) => {
                let id = worker.role.clone();
                if let Some(swarm) = self.swarms.get_mut(swarm_idx) {
                    swarm.workers.push(worker);
                }
                self.start_all_pane_watchers();
                format!("Added {id} (starting worker loop)")
            }
            Err(e) => format!("Failed to add worker: {e}"),
        })
    }
}

/// "Added worker 4" from `detail.added` (and `started`, a paused one resumed).
fn added_line(result: &CtlResult) -> String {
    let nums = |k: &str| -> Vec<String> {
        result.detail.get(k).and_then(|v| v.as_array()).into_iter().flatten().filter_map(|n| n.as_u64()).map(|n| n.to_string()).collect()
    };
    let (added, started) = (nums("added"), nums("started"));
    let mut parts = Vec::new();
    if !added.is_empty() {
        parts.push(format!("added worker {}", added.join(", ")));
    }
    if !started.is_empty() {
        parts.push(format!("resumed paused worker {}", started.join(", ")));
    }
    if parts.is_empty() {
        "autocoder-ctl add-worker: done".to_string()
    } else {
        let line = parts.join("; ");
        format!("{}{} (autocoder-ctl)", line[..1].to_uppercase(), &line[1..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn done(stdout: &str) -> CtlResult {
        match ctl::parse_result("add-worker", stdout.as_bytes(), b"", Some(0)) {
            CtlRun::Done(r) => r,
            CtlRun::Unsupported => panic!("unsupported"),
        }
    }

    #[test]
    fn says_which_workers_were_added_or_resumed() {
        let r = done(r#"{"ok":true,"detail":{"added":[4],"started":[2],"prompts":[]}}"#);
        assert_eq!(added_line(&r), "Added worker 4; resumed paused worker 2 (autocoder-ctl)");
        let r = done(r#"{"ok":true,"detail":{"added":[],"started":[]}}"#);
        assert_eq!(added_line(&r), "autocoder-ctl add-worker: done");
    }

    #[tokio::test]
    async fn a_remote_host_launches_with_the_tuis_own_path() {
        let config = SwarmConfig {
            repo_path: std::env::temp_dir(),
            agent_type: crate::model::swarm::AgentType::Claude,
            num_workers: 1,
            agents_dir: std::env::temp_dir(),
            resume_seeds: None,
        };
        let said = std::cell::RefCell::new(String::new());
        let remote = ServerTransport::new(Some("buildbox".to_string()));
        let handled = launch_via_ctl(&config, false, &remote, &|m: &str| said.borrow_mut().push_str(m), || panic!("no discovery")).await;
        assert!(!handled);
        assert!(said.borrow().is_empty());
    }
}
