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
//! Who the manager is comes from evidence, strongest first ([`choose_manager`]):
//!
//! 1. the pane an autocoder swarm manifest (`.autocoder/swarm/*.json`) names
//!    as the manager's, if that pane is live;
//! 2. an agent process carrying `AUTOCODER_MANAGER=<session>` in its
//!    environment (the marker the launcher exports);
//! 3. a label: a pane or workspace named `manager-…` (tmux: the `review`
//!    window);
//! 4. any agent in the main checkout.
//!
//! The rule that matched is recorded, and a tie at that rule picks one
//! deterministically and marks the choice ambiguous.
//!
//! [`recognize`] and [`choose_manager`] take only plain facts, so every source
//! (and the terminal view) can share them; [`gather`], [`read_manifests`] and
//! [`process_marker`] collect those facts.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

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

/// What an autocoder swarm manifest says about its manager.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SwarmManifest {
    /// The swarm's session (`claude-shop`): the value of its manager's
    /// `AUTOCODER_MANAGER` marker.
    pub session: String,
    /// The manager's pane in every multiplexer the manifest names
    /// (`herdrPane`, `tmuxTarget`, `cmuxWorkspace`).
    pub manager_panes: Vec<String>,
}

/// Every manifest in `text`: the `.autocoder/swarm/*.json` files one after
/// another, as `cat` prints them. A file that doesn't parse is skipped.
pub fn parse_manifests(text: &str) -> Vec<SwarmManifest> {
    serde_json::Deserializer::from_str(text)
        .into_iter::<Value>()
        .map_while(Result::ok)
        .filter_map(|v| {
            let session = v.get("session")?.as_str()?.to_string();
            let manager = v.get("manager");
            let manager_panes = ["herdrPane", "tmuxTarget", "cmuxWorkspace"]
                .iter()
                .filter_map(|k| manager?.get(*k)?.as_str())
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect();
            Some(SwarmManifest { session, manager_panes })
        })
        .collect()
}

/// The swarm manifests in a repo's main checkout. None when it has none (or
/// they can't be read).
pub async fn read_manifests(transport: &ServerTransport, root: &Path) -> Vec<SwarmManifest> {
    let cat = ["-c".to_string(), "cat .autocoder/swarm/*.json 2>/dev/null".to_string()];
    match transport.output("sh", &cat, Some(root)).await {
        Ok(out) => parse_manifests(&String::from_utf8_lossy(&out.stdout)),
        Err(_) => Vec::new(),
    }
}

