//! Swarm recognition: which running agents belong to which repo, and where.
//!
//! A swarm is keyed by its repo's identity, the absolute path of the git
//! common directory (`git rev-parse --path-format=absolute --git-common-dir`),
//! never by a folder name: two repos called `shop` stay apart, and one repo
//! seen through tmux and through herdr is one swarm.
//!
//! An agent belongs to a repo when its cwd is inside one of that repo's
//! worktrees (`git worktree list --porcelain`), at any depth. An agent in the
//! main checkout is a manager candidate; one in any other worktree is a
//! worker. A worker's number is the `N` of a `-wt-<N>` worktree name; a
//! worktree named otherwise takes the lowest number no `-wt-<N>` worktree
//! uses, in worktree order.
//!
//! [`recognize`] takes only plain facts, so every source (and the terminal
//! view) can share it; [`gather`] collects those facts from git.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use crate::transport::ServerTransport;

/// One running agent as a source sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentFact {
    /// Where the agent's pane lives: `tmux`, `herdr`, ...
    pub source: String,
    /// The pane, in its source's own addressing (`claude-shop:1.0`, `w5:p1`).
    pub pane: String,
    pub cwd: PathBuf,
}

/// One worktree of a repo, as `git worktree list --porcelain` lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: Option<String>,
}

/// A repo: its identity and its worktrees, the main checkout first (git
/// always lists it first).
#[derive(Debug, Clone, PartialEq)]
pub struct RepoFacts {
    pub common_dir: PathBuf,
    pub worktrees: Vec<Worktree>,
}

/// Where in its repo an agent sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Place {
    /// The main checkout: a manager candidate.
    Main,
    /// Another worktree: worker `N`.
    Worker(u32),
}

/// An agent placed in a repo.
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    /// Index into the agents passed to [`recognize`].
    pub agent: usize,
    /// The worktree the agent's cwd is in.
    pub worktree: PathBuf,
    pub branch: Option<String>,
    pub place: Place,
}

/// The agents found in one repo.
#[derive(Debug, Clone, PartialEq)]
pub struct RepoSwarm {
    /// The repo's identity: its git common directory.
    pub key: PathBuf,
    /// The main checkout.
    pub root: PathBuf,
    /// Members in order: main-checkout agents first, then workers by number.
    pub members: Vec<Member>,
}

impl RepoSwarm {
    /// The repo's folder name, for display (not identity).
    pub fn name(&self) -> String {
        self.root.file_name().and_then(|b| b.to_str()).unwrap_or("swarm").to_string()
    }

    /// Agents in the main checkout: the manager candidates.
    pub fn main_members(&self) -> impl Iterator<Item = &Member> {
        self.members.iter().filter(|m| m.place == Place::Main)
    }

    /// Agents in other worktrees.
    pub fn workers(&self) -> impl Iterator<Item = (u32, &Member)> {
        self.members.iter().filter_map(|m| match m.place {
            Place::Worker(n) => Some((n, m)),
            Place::Main => None,
        })
    }
}

/// `shop-wt-3` → 3.
pub fn wt_number(dir: &Path) -> Option<u32> {
    let base = dir.file_name()?.to_str()?;
    let (stem, n) = base.rsplit_once("-wt-")?;
    if stem.is_empty() {
        return None;
    }
    n.parse().ok()
}

/// Each non-main worktree's worker number: its `-wt-<N>`, else the lowest
/// number no other worktree claims, in worktree order.
fn worker_numbers(worktrees: &[Worktree]) -> HashMap<PathBuf, u32> {
    let others = worktrees.iter().skip(1);
    let mut numbers: HashMap<PathBuf, u32> = HashMap::new();
    for wt in others.clone() {
        if let Some(n) = wt_number(&wt.path)
            && !numbers.values().any(|&m| m == n)
        {
            numbers.insert(wt.path.clone(), n);
        }
    }
    let mut next = 1;
    for wt in others {
        if numbers.contains_key(&wt.path) {
            continue;
        }
        while numbers.values().any(|&m| m == next) {
            next += 1;
        }
        numbers.insert(wt.path.clone(), next);
    }
    numbers
}

