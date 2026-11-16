//! A valley of related repos (#510, phase 1 of #494): a group of repos that
//! depend on each other, drawn as one valley, libraries upstream and their
//! consumers downstream.
//!
//! A group is declared in `~/.agents-ui/group.json` (or the file `--group
//! <path>` names): a list of groups, each with a name and its repos (by
//! project name or path), each repo with what it depends on and how:
//!
//! ```json
//! [{ "name": "shop", "repos": [
//!     { "repo": "lib-core" },
//!     { "repo": "api", "depends_on": [{ "repo": "lib-core", "via": "package" }] },
//!     { "repo": "web", "depends_on": [{ "repo": "lib-core", "via": "package" },
//!                                     { "repo": "api", "via": "api" }] } ] }]
//! ```
//!
//! `GET /api/valley` answers with each group's repos upstream to downstream
//! (`package` edges ordered; `api` edges are drawn but don't order), its
//! edges, and a summary of each repo from its village. A repo with no
//! running swarm is still there, dormant. What is wrong with the manifest (bad
//! JSON, an unknown repo, a cycle) is said in the answer, never a crash.
//! Each group also carries its `ropes` and `caravans`: issues linked across
//! its repos on GitHub (`crossrepo.rs`, #528).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::village::{HouseStatus, VillageSnapshot};

/// How one repo depends on another.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Via {
    /// It builds against the other's package: the river runs from it.
    #[default]
    Package,
    /// It calls the other's API: a road between them (the grid, in phase 3).
    Api,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DependsOn {
    repo: String,
    via: Via,
    /// For an `api` edge: the provider's version range this repo expects (#512).
    #[serde(default)]
    expects: Option<String>,
    /// For an `api` edge: a contract file in the provider's repo whose
    /// `info.version` is compared instead of its release (#512).
    #[serde(default)]
    contract: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepoDef {
    repo: String,
    #[serde(default)]
    depends_on: Vec<DependsOn>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupDef {
    name: String,
    repos: Vec<RepoDef>,
}

/// An edge from the repo depended on (upstream) to the one depending on it.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub via: Via,
    /// For a `package` edge (#511): the version of `from` that `to` pins
    /// (`pins.rs`), `from`'s latest release and when it was made, and
    /// whether the pin is older. Each absent when it can't be read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behind: Option<bool>,
    /// For an `api` edge (#512): what `to` expects of `from`, and the
    /// contract file compared, as the manifest says.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expects: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract: Option<String>,
}

/// What the valley says about one repo, from its village.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Summary {
    pub working: usize,
    pub idle: usize,
    /// Houses blocked or stalled.
    pub blocked: usize,
    pub backlog: usize,
    /// What its last stop runs: a release tag, else a short commit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
    /// `red` (something waits on you, the build is broken, a deploy
    /// failed), `amber` (a house blocked or stalled), `green`, or `dormant`.
    pub health: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ValleyRepo {
    /// Its name: the project name, or a path's last part.
    pub name: String,
    /// As the manifest wrote it.
    pub repo: String,
    /// The village it is, when one is running: what a click opens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub dormant: bool,
    /// How far downstream: 0 for a repo that builds on no other in the group.
    pub depth: usize,
    pub summary: Summary,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ValleyGroup {
    pub name: String,
    /// Upstream to downstream.
    pub repos: Vec<ValleyRepo>,
    pub edges: Vec<Edge>,
    /// The repos joined by `api` edges, by environment (`grid.rs`, #512).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid: Option<super::grid::Grid>,
    /// What is wrong with this group's part of the manifest.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
    /// Open issues held up by an open issue in another repo of the group
    /// (`crossrepo.rs`, #528), read from GitHub.
    #[serde(default)]
    pub ropes: Vec<super::crossrepo::Rope>,
    /// Parent issues whose subtasks are spread over several repos of the group.
    #[serde(default)]
    pub caravans: Vec<super::crossrepo::Caravan>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ValleyResponse {
    /// The manifest read, and whether it was there.
    pub manifest: String,
    pub found: bool,
    pub groups: Vec<ValleyGroup>,
    /// What is wrong with the manifest as a whole (it isn't JSON, say).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

static MANIFEST: OnceLock<PathBuf> = OnceLock::new();

/// Read the groups from `path` instead of `~/.agents-ui/group.json` (`--group`).
pub fn set_manifest_path(path: PathBuf) {
    let _ = MANIFEST.set(path);
}

/// The manifest the valley reads.
pub fn manifest_path() -> PathBuf {
    MANIFEST.get().cloned().unwrap_or_else(|| {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".agents-ui").join("group.json")
    })
}

/// A repo's name: a project name as it is, a path by its last part.
fn repo_name(repo: &str) -> String {
    let trimmed = repo.trim_end_matches('/');
    if trimmed.contains('/') {
        Path::new(trimmed).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| trimmed.to_string())
    } else {
        trimmed.to_string()
    }
}

/// The village a manifest repo is: by project name, by path, or by a path's last part.
fn village_for<'a>(repo: &str, villages: &'a [VillageSnapshot]) -> Option<&'a VillageSnapshot> {
    let home = dirs::home_dir().map(|h| h.to_string_lossy().into_owned()).unwrap_or_default();
    let path = repo.strip_prefix("~/").map_or_else(|| repo.to_string(), |rest| format!("{home}/{rest}"));
    let path = path.trim_end_matches('/');
    let name = repo_name(repo);
    villages
        .iter()
        .find(|v| v.project == repo || (!v.repo_path.is_empty() && v.repo_path.trim_end_matches('/') == path))
        .or_else(|| villages.iter().find(|v| v.project == name))
}

