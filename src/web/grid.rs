//! The valley's environment grid and contract mismatches (#512, phase 3 of
//! #494).
//!
//! Repos joined by `api` edges are a grid of repos × environments (each
//! repo's `.autocoder.json` environments, as its pipeline's stops), each cell
//! the version deployed there. An `api` edge may say what version range of
//! the provider its consumer expects (`"expects": ">=2.3"`), and optionally a
//! contract file in the provider's repo (`"contract": "openapi.json"`) whose
//! `info.version` is the version compared, read at the commit deployed in
//! that environment; without one, the deployed release's version is.
//!
//! A provider's cell is marked when its version is outside what a neighbour
//! deployed in the same environment expects. A version that can't be read
//! (a bare commit, no contract there) is not marked: it is unknown, not wrong.

use serde::Serialize;

use super::valley::{Edge, Via};
use super::village::{Stop, StopKind};

/// One environment of one repo: what is deployed there.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Cell {
    pub repo: String,
    pub env: String,
    /// The release tag deployed there, else a short commit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Neighbours here whose expectations this cell's version is outside.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mismatches: Vec<Mismatch>,
}

/// A provider's version outside what a consumer in the same environment expects.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Mismatch {
    /// The consumer that expects something else.
    pub neighbour: String,
    pub env: String,
    pub expects: String,
    /// The provider's version compared: its release, or its contract's.
    pub version: String,
    /// The contract file the version was read from, when one was named.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract: Option<String>,
}

/// Repos × environments, for the repos joined by `api` edges.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Grid {
    /// Every environment any of them has, in pipeline order (first seen first).
    pub envs: Vec<String>,
    /// The repos, in valley order.
    pub repos: Vec<String>,
    /// A cell for each repo in each environment it deploys to.
    pub cells: Vec<Cell>,
}

/// What a stop says is deployed: its tag, else its commit, short.
fn deployed(s: &Stop) -> Option<String> {
    s.tag.clone().or_else(|| s.sha.as_deref().map(|sha| sha.chars().take(7).collect()))
}

/// The grid for `repos` (in valley order, each with its pipeline's stops),
/// or None when no `api` edge joins two of them. `contract(provider, file,
/// sha)` reads a contract file's version at a commit.
pub fn build(repos: &[(String, Vec<Stop>)], edges: &[Edge], contract: &dyn Fn(&str, &str, &str) -> Option<String>) -> Option<Grid> {
    let joined: Vec<&String> = repos
        .iter()
        .map(|(n, _)| n)
        .filter(|n| edges.iter().any(|e| e.via == Via::Api && (&e.from == *n || &e.to == *n)))
        .collect();
    if joined.is_empty() {
        return None;
    }
    let stops_of = |name: &str| repos.iter().find(|(n, _)| n == name).map(|(_, s)| s.as_slice()).unwrap_or(&[]);
    let envs_of = |name: &str| stops_of(name).iter().filter(|s| s.kind == StopKind::Environment).cloned().collect::<Vec<_>>();
    let mut envs: Vec<String> = Vec::new();
    for n in &joined {
        for s in envs_of(n) {
            if !envs.contains(&s.name) {
                envs.push(s.name.clone());
            }
        }
    }
    let mut cells: Vec<Cell> = Vec::new();
    for n in &joined {
        for s in envs_of(n) {
            cells.push(Cell { repo: (*n).clone(), env: s.name.clone(), version: deployed(&s), mismatches: Vec::new() });
        }
    }
    for e in edges.iter().filter(|e| e.via == Via::Api) {
        let Some(expects) = e.expects.as_deref() else { continue };
        for env in &envs {
            // Only where both are deployed: a consumer not there expects nothing there.
            let Some(provider) = envs_of(&e.from).into_iter().find(|s| &s.name == env) else { continue };
            if !envs_of(&e.to).iter().any(|s| &s.name == env) {
                continue;
            }
            let version = match (&e.contract, provider.sha.as_deref()) {
                (Some(file), Some(sha)) => contract(&e.from, file, sha),
                (Some(_), None) => None,
                (None, _) => provider.tag.clone(),
            };
            let Some(version) = version else { continue };
            if super::pins::satisfies(expects, &version) == Some(false)
                && let Some(cell) = cells.iter_mut().find(|c| c.repo == e.from && &c.env == env)
            {
                cell.mismatches.push(Mismatch { neighbour: e.to.clone(), env: env.clone(), expects: expects.to_string(), version, contract: e.contract.clone() });
            }
        }
    }
    Some(Grid { envs, repos: joined.into_iter().cloned().collect(), cells })
}

