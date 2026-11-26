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
use crate::model::recognize::AgentExplanation;
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
    /// Drop what it has cached for `repo`, so the next read asks again: an
    /// issue the village just changed (a proposal approved from the board)
    /// moves on the next poll, not a TTL later.
    fn forget(&self, _repo: &Path) {}
}

/// Who is working: the swarms running under some agent host.
pub trait SwarmSource: Send + Sync {
    fn name(&self) -> &'static str;
    /// Every swarm this host runs; none when the host isn't there.
    fn swarms<'a>(&'a self, transport: &'a ServerTransport) -> BoxFuture<'a, Vec<SwarmSnapshot>>;
    /// Every agent this source saw and what became of it: its swarm and
    /// role, the rule that made a manager, or why it was left out
    /// (`GET /api/swarms?explain=1`). By default, the agents of the swarms
    /// it shows; a source that can leave agents out says why.
    fn explain<'a>(&'a self, transport: &'a ServerTransport) -> BoxFuture<'a, Vec<AgentExplanation>> {
        Box::pin(async move { explain_snapshots(self.name(), &self.swarms(transport).await) })
    }
}

/// The agents of `swarms` as a source that leaves none out explains them.
pub fn explain_snapshots(source: &str, swarms: &[SwarmSnapshot]) -> Vec<AgentExplanation> {
    swarms
        .iter()
        .flat_map(|s| {
            let manager = (s.manager.state != "Missing").then_some(&s.manager);
            manager.into_iter().chain(&s.workers).map(move |a| AgentExplanation {
                source: source.to_string(),
                pane: a.tmux_target.clone(),
                cwd: a.worktree_path.clone(),
                swarm: Some(s.project_name.clone()),
                repo: (!s.repo_key.is_empty()).then(|| s.repo_key.clone()),
                role: Some(a.role.clone()),
                manager_rule: a.is_manager.then_some(s.manager_by.as_ref()).flatten().map(|p| p.rule),
                manager_because: a.is_manager.then_some(s.manager_by.as_ref()).flatten().map(|p| p.rule.describe().to_string()),
                ambiguous: a.is_manager.then_some(s.manager_by.as_ref()).flatten().map(|p| p.ambiguous),
                ..AgentExplanation::default()
            })
        })
        .collect()
}

/// Every agent every source saw, in source order.
pub async fn explain(sources: &[Box<dyn SwarmSource>], transport: &ServerTransport) -> Vec<AgentExplanation> {
    let mut out = Vec::new();
    for source in sources {
        out.extend(source.explain(transport).await);
    }
    out
}

/// The backlog a swarm works from.
pub trait IssueTracker: Send + Sync {
    fn name(&self) -> &'static str;
    /// Open issues for the repository at `repo`; none when it can't be read.
    fn issues<'a>(&'a self, transport: &'a ServerTransport, repo: &'a Path, project: &'a str) -> BoxFuture<'a, Vec<IssueSnapshot>>;
    /// Which subtasks make each open feature (a parent issue split into
    /// subtasks), and each recently closed one marked `closed`; none when the
    /// tracker can't say.
    fn features<'a>(&'a self, _transport: &'a ServerTransport, _repo: &'a Path) -> BoxFuture<'a, Vec<FeatureLink>> {
        Box::pin(async { Vec::new() })
    }
    /// Recently closed issues, for the titles of landed work; none when the
    /// tracker can't say.
    fn closed<'a>(&'a self, _transport: &'a ServerTransport, _repo: &'a Path) -> BoxFuture<'a, Vec<IssueSnapshot>> {
        Box::pin(async { Vec::new() })
    }
    /// The merge job's recent comments, `(issue, at, body)`, for where the
    /// gate handed work back (`gate::bounce_history`); none when the tracker can't say.
    fn merge_comments<'a>(&'a self, _transport: &'a ServerTransport, _repo: &'a Path) -> BoxFuture<'a, Vec<(u32, String, String)>> {
        Box::pin(async { Vec::new() })
    }
    /// A summary of recent work for the village (`github.rs`, #498), cached
    /// by the tracker; none when it can't say.
    fn summary<'a>(&'a self, _transport: &'a ServerTransport, _repo: &'a Path, _integration: &'a str) -> BoxFuture<'a, Option<super::github::Summary>> {
        Box::pin(async { None })
    }
    /// Drop what it cached for `repo`.
    fn forget(&self, _repo: &Path) {}
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
    /// One summary per repo, refreshed every 90 s at most (#498).
    summaries: super::github::Summaries,
}