/// The directory a manifest repo is in, on this machine: its path when the
/// manifest gives one that is there, else its village's repo.
fn repo_dir(repo: &str, villages: &[VillageSnapshot]) -> Option<PathBuf> {
    let home = dirs::home_dir().map(|h| h.to_string_lossy().into_owned()).unwrap_or_default();
    let path = PathBuf::from(repo.strip_prefix("~/").map_or_else(|| repo.to_string(), |rest| format!("{home}/{rest}")));
    if repo.contains('/') && path.is_dir() {
        return Some(path);
    }
    village_for(repo, villages).map(|v| PathBuf::from(&v.repo_path)).filter(|p| !p.as_os_str().is_empty() && p.is_dir())
}

/// How long what was read from repos on disk (tags, manifests, lockfiles)
/// is reused: the page polls every few seconds, and they change rarely.
const VERSIONS_FRESH: std::time::Duration = std::time::Duration::from_secs(30);

/// A library's latest release (its tag, when it was made) and the names it
/// is known by; a consumer's pin of it. Kept for [`VERSIONS_FRESH`].
type LibVersions = (Option<(String, Option<String>)>, Vec<String>);
type Kept<T> = std::sync::Mutex<HashMap<String, (std::time::Instant, T)>>;

fn kept<T: Clone>(cache: &'static OnceLock<Kept<T>>, key: String, read: impl FnOnce() -> T) -> T {
    let map = cache.get_or_init(Default::default);
    if let Some((at, v)) = map.lock().unwrap_or_else(|e| e.into_inner()).get(&key)
        && at.elapsed() < VERSIONS_FRESH
    {
        return v.clone();
    }
    let v = read();
    map.lock().unwrap_or_else(|e| e.into_inner()).insert(key, (std::time::Instant::now(), v.clone()));
    v
}

/// A library's newest version tag and its names, from its repo on disk.
fn lib_versions(dir: Option<&Path>, name: &str) -> LibVersions {
    static CACHE: OnceLock<Kept<LibVersions>> = OnceLock::new();
    let key = format!("{}\0{name}", dir.map(|d| d.to_string_lossy().into_owned()).unwrap_or_default());
    kept(&CACHE, key, || {
        let tag = dir.and_then(super::pins::latest_tag);
        (tag, super::pins::package_names(dir.unwrap_or(Path::new("/nonexistent")), name))
    })
}

/// The commit `origin/<branch>` names in the repo at `dir`, kept [`VERSIONS_FRESH`].
fn branch_head(dir: &Path, branch: &str) -> Option<String> {
    if branch.starts_with('-') || branch.contains("..") {
        return None;
    }
    static CACHE: OnceLock<Kept<Option<String>>> = OnceLock::new();
    kept(&CACHE, format!("{}\0{branch}", dir.to_string_lossy()), || {
        let rev = format!("origin/{branch}^{{commit}}");
        let out = std::process::Command::new("git").arg("-C").arg(dir).args(["rev-parse", "--verify", "-q", &rev]).output().ok().filter(|o| o.status.success())?;
        let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (!sha.is_empty() && sha.chars().all(|c| c.is_ascii_hexdigit())).then_some(sha)
    })
}

/// The newest version tag on commit `sha` in the repo at `dir`, kept
/// [`VERSIONS_FRESH`].
fn tag_at(dir: &Path, sha: &str) -> Option<String> {
    if !sha.chars().all(|c| c.is_ascii_hexdigit()) || sha.is_empty() {
        return None;
    }
    static CACHE: OnceLock<Kept<Option<String>>> = OnceLock::new();
    kept(&CACHE, format!("{}\0{sha}", dir.to_string_lossy()), || {
        let out = std::process::Command::new("git").arg("-C").arg(dir).args(["tag", "--points-at", sha]).output().ok().filter(|o| o.status.success())?;
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::trim)
            .filter(|t| super::pins::release_version(t).is_some() && !super::pins::is_prerelease(t))
            .max_by(|a, b| super::pins::compare(a, b).unwrap_or(std::cmp::Ordering::Equal))
            .map(str::to_string)
    })
}

/// What the repo at `dir` pins of a library known by `names`.
fn pin_of(dir: &Path, names: &[String]) -> Option<(String, super::pins::Pin)> {
    static CACHE: OnceLock<Kept<Option<(String, super::pins::Pin)>>> = OnceLock::new();
    kept(&CACHE, format!("{}\0{}", dir.to_string_lossy(), names.join("\0")), || super::pins::pinned(dir, names))
}

