//! Deciding which command `open` runs, for a validated link and host (#576).
//! Pure: no process is started here, so the decision is testable on its own.
//!
//! Never `herdr agent focus`/`agent.focus`: confirmed live against a running
//! herdr 0.9.5 server (laird/agents-ui#576) that it moves a single pointer
//! shared by every attached client, not one scoped to the caller. Both
//! branches below instead open a fresh terminal attached to just the one
//! target pane, so no other client's view can move.

use super::host::HostKind;
use super::url::{AttachUrl, Mux};

/// The command to run in a freshly opened terminal.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub program: String,
    pub args: Vec<String>,
}

/// `herdr agent attach` is local-only: herdr's own `--machine` forwarding
/// excludes `attach` (CLI reference, "Agents"), so a remote host instead
/// runs it directly over an interactive SSH exec -- the same pattern this
/// link's tmux branch already uses for a tmux swarm.
pub fn plan(url: &AttachUrl, host: HostKind) -> Plan {
    match (host, url.mux) {
        (HostKind::Local, Mux::Herdr) => Plan { program: "herdr".into(), args: vec!["agent".into(), "attach".into(), url.pane.clone()] },
        (HostKind::Local, Mux::Tmux) => Plan { program: "tmux".into(), args: vec!["attach".into(), "-t".into(), url.pane.clone()] },
        (HostKind::Known, Mux::Herdr) => Plan {
            program: "ssh".into(),
            args: vec!["-t".into(), url.host.clone(), "herdr".into(), "agent".into(), "attach".into(), url.pane.clone()],
        },
        (HostKind::Known, Mux::Tmux) => Plan {
            program: "ssh".into(),
            args: vec!["-t".into(), url.host.clone(), "tmux".into(), "attach".into(), "-t".into(), url.pane.clone()],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(mux: Mux, pane: &str) -> AttachUrl {
        AttachUrl { host: "nuc-1".to_string(), mux, pane: pane.to_string(), project: None, role: None }
    }

    #[test]
    fn local_herdr_attaches_directly() {
        assert_eq!(
            plan(&url(Mux::Herdr, "w1F:p1"), HostKind::Local),
            Plan { program: "herdr".into(), args: vec!["agent".into(), "attach".into(), "w1F:p1".into()] }
        );
    }

    #[test]
    fn local_tmux_attaches_directly() {
        assert_eq!(
            plan(&url(Mux::Tmux, "claude-shop:1.0"), HostKind::Local),
            Plan { program: "tmux".into(), args: vec!["attach".into(), "-t".into(), "claude-shop:1.0".into()] }
        );
    }

    #[test]
    fn remote_herdr_runs_over_ssh_not_the_shared_focus_pointer() {
        let p = plan(&url(Mux::Herdr, "w1F:p1"), HostKind::Known);
        assert_eq!(p.program, "ssh");
        assert_eq!(p.args, vec!["-t", "nuc-1", "herdr", "agent", "attach", "w1F:p1"]);
        assert!(!p.args.iter().any(|a| a == "focus"), "must never call the shared agent.focus");
    }

    #[test]
    fn remote_tmux_runs_over_ssh() {
        let p = plan(&url(Mux::Tmux, "claude-shop:1.0"), HostKind::Known);
        assert_eq!(p.program, "ssh");
        assert_eq!(p.args, vec!["-t", "nuc-1", "tmux", "attach", "-t", "claude-shop:1.0"]);
    }
}
