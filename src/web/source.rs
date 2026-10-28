//! Sources: where the village's data comes from, in layers that can each be
//! swapped without touching the others.
//!
//! ```text
//! villages   VillageSource   swarm villages (built from the layers below), temporal
//!   swarms   SwarmSource     who is working:            tmux, herdr, saved
//!   issues   IssueTracker    the backlog:               github, none
//!   pipeline PipelineSource  where pushed work is:      autocoder, none
//!   activity ActivitySource  what each agent is doing:  claude, none
//! transport  ServerTransport where commands run:        local, or ssh with --server
//! ```
//!
//! Worlds sit above all of this and only ever see `VillageSnapshot`s and
//! issues, so a platform from another orchestrator, or a dev village fed by
//! another tracker, pairs up the same way. A new source is one impl plus a
//! name in the registry at the bottom; `--source <layer>=<name>[,<name>]`
//! picks the ones that run.

use std::collections::HashMap;
use std::hash::Hash;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::future::BoxFuture;
use serde::Serialize;

use super::temporal::{TemporalSource, TemporalVillages};
use super::village::{Pipeline, SwarmVillages, VillageSnapshot};
use super::world::{WorldConfig, WorldSnapshot, build_world};
use super::{IssueSnapshot, SwarmSnapshot};
use crate::transport::ServerTransport;

// ---------------------------------------------------------------------------
// Layers
// ---------------------------------------------------------------------------

/// A village, with the issues behind it so worlds can find incidents.
#[derive(Debug, Clone)]
pub struct Reading {
    pub village: VillageSnapshot,
    pub issues: Vec<IssueSnapshot>,
}

/// The top layer: anything that can be drawn as villages.
pub trait VillageSource: Send + Sync {
    fn name(&self) -> String;
    /// `swarms` are the ones discovery found; sources that don't build on
    /// swarms ignore them.
    fn read<'a>(&'a self, transport: &'a ServerTransport, swarms: &'a [SwarmSnapshot]) -> BoxFuture<'a, Vec<Reading>>;
}

/// Who is working: the swarms running under some agent host.
pub trait SwarmSource: Send + Sync {
    fn name(&self) -> &'static str;
    /// Every swarm this host runs; none when the host isn't there.
    fn swarms<'a>(&'a self, transport: &'a ServerTransport) -> BoxFuture<'a, Vec<SwarmSnapshot>>;
}

/// The backlog a swarm works from.
pub trait IssueTracker: Send + Sync {
    fn name(&self) -> &'static str;
    /// Open issues for the repository at `repo`; none when it can't be read.
    fn issues<'a>(&'a self, transport: &'a ServerTransport, repo: &'a Path, project: &'a str) -> BoxFuture<'a, Vec<IssueSnapshot>>;
}

/// Where pushed work has got to on its way to production.
pub trait PipelineSource: Send + Sync {
    fn name(&self) -> &'static str;
    fn pipeline<'a>(&'a self, transport: &'a ServerTransport, repo: &'a Path) -> BoxFuture<'a, Pipeline>;
}

/// What each agent in a village is doing right now: sub-agents and actions.
pub trait ActivitySource: Send + Sync {
    fn name(&self) -> &'static str;
    fn attach(&self, transport: &ServerTransport, village: &mut VillageSnapshot);
}

// ---------------------------------------------------------------------------
// Plain implementations
// ---------------------------------------------------------------------------

/// No backlog, no pipeline, no activity: the layer is switched off.
pub struct Nothing;

impl IssueTracker for Nothing {
    fn name(&self) -> &'static str {
        "none"
    }
    fn issues<'a>(&'a self, _: &'a ServerTransport, _: &'a Path, _: &'a str) -> BoxFuture<'a, Vec<IssueSnapshot>> {
        Box::pin(async { Vec::new() })
    }
}

impl PipelineSource for Nothing {
    fn name(&self) -> &'static str {
        "none"
    }
    fn pipeline<'a>(&'a self, _: &'a ServerTransport, _: &'a Path) -> BoxFuture<'a, Pipeline> {
        Box::pin(async { Pipeline::default() })
    }
}

impl ActivitySource for Nothing {
    fn name(&self) -> &'static str {
        "none"
    }
    fn attach(&self, _: &ServerTransport, _: &mut VillageSnapshot) {}
}

/// GitHub issues, through `gh`.
pub struct GitHubIssues;

impl IssueTracker for GitHubIssues {
    fn name(&self) -> &'static str {
        "github"
    }
    fn issues<'a>(&'a self, transport: &'a ServerTransport, repo: &'a Path, project: &'a str) -> BoxFuture<'a, Vec<IssueSnapshot>> {
        Box::pin(async move {
            crate::github::fetch_issues(transport, repo)
                .await
                .map(|list| list.iter().map(|i| IssueSnapshot::from_issue(i, project)).collect())
                .unwrap_or_default()
        })
    }
}