/// A `package` edge's versions (#511): what the consumer pins, the
/// library's latest release (its `release` stop's tag, else its newest
/// version tag), and whether the pin is behind it: an exact pin older than
/// the release, or a range that doesn't take it (`pins::behind`).
fn versions(e: &mut Edge, lib: &str, user: &str, villages: &[VillageSnapshot]) {
    let lib_dir = repo_dir(lib, villages);
    let (tag, names) = lib_versions(lib_dir.as_deref(), &repo_name(lib));
    let stop = village_for(lib, villages).and_then(|v| v.pipeline.stops.iter().find(|s| s.name == "release" && s.tag.is_some()));
    let (latest, latest_at) = match stop {
        Some(s) => (s.tag.clone(), s.deployed_at.clone()),
        None => tag.map_or((None, None), |(t, at)| (Some(t), at)),
    };
    let pin = repo_dir(user, villages).and_then(|d| pin_of(&d, &names));
    e.behind = match (&pin, &latest) {
        (Some((p, kind)), Some(l)) => super::pins::behind(p, *kind, l),
        _ => None,
    };
    e.pinned = pin.map(|(p, _)| p);
    e.latest = latest;
    e.latest_at = latest_at;
}

/// What the valley says about a village.
fn summarize(v: &VillageSnapshot) -> Summary {
    let count = |st: HouseStatus| v.houses.iter().filter(|h| h.status == st).count();
    let blocked = count(HouseStatus::Blocked) + count(HouseStatus::Stalled);
    let last = v.pipeline.stops.last();
    let release = last.and_then(|s| s.tag.clone().or_else(|| s.sha.as_deref().map(|sha| sha.chars().take(7).collect())));
    let build_red = v.build.as_ref().is_some_and(|b| b.state == super::build::BuildState::Red);
    let deploy_failed = v.pipeline.stops.iter().any(|s| s.health.as_ref().is_some_and(|h| h.state == super::village::HealthState::Failed));
    let waiting = !v.board.is_empty() || v.manager.status == HouseStatus::Blocked;
    let health = if build_red || deploy_failed || waiting {
        "red"
    } else if blocked > 0 {
        "amber"
    } else {
        "green"
    };
    Summary {
        working: count(HouseStatus::Working),
        idle: count(HouseStatus::Idle) + count(HouseStatus::Done),
        blocked,
        backlog: v.backlog.len(),
        release,
        health: health.to_string(),
    }
}

/// A village at rest: its manager stopped and nobody in a house (`dormant.rs`).
fn is_dormant(v: &VillageSnapshot) -> bool {
    v.manager.status == HouseStatus::Stopped && v.houses.is_empty()
}

/// One group laid out: its repos ordered upstream to downstream along the
/// `package` edges, with what is wrong said in `errors`.
fn build_group(def: &GroupDef, villages: &[VillageSnapshot], dormant: &HashMap<String, Vec<super::village::Stop>>) -> ValleyGroup {
    let mut errors = Vec::new();
    // The repos, first mention wins.
    let mut defs: Vec<&RepoDef> = Vec::new();
    for r in &def.repos {
        let name = repo_name(&r.repo);
        if name.is_empty() {
            errors.push(format!("{}: a repo with no name", def.name));
        } else if defs.iter().any(|d| repo_name(&d.repo) == name) {
            errors.push(format!("{}: {name} is listed twice", def.name));
        } else {
            defs.push(r);
        }
    }
    let names: Vec<String> = defs.iter().map(|d| repo_name(&d.repo)).collect();
    let known: HashSet<&str> = names.iter().map(String::as_str).collect();
    let mut edges = Vec::new();
    for (d, to) in defs.iter().zip(&names) {
        for dep in &d.depends_on {
            let from = repo_name(&dep.repo);
            if !known.contains(from.as_str()) {
                errors.push(format!("{}: {to} depends on {from}, which is not in the group", def.name));
            } else if &from == to {
                errors.push(format!("{}: {to} depends on itself", def.name));
            } else if !edges.iter().any(|e: &Edge| e.from == from && e.to == *to && e.via == dep.via) {
                edges.push(Edge { from, to: to.clone(), via: dep.via, expects: dep.expects.clone(), contract: dep.contract.clone(), ..Default::default() });
            }
        }
    }
    let depth = order(&names, &edges, &def.name, &mut errors);
    let def_of = |name: &str| defs.iter().zip(&names).find(|(_, n)| n.as_str() == name).map(|(d, _)| *d);
    for e in edges.iter_mut().filter(|e| e.via == Via::Package) {
        let (Some(lib), Some(user)) = (def_of(&e.from), def_of(&e.to)) else { continue };
        versions(e, &lib.repo, &user.repo, villages);
    }
    let mut repos: Vec<ValleyRepo> = defs
        .iter()
        .zip(&names)
        .map(|(d, name)| {
            let village = village_for(&d.repo, villages);
            let dormant = village.is_none_or(is_dormant);
            let mut summary = village.map(summarize).unwrap_or_default();
            // At rest, unless something there still waits on you.
            if dormant && summary.health != "red" {
                summary.health = "dormant".to_string();
            }
            ValleyRepo {
                name: name.clone(),
                repo: d.repo.clone(),
                project: village.map(|v| v.project.clone()),
                dormant,
                depth: depth[name],
                summary,
            }
        })
        .collect();
    // Upstream first; within a depth, as the manifest lists them.
    repos.sort_by_key(|r| r.depth);
    // Each repo's environments: its village's stops, or those read from its repo.
    let stops: Vec<(String, Vec<super::village::Stop>)> = repos
        .iter()
        .map(|r| {
            let mut s = village_for(&r.repo, villages).map(|v| v.pipeline.stops.clone()).or_else(|| dormant.get(&r.name).cloned()).unwrap_or_default();
            // A stop known only by its commit (an environment branch) takes
            // the version tag on that commit, when its repo is here.
            if let Some(dir) = repo_dir(&r.repo, villages) {
                // An environment read from its branch names no commit: its
                // branch's head is what runs there.
                let config = std::fs::read_to_string(dir.join(".autocoder.json")).unwrap_or_default();
                let branches = super::village::parse_environments(&config);
                for stop in s.iter_mut().filter(|st| st.sha.is_none() && st.kind == super::village::StopKind::Environment) {
                    let branch = branches.iter().find(|e| e.name == stop.name).map_or_else(|| format!("env/{}", stop.name), |e| e.branch.clone());
                    stop.sha = branch_head(&dir, &branch);
                }
                for stop in s.iter_mut().filter(|st| st.tag.is_none()) {
                    if let Some(sha) = stop.sha.clone() {
                        stop.tag = tag_at(&dir, &sha);
                    }
                }
            }
            (r.name.clone(), s)
        })
        .collect();
    let contract = |provider: &str, file: &str, sha: &str| -> Option<String> {
        let dir = repo_dir(&def_of(provider)?.repo, villages)?;
        static CACHE: OnceLock<Kept<Option<String>>> = OnceLock::new();
        kept(&CACHE, format!("{}\0{file}\0{sha}", dir.to_string_lossy()), || super::grid::contract_at(&dir, file, sha))
    };
    let grid = super::grid::build(&stops, &edges, &contract);
    ValleyGroup { name: def.name.clone(), repos, edges, grid, errors, ropes: Vec::new(), caravans: Vec::new() }
}