/// Group agents into swarms by the repo whose worktree holds their cwd.
/// Agents outside every repo given are left out; repos with no agent in them
/// are not swarms here. Swarms come out in key order.
pub fn recognize(agents: &[AgentFact], repos: &[RepoFacts]) -> Vec<RepoSwarm> {
    // The same repo may be reported twice (once per source); one entry per key.
    let mut by_key: BTreeMap<&Path, &RepoFacts> = BTreeMap::new();
    for repo in repos {
        by_key.entry(repo.common_dir.as_path()).or_insert(repo);
    }
    let numbers: HashMap<&Path, HashMap<PathBuf, u32>> =
        by_key.iter().map(|(k, r)| (*k, worker_numbers(&r.worktrees))).collect();

    let mut swarms: BTreeMap<&Path, Vec<Member>> = BTreeMap::new();
    for (i, agent) in agents.iter().enumerate() {
        // The deepest worktree holding the cwd, so a worktree nested inside
        // the main checkout (`.worktrees/x`) wins over the checkout itself.
        let best = by_key
            .iter()
            .flat_map(|(key, repo)| repo.worktrees.iter().enumerate().map(move |(j, wt)| (*key, j, wt)))
            .filter(|(_, _, wt)| agent.cwd.starts_with(&wt.path))
            .max_by_key(|(_, _, wt)| wt.path.components().count());
        let Some((key, j, wt)) = best else { continue };
        let place = if j == 0 { Place::Main } else { Place::Worker(numbers[key][&wt.path]) };
        swarms.entry(key).or_default().push(Member {
            agent: i,
            worktree: wt.path.clone(),
            branch: wt.branch.clone(),
            place,
        });
    }

    swarms
        .into_iter()
        .map(|(key, mut members)| {
            members.sort_by_key(|m| (m.place, m.agent));
            let root = by_key[key].worktrees.first().map(|w| w.path.clone()).unwrap_or_default();
            RepoSwarm { key: key.to_path_buf(), root, members }
        })
        .collect()
}

/// `git worktree list --porcelain` → the worktrees in git's order (main
/// first). Paths are canonicalised so a symlinked worktree still matches.
pub fn parse_worktrees(stdout: &str) -> Vec<Worktree> {
    let mut out: Vec<Worktree> = Vec::new();
    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            let p = PathBuf::from(path.trim());
            out.push(Worktree { path: std::fs::canonicalize(&p).unwrap_or(p), branch: None });
        } else if let Some(reference) = line.strip_prefix("branch ")
            && let Some(last) = out.last_mut()
        {
            let r = reference.trim();
            last.branch = Some(r.strip_prefix("refs/heads/").unwrap_or(r).to_string());
        }
    }
    out
}