/// Values reused until they're `ttl` old, so slow sources (`gh`, `git
/// ls-remote`, Temporal's API) run at most once per TTL per key however many
/// browsers are watching.
pub struct TtlCache<K, V> {
    ttl: Duration,
    entries: Mutex<HashMap<K, (Instant, V)>>,
}

impl<K: Eq + Hash, V: Clone> TtlCache<K, V> {
    pub fn new(ttl: Duration) -> Self {
        Self { ttl, entries: Mutex::new(HashMap::new()) }
    }

    pub async fn get_or<F: Future<Output = V>>(&self, key: K, fetch: impl FnOnce() -> F) -> V {
        let fresh = self
            .entries
            .lock()
            .ok()
            .and_then(|m| m.get(&key).filter(|(at, _)| at.elapsed() < self.ttl).map(|(_, v)| v.clone()));
        if let Some(v) = fresh {
            return v;
        }
        let v = fetch().await;
        if let Ok(mut m) = self.entries.lock() {
            m.insert(key, (Instant::now(), v.clone()));
        }
        v
    }
}

// ---------------------------------------------------------------------------
// Registry: names on the command line to implementations
// ---------------------------------------------------------------------------

pub const SWARM_SOURCES: &[&str] = &["tmux", "herdr", "saved"];
pub const ISSUE_TRACKERS: &[&str] = &["github", "none"];
pub const PIPELINE_SOURCES: &[&str] = &["autocoder", "none"];
pub const ACTIVITY_SOURCES: &[&str] = &["claude", "none"];

fn swarm_source(name: &str) -> Option<Box<dyn SwarmSource>> {
    match name {
        "tmux" => Some(Box::new(super::discovery::TmuxSwarms::default())),
        "herdr" => Some(Box::new(super::herdr::HerdrSwarms)),
        "saved" => Some(Box::new(super::discovery::SavedSwarms)),
        _ => None,
    }
}

fn issue_tracker(name: &str) -> Option<Box<dyn IssueTracker>> {
    match name {
        "github" => Some(Box::new(GitHubIssues)),
        "none" => Some(Box::new(Nothing)),
        _ => None,
    }
}

fn pipeline_source(name: &str) -> Option<Box<dyn PipelineSource>> {
    match name {
        "autocoder" => Some(Box::new(super::village::AutocoderPipeline)),
        "none" => Some(Box::new(Nothing)),
        _ => None,
    }
}

fn activity_source(name: &str) -> Option<Box<dyn ActivitySource>> {
    match name {
        "claude" => Some(Box::new(super::village::ClaudeActivity)),
        "none" => Some(Box::new(Nothing)),
        _ => None,
    }
}

/// Which implementation runs at each layer, by name.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceConfig {
    /// Tried in order; the first to show a project owns it.
    pub swarms: Vec<String>,
    pub issues: String,
    pub pipeline: String,
    pub activity: String,
}

impl Default for SourceConfig {
    fn default() -> Self {
        Self {
            swarms: SWARM_SOURCES.iter().map(|s| s.to_string()).collect(),
            issues: "github".into(),
            pipeline: "autocoder".into(),
            activity: "claude".into(),
        }
    }
}

impl SourceConfig {
    /// Apply one `--source <layer>=<name>[,<name>]`. `swarms=none` turns
    /// swarm discovery off, for a pod that only watches Temporal.
    pub fn set(&mut self, spec: &str) -> Result<(), String> {
        let (layer, names) = spec
            .split_once('=')
            .ok_or_else(|| format!("--source value '{spec}' is not <layer>=<name>, e.g. issues=none"))?;
        let names: Vec<String> = names.split(',').map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect();
        let check = |known: &[&str], name: &str| {
            known
                .contains(&name)
                .then_some(())
                .ok_or_else(|| format!("unknown {layer} source '{name}'; expected one of {}", known.join(", ")))
        };
        let one = |known: &[&str]| -> Result<String, String> {
            match names.as_slice() {
                [name] => check(known, name).map(|_| name.clone()),
                _ => Err(format!("--source {layer}= takes one of {}", known.join(", "))),
            }
        };
        match layer.trim() {
            "swarms" => {
                if names == ["none"] {
                    self.swarms.clear();
                    return Ok(());
                }
                if names.is_empty() {
                    return Err(format!("--source swarms= takes some of {}, or none", SWARM_SOURCES.join(", ")));
                }
                for name in &names {
                    check(SWARM_SOURCES, name)?;
                }
                self.swarms = names;
            }
            "issues" => self.issues = one(ISSUE_TRACKERS)?,
            "pipeline" => self.pipeline = one(PIPELINE_SOURCES)?,
            "activity" => self.activity = one(ACTIVITY_SOURCES)?,
            other => return Err(format!("unknown source layer '{other}'; expected swarms, issues, pipeline or activity")),
        }
        Ok(())
    }