/// Each repo's depth: the longest run of `package` edges above it. Repos on
/// a cycle are said in `errors` and put below everything else.
fn order(names: &[String], edges: &[Edge], group: &str, errors: &mut Vec<String>) -> HashMap<String, usize> {
    let pkg: Vec<&Edge> = edges.iter().filter(|e| e.via == Via::Package).collect();
    let mut indegree: HashMap<&str, usize> = names.iter().map(|n| (n.as_str(), 0)).collect();
    for e in &pkg {
        *indegree.get_mut(e.to.as_str()).unwrap() += 1;
    }
    let mut depth: HashMap<String, usize> = HashMap::new();
    let mut ready: Vec<&str> = names.iter().map(String::as_str).filter(|n| indegree[n] == 0).collect();
    ready.iter().for_each(|n| {
        depth.insert(n.to_string(), 0);
    });
    let mut i = 0;
    while i < ready.len() {
        let n = ready[i];
        i += 1;
        for e in pkg.iter().filter(|e| e.from == n) {
            let d = depth[n] + 1;
            let slot = depth.entry(e.to.clone()).or_insert(0);
            *slot = (*slot).max(d);
            let k = indegree.get_mut(e.to.as_str()).unwrap();
            *k -= 1;
            if *k == 0 {
                ready.push(e.to.as_str());
            }
        }
    }
    let stuck: Vec<&String> = names.iter().filter(|n| indegree[n.as_str()] > 0).collect();
    if !stuck.is_empty() {
        // Name only the repos on a cycle: drop, again and again, any stuck repo
        // no other stuck repo builds on (it is merely downstream of a cycle).
        let mut on: Vec<&str> = stuck.iter().map(|s| s.as_str()).collect();
        loop {
            let keep: Vec<&str> = on.iter().copied().filter(|n| pkg.iter().any(|e| e.from == *n && on.contains(&e.to.as_str()))).collect();
            if keep.len() == on.len() {
                break;
            }
            on = keep;
        }
        errors.push(format!("{group}: a dependency cycle among {}", on.join(", ")));
        let below = depth.values().copied().max().map_or(0, |d| d + 1);
        for n in stuck {
            depth.insert(n.clone(), below);
        }
    }
    depth
}

/// The valley from a manifest's text and the villages running now.
pub fn valley(text: &str, villages: &[VillageSnapshot]) -> (Vec<ValleyGroup>, Vec<String>) {
    valley_with(text, villages, &HashMap::new())
}