/// The repos holding these cwds, from git: one `rev-parse` per distinct cwd
/// and one `worktree list` per distinct repo. A cwd outside any repo (or
/// where git fails) adds nothing.
pub async fn gather(transport: &ServerTransport, cwds: &[PathBuf]) -> Vec<RepoFacts> {
    let mut seen_cwds: Vec<&PathBuf> = Vec::new();
    let mut repos: Vec<RepoFacts> = Vec::new();
    for cwd in cwds {
        if seen_cwds.contains(&cwd) {
            continue;
        }
        seen_cwds.push(cwd);
        // Already known: this cwd is inside a worktree we have listed.
        if repos.iter().any(|r| r.worktrees.iter().any(|w| cwd.starts_with(&w.path))) {
            continue;
        }
        let args: Vec<String> = ["rev-parse", "--path-format=absolute", "--git-common-dir"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let Ok(out) = transport.output("git", &args, Some(cwd)).await else { continue };
        if !out.status.success() {
            continue;
        }
        let raw = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
        let common_dir = std::fs::canonicalize(&raw).unwrap_or(raw);
        if repos.iter().any(|r| r.common_dir == common_dir) {
            continue;
        }
        let args: Vec<String> = ["worktree", "list", "--porcelain"].iter().map(|s| s.to_string()).collect();
        let Ok(out) = transport.output("git", &args, Some(cwd)).await else { continue };
        if !out.status.success() {
            continue;
        }
        repos.push(RepoFacts { common_dir, worktrees: parse_worktrees(&String::from_utf8_lossy(&out.stdout)) });
    }
    repos
}

/// A cwd as the recognizer compares it: canonical when it exists here.
pub fn canonical(cwd: &str) -> PathBuf {
    let p = PathBuf::from(cwd.trim_end_matches('/'));
    std::fs::canonicalize(&p).unwrap_or(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(source: &str, pane: &str, cwd: &str) -> AgentFact {
        AgentFact { source: source.into(), pane: pane.into(), cwd: cwd.into() }
    }

    fn repo(common: &str, worktrees: &[&str]) -> RepoFacts {
        RepoFacts {
            common_dir: common.into(),
            worktrees: worktrees.iter().map(|p| Worktree { path: p.into(), branch: None }).collect(),
        }
    }

    fn shop() -> RepoFacts {
        repo("/src/shop/.git", &["/src/shop", "/src/shop-wt-1", "/src/shop-wt-2"])
    }

    #[test]
    fn a_cwd_in_a_worktree_subdirectory_belongs_to_that_worktree() {
        let agents = [agent("herdr", "w1:p1", "/src/shop/docs"), agent("herdr", "w2:p1", "/src/shop-wt-2/src/web")];
        let swarms = recognize(&agents, &[shop()]);
        assert_eq!(swarms.len(), 1);
        let s = &swarms[0];
        assert_eq!(s.key, PathBuf::from("/src/shop/.git"));
        assert_eq!(s.root, PathBuf::from("/src/shop"));
        assert_eq!(s.name(), "shop");
        assert_eq!(s.members[0].place, Place::Main);
        assert_eq!(s.members[1].place, Place::Worker(2));
        assert_eq!(s.members[1].worktree, PathBuf::from("/src/shop-wt-2"));
    }

    #[test]
    fn a_worktree_not_named_wt_n_is_numbered_by_its_order() {
        let r = repo("/src/shop/.git", &["/src/shop", "/tmp/hotfix", "/src/shop-wt-1", "/src/shop/.worktrees/try"]);
        let agents = [
            agent("tmux", "a", "/tmp/hotfix"),
            agent("tmux", "b", "/src/shop/.worktrees/try/x"),
            agent("tmux", "c", "/src/shop-wt-1"),
        ];
        let swarms = recognize(&agents, &[r]);
        let places: Vec<(usize, Place)> = swarms[0].members.iter().map(|m| (m.agent, m.place)).collect();
        // -wt-1 keeps 1; the others take 2 and 3 in worktree order, and the
        // worktree nested in the main checkout is a worker, not the manager.
        assert_eq!(places, vec![(2, Place::Worker(1)), (0, Place::Worker(2)), (1, Place::Worker(3))]);
    }

    #[test]
    fn two_repos_with_the_same_basename_stay_apart() {
        let a = repo("/work/shop/.git", &["/work/shop"]);
        let b = repo("/home/me/shop/.git", &["/home/me/shop", "/home/me/shop-wt-1"]);
        let agents = [agent("herdr", "w1:p1", "/work/shop"), agent("herdr", "w2:p1", "/home/me/shop-wt-1")];
        let swarms = recognize(&agents, &[a, b]);
        assert_eq!(swarms.len(), 2);
        assert!(swarms.iter().all(|s| s.name() == "shop"));
        assert_ne!(swarms[0].key, swarms[1].key);
        assert!(swarms.iter().all(|s| s.members.len() == 1));
    }

    #[test]
    fn one_repo_seen_through_tmux_and_herdr_is_one_swarm() {
        let agents = [agent("tmux", "claude-shop:review.0", "/src/shop"), agent("herdr", "w2:p1", "/src/shop-wt-1")];
        // Each source gathered the same repo.
        let swarms = recognize(&agents, &[shop(), shop()]);
        assert_eq!(swarms.len(), 1);
        let sources: Vec<&str> = swarms[0].members.iter().map(|m| agents[m.agent].source.as_str()).collect();
        assert_eq!(sources, vec!["tmux", "herdr"]);
    }

    #[test]
    fn an_agent_outside_any_repo_is_ignored() {
        let agents = [agent("herdr", "w4:p1", "/home/me/notes"), agent("herdr", "w5:p1", "/src/shopping")];
        // `/src/shopping` shares a prefix string with `/src/shop` but is not inside it.
        assert!(recognize(&agents, &[shop()]).is_empty());
    }

    #[test]
    fn duplicate_wt_numbers_do_not_collide() {
        let r = repo("/r/.git", &["/r", "/a/r-wt-1", "/b/r-wt-1"]);
        let numbers = worker_numbers(&r.worktrees);
        assert_eq!(numbers[Path::new("/a/r-wt-1")], 1);
        assert_eq!(numbers[Path::new("/b/r-wt-1")], 2);
    }

    #[test]
    fn parses_worktrees_in_order_with_branches() {
        let porcelain = "worktree /nonexistent/shop\nHEAD abc\nbranch refs/heads/master\n\n\
worktree /nonexistent/hotfix\nHEAD def\ndetached\n\n\
worktree /nonexistent/shop-wt-1\nHEAD 123\nbranch refs/heads/feature/issue-7\n";
        let wts = parse_worktrees(porcelain);
        assert_eq!(wts.len(), 3);
        assert_eq!(wts[0].path, PathBuf::from("/nonexistent/shop"));
        assert_eq!(wts[0].branch.as_deref(), Some("master"));
        assert_eq!(wts[1].branch, None);
        assert_eq!(wts[2].branch.as_deref(), Some("feature/issue-7"));
    }

    #[test]
    fn wt_numbers_come_from_the_folder_name() {
        assert_eq!(wt_number(Path::new("/src/shop-wt-3")), Some(3));
        assert_eq!(wt_number(Path::new("/src/shop")), None);
        assert_eq!(wt_number(Path::new("/src/my-wt-x")), None);
        assert_eq!(wt_number(Path::new("/src/-wt-2")), None);
    }
}
