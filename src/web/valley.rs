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

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::village::{HouseStatus, VillageSnapshot};

/// How one repo depends on another.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Via {
    /// It builds against the other's package: the river runs from it.
    Package,
    /// It calls the other's API: a road between them (the grid, in phase 3).
    Api,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DependsOn {
    repo: String,
    via: Via,
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
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub via: Via,
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
    /// What is wrong with this group's part of the manifest.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
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
fn build_group(def: &GroupDef, villages: &[VillageSnapshot]) -> ValleyGroup {
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
                edges.push(Edge { from, to: to.clone(), via: dep.via });
            }
        }
    }
    let depth = order(&names, &edges, &def.name, &mut errors);
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
    ValleyGroup { name: def.name.clone(), repos, edges, errors }
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
                .map(|g| build_group(g, villages))
                .collect();
            (groups, errors)
        }
        Err(e) => (Vec::new(), vec![format!("can't read the manifest: {e}")]),
    }
}

/// The valley for the manifest at `path`.
pub fn read(path: &Path, villages: &[VillageSnapshot]) -> ValleyResponse {
    let manifest = path.to_string_lossy().into_owned();
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let (groups, errors) = valley(&text, villages);
            ValleyResponse { manifest, found: true, groups, errors }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => ValleyResponse { manifest, found: false, groups: Vec::new(), errors: Vec::new() },
        Err(e) => ValleyResponse { manifest, found: true, groups: Vec::new(), errors: vec![format!("can't read the manifest: {e}")] },
    }
}

/// `GET /api/valley`: every group in the manifest, from the villages now.
pub async fn api_valley_handler(
    axum::extract::State(state): axum::extract::State<super::server::WebServerState>,
) -> axum::Json<serde_json::Value> {
    let swarms = state.swarms.read().map(|s| s.clone()).unwrap_or_default();
    let snapshot = state.village.snapshot(&state.transport, &swarms).await;
    axum::Json(serde_json::json!(read(&manifest_path(), &snapshot.villages)))
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
        assert!(g.edges.contains(&Edge { from: "api".into(), to: "web".into(), via: Via::Api }));
        assert!(g.edges.contains(&Edge { from: "lib-ui".into(), to: "api".into(), via: Via::Package }));
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