/// [`valley`], with the environment stops of repos that have no village,
/// read from their repos (`dormant_stops`), by repo name.
pub fn valley_with(text: &str, villages: &[VillageSnapshot], dormant: &HashMap<String, Vec<super::village::Stop>>) -> (Vec<ValleyGroup>, Vec<String>) {
    match serde_json::from_str::<Vec<GroupDef>>(text) {
        Ok(defs) => {
            let mut errors = Vec::new();
            let mut seen = HashSet::new();
            let groups = defs
                .iter()
                .filter(|g| {
                    let fresh = seen.insert(g.name.clone());
                    if !fresh {
                        errors.push(format!("the group {} is declared twice; the first is used", g.name));
                    }
                    fresh
                })
                .map(|g| build_group(g, villages, dormant))
                .collect();
            (groups, errors)
        }
        Err(e) => (Vec::new(), vec![format!("can't read the manifest: {e}")]),
    }
}

/// The valley for the manifest at `path`.
pub fn read(path: &Path, villages: &[VillageSnapshot]) -> ValleyResponse {
    read_with(path, villages, &HashMap::new())
}

/// [`read`], with the stops of repos that have no village.
pub fn read_with(path: &Path, villages: &[VillageSnapshot], dormant: &HashMap<String, Vec<super::village::Stop>>) -> ValleyResponse {
    let manifest = path.to_string_lossy().into_owned();
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let (groups, errors) = valley_with(&text, villages, dormant);
            ValleyResponse { manifest, found: true, groups, errors }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => ValleyResponse { manifest, found: false, groups: Vec::new(), errors: Vec::new() },
        Err(e) => ValleyResponse { manifest, found: true, groups: Vec::new(), errors: vec![format!("can't read the manifest: {e}")] },
    }
}

/// How long a repo-without-a-village's pipeline is reused: reading it asks
/// origin and GitHub, so no oftener than a village refreshes its own.
const DORMANT_FRESH: std::time::Duration = std::time::Duration::from_secs(120);

/// The environment stops of each manifest repo that has no village but is
/// on this machine and is joined to another by an `api` edge, so is in an
/// environment grid (#512): its pipeline, read from its `.autocoder.json`
/// and git, kept [`DORMANT_FRESH`]. One request refreshes at a time; the
/// others wait for it and use what it read. By repo name.
async fn dormant_stops(transport: &super::super::transport::ServerTransport, villages: &[VillageSnapshot]) -> HashMap<String, Vec<super::village::Stop>> {
    type Pipelines = tokio::sync::Mutex<HashMap<PathBuf, (std::time::Instant, Vec<super::village::Stop>)>>;
    static CACHE: OnceLock<Pipelines> = OnceLock::new();
    let Ok(text) = std::fs::read_to_string(manifest_path()) else { return HashMap::new() };
    let Ok(defs) = serde_json::from_str::<Vec<GroupDef>>(&text) else { return HashMap::new() };
    // The repos an api edge joins: only they are in a grid.
    let mut joined: HashSet<String> = HashSet::new();
    for r in defs.iter().flat_map(|g| &g.repos) {
        for d in r.depends_on.iter().filter(|d| d.via == Via::Api) {
            joined.insert(repo_name(&r.repo));
            joined.insert(repo_name(&d.repo));
        }
    }
    let mut cache = CACHE.get_or_init(Default::default).lock().await;
    let mut out = HashMap::new();
    for r in defs.iter().flat_map(|g| &g.repos) {
        let name = repo_name(&r.repo);
        if !joined.contains(&name) || village_for(&r.repo, villages).is_some() {
            continue;
        }
        let Some(dir) = repo_dir(&r.repo, villages) else { continue };
        let stops = match cache.get(&dir).filter(|(at, _)| at.elapsed() < DORMANT_FRESH) {
            Some((_, s)) => s.clone(),
            None => {
                let s = super::village::read_pipeline(transport, &dir).await.stops;
                cache.insert(dir.clone(), (std::time::Instant::now(), s.clone()));
                s
            }
        };
        out.insert(name, stops);
    }
    out
}

/// Each group's ropes and caravans (#528): the issues of every repo of it on
/// this machine and on GitHub, as last read in the background
/// (`crossrepo::fetch`): the first poll has none, the next has them. A repo
/// with no directory here, or one GitHub can't be asked about, is left out.
fn add_links(transport: &super::super::transport::ServerTransport, groups: &mut [ValleyGroup], villages: &[VillageSnapshot]) {
    for g in groups.iter_mut() {
        let (mut repos, mut issues) = (Vec::new(), HashMap::new());
        for r in &g.repos {
            let Some(dir) = repo_dir(&r.repo, villages) else { continue };
            let read = super::crossrepo::fetch(transport, &dir);
            let Some(slug) = read.slug else { continue };
            if let Some(list) = read.issues {
                issues.insert(r.name.clone(), list);
            }
            repos.push((r.name.clone(), slug));
        }
        (g.ropes, g.caravans) = super::crossrepo::links(&repos, &issues);
    }
}

