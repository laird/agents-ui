//! Dormant swarms (#398): a repo with an autocoder swarm manifest
//! (`.autocoder/swarm/*.json`) but no live agent is a swarm at rest, not a
//! swarm that is gone. It shows with its manager `Dormant: <state>` (the
//! manifest's `state`, `paused` say) and no workers; the village draws its
//! town hall dark and nobody out.
//!
//! Manifests are looked for only in repos agents-tui already knows: every repo
//! a source has shown live in this run, and the repos of saved swarms
//! (`~/.agents-ui/`). The disk is never walked.
//!
//! Live evidence always wins: a repo that any source shows live (or stopped,
//! from saved state) is that swarm, and no dormant one is added for it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::model::recognize::{SwarmManifest, canonical, read_manifests};
use crate::transport::ServerTransport;

use super::{AgentSnapshot, SwarmSnapshot};

/// The manager's state word for a dormant swarm: `Dormant`, then what the
/// manifest says (`Dormant: paused`). The village reads it as stopped.
pub const DORMANT: &str = "Dormant";

/// The swarm a repo's manifests describe when nothing in it runs. None when
/// it has no manifest.
pub fn dormant_swarm(root: &Path, manifests: &[SwarmManifest]) -> Option<SwarmSnapshot> {
    let first = manifests.first()?;
    let project = root.file_name().and_then(|b| b.to_str()).unwrap_or("swarm").to_string();
    let state = match manifests.iter().find_map(|m| m.state.as_deref()) {
        Some(s) => format!("{DORMANT}: {s}"),
        None => DORMANT.to_string(),
    };
    let manager = AgentSnapshot {
        id: format!("{project}/manager"),
        role: "manager".to_string(),
        state,
        is_manager: true,
        waiting_for_input: false,
        current_issue: None,
        current_issue_title: None,
        pane_content: format!(
            "Dormant: the swarm manifest for {} is here, but none of its agents is running.",
            first.session
        ),
        tmux_target: String::new(),
        health: "Unknown".to_string(),
        completed_issue_count: 0,
        resurrection_attempts: 0,
        status_timestamp: None,
        worktree_path: root.to_string_lossy().into_owned(),
        branch: None,
        mux: Default::default(),
        context_pct: None,
        context_window: None,
    };
    Some(SwarmSnapshot {
        project_name: project,
        repo_path: root.to_string_lossy().into_owned(),
        repo_key: String::new(),
        manager_by: None,
        agent_type: "Claude".to_string(),
        workflow: None,
        tmux_session: first.session.clone(),
        stopped: false,
        busy_count: 0,
        idle_count: 0,
        attention_count: 0,
        manager,
        workers: Vec::new(),
        issues: Vec::new(),
    })
}

/// The known repos no swarm in `swarms` already stands for: where to look
/// for a dormant one.
pub fn candidates(known: &BTreeSet<PathBuf>, swarms: &[SwarmSnapshot]) -> Vec<PathBuf> {
    let shown: BTreeSet<PathBuf> = swarms.iter().filter(|s| !s.repo_path.is_empty()).map(|s| canonical(&s.repo_path)).collect();
    known.iter().filter(|r| !shown.contains(*r)).cloned().collect()
}

/// The repos agents-tui knows, held across polls: every repo a source has
/// shown, plus saved swarms' repos.
#[derive(Default)]
pub struct DormantSwarms {
    known: BTreeSet<PathBuf>,
}

impl DormantSwarms {
    /// Remember the repos `swarms` came from.
    pub fn remember(&mut self, swarms: &[SwarmSnapshot]) {
        self.known.extend(swarms.iter().filter(|s| !s.repo_path.is_empty()).map(|s| canonical(&s.repo_path)));
    }

    /// Remember `swarms`' repos, then add a dormant swarm for each known repo
    /// that has a manifest but nothing in `swarms`.
    pub async fn add(&mut self, transport: &ServerTransport, swarms: &mut Vec<SwarmSnapshot>) {
        self.remember(swarms);
        self.known.extend(saved_repos());
        for root in candidates(&self.known, swarms) {
            if let Some(dormant) = dormant_swarm(&root, &read_manifests(transport, &root).await) {
                swarms.push(dormant);
            }
        }
    }
}

/// The repos of every saved swarm (`~/.agents-ui/`).
fn saved_repos() -> Vec<PathBuf> {
    use crate::config::persistence::{list_saved_swarms, load_swarm_state};
    list_saved_swarms()
        .unwrap_or_default()
        .iter()
        .filter_map(|name| load_swarm_state(name).ok().flatten())
        .filter(|s| !s.repo_path.is_empty())
        .map(|s| canonical(&s.repo_path))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::recognize::parse_manifests;

    const MANIFEST: &str = r#"{"version":1,"session":"claude-shop","projectRoot":"/src/shop","state":"paused",
        "workers":[],"manager":{"herdrPane":"w5:p1","tmuxTarget":null}}"#;

    fn live(root: &str) -> SwarmSnapshot {
        let mut s = dormant_swarm(Path::new(root), &parse_manifests(MANIFEST)).unwrap();
        s.manager.state = "Working".to_string();
        s
    }

    #[test]
    fn a_manifest_with_nothing_live_is_a_dormant_swarm() {
        let s = dormant_swarm(Path::new("/src/shop"), &parse_manifests(MANIFEST)).unwrap();
        assert_eq!(s.project_name, "shop");
        assert_eq!(s.repo_path, "/src/shop");
        assert_eq!(s.manager.state, "Dormant: paused", "the manifest's state");
        assert!(s.workers.is_empty());
        assert_eq!(s.tmux_session, "claude-shop");
        assert!(!s.stopped, "dormant, not a saved, stopped swarm");
        // The village draws it at rest: the hall dark, nobody out.
        assert_eq!(crate::web::village::agent_status(&s.manager), crate::web::village::HouseStatus::Stopped);
        let v = crate::web::village::build(&s, &[], Default::default());
        assert!(v.houses.is_empty());
        assert_eq!(v.manager.status, crate::web::village::HouseStatus::Stopped);
    }

    #[test]
    fn a_manifest_without_a_state_is_just_dormant() {
        let m = parse_manifests(r#"{"session":"claude-shop","manager":{}}"#);
        assert_eq!(dormant_swarm(Path::new("/src/shop"), &m).unwrap().manager.state, "Dormant");
    }

    #[test]
    fn no_manifest_and_nothing_live_is_no_swarm() {
        assert!(dormant_swarm(Path::new("/src/shop"), &[]).is_none());
        assert!(dormant_swarm(Path::new("/src/shop"), &parse_manifests("not json")).is_none());
    }

    #[test]
    fn a_live_swarm_wins_over_its_manifest() {
        // The same repo, live: not a candidate, so no dormant swarm is added.
        let known = BTreeSet::from([PathBuf::from("/src/shop"), PathBuf::from("/src/billing")]);
        assert_eq!(candidates(&known, &[live("/src/shop")]), vec![PathBuf::from("/src/billing")]);
        assert_eq!(candidates(&known, &[live("/src/shop"), live("/src/billing")]), Vec::<PathBuf>::new());
        assert_eq!(candidates(&known, &[]).len(), 2);
    }

    #[test]
    fn repos_seen_live_are_remembered_after_they_stop() {
        let mut d = DormantSwarms::default();
        d.remember(&[live("/src/shop")]);
        // The next poll shows nothing: the repo is still known, so its manifest is looked for.
        assert_eq!(candidates(&d.known, &[]), vec![PathBuf::from("/src/shop")]);
    }
}