    pub fn swarm_sources(&self) -> Vec<Box<dyn SwarmSource>> {
        self.swarms.iter().filter_map(|n| swarm_source(n)).collect()
    }

    /// The villages built from discovered swarms, with this config's issues,
    /// pipeline and activity.
    pub fn swarm_villages(&self) -> SwarmVillages {
        SwarmVillages::new(
            issue_tracker(&self.issues).unwrap_or_else(|| Box::new(Nothing)),
            pipeline_source(&self.pipeline).unwrap_or_else(|| Box::new(Nothing)),
            activity_source(&self.activity).unwrap_or_else(|| Box::new(Nothing)),
        )
    }
}

/// Every swarm the sources show, in order; a project already shown by an
/// earlier source is skipped (tmux before herdr before saved swarms).
pub async fn discover(sources: &[Box<dyn SwarmSource>], transport: &ServerTransport) -> Vec<SwarmSnapshot> {
    let mut swarms: Vec<SwarmSnapshot> = Vec::new();
    for source in sources {
        for snap in source.swarms(transport).await {
            if !swarms.iter().any(|s| s.project_name == snap.project_name) {
                swarms.push(snap);
            }
        }
    }
    swarms
}

// ---------------------------------------------------------------------------
// The composition: every village source, and the worlds above them
// ---------------------------------------------------------------------------

/// What `/api/village` serves: every village, and every world pairing a dev
/// village with its platform.
#[derive(Debug, Clone, Default, Serialize)]
pub struct VillageResponse {
    pub villages: Vec<VillageSnapshot>,
    pub worlds: Vec<WorldSnapshot>,
}

/// The village sources the web view reads, shared between requests.
#[derive(Clone)]
pub struct Villages {
    sources: Arc<Vec<Box<dyn VillageSource>>>,
    worlds: Arc<Vec<WorldConfig>>,
}

impl Default for Villages {
    /// Swarm villages from the default layers, no Temporal, no worlds.
    fn default() -> Self {
        Self::new(vec![Box::new(SourceConfig::default().swarm_villages())], Vec::new())
    }
}

impl Villages {
    pub fn new(sources: Vec<Box<dyn VillageSource>>, worlds: Vec<WorldConfig>) -> Self {
        Self { sources: Arc::new(sources), worlds: Arc::new(worlds) }
    }

    /// Swarm villages from `config`, a village per Temporal namespace, and a
    /// world per pairing.
    pub fn from_config(config: &SourceConfig, temporal: Vec<TemporalSource>, worlds: Vec<WorldConfig>) -> Self {
        let mut sources: Vec<Box<dyn VillageSource>> = vec![Box::new(config.swarm_villages())];
        sources.extend(temporal.into_iter().map(|t| Box::new(TemporalVillages::new(t)) as Box<dyn VillageSource>));
        Self::new(sources, worlds)
    }

    pub fn source_names(&self) -> Vec<String> {
        self.sources.iter().map(|s| s.name()).collect()
    }

