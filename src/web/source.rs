//! Sources: where the village's data comes from, in layers that can each be
//! swapped without touching the others.
//!
//! ```text
//! villages   VillageSource   swarm villages (built from the layers below), temporal,
//!                            any framework's agents-village reports (framework.rs)
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
use super::{AgentSnapshot, IssueSnapshot, SwarmSnapshot};
use crate::model::issue::FeatureLink;
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
    /// Which subtasks make each open feature (a parent issue split into
    /// subtasks); none when the tracker can't say.
    fn features<'a>(&'a self, _transport: &'a ServerTransport, _repo: &'a Path) -> BoxFuture<'a, Vec<FeatureLink>> {
        Box::pin(async { Vec::new() })
    }
    /// Recently closed issues, for the titles of landed work; none when the
    /// tracker can't say.
    fn closed<'a>(&'a self, _transport: &'a ServerTransport, _repo: &'a Path) -> BoxFuture<'a, Vec<IssueSnapshot>> {
        Box::pin(async { Vec::new() })
    }
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

/// GitHub issues, through `gh`. Feature links are cached per parent, so each
/// costs at most one `gh` round trip per refresh.
pub struct GitHubIssues {
    links: crate::github::FeatureLinks,
}

impl Default for GitHubIssues {
    fn default() -> Self {
        Self { links: crate::github::FeatureLinks::new(super::village::extras_ttl()) }
    }
}