impl Default for GitHubIssues {
    fn default() -> Self {
        Self { links: crate::github::FeatureLinks::new(super::village::extras_ttl()), summaries: Default::default() }
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
            let (open, closed) = crate::github::fetch_feature_parents(transport, repo).await;
            let mut links = self.links.get(transport, repo, &open).await;
            let ended = self.links.get(transport, repo, &closed).await.into_iter().map(|l| FeatureLink { closed: true, ..l });
            links.extend(ended);
            links
        })
    }
    fn merge_comments<'a>(&'a self, transport: &'a ServerTransport, repo: &'a Path) -> BoxFuture<'a, Vec<(u32, String, String)>> {
        Box::pin(async move { crate::github::fetch_merge_comments(transport, repo).await.unwrap_or_default() })
    }
    fn closed<'a>(&'a self, transport: &'a ServerTransport, repo: &'a Path) -> BoxFuture<'a, Vec<IssueSnapshot>> {
        Box::pin(async move {
            let list = crate::github::fetch_closed_issues(transport, repo).await.unwrap_or_default();
            list.iter().map(|i| IssueSnapshot::from_issue(i, None)).collect()
        })
    }
    fn summary<'a>(&'a self, transport: &'a ServerTransport, repo: &'a Path, integration: &'a str) -> BoxFuture<'a, Option<super::github::Summary>> {
        Box::pin(self.summaries.get(transport, repo, integration))
    }
    fn forget(&self, repo: &Path) {
        self.summaries.forget(repo);
    }
}

/// Values read in the background (#534): a request is answered at once from
/// what was last read, however old, and a value older than `ttl` (or one
/// `get`'s caller no longer trusts) is read again in a spawned task. So no
/// request waits on `gh` or `git ls-remote`: only the call that starts the
/// first read of a key, or the first after [`Background::forget`], waits, and
/// at most `cold_wait`. One read per key at a time.
pub struct Background<K, V> {
    ttl: Duration,
    cold_wait: Duration,
    entries: Arc<Mutex<HashMap<K, Slot<V>>>>,
}

struct Slot<V> {
    value: Option<(Instant, V)>,
    /// Forgotten: the value is kept to serve, but no longer trusted.
    stale: bool,
    /// Bumped by `forget`: a read started before it doesn't write back.
    generation: u64,
    /// A read in flight; its sender is dropped when it ends.
    reading: Option<tokio::sync::watch::Receiver<()>>,
}

impl<V> Default for Slot<V> {
    fn default() -> Self {
        Self { value: None, stale: false, generation: 0, reading: None }
    }
}

impl<K, V> Background<K, V>
where
    K: Eq + Hash + Clone + Send + 'static,
    V: Clone + Send + 'static,
{
    pub fn new(ttl: Duration, cold_wait: Duration) -> Self {
        Self { ttl, cold_wait, entries: Arc::new(Mutex::new(HashMap::new())) }
    }

    /// The last value read for `key`, starting a read when it is older than
    /// the TTL, was forgotten, or `trusted` says it no longer holds. The call
    /// that starts a first read, or the first after a `forget`, waits up to
    /// `cold_wait` for it; any other call doesn't wait. None until a first
    /// read has finished.
    pub async fn get<F>(&self, key: K, trusted: impl FnOnce(&V) -> bool, fetch: impl FnOnce() -> F) -> Option<V>
    where
        F: Future<Output = V> + Send + 'static,
    {
        let wait = {
            let mut m = self.entries.lock().unwrap_or_else(|e| e.into_inner());
            let slot = m.entry(key.clone()).or_default();
            let fresh = !slot.stale && slot.value.as_ref().is_some_and(|(at, v)| at.elapsed() < self.ttl && trusted(v));
            let start = !fresh && slot.reading.is_none();
            if start {
                let (done, rx) = tokio::sync::watch::channel(());
                slot.reading = Some(rx);
                let (entries, key, generation, work) = (self.entries.clone(), key.clone(), slot.generation, fetch());
                tokio::spawn(async move {
                    use futures::FutureExt;
                    // A read that panics is retried next time, not stuck "in flight".
                    let read = std::panic::AssertUnwindSafe(work).catch_unwind().await;
                    let mut m = entries.lock().unwrap_or_else(|e| e.into_inner());
                    // Dropped meanwhile (`drop_where`): nothing to write back.
                    // Forgotten meanwhile: what it read is from before, and a
                    // newer read may already be running.
                    if let Some(slot) = m.get_mut(&key)
                        && slot.generation == generation
                    {
                        if let Ok(v) = read {
                            slot.value = Some((Instant::now(), v));
                            slot.stale = false;
                        }
                        slot.reading = None;
                    }
                    drop(done);
                });
            }
            match slot.value {
                None if start => slot.reading.clone(),
                Some(_) if start && slot.stale => slot.reading.clone(),
                _ => None,
            }
        };
        if let Some(mut wait) = wait {
            let _ = tokio::time::timeout(self.cold_wait, wait.changed()).await;
        }
        self.peek(&key)
    }

    /// The last value read for `key`, if any, without starting a read.
    pub fn peek(&self, key: &K) -> Option<V> {
        let m = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        m.get(key).and_then(|s| s.value.as_ref()).map(|(_, v)| v.clone())
    }

    /// Stop trusting `key`'s value: the next `get` reads it again, waiting
    /// up to `cold_wait`, and is served the old value if the read takes longer.
    /// A read already running is disowned.
    pub fn forget(&self, key: &K) {
        if let Some(slot) = self.entries.lock().unwrap_or_else(|e| e.into_inner()).get_mut(key) {
            slot.stale = true;
            slot.generation += 1;
            slot.reading = None;
        }
    }

    /// Drop every value whose key matches.
    pub fn drop_where(&self, mut drop: impl FnMut(&K) -> bool) {
        self.entries.lock().unwrap_or_else(|e| e.into_inner()).retain(|k, _| !drop(k));
    }
}