    pub async fn snapshot(&self, transport: &ServerTransport, swarms: &[SwarmSnapshot]) -> VillageResponse {
        let mut readings = Vec::new();
        for source in self.sources.iter() {
            readings.extend(source.read(transport, swarms).await);
        }
        let worlds = self
            .worlds
            .iter()
            .filter_map(|w| {
                let dev = readings.iter().find(|r| r.village.project == w.dev)?;
                let platform = readings.iter().find(|r| r.village.project == w.platform)?;
                Some(build_world(w, &dev.village, &dev.issues, &platform.village))
            })
            .collect();
        VillageResponse { villages: readings.into_iter().map(|r| r.village).collect(), worlds }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::village::{House, HouseKind, HouseStatus};

    #[test]
    fn source_specs_swap_layers() {
        let mut c = SourceConfig::default();
        assert_eq!(c.swarms, vec!["tmux", "herdr", "saved"]);
        c.set("swarms=herdr").unwrap();
        c.set("issues=none").unwrap();
        c.set("pipeline=none").unwrap();
        c.set("activity=none").unwrap();
        assert_eq!(c, SourceConfig { swarms: vec!["herdr".into()], issues: "none".into(), pipeline: "none".into(), activity: "none".into() });
        c.set("swarms=none").unwrap();
        assert!(c.swarms.is_empty());
        assert!(c.set("issues=jira").unwrap_err().contains("github, none"));
        assert!(c.set("issues=github,none").is_err());
        assert!(c.set("weather=sunny").is_err());
        assert!(c.set("issues").is_err());
        assert_eq!(SourceConfig::default().swarm_sources().len(), SWARM_SOURCES.len());
    }

    fn empty_swarm(project: &str) -> SwarmSnapshot {
        let agent = |role: &str| crate::web::AgentSnapshot {
            id: format!("{project}/{role}"),
            role: role.into(),
            state: "Idle".into(),
            is_manager: role == "manager",
            waiting_for_input: false,
            current_issue: None,
            current_issue_title: None,
            pane_content: String::new(),
            tmux_target: String::new(),
            health: "Healthy".into(),
            completed_issue_count: 0,
            resurrection_attempts: 0,
            status_timestamp: None,
            worktree_path: String::new(),
            branch: None,
        };
        SwarmSnapshot {
            project_name: project.into(),
            repo_path: String::new(),
            agent_type: "claude".into(),
            workflow: None,
            tmux_session: String::new(),
            stopped: false,
            busy_count: 0,
            idle_count: 0,
            attention_count: 0,
            manager: agent("manager"),
            workers: Vec::new(),
            issues: Vec::new(),
        }
    }

    /// A fixed set of swarms, standing in for tmux or herdr.
    struct Fixed(&'static str, Vec<&'static str>);

    impl SwarmSource for Fixed {
        fn name(&self) -> &'static str {
            self.0
        }
        fn swarms<'a>(&'a self, _: &'a ServerTransport) -> BoxFuture<'a, Vec<SwarmSnapshot>> {
            let snaps = self
                .1
                .iter()
                .map(|p| {
                    let mut s = empty_swarm(p);
                    s.agent_type = self.0.to_string();
                    s
                })
                .collect();
            Box::pin(async move { snaps })
        }
    }

    #[tokio::test]
    async fn earlier_swarm_sources_own_a_project() {
        let sources: Vec<Box<dyn SwarmSource>> = vec![Box::new(Fixed("tmux", vec!["shop"])), Box::new(Fixed("herdr", vec!["shop", "docs"]))];
        let got: Vec<(String, String)> = discover(&sources, &ServerTransport::default())
            .await
            .into_iter()
            .map(|s| (s.project_name, s.agent_type))
            .collect();
        assert_eq!(got, vec![("shop".into(), "tmux".into()), ("docs".into(), "herdr".into())]);
    }

    /// A village source with one fixed village, standing in for a platform.
    struct Platform;

    impl VillageSource for Platform {
        fn name(&self) -> String {
            "platform".into()
        }
        fn read<'a>(&'a self, _: &'a ServerTransport, _: &'a [SwarmSnapshot]) -> BoxFuture<'a, Vec<Reading>> {
            let house = |id: &str| House {
                id: id.into(),
                role: "CheckoutWorkstream".into(),
                kind: HouseKind::Workshop,
                status: HouseStatus::Working,
                issue: None,
                issue_title: None,
                priority: None,
                crew: Vec::new(),
                subagents: Vec::new(),
                action: None,
            };
            let village = VillageSnapshot {
                project: "ops".into(),
                workflow: None,
                repo_path: String::new(),
                manager: house("workers"),
                houses: vec![house("checkout-7")],
                backlog: Vec::new(),
                board: Vec::new(),
                pipeline: Pipeline::default(),
                builds: vec!["abcdef1".into()],
            };
            Box::pin(async move { vec![Reading { village, issues: Vec::new() }] })
        }
    }

    #[tokio::test]
    async fn any_village_source_can_be_a_world_platform() {
        let villages = Villages::new(
            vec![Box::new(SourceConfig { issues: "none".into(), pipeline: "none".into(), activity: "none".into(), ..SourceConfig::default() }.swarm_villages()), Box::new(Platform)],
            vec![WorldConfig::parse("shop=ops", "incident").unwrap()],
        );
        assert_eq!(villages.source_names(), vec!["swarms (none, none, none)", "platform"]);
        let swarms = vec![empty_swarm("shop")];
        let r = villages.snapshot(&ServerTransport::default(), &swarms).await;
        assert_eq!(r.villages.iter().map(|v| v.project.as_str()).collect::<Vec<_>>(), vec!["shop", "ops"]);
        assert_eq!(r.worlds.len(), 1);
        assert_eq!(r.worlds[0].platform_builds, vec!["abcdef1"]);
        assert_eq!(r.worlds[0].landed, Some(false), "the dev village has no release");
    }
}