impl IssueTracker for GitHubIssues {
    fn name(&self) -> &'static str {
        "github"
    }
    fn issues<'a>(&'a self, transport: &'a ServerTransport, repo: &'a Path, _project: &'a str) -> BoxFuture<'a, Vec<IssueSnapshot>> {
        Box::pin(async move {
            let Ok(list) = crate::github::fetch_issues(transport, repo).await else {
                return Vec::new();
            };
            // `gh` reports each issue's URL; the origin remote is only a
            // fallback, resolved once and only if some issue came without one.
            let repo_url = match list.iter().any(|i| i.url.is_none()) {
                true => crate::github::origin_web_url(transport, repo).await,
                false => None,
            };
            list.iter().map(|i| IssueSnapshot::from_issue(i, repo_url.as_deref())).collect()
        })
    }
    fn features<'a>(&'a self, transport: &'a ServerTransport, repo: &'a Path) -> BoxFuture<'a, Vec<FeatureLink>> {
        Box::pin(async move {
            let parents = crate::github::fetch_feature_parents(transport, repo).await;
            self.links.get(transport, repo, &parents).await
        })
    }
    fn closed<'a>(&'a self, transport: &'a ServerTransport, repo: &'a Path) -> BoxFuture<'a, Vec<IssueSnapshot>> {
        Box::pin(async move {
            let list = crate::github::fetch_closed_issues(transport, repo).await.unwrap_or_default();
            list.iter().map(|i| IssueSnapshot::from_issue(i, None)).collect()
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
        "github" => Some(Box::new(GitHubIssues::default())),
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

/// Every swarm the sources show, in order (tmux before herdr before saved
/// swarms), one per repo:
///
/// - The same repo (`repo_key`) from two sources is one swarm: the earlier
///   source's, plus the later one's manager when the earlier had none and its
///   workers in worktrees the earlier didn't show.
/// - Two different repos with the same folder name are both kept; the later
///   one is named `<name>@<parent folder>` so pages can tell them apart.
/// - A swarm with no `repo_key` (its source couldn't ask git) is matched by
///   name, and an earlier source owns the name.
pub async fn discover(sources: &[Box<dyn SwarmSource>], transport: &ServerTransport) -> Vec<SwarmSnapshot> {
    let mut swarms: Vec<SwarmSnapshot> = Vec::new();
    for source in sources {
        for snap in source.swarms(transport).await {
            add_swarm(&mut swarms, snap);
        }
    }
    swarms
}

fn add_swarm(swarms: &mut Vec<SwarmSnapshot>, mut snap: SwarmSnapshot) {
    if !snap.repo_key.is_empty()
        && let Some(same) = swarms.iter_mut().find(|s| s.repo_key == snap.repo_key)
    {
        merge_swarm(same, snap);
        return;
    }
    let Some(namesake) = swarms.iter().find(|s| s.project_name == snap.project_name) else {
        swarms.push(snap);
        return;
    };
    if namesake.repo_key.is_empty() || snap.repo_key.is_empty() {
        return;
    }
    // A different repo with the same folder name.
    let parent = std::path::Path::new(&snap.repo_path)
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|p| p.to_str())
        .unwrap_or("other")
        .to_string();
    let mut name = format!("{}@{parent}", snap.project_name);
    let mut n = 2;
    while swarms.iter().any(|s| s.project_name == name) {
        name = format!("{}@{parent}-{n}", snap.project_name);
        n += 1;
    }
    let old = format!("{}/", snap.project_name);
    for agent in std::iter::once(&mut snap.manager).chain(snap.workers.iter_mut()) {
        if let Some(role) = agent.id.strip_prefix(&old) {
            agent.id = format!("{name}/{role}");
        }
    }
    snap.project_name = name;
    swarms.push(snap);
}

/// Fold `other` (the same repo, from a later source) into `swarm`.
fn merge_swarm(swarm: &mut SwarmSnapshot, other: SwarmSnapshot) {
    let project = swarm.project_name.clone();
    let rename = |mut a: AgentSnapshot, role: String| {
        a.id = format!("{project}/{role}");
        a.role = role;
        a
    };
    if swarm.manager.state == "Missing" && other.manager.state != "Missing" {
        swarm.manager = rename(other.manager, "manager".to_string());
    }
    for worker in other.workers {
        let shown = swarm.workers.iter().any(|w| {
            (!worker.worktree_path.is_empty() && w.worktree_path == worker.worktree_path)
                || (w.mux == worker.mux && w.tmux_target == worker.tmux_target)
        });
        if shown {
            continue;
        }
        let mut role = worker.role.clone();
        let mut n = swarm.workers.len() as u32 + 1;
        while swarm.workers.iter().any(|w| w.role == role) {
            role = format!("worker-{n}");
            n += 1;
        }
        if worker.state.starts_with("Working") {
            swarm.busy_count += 1;
        } else {
            swarm.idle_count += 1;
        }
        if worker.waiting_for_input {
            swarm.attention_count += 1;
        }
        swarm.workers.push(rename(worker, role));
    }
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

    /// Swarm villages from `config`, a village per Temporal namespace, the
    /// villages each outside framework reports, and a world per pairing.
    pub fn from_config(
        config: &SourceConfig,
        temporal: Vec<TemporalSource>,
        kubernetes: Vec<super::kubernetes::KubeSource>,
        frameworks: Vec<super::framework::ReportSpec>,
        worlds: Vec<WorldConfig>,
    ) -> Self {
        let mut sources: Vec<Box<dyn VillageSource>> = vec![Box::new(config.swarm_villages())];
        sources.extend(temporal.into_iter().map(|t| Box::new(TemporalVillages::new(t)) as Box<dyn VillageSource>));
        sources.extend(kubernetes.into_iter().map(|k| Box::new(super::kubernetes::KubeVillages::new(k)) as Box<dyn VillageSource>));
        sources.extend(frameworks.into_iter().map(|f| Box::new(super::framework::ReportVillages::new(f)) as Box<dyn VillageSource>));
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
        let built: Vec<_> = self
            .worlds
            .iter()
            .filter_map(|w| {
                let dev = readings.iter().find(|r| r.village.project == w.dev)?;
                let platform = readings.iter().find(|r| r.village.project == w.platform)?;
                Some((w.dev.clone(), build_world(w, &dev.village, &dev.issues, &platform.village)))
            })
            .collect();
        // A platform's open incidents are the health of the environment it runs.
        for (dev_name, world) in &built {
            if let Some(dev) = readings.iter_mut().find(|r| &r.village.project == dev_name) {
                super::world::mark_incidents(&mut dev.village, world);
            }
        }
        let worlds = built.into_iter().map(|(_, w)| w).collect();
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
            mux: Default::default(),
        };
        SwarmSnapshot {
            project_name: project.into(),
            repo_path: String::new(),
            repo_key: String::new(),
            manager_by: None,
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

    /// Swarms given whole, standing in for a source that asked git.
    struct Given(&'static str, Vec<SwarmSnapshot>);

    impl SwarmSource for Given {
        fn name(&self) -> &'static str {
            self.0
        }
        fn swarms<'a>(&'a self, _: &'a ServerTransport) -> BoxFuture<'a, Vec<SwarmSnapshot>> {
            let snaps = self.1.clone();
            Box::pin(async move { snaps })
        }
    }

    fn keyed(project: &str, root: &str, workers: &[(&str, &str, crate::web::Mux)]) -> SwarmSnapshot {
        let mut s = empty_swarm(project);
        s.repo_path = root.to_string();
        s.repo_key = format!("{root}/.git");
        for (role, wt, mux) in workers {
            let mut w = s.manager.clone();
            w.id = format!("{project}/{role}");
            w.role = role.to_string();
            w.is_manager = false;
            w.state = "Working #1".into();
            w.worktree_path = wt.to_string();
            w.tmux_target = format!("{wt}-pane");
            w.mux = *mux;
            s.workers.push(w);
            s.busy_count += 1;
        }
        s
    }

    #[tokio::test]
    async fn one_repo_seen_through_tmux_and_herdr_is_one_swarm() {
        use crate::web::Mux;
        let mut tmux = keyed("shop", "/src/shop", &[("worker-1", "/src/shop-wt-1", Mux::Tmux)]);
        tmux.manager.state = "Missing".into();
        let herdr = keyed(
            "shop",
            "/src/shop",
            &[("worker-1", "/src/shop-wt-1", Mux::Herdr), ("worker-1", "/src/hotfix", Mux::Herdr)],
        );
        let sources: Vec<Box<dyn SwarmSource>> = vec![Box::new(Given("tmux", vec![tmux])), Box::new(Given("herdr", vec![herdr]))];
        let got = discover(&sources, &ServerTransport::default()).await;
        assert_eq!(got.len(), 1);
        let s = &got[0];
        // herdr's manager stands in for tmux's missing one.
        assert_ne!(s.manager.state, "Missing");
        // wt-1 is shown once (tmux's); herdr's other worktree joins under a free number.
        let workers: Vec<(&str, &str, Mux)> = s.workers.iter().map(|w| (w.role.as_str(), w.worktree_path.as_str(), w.mux)).collect();
        assert_eq!(workers, vec![("worker-1", "/src/shop-wt-1", Mux::Tmux), ("worker-2", "/src/hotfix", Mux::Herdr)]);
        assert_eq!(s.workers[1].id, "shop/worker-2");
        assert_eq!(s.busy_count, 2);
    }

    #[tokio::test]
    async fn two_repos_with_the_same_folder_name_stay_apart() {
        let work = keyed("shop", "/work/shop", &[]);
        let home = keyed("shop", "/home/me/shop", &[]);
        let sources: Vec<Box<dyn SwarmSource>> = vec![Box::new(Given("herdr", vec![work, home]))];
        let got = discover(&sources, &ServerTransport::default()).await;
        let names: Vec<(&str, &str)> = got.iter().map(|s| (s.project_name.as_str(), s.manager.id.as_str())).collect();
        assert_eq!(names, vec![("shop", "shop/manager"), ("shop@me", "shop@me/manager")]);
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
                links: Vec::new(),
                question: None,
                answer_url: None,
                branch: None,
                since: None,
            };
            let village = VillageSnapshot {
                project: "ops".into(),
                plane: crate::web::framework::Plane::Runtime,
                framework: "fixed".into(),
                workflow: None,
                repo_path: String::new(),
                manager: house("workers"),
                houses: vec![house("checkout-7")],
                backlog: Vec::new(),
                board: Vec::new(),
                pipeline: Pipeline::default(),
                builds: vec!["abcdef1".into()],
                features: Vec::new(),
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