/// The `AUTOCODER_MANAGER` marker in the environment of any of `pids` or
/// their descendants (`/proc/<pid>/environ`), the first one found.
pub async fn process_marker(transport: &ServerTransport, pids: &[u32]) -> Option<String> {
    if pids.is_empty() {
        return None;
    }
    const WALK: &str = r#"walk() { for p in "$@"; do
  v=$(tr '\0' '\n' < /proc/$p/environ 2>/dev/null | sed -n 's/^AUTOCODER_MANAGER=//p' | head -n 1)
  if [ -n "$v" ]; then echo "$v"; exit 0; fi
  walk $(cat /proc/$p/task/*/children 2>/dev/null)
done; }; walk "$@""#;
    let mut args = vec!["-c".to_string(), WALK.to_string(), "sh".to_string()];
    args.extend(pids.iter().map(u32::to_string));
    let out = transport.output("sh", &args, None).await.ok()?;
    let marker = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!marker.is_empty()).then_some(marker)
}

/// What a source knows about one agent that could make it the manager.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Evidence {
    /// Every name its pane goes by (herdr `w5:p1`; tmux `claude-shop:1.0` and
    /// `%12`), to match a manifest's manager pane.
    pub panes: Vec<String>,
    /// The `AUTOCODER_MANAGER` value in its process environment.
    pub marker: Option<String>,
    /// The pane's or workspace's name (tmux: the window's name).
    pub label: Option<String>,
}

/// Which rule picked the manager, strongest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagerRule {
    ManifestPane,
    ProcessMarker,
    Label,
    MainCheckout,
}

impl ManagerRule {
    pub fn describe(self) -> &'static str {
        match self {
            Self::ManifestPane => "the pane the swarm manifest names",
            Self::ProcessMarker => "its AUTOCODER_MANAGER process marker",
            Self::Label => "its manager label",
            Self::MainCheckout => "it is the agent in the main checkout",
        }
    }
}

/// The manager [`choose_manager`] picked.
#[derive(Debug, Clone, PartialEq)]
pub struct ManagerChoice {
    /// Index into the swarm's `members`.
    pub member: usize,
    pub rule: ManagerRule,
    /// How many agents the winning rule matched; more than one is a tie.
    pub candidates: usize,
}

impl ManagerChoice {
    /// Several agents matched the winning rule equally, and one was picked.
    pub fn ambiguous(&self) -> bool {
        self.candidates > 1
    }
}

/// A pane or workspace label that names the manager: `manager-<project>`, or
/// tmux's `review` window.
pub fn is_manager_label(label: &str) -> bool {
    label.starts_with("manager") || label.eq_ignore_ascii_case("review")
}

/// Pick a swarm's manager by evidence (see the module docs). `evidence` is
/// indexed like the agents passed to [`recognize`]; `manifests` are the
/// repo's. A marker counts when it names one of the manifests' sessions, or
/// is any marker when the repo has no manifest. Ties go to an agent in the
/// main checkout, then the lowest pane name. None when no rule matches.
pub fn choose_manager(swarm: &RepoSwarm, evidence: &[Evidence], manifests: &[SwarmManifest]) -> Option<ManagerChoice> {
    let ev = |m: &Member| evidence.get(m.agent);
    let rules: [(ManagerRule, &dyn Fn(&Member) -> bool); 4] = [
        (ManagerRule::ManifestPane, &|m| {
            ev(m).is_some_and(|e| {
                manifests.iter().any(|f| f.manager_panes.iter().any(|p| e.panes.contains(p)))
            })
        }),
        (ManagerRule::ProcessMarker, &|m| {
            ev(m).and_then(|e| e.marker.as_deref()).is_some_and(|marker| {
                manifests.is_empty() || manifests.iter().any(|f| f.session == marker)
            })
        }),
        (ManagerRule::Label, &|m| ev(m).and_then(|e| e.label.as_deref()).is_some_and(is_manager_label)),
        (ManagerRule::MainCheckout, &|m| m.place == Place::Main),
    ];
    for (rule, matches) in rules {
        let mut found: Vec<usize> = (0..swarm.members.len()).filter(|&i| matches(&swarm.members[i])).collect();
        if found.is_empty() {
            continue;
        }
        let first_pane = |i: usize| ev(&swarm.members[i]).and_then(|e| e.panes.first().cloned()).unwrap_or_default();
        found.sort_by_key(|&i| (swarm.members[i].place != Place::Main, first_pane(i), swarm.members[i].agent));
        return Some(ManagerChoice { member: found[0], rule, candidates: found.len() });
    }
    None
}

/// Why one agent was, or wasn't, made part of a swarm: what
/// `GET /api/swarms?explain=1` lists, one per agent each source saw.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct AgentExplanation {
    /// The source that saw it: `tmux`, `herdr`, `saved`, ...
    pub source: String,
    pub pane: String,
    pub cwd: String,
    /// Its pane's name or workspace label (tmux: the window's name).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The swarm it was put in (the project's name), when it was.
    pub swarm: Option<String>,
    /// That swarm's repo identity (its git common directory).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// `manager` or `worker-<N>`, when it was given one.
    pub role: Option<String>,
    /// For the manager: the evidence rule that picked it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manager_rule: Option<ManagerRule>,
    /// For the manager: that rule, in words.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manager_because: Option<String>,
    /// For the manager: several agents matched the rule and one was picked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ambiguous: Option<bool>,
    /// Why it was left out of every swarm, or given no role in one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left_out: Option<String>,
}

impl AgentExplanation {
    /// The agent as its source saw it, before any judgement.
    pub fn seen(fact: &AgentFact, name: Option<String>) -> Self {
        Self {
            source: fact.source.clone(),
            pane: fact.pane.clone(),
            cwd: fact.cwd.to_string_lossy().into_owned(),
            name,
            ..Self::default()
        }
    }

    /// Mark it as its swarm's manager, picked by `choice`.
    pub fn managing(&mut self, choice: &ManagerChoice) {
        self.role = Some("manager".to_string());
        self.manager_rule = Some(choice.rule);
        self.manager_because = Some(choice.rule.describe().to_string());
        self.ambiguous = Some(choice.ambiguous());
    }
}

/// Why an agent whose cwd no repo holds was left out.
pub const OUTSIDE_ANY_REPO: &str = "cwd is outside any git worktree (or git could not read it)";
/// Why a repo with a lone, unmarked agent in its main checkout is no swarm.
pub const LONE_AGENT: &str =
    "lone agent in the main checkout: no worker, no swarm manifest, no AUTOCODER_MANAGER marker and no manager label, so an ordinary session, not a swarm";

/// How the recognizer judged one repo's agents.
#[derive(Debug, Clone, PartialEq)]
pub struct Judged {
    pub swarm: RepoSwarm,
    pub choice: Option<ManagerChoice>,
    /// Whether the repo counts as a swarm; when not, why not.
    pub left_out: Option<String>,
}

/// Every agent and what became of it: its swarm and role, the rule that made
/// a manager, or why it was left out. `names` is indexed like `agents`.
pub fn explain(agents: &[AgentFact], names: &[Option<String>], judged: &[Judged]) -> Vec<AgentExplanation> {
    agents
        .iter()
        .enumerate()
        .map(|(i, fact)| {
            let mut out = AgentExplanation::seen(fact, names.get(i).cloned().flatten());
            let Some((j, k)) = judged
                .iter()
                .find_map(|j| j.swarm.members.iter().position(|m| m.agent == i).map(|k| (j, k)))
            else {
                out.left_out = Some(OUTSIDE_ANY_REPO.to_string());
                return out;
            };
            if let Some(why) = &j.left_out {
                out.left_out = Some(why.clone());
                return out;
            }
            out.swarm = Some(j.swarm.name());
            out.repo = Some(j.swarm.key.to_string_lossy().into_owned());
            let member = &j.swarm.members[k];
            match (&j.choice, member.place) {
                (Some(c), _) if c.member == k => out.managing(c),
                (Some(c), Place::Main) => {
                    let manager = &agents[j.swarm.members[c.member].agent].pane;
                    out.left_out = Some(format!("in the main checkout, but {manager} is the manager ({})", c.rule.describe()));
                }
                (None, Place::Main) => out.left_out = Some("in the main checkout, but no manager was chosen".to_string()),
                (_, Place::Worker(n)) => out.role = Some(format!("worker-{n}")),
            }
            out
        })
        .collect()
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

    fn ev(pane: &str, marker: Option<&str>, label: Option<&str>) -> Evidence {
        Evidence { panes: vec![pane.into()], marker: marker.map(Into::into), label: label.map(Into::into) }
    }

    fn manifest(session: &str, pane: &str) -> SwarmManifest {
        SwarmManifest { session: session.into(), manager_panes: vec![pane.into()] }
    }

    /// Three agents in the main checkout and one worker, each with `evidence`.
    fn manager_of(evidence: &[Evidence], manifests: &[SwarmManifest]) -> Option<(String, ManagerRule, bool)> {
        let agents: Vec<AgentFact> = evidence
            .iter()
            .enumerate()
            .map(|(i, e)| agent("herdr", &e.panes[0], if i < 3 { "/src/shop" } else { "/src/shop-wt-1" }))
            .collect();
        let swarm = recognize(&agents, &[shop()]).remove(0);
        let c = choose_manager(&swarm, evidence, manifests)?;
        Some((evidence[swarm.members[c.member].agent].panes[0].clone(), c.rule, c.ambiguous()))
    }

    #[test]
    fn the_manifest_pane_wins_over_a_labelled_pane() {
        let e = [ev("w1:p1", None, Some("manager-shop")), ev("w2:p1", None, None), ev("w3:p1", None, None), ev("w4:p1", None, None)];
        let got = manager_of(&e, &[manifest("claude-shop", "w2:p1")]);
        assert_eq!(got, Some(("w2:p1".into(), ManagerRule::ManifestPane, false)));
    }

    #[test]
    fn the_process_marker_wins_over_a_label() {
        let e = [
            ev("w1:p1", None, Some("manager-shop")),
            ev("w2:p1", Some("claude-shop"), None),
            ev("w3:p1", None, None),
            ev("w4:p1", None, None),
        ];
        assert_eq!(manager_of(&e, &[]), Some(("w2:p1".into(), ManagerRule::ProcessMarker, false)));
        // With a manifest, the marker must name its session.
        assert_eq!(
            manager_of(&e, &[manifest("claude-other", "w9:p9")]),
            Some(("w1:p1".into(), ManagerRule::Label, false))
        );
    }

    #[test]
    fn two_unlabelled_agents_in_the_main_checkout_are_ambiguous() {
        let e = [ev("w3:p1", None, None), ev("w1:p1", None, None), ev("w2:p1", None, None), ev("w4:p1", None, None)];
        // Deterministic: the lowest pane, whatever order they were seen in.
        assert_eq!(manager_of(&e, &[]), Some(("w1:p1".into(), ManagerRule::MainCheckout, true)));
    }

    #[test]
    fn a_dead_manifest_pane_falls_through_to_the_next_rule() {
        let e = [ev("w1:p1", None, None), ev("w2:p1", None, Some("manager-shop")), ev("w3:p1", None, None), ev("w4:p1", None, None)];
        // The manifest names w8:p1, which no live agent has.
        let got = manager_of(&e, &[manifest("claude-shop", "w8:p1")]);
        assert_eq!(got, Some(("w2:p1".into(), ManagerRule::Label, false)));
    }

    #[test]
    fn a_tmux_pane_matches_the_manifest_by_its_pane_id() {
        let mut e = [ev("claude-shop:1.0", None, Some("review")), ev("claude-shop:2.0", None, None), ev("x", None, None), ev("y", None, None)];
        e[1].panes.push("%12".into());
        let got = manager_of(&e, &[manifest("claude-shop", "%12")]);
        assert_eq!(got, Some(("claude-shop:2.0".into(), ManagerRule::ManifestPane, false)));
    }

    #[test]
    fn workers_alone_have_no_manager() {
        let agents = [agent("herdr", "w4:p1", "/src/shop-wt-1")];
        let swarm = recognize(&agents, &[shop()]).remove(0);
        assert_eq!(choose_manager(&swarm, &[Evidence::default()], &[]), None);
    }

    #[test]
    fn parses_manifests_one_after_another() {
        let text = r#"{"version":1,"session":"claude-shop","manager":{"tmuxTarget":null,"cmuxWorkspace":null,"herdrPane":"w5:p1"}}
{"session":"codex-shop","manager":{"tmuxTarget":"%3"}}
{"no":"session"}"#;
        let m = parse_manifests(text);
        assert_eq!(m, vec![manifest("claude-shop", "w5:p1"), manifest("codex-shop", "%3")]);
        assert!(parse_manifests("").is_empty());
    }

    #[test]
    fn manager_labels() {
        assert!(is_manager_label("manager-shop"));
        assert!(is_manager_label("review"));
        assert!(!is_manager_label("wt1-shop"));
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn the_process_marker_is_found_in_a_descendant() {
        // A shell (no marker) whose child carries the marker, as a pane's shell
        // runs the agent.
        let mut shell = std::process::Command::new("sh")
            .args(["-c", "AUTOCODER_MANAGER=claude-test sleep 30 & wait"])
            .env_remove("AUTOCODER_MANAGER")
            .spawn()
            .unwrap();
        let transport = ServerTransport::new(None);
        let mut found = None;
        for _ in 0..50 {
            found = process_marker(&transport, &[shell.id()]).await;
            if found.is_some() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert_eq!(found.as_deref(), Some("claude-test"));
        assert_eq!(process_marker(&transport, &[]).await, None);
        let _ = std::process::Command::new("pkill").args(["-P", &shell.id().to_string()]).status();
        let _ = shell.kill();
        let _ = shell.wait();
    }
}