/// `GET /api/valley`: every group in the manifest, from the villages now.
pub async fn api_valley_handler(
    axum::extract::State(state): axum::extract::State<super::server::WebServerState>,
) -> axum::Json<serde_json::Value> {
    let swarms = state.swarms.read().map(|s| s.clone()).unwrap_or_default();
    let snapshot = state.village.snapshot(&state.transport, &swarms).await;
    let dormant = dormant_stops(&state.transport, &snapshot.villages).await;
    // Reading repos on disk (tags, lockfiles) is blocking work (#511).
    let villages = snapshot.villages;
    let around = villages.clone();
    let answer = tokio::task::spawn_blocking(move || read_with(&manifest_path(), &villages, &dormant)).await;
    axum::Json(match answer {
        Ok(mut valley) => {
            add_links(&state.transport, &mut valley.groups, &around);
            serde_json::json!(valley)
        }
        Err(e) => serde_json::json!({ "manifest": manifest_path().to_string_lossy(), "found": true, "groups": [], "errors": [format!("can't read the valley: {e}")] }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::village::{House, Stop};

    fn village(project: &str, houses: &[HouseStatus]) -> VillageSnapshot {
        VillageSnapshot {
            project: project.into(),
            repo_path: format!("/src/{project}"),
            manager: House { status: HouseStatus::Idle, ..Default::default() },
            houses: houses.iter().map(|s| House { status: *s, ..Default::default() }).collect(),
            ..Default::default()
        }
    }

    const SHOP: &str = r#"[{ "name": "shop", "repos": [
        { "repo": "web", "depends_on": [{ "repo": "lib-core", "via": "package" }, { "repo": "api", "via": "api" }] },
        { "repo": "api", "depends_on": [{ "repo": "lib-core", "via": "package" }, { "repo": "/src/lib-ui", "via": "package" }] },
        { "repo": "/src/lib-ui", "depends_on": [{ "repo": "lib-core", "via": "package" }] },
        { "repo": "lib-core" } ] }]"#;

    /// #528: every group carries its ropes and caravans, empty until GitHub is read.
    #[test]
    fn groups_carry_ropes_and_caravans() {
        let (groups, _) = valley(SHOP, &[]);
        let json = serde_json::to_value(&groups[0]).unwrap();
        assert_eq!(json["ropes"], serde_json::json!([]));
        assert_eq!(json["caravans"], serde_json::json!([]));
    }

    #[test]
    fn package_edges_say_what_is_pinned_and_whether_it_is_behind() {
        let root = std::env::temp_dir().join(format!("agents-ui-valley-pins-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = |n: &str| {
            let d = root.join(n);
            std::fs::create_dir_all(&d).unwrap();
            d
        };
        // lib-core: no village, so its newest version tag is its latest release.
        let lib = dir("lib-core");
        std::fs::write(lib.join("Cargo.toml"), "[package]\nname = \"lib-core\"\n").unwrap();
        for args in [&["init", "-q"][..], &["commit", "-q", "--allow-empty", "-m", "a"], &["tag", "v1.3.0"], &["tag", "v1.4.0"]] {
            let ok = std::process::Command::new("git").arg("-C").arg(&lib).args(args).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap().status.success();
            assert!(ok);
        }
        std::fs::write(dir("shop").join("Cargo.lock"), "[[package]]\nname = \"lib-core\"\nversion = \"1.3.0\"\n").unwrap();
        std::fs::write(dir("web").join("package-lock.json"), r#"{"packages":{"node_modules/lib-core":{"version":"1.4.0"}}}"#).unwrap();
        dir("docs");
        // lib-ui: its village's release stop is its latest release.
        let mut ui = village("lib-ui", &[]);
        ui.repo_path = dir("lib-ui").to_string_lossy().into_owned();
        ui.pipeline.stops.push(Stop { name: "release".into(), tag: Some("v2.0.0".into()), deployed_at: Some("2026-11-16T09:00:00Z".into()), ..Default::default() });
        std::fs::write(dir("api").join("package.json"), r#"{"dependencies":{"lib-ui":"^1.9.0"}}"#).unwrap();
        let p = |n: &str| root.join(n).to_string_lossy().into_owned();
        let text = format!(
            r#"[{{ "name": "g", "repos": [ {{ "repo": "{lib}" }}, {{ "repo": "lib-ui" }},
                {{ "repo": "{shop}", "depends_on": [{{ "repo": "{lib}", "via": "package" }}] }},
                {{ "repo": "{web}", "depends_on": [{{ "repo": "{lib}", "via": "package" }}, {{ "repo": "{shop}", "via": "api" }}] }},
                {{ "repo": "{api}", "depends_on": [{{ "repo": "lib-ui", "via": "package" }}] }},
                {{ "repo": "{docs}", "depends_on": [{{ "repo": "{lib}", "via": "package" }}] }} ] }}]"#,
            lib = p("lib-core"), shop = p("shop"), web = p("web"), api = p("api"), docs = p("docs")
        );
        let (groups, errors) = valley(&text, &[ui]);
        assert!(errors.is_empty() && groups[0].errors.is_empty(), "{errors:?} {:?}", groups[0].errors);
        let edge = |from: &str, to: &str| groups[0].edges.iter().find(|e| e.from == from && e.to == to).unwrap().clone();
        let e = edge("lib-core", "shop");
        assert_eq!((e.pinned.as_deref(), e.latest.as_deref(), e.behind), (Some("1.3.0"), Some("v1.4.0"), Some(true)));
        assert!(e.latest_at.is_some(), "when the tag was made");
        assert_eq!(edge("lib-core", "web").behind, Some(false));
        let e = edge("lib-ui", "api");
        assert_eq!((e.pinned.as_deref(), e.latest.as_deref(), e.latest_at.as_deref(), e.behind), (Some("^1.9.0"), Some("v2.0.0"), Some("2026-11-16T09:00:00Z"), Some(true)));
        let e = edge("lib-core", "docs");
        assert_eq!((e.pinned, e.behind), (None, None), "nothing to read: skipped, not an error");
        let e = edge("shop", "web");
        assert_eq!((e.pinned, e.latest), (None, None), "an api edge has no versions");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_grid_marks_a_provider_outside_what_its_consumer_in_the_same_environment_expects() {
        use crate::web::village::StopKind;
        let env = |name: &str, tag: Option<&str>| Stop { name: name.into(), tag: tag.map(Into::into), sha: Some("abc1234".into()), kind: StopKind::Environment, ..Default::default() };
        let mut shop = village("shop", &[HouseStatus::Working]);
        shop.pipeline.stops = vec![env("staging", None), env("prod", None)];
        let dormant = HashMap::from([("billing".to_string(), vec![env("staging", Some("v2.2.0")), env("prod", Some("v2.4.0"))])]);
        let text = r#"[{ "name": "g", "repos": [ { "repo": "billing" },
            { "repo": "shop", "depends_on": [{ "repo": "billing", "via": "api", "expects": ">=2.3" }] } ] }]"#;
        let (groups, errors) = valley_with(text, &[shop], &dormant);
        assert!(errors.is_empty() && groups[0].errors.is_empty());
        let grid = groups[0].grid.as_ref().expect("an api edge: a grid");
        assert_eq!(grid.envs, vec!["staging", "prod"]);
        let cell = |r: &str, e: &str| grid.cells.iter().find(|c| c.repo == r && c.env == e).unwrap();
        assert_eq!(cell("billing", "staging").mismatches.len(), 1);
        assert_eq!(cell("billing", "staging").mismatches[0].neighbour, "shop");
        assert!(cell("billing", "prod").mismatches.is_empty());
        assert_eq!(groups[0].edges[0].expects.as_deref(), Some(">=2.3"), "what the manifest said is on the edge");
        // No api edges: no grid.
        let (plain, _) = valley(SHOP.replace(r#", { "repo": "api", "via": "api" }"#, "").as_str(), &[]);
        assert!(plain[0].grid.is_none());
    }

    #[test]
    fn repos_go_upstream_to_downstream_along_the_package_edges() {
        let (groups, errors) = valley(SHOP, &[]);
        assert!(errors.is_empty(), "{errors:?}");
        let g = &groups[0];
        assert!(g.errors.is_empty(), "{:?}", g.errors);
        let order: Vec<(&str, usize)> = g.repos.iter().map(|r| (r.name.as_str(), r.depth)).collect();
        // web only calls api's API, so it is not below it; api builds on lib-ui, two down.
        assert_eq!(order, vec![("lib-core", 0), ("web", 1), ("lib-ui", 1), ("api", 2)]);
        assert_eq!(g.edges.len(), 5);
        assert!(g.edges.contains(&Edge { from: "api".into(), to: "web".into(), via: Via::Api, ..Default::default() }));
        assert!(g.edges.contains(&Edge { from: "lib-ui".into(), to: "api".into(), via: Via::Package, ..Default::default() }));
    }

    #[test]
    fn a_cycle_is_said_and_its_repos_go_last() {
        let text = r#"[{ "name": "loop", "repos": [
            { "repo": "a", "depends_on": [{ "repo": "b", "via": "package" }] },
            { "repo": "b", "depends_on": [{ "repo": "a", "via": "package" }] },
            { "repo": "c" },
            { "repo": "d", "depends_on": [{ "repo": "c", "via": "package" }] } ] }]"#;
        let (groups, _) = valley(text, &[]);
        let g = &groups[0];
        assert_eq!(g.errors, vec!["loop: a dependency cycle among a, b"]);
        let order: Vec<(&str, usize)> = g.repos.iter().map(|r| (r.name.as_str(), r.depth)).collect();
        assert_eq!(order, vec![("c", 0), ("d", 1), ("a", 2), ("b", 2)]);
        // A repo merely downstream of a cycle is placed with it but not named on it.
        let text = r#"[{ "name": "loop", "repos": [
            { "repo": "a", "depends_on": [{ "repo": "b", "via": "package" }] },
            { "repo": "b", "depends_on": [{ "repo": "a", "via": "package" }] },
            { "repo": "e", "depends_on": [{ "repo": "a", "via": "package" }] } ] }]"#;
        assert_eq!(valley(text, &[]).0[0].errors, vec!["loop: a dependency cycle among a, b"]);
        // An api cycle is no cycle: it orders nothing.
        let text = r#"[{ "name": "calls", "repos": [
            { "repo": "a", "depends_on": [{ "repo": "b", "via": "api" }] },
            { "repo": "b", "depends_on": [{ "repo": "a", "via": "api" }] } ] }]"#;
        assert!(valley(text, &[]).0[0].errors.is_empty());
    }

    #[test]
    fn unknown_repos_twice_listed_repos_and_self_edges_are_said_and_left_out() {
        let text = r#"[{ "name": "g", "repos": [
            { "repo": "a", "depends_on": [{ "repo": "ghost", "via": "package" }, { "repo": "a", "via": "api" }] },
            { "repo": "a" } ] },
            { "name": "g", "repos": [] }]"#;
        let (groups, errors) = valley(text, &[]);
        assert_eq!(errors, vec!["the group g is declared twice; the first is used"]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].errors, vec![
            "g: a is listed twice",
            "g: a depends on ghost, which is not in the group",
            "g: a depends on itself",
        ]);
        assert!(groups[0].edges.is_empty());
        assert_eq!(groups[0].repos.len(), 1);
    }

    #[test]
    fn malformed_json_is_said_not_a_crash() {
        let (groups, errors) = valley("{ not json", &[]);
        assert!(groups.is_empty());
        assert!(errors[0].starts_with("can't read the manifest:"), "{errors:?}");
        // A wrong `via` and a misspelt key are malformed too.
        assert!(!valley(r#"[{ "name": "g", "repos": [{ "repo": "a", "depends_on": [{ "repo": "b", "via": "npm" }] }] }]"#, &[]).1.is_empty());
        assert!(!valley(r#"[{ "name": "g", "repos": [{ "repo": "a", "depends": [] }] }]"#, &[]).1.is_empty());
    }

    #[test]
    fn each_repo_is_summarised_from_its_village_or_is_dormant() {
        let mut core = village("lib-core", &[HouseStatus::Working, HouseStatus::Working, HouseStatus::Idle]);
        core.pipeline.stops = vec![Stop { name: "release".into(), tag: Some("v1.4.0".into()), ..Default::default() }];
        core.backlog = vec![Default::default(); 5];
        let mut web = village("web", &[HouseStatus::Blocked, HouseStatus::Idle]);
        web.repo_path = "/elsewhere/web".into();
        let mut lib_ui = village("lib-ui", &[]);
        lib_ui.board = vec![Default::default()];
        let (groups, _) = valley(SHOP, &[core, web, lib_ui]);
        let by = |n: &str| groups[0].repos.iter().find(|r| r.name == n).unwrap().clone();
        let core = by("lib-core");
        assert_eq!(core.project.as_deref(), Some("lib-core"));
        assert_eq!(core.summary, Summary { working: 2, idle: 1, blocked: 0, backlog: 5, release: Some("v1.4.0".into()), health: "green".into() });
        assert_eq!(by("web").summary.health, "amber");
        // Found by its path; something waits on you there.
        assert_eq!(by("lib-ui").summary.health, "red");
        // No village: dormant, still in the valley.
        let api = by("api");
        assert!(api.dormant && api.project.is_none());
        assert_eq!(api.summary.health, "dormant");
    }

    #[test]
    fn a_dormant_village_is_dormant_but_can_be_opened() {
        let mut at_rest = village("lib-core", &[]);
        at_rest.manager.status = HouseStatus::Stopped;
        let (groups, _) = valley(SHOP, &[at_rest]);
        let core = groups[0].repos.iter().find(|r| r.name == "lib-core").unwrap();
        assert!(core.dormant);
        assert_eq!(core.project.as_deref(), Some("lib-core"));
        assert_eq!(core.summary.health, "dormant");
        // At rest, but something on its board waits on you: still red.
        let mut waiting = village("lib-core", &[]);
        waiting.manager.status = HouseStatus::Stopped;
        waiting.board = vec![Default::default()];
        let (groups, _) = valley(SHOP, &[waiting]);
        assert_eq!(groups[0].repos.iter().find(|r| r.name == "lib-core").unwrap().summary.health, "red");
    }

    #[test]
    fn a_path_is_matched_with_or_without_its_trailing_slash() {
        let text = r#"[{ "name": "g", "repos": [{ "repo": "/src/web/" }] }]"#;
        let mut web = village("frontend", &[HouseStatus::Working]);
        web.repo_path = "/src/web".into();
        let mut decoy = village("web", &[]);
        decoy.repo_path = "/elsewhere/web".into();
        let (groups, _) = valley(text, &[decoy, web]);
        assert_eq!(groups[0].repos[0].project.as_deref(), Some("frontend"), "the path wins over a same-named village");
    }

    #[test]
    fn a_missing_manifest_is_not_an_error() {
        let path = std::env::temp_dir().join(format!("agents-ui-valley-missing-{}.json", std::process::id()));
        let r = read(&path, &[]);
        assert!(!r.found && r.groups.is_empty() && r.errors.is_empty());
        std::fs::write(&path, SHOP).unwrap();
        let r = read(&path, &[]);
        std::fs::remove_file(&path).ok();
        assert!(r.found);
        assert_eq!(r.groups[0].repos.len(), 4);
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["groups"][0]["edges"][0], serde_json::json!({"from": "lib-core", "to": "web", "via": "package"}));
        assert_eq!(json["groups"][0]["repos"][0]["summary"]["health"], "dormant");
    }
}