/// Runs `work`, and logs it (at debug) when it took longer than [`SLOW`]
/// (#534): which `gh` or `git` read is slow, now that none of them keeps a
/// request waiting.
pub async fn timed<T>(what: impl std::fmt::Display, work: impl Future<Output = T>) -> T {
    let start = Instant::now();
    let v = work.await;
    let took = start.elapsed();
    if took > SLOW {
        tracing::debug!("village: {what} took {:.1}s", took.as_secs_f64());
    }
    v
}

/// A village step slower than this is logged ([`timed`]).
pub const SLOW: Duration = Duration::from_secs(1);

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

    /// Drop `key`'s value, so the next `get_or` fetches it again.
    pub fn forget(&self, key: &K) {
        if let Ok(mut m) = self.entries.lock() {
            m.remove(key);
        }
    }

    /// Drop every value whose key matches.
    pub fn forget_where(&self, mut drop: impl FnMut(&K) -> bool) {
        if let Ok(mut m) = self.entries.lock() {
            m.retain(|k, _| !drop(k));
        }
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

    /// Every source forgets what it cached for `repo` (`VillageSource::forget`).
    pub fn forget(&self, repo: &Path) {
        for source in self.sources.iter() {
            source.forget(repo);
        }
    }

    pub fn source_names(&self) -> Vec<String> {
        self.sources.iter().map(|s| s.name()).collect()
    }

    pub async fn snapshot(&self, transport: &ServerTransport, swarms: &[SwarmSnapshot]) -> VillageResponse {
        let (readings, worlds) = self.read(transport, swarms).await;
        VillageResponse { villages: readings.into_iter().map(|r| r.village).collect(), worlds }
    }

    /// Every village with its issues, as `snapshot` draws them: what
    /// `/api/report` exports.
    pub async fn readings(&self, transport: &ServerTransport, swarms: &[SwarmSnapshot]) -> Vec<Reading> {
        self.read(transport, swarms).await.0
    }

    async fn read(&self, transport: &ServerTransport, swarms: &[SwarmSnapshot]) -> (Vec<Reading>, Vec<WorldSnapshot>) {
        let mut readings = Vec::new();
        for source in self.sources.iter() {
            // A source that keeps a request waiting is a bug (#534): say which.
            let start = Instant::now();
            readings.extend(source.read(transport, swarms).await);
            if start.elapsed() > SLOW {
                tracing::warn!("village: source {} kept the request waiting {:.1}s", source.name(), start.elapsed().as_secs_f64());
            }
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
        (readings, built.into_iter().map(|(_, w)| w).collect())
    }
}

#[cfg(test)]
mod tests {

    /// Waits (up to 10 s) until `cache` holds `want` for `key`: a background
    /// read has ended. Generous, so a loaded CI runner doesn't fail it.
    async fn settles(cache: &Background<u32, u32>, key: u32, want: u32) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while cache.peek(&key) != Some(want) {
            assert!(Instant::now() < deadline, "the read never stored {want}");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// A read that takes `secs`, counting itself in `reads`.
    fn slow_read(reads: &Arc<std::sync::atomic::AtomicUsize>, secs: f64, v: u32) -> impl FnOnce() -> futures::future::BoxFuture<'static, u32> {
        let reads = reads.clone();
        move || {
            Box::pin(async move {
                reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                tokio::time::sleep(Duration::from_secs_f64(secs)).await;
                v
            })
        }
    }

    /// #534: a value past its TTL is served as it was while it is read
    /// again in the background, and only one read runs at a time.
    #[tokio::test]
    async fn a_stale_value_is_served_while_it_is_read_again() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let reads = Arc::new(AtomicUsize::new(0));
        let cache: Background<u32, u32> = Background::new(Duration::from_millis(10), Duration::from_secs(30));
        assert_eq!(cache.get(1, |_| true, || async { 10 }).await, Some(10), "the first read is waited for");
        tokio::time::sleep(Duration::from_millis(20)).await;
        let start = Instant::now();
        assert_eq!(cache.get(1, |_| true, slow_read(&reads, 1.5, 20)).await, Some(10), "stale, not waited for");
        assert_eq!(cache.get(1, |_| true, slow_read(&reads, 1.5, 30)).await, Some(10));
        assert!(start.elapsed() < Duration::from_secs(1), "no request waited on the read");
        settles(&cache, 1, 20).await;
        assert_eq!(reads.load(Ordering::SeqCst), 1, "one read at a time");
        // A value its caller no longer trusts is read again before the TTL.
        let fresh: Background<u32, u32> = Background::new(Duration::from_secs(3600), Duration::from_secs(30));
        assert_eq!(fresh.get(1, |_| true, || async { 1 }).await, Some(1));
        assert_eq!(fresh.get(1, |_| true, || async { 2 }).await, Some(1), "trusted and fresh");
        fresh.get(1, |v| *v != 1, || async { 3 }).await;
        settles(&fresh, 1, 3).await;
    }

    /// #534: a first read slower than the cold wait isn't waited for past
    /// it, later calls don't wait at all, and a read that panics is retried.
    #[tokio::test]
    async fn a_slow_first_read_waits_only_the_cold_wait() {
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let cache: Background<u32, u32> = Background::new(Duration::from_secs(3600), Duration::from_millis(500));
        let start = Instant::now();
        assert_eq!(cache.get(1, |_| true, slow_read(&reads, 60.0, 1)).await, None);
        assert!(start.elapsed() < Duration::from_secs(5), "waited the cold wait, not the read");
        let again = Instant::now();
        assert_eq!(cache.get(1, |_| true, slow_read(&reads, 60.0, 1)).await, None);
        assert!(again.elapsed() < Duration::from_millis(400), "only the call that started the read waits");

        assert_eq!(cache.get(2, |_| true, || async { panic!("gh crashed") }).await, None);
        assert_eq!(cache.get(2, |_| true, || async { 7 }).await, Some(7), "not stuck after a panic");
    }

    /// #534: after `forget` (a user's action) the next call reads again and
    /// waits for it, up to the cold wait; a slower read leaves the old value
    /// served, not nothing; and a read begun before the forget never writes
    /// back what it saw.
    #[tokio::test]
    async fn a_forgotten_value_is_read_again_and_an_older_read_is_disowned() {
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let cache: Background<u32, u32> = Background::new(Duration::from_millis(10), Duration::from_secs(30));
        assert_eq!(cache.get(1, |_| true, || async { 1 }).await, Some(1));
        tokio::time::sleep(Duration::from_millis(20)).await;
        // A read from before the action, still running when it is forgotten.
        assert_eq!(cache.get(1, |_| true, slow_read(&reads, 1.0, 99)).await, Some(1));
        cache.forget(&1);
        assert_eq!(cache.get(1, |_| true, || async { 2 }).await, Some(2), "read again, and waited for");
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert_eq!(cache.peek(&1), Some(2), "the older read didn't write back");

        let quick: Background<u32, u32> = Background::new(Duration::from_secs(3600), Duration::from_millis(100));
        assert_eq!(quick.get(1, |_| true, || async { 1 }).await, Some(1));
        quick.forget(&1);
        assert_eq!(quick.get(1, |_| true, slow_read(&reads, 1.0, 2)).await, Some(1), "the old value, not nothing");
        settles(&quick, 1, 2).await;
    }

    #[tokio::test]
    async fn a_forgotten_entry_is_fetched_again() {
        let cache: TtlCache<u32, u32> = TtlCache::new(Duration::from_secs(3600));
        assert_eq!(cache.get_or(1, || async { 10 }).await, 10);
        assert_eq!(cache.get_or(1, || async { 20 }).await, 10, "cached");
        cache.forget(&1);
        assert_eq!(cache.get_or(1, || async { 30 }).await, 30, "asked again");
    }
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
            health: "Healthy".into(),
            ..Default::default()
        };
        SwarmSnapshot {
            project_name: project.into(),
            agent_type: "claude".into(),
            manager: agent("manager"),
            ..Default::default()
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
                ..Default::default()
            };
            let village = VillageSnapshot {
                project: "ops".into(),
                plane: crate::web::framework::Plane::Runtime,
                framework: "fixed".into(),
                manager: house("workers"),
                houses: vec![house("checkout-7")],
                builds: vec!["abcdef1".into()],
                ..Default::default()
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