/// An OpenAPI (or any) contract's `info.version`: from JSON, or a YAML
/// `info:` block's `version:` line.
pub fn contract_version(text: &str) -> Option<String> {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        return v.get("info")?.get("version").and_then(|v| v.as_str().map(str::to_string).or_else(|| Some(v.to_string())));
    }
    let mut in_info = false;
    for line in text.lines() {
        let indented = line.starts_with([' ', '\t']);
        let t = line.trim();
        if !indented && !t.is_empty() && !t.starts_with('#') {
            in_info = t == "info:";
            continue;
        }
        if in_info && let Some(v) = t.strip_prefix("version:") {
            let v = v.trim().trim_matches(['"', '\'']);
            return (!v.is_empty()).then(|| v.to_string());
        }
    }
    None
}

/// A contract file's version at a commit in the repo at `dir` (`git show`).
pub fn contract_at(dir: &std::path::Path, file: &str, sha: &str) -> Option<String> {
    // A path or sha shaped like an option is not one.
    if file.starts_with('-') || sha.starts_with('-') || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let out = std::process::Command::new("git").arg("-C").arg(dir).args(["show", &format!("{sha}:{file}")]).output().ok().filter(|o| o.status.success())?;
    contract_version(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(name: &str, tag: Option<&str>, sha: &str) -> Stop {
        Stop { name: name.into(), tag: tag.map(Into::into), sha: Some(sha.into()), kind: StopKind::Environment, ..Default::default() }
    }

    fn api(from: &str, to: &str, expects: Option<&str>, contract: Option<&str>) -> Edge {
        Edge { from: from.into(), to: to.into(), via: Via::Api, expects: expects.map(Into::into), contract: contract.map(Into::into), ..Default::default() }
    }

    fn none(_: &str, _: &str, _: &str) -> Option<String> {
        None
    }

    #[test]
    fn the_grid_is_the_api_joined_repos_by_their_environments() {
        let repos = vec![
            ("lib".to_string(), vec![env("prod", Some("v1.0.0"), "aaaaaaa1")]),
            ("billing".to_string(), vec![env("staging", Some("v2.2.0"), "bbbbbbb1"), env("prod", Some("v2.4.0"), "bbbbbbb2")]),
            ("shop".to_string(), vec![env("dev", None, "ccccccc1234"), env("staging", Some("v5.0.0"), "ccccccc2"), Stop { name: "green".into(), kind: StopKind::Gate, ..Default::default() }]),
        ];
        let g = build(&repos, &[api("billing", "shop", None, None)], &none).unwrap();
        assert_eq!(g.repos, vec!["billing", "shop"], "only the repos an api edge joins");
        assert_eq!(g.envs, vec!["staging", "prod", "dev"], "first seen first; no gates");
        let cell = |r: &str, e: &str| g.cells.iter().find(|c| c.repo == r && c.env == e).cloned();
        assert_eq!(cell("billing", "prod").unwrap().version.as_deref(), Some("v2.4.0"));
        assert_eq!(cell("shop", "dev").unwrap().version.as_deref(), Some("ccccccc"), "no tag: a short commit");
        assert!(cell("shop", "prod").is_none(), "not deployed there: no cell");
        assert!(build(&repos, &[], &none).is_none(), "no api edges: no grid");
    }

    #[test]
    fn a_provider_outside_what_its_consumer_expects_is_marked_per_environment() {
        let repos = vec![
            ("billing".to_string(), vec![env("dev", Some("v2.5.0"), "b0"), env("staging", Some("v2.2.0"), "b1"), env("prod", Some("v2.4.0"), "b2"), env("qa", None, "b3")]),
            ("shop".to_string(), vec![env("dev", None, "c0"), env("staging", None, "c1"), env("prod", None, "c2"), env("qa", None, "c3")]),
        ];
        let g = build(&repos, &[api("billing", "shop", Some(">=2.3"), None)], &none).unwrap();
        let marks = |e: &str| g.cells.iter().find(|c| c.repo == "billing" && c.env == e).unwrap().mismatches.clone();
        assert!(marks("prod").is_empty(), "in range");
        assert!(marks("dev").is_empty(), "in range");
        assert_eq!(
            marks("staging"),
            vec![Mismatch { neighbour: "shop".into(), env: "staging".into(), expects: ">=2.3".into(), version: "v2.2.0".into(), contract: None }],
            "out of range"
        );
        assert!(marks("qa").is_empty(), "an unknown version (a bare commit) is not marked");
        assert!(g.cells.iter().filter(|c| c.repo == "shop").all(|c| c.mismatches.is_empty()), "the consumer isn't marked");
    }

    #[test]
    fn a_contract_file_s_version_is_compared_at_the_deployed_commit() {
        let repos = vec![
            ("billing".to_string(), vec![env("staging", Some("v9.0.0"), "b1"), env("prod", Some("v9.0.0"), "b2")]),
            ("shop".to_string(), vec![env("staging", None, "c1"), env("prod", None, "c2")]),
        ];
        let contract = |repo: &str, file: &str, sha: &str| -> Option<String> {
            assert_eq!((repo, file), ("billing", "openapi.yaml"));
            match sha {
                "b1" => Some("2.1.0".into()),
                _ => None,
            }
        };
        let g = build(&repos, &[api("billing", "shop", Some("^2.3"), Some("openapi.yaml"))], &contract).unwrap();
        let staging = &g.cells.iter().find(|c| c.env == "staging" && c.repo == "billing").unwrap().mismatches;
        assert_eq!(staging[0].version, "2.1.0");
        assert_eq!(staging[0].contract.as_deref(), Some("openapi.yaml"));
        let prod = &g.cells.iter().find(|c| c.env == "prod" && c.repo == "billing").unwrap().mismatches;
        assert!(prod.is_empty(), "no contract there: unknown, not marked");
    }

    #[test]
    fn a_contract_s_version_from_json_or_yaml() {
        assert_eq!(contract_version(r#"{"openapi":"3.0.0","info":{"title":"B","version":"2.4.1"}}"#).as_deref(), Some("2.4.1"));
        let yaml = "openapi: 3.0.0\n# the api\ninfo:\n  title: Billing\n  version: '2.2.0'\npaths: {}\n";
        assert_eq!(contract_version(yaml).as_deref(), Some("2.2.0"));
        assert_eq!(contract_version("paths:\n  version: 1\n"), None, "only info's version");
        assert_eq!(contract_version("{}"), None);
    }

    #[test]
    fn a_contract_is_read_at_a_commit() {
        let d = std::env::temp_dir().join(format!("agents-ui-grid-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git").arg("-C").arg(&d).args(args).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap();
            assert!(out.status.success(), "git {args:?}");
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        git(&["init", "-q"]);
        std::fs::write(d.join("openapi.json"), r#"{"info":{"version":"2.2.0"}}"#).unwrap();
        git(&["add", "-A"]);
        git(&["-c", "commit.gpgsign=false", "commit", "-qm", "a"]);
        let sha = git(&["rev-parse", "HEAD"]);
        assert_eq!(contract_at(&d, "openapi.json", &sha).as_deref(), Some("2.2.0"));
        assert_eq!(contract_at(&d, "missing.json", &sha), None);
        assert_eq!(contract_at(&d, "openapi.json", "--output=x"), None, "not an option");
        std::fs::remove_dir_all(&d).ok();
    }
}
