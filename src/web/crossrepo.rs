//! Links between the repos of a valley's group (#528, phase 4 of #494): what
//! `/api/valley` carries as each group's `ropes` and `caravans`.
//!
//! - A **rope** ties an open issue in one repo to the open issue in another
//!   repo of the group that blocks it: a `Depends on:` or `Blocked by:` line
//!   naming `owner/repo#N` or `https://github.com/owner/repo/issues/N`. A
//!   closed blocker blocks nothing, and a link to a repo outside the group is
//!   not drawn.
//! - A **caravan** is a parent issue whose subtasks live in more than one repo
//!   of the group: a subtask says `Sub-task of <ref>` (or `Parent: <ref>`),
//!   or the parent lists it in a task list (`- [ ] <ref>`). One crate per
//!   repo, `loaded` once every subtask there has landed (closed as
//!   completed); the caravan has `arrived` when every crate is loaded.
//!
//! Both are read from issue bodies, as `gh issue list --json number,title,
//! state,stateReason,body` gives them, and from GitHub's own sub-issue and
//! blocked-by relations (#532), which that listing leaves out: one `gh api
//! graphql` read of the 100 most recently updated issues' `subIssues` and
//! `blockedBy` ([`NATIVE_QUERY`]). A native sub-issue counts as a task-list
//! entry, a native blocker as a `Blocked by:` line. When the GraphQL read
//! fails, the bodies alone are used. Each repo is read once every 90 s, in
//! the background: a poll never waits on GitHub (`fetch`). A repo GitHub
//! can't be asked about is left out, not an error.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::transport::ServerTransport;

/// An issue as `gh issue list --json number,title,state,stateReason,body` gives it.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CrossIssue {
    #[serde(default)]
    pub number: u32,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub state: String,
    #[serde(default, rename = "stateReason")]
    pub state_reason: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    /// GitHub's own relations, from [`NATIVE_QUERY`]: not in `gh issue list`.
    #[serde(skip)]
    pub native: Native,
}

/// An issue's native relations, as (`owner/repo` lowercased, number).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Native {
    /// Its sub-issues.
    pub subs: Vec<(String, u32)>,
    /// The issues it is blocked by.
    pub blocked_by: Vec<(String, u32)>,
    /// It has more sub-issues than the read returned: its caravan can't be
    /// known to have arrived.
    pub subs_cut: bool,
}

/// The native relations of a repo's 100 most recently updated issues, open
/// or closed: about 7,000 nodes, well inside GraphQL's limits (a call costs 2
/// of the 5,000 GraphQL points an hour). A relation on an older issue is not
/// seen; past 50 sub-issues an epic's caravan never shows as arrived.
pub const NATIVE_QUERY: &str = "query($owner:String!,$name:String!){repository(owner:$owner,name:$name){issues(first:100,orderBy:{field:UPDATED_AT,direction:DESC}){nodes{number \
    subIssues(first:50){totalCount nodes{number repository{nameWithOwner}}} blockedBy(first:20){nodes{number repository{nameWithOwner}}}}}}}";

/// [`NATIVE_QUERY`]'s answer → each issue's relations, by number; `None`
/// when it isn't one (an error, a schema without these fields).
pub fn parse_native(json: &[u8]) -> Option<HashMap<u32, Native>> {
    let v: serde_json::Value = serde_json::from_slice(json).ok()?;
    let nodes = v.pointer("/data/repository/issues/nodes")?.as_array()?;
    let refs = |n: &serde_json::Value, field: &str| -> Vec<(String, u32)> {
        n.get(field)
            .and_then(|f| f.get("nodes"))
            .and_then(|a| a.as_array())
            .into_iter()
            .flatten()
            .filter_map(|r| {
                let slug = r.pointer("/repository/nameWithOwner")?.as_str()?.to_ascii_lowercase();
                Some((slug, u32::try_from(r.get("number")?.as_u64()?).ok()?))
            })
            .collect()
    };
    Some(
        nodes
            .iter()
            .filter_map(|n| {
                let subs = refs(n, "subIssues");
                let total = n.pointer("/subIssues/totalCount").and_then(|t| t.as_u64()).unwrap_or(0);
                let subs_cut = total > subs.len() as u64;
                Some((u32::try_from(n.get("number")?.as_u64()?).ok()?, Native { subs, blocked_by: refs(n, "blockedBy"), subs_cut }))
            })
            .filter(|(_, r)| !r.subs.is_empty() || !r.blocked_by.is_empty())
            .collect(),
    )
}

/// Give each issue its native relations.
pub fn attach_native(issues: &mut [CrossIssue], native: &HashMap<u32, Native>) {
    for i in issues {
        if let Some(r) = native.get(&i.number) {
            i.native = r.clone();
        }
    }
}

impl CrossIssue {
    fn open(&self) -> bool {
        self.state.eq_ignore_ascii_case("open")
    }
    /// Closed as completed: work that landed.
    fn landed(&self) -> bool {
        self.state.eq_ignore_ascii_case("closed")
            && !matches!(self.state_reason.as_deref().map(str::to_ascii_uppercase).as_deref(), Some("NOT_PLANNED") | Some("DUPLICATE"))
    }
}

/// One issue in one repo of the group.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct IssueRef {
    /// The repo's name in the group.
    pub repo: String,
    pub number: u32,
    pub title: String,
    /// `open` or `closed`.
    pub state: String,
}

/// An open issue held up by an open issue in another repo of the group.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Rope {
    /// The issue that waits.
    pub from: IssueRef,
    /// The issue it waits on.
    pub to: IssueRef,
}

/// A parent's subtasks in one repo.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct CaravanCrate {
    pub repo: String,
    pub issues: Vec<u32>,
    /// Every subtask here has landed.
    pub loaded: bool,
}

/// A parent issue whose subtasks are spread over several repos of the group.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Caravan {
    pub parent: IssueRef,
    /// One per repo, in the group's order.
    pub crates: Vec<CaravanCrate>,
    /// Every crate is loaded.
    pub arrived: bool,
}

/// `owner/repo`, lowercased, as GitHub names it.
pub fn slug_of(web_url: &str) -> Option<String> {
    let rest = web_url.trim().trim_end_matches('/').strip_prefix("https://github.com/")?;
    let mut parts = rest.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(o), Some(r), None) if !o.is_empty() && !r.is_empty() => Some(format!("{o}/{r}").to_ascii_lowercase()),
        _ => None,
    }
}

fn is_slug_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')
}

/// The issues a line names, as (`owner/repo` lowercased, number): `#N` in
/// `own`, `owner/repo#N`, and `https://github.com/owner/repo/issues/N`.
pub fn refs(line: &str, own: &str) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    let digits = |s: &str| s.chars().take_while(char::is_ascii_digit).collect::<String>().parse::<u32>().ok();
    // Links first, then remove them so their `#` or number is not read twice.
    let mut rest = line.to_string();
    while let Some(k) = rest.find("https://github.com/") {
        let tail = &rest[k + "https://github.com/".len()..];
        let mut parts = tail.splitn(4, '/');
        if let (Some(o), Some(r), Some("issues" | "pull"), Some(n)) = (parts.next(), parts.next(), parts.next(), parts.next())
            && let Some(n) = digits(n)
        {
            out.push((format!("{o}/{r}").to_ascii_lowercase(), n));
        }
        let end = tail.find(char::is_whitespace).map_or(rest.len(), |e| k + "https://github.com/".len() + e);
        rest.replace_range(k..end, " ");
    }
    let b: Vec<char> = rest.chars().collect();
    for (k, c) in b.iter().enumerate() {
        if *c != '#' {
            continue;
        }
        let num: String = b[k + 1..].iter().take_while(|c| c.is_ascii_digit()).collect();
        let Ok(n) = num.parse::<u32>() else { continue };
        // What runs into the `#` from the left: `owner/repo`, or nothing.
        let mut s = k;
        while s > 0 && (is_slug_char(b[s - 1]) || b[s - 1] == '/') {
            s -= 1;
        }
        let before: String = b[s..k].iter().collect();
        if before.is_empty() {
            out.push((own.to_string(), n));
        } else if before.matches('/').count() == 1 && !before.starts_with('/') && !before.ends_with('/') {
            out.push((before.to_ascii_lowercase(), n));
        }
    }
    out
}

/// A line's label, lowercased, with list marks and emphasis taken off:
/// `- **Depends on:** x` → `depends on: x`.
fn plain(line: &str) -> String {
    line.trim().trim_start_matches(['-', '*', '+', '>', ' ']).replace("**", "").replace('_', " ").trim().to_ascii_lowercase()
}

fn blockers(body: &str, own: &str) -> Vec<(String, u32)> {
    body.lines()
        .filter(|l| {
            let p = plain(l);
            p.starts_with("depends on") || p.starts_with("blocked by")
        })
        .flat_map(|l| refs(l, own))
        .collect()
}

/// The parents a subtask names: `Sub-task of <ref>`, `Subtask of <ref>`, `Parent: <ref>`.
fn parents(body: &str, own: &str) -> Vec<(String, u32)> {
    body.lines()
        .filter(|l| {
            let p = plain(l).trim_start_matches('#').trim().to_string();
            p.starts_with("sub-task of") || p.starts_with("subtask of") || p.starts_with("parent:")
        })
        .flat_map(|l| refs(l, own))
        .collect()
}

/// The subtasks a parent lists in a task list: `- [ ] <ref>`, `- [x] <ref>`.
fn listed(body: &str, own: &str) -> Vec<(String, u32)> {
    body.lines()
        .filter_map(|l| {
            let t = l.trim_start().trim_start_matches(['-', '*', '+']).trim_start();
            t.strip_prefix("[ ]").or_else(|| t.strip_prefix("[x]")).or_else(|| t.strip_prefix("[X]"))
        })
        .flat_map(|l| refs(l, own).into_iter().take(1))
        .collect()
}

/// The ropes and caravans among a group's repos: `repos` is each repo's name
/// in the group and its `owner/repo` (lowercased), upstream first; `issues`
/// is what was read of each, by name. A repo with no issues read is left out.
pub fn links(repos: &[(String, String)], issues: &HashMap<String, Vec<CrossIssue>>) -> (Vec<Rope>, Vec<Caravan>) {
    let name_of: HashMap<&str, &str> = repos.iter().map(|(n, s)| (s.as_str(), n.as_str())).collect();
    let order: HashMap<&str, usize> = repos.iter().enumerate().map(|(k, (n, _))| (n.as_str(), k)).collect();
    let find = |slug: &str, n: u32| -> Option<(&str, &CrossIssue)> {
        let name = *name_of.get(slug)?;
        issues.get(name)?.iter().find(|i| i.number == n).map(|i| (name, i))
    };
    let iref = |repo: &str, i: &CrossIssue| IssueRef {
        repo: repo.to_string(),
        number: i.number,
        title: i.title.clone(),
        state: if i.open() { "open" } else { "closed" }.into(),
    };

    let mut ropes = Vec::new();
    let mut children: BTreeMap<(String, u32), BTreeSet<(String, u32)>> = BTreeMap::new();
    // Parents with more native sub-issues than were read (#532).
    let mut cut: BTreeSet<(String, u32)> = BTreeSet::new();
    for (name, slug) in repos {
        for i in issues.get(name).into_iter().flatten() {
            let body = i.body.as_deref().unwrap_or("");
            if i.open() {
                let mut seen = BTreeSet::new();
                for (s, n) in blockers(body, slug).into_iter().chain(i.native.blocked_by.iter().cloned()) {
                    if s == *slug || !seen.insert((s.clone(), n)) {
                        continue;
                    }
                    if let Some((other, b)) = find(&s, n).filter(|(_, b)| b.open()) {
                        ropes.push(Rope { from: iref(name, i), to: iref(other, b) });
                    }
                }
            }
            for p in parents(body, slug) {
                if name_of.contains_key(p.0.as_str()) {
                    children.entry(p).or_default().insert((slug.clone(), i.number));
                }
            }
            if i.native.subs_cut {
                cut.insert((slug.clone(), i.number));
            }
            for c in listed(body, slug).into_iter().chain(i.native.subs.iter().cloned()) {
                if name_of.contains_key(c.0.as_str()) {
                    children.entry((slug.clone(), i.number)).or_default().insert(c);
                }
            }
        }
    }

    let mut caravans = Vec::new();
    for ((pslug, pn), kids) in children {
        let partial = cut.contains(&(pslug.clone(), pn));
        let Some((pname, parent)) = find(&pslug, pn) else { continue };
        let mut by_repo: BTreeMap<&str, (Vec<u32>, bool)> = BTreeMap::new();
        for (s, n) in &kids {
            // A subtask in the group the read didn't reach (older than it) is
            // not known to have landed: its crate isn't loaded.
            let (repo, landed) = match find(s, *n) {
                Some((repo, kid)) => (repo, kid.landed()),
                None => match name_of.get(s.as_str()) {
                    Some(repo) => (*repo, false),
                    None => continue,
                },
            };
            let e = by_repo.entry(repo).or_insert((Vec::new(), true));
            e.0.push(*n);
            e.1 &= landed;
        }
        if by_repo.len() < 2 {
            continue;
        }
        let mut crates: Vec<CaravanCrate> = by_repo.into_iter().map(|(repo, (issues, loaded))| CaravanCrate { repo: repo.into(), issues, loaded }).collect();
        crates.sort_by_key(|c| order.get(c.repo.as_str()).copied().unwrap_or(usize::MAX));
        // Sub-issues the read cut off aren't known to have landed.
        let arrived = !partial && crates.iter().all(|c| c.loaded);
        caravans.push(Caravan { parent: iref(pname, parent), crates, arrived });
    }
    (ropes, caravans)
}

/// How long a repo's issues are reused: as long as the village's GitHub summary.
const FRESH: Duration = Duration::from_secs(90);

/// What was last read of one repo: when, its `owner/repo`, and its issues.
#[derive(Debug, Clone, Default)]
pub struct RepoRead {
    pub slug: Option<String>,
    pub issues: Option<Vec<CrossIssue>>,
}

type Reads = std::sync::Mutex<HashMap<std::path::PathBuf, (Instant, RepoRead)>>;
static READS: OnceLock<Reads> = OnceLock::new();
/// The repos being read now: one refresh each at a time.
static READING: OnceLock<std::sync::Mutex<std::collections::HashSet<std::path::PathBuf>>> = OnceLock::new();

/// A repo's `owner/repo` and issues for [`links`], as last read: never read
/// on the poll that asks. When nothing has been read yet, or it is older
/// than [`FRESH`], a read starts in the background (one per repo at a time)
/// and this poll gets what there was, or nothing; the next poll gets the new
/// read. A repo GitHub can't be asked about reads as no slug or no issues.
pub fn fetch(transport: &ServerTransport, dir: &Path) -> RepoRead {
    let reads = READS.get_or_init(Default::default);
    let held = reads.lock().unwrap_or_else(|e| e.into_inner()).get(dir).cloned();
    if held.as_ref().is_some_and(|(at, _)| at.elapsed() < FRESH) {
        return held.map(|(_, r)| r).unwrap_or_default();
    }
    // No runtime (a plain test): nothing to read in, and nothing marked as being read.
    if tokio::runtime::Handle::try_current().is_ok()
        && READING.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner()).insert(dir.to_path_buf())
    {
        let (transport, dir) = (transport.clone(), dir.to_path_buf());
        tokio::spawn(async move {
            let read = read_repo(&transport, &dir).await;
            READS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner()).insert(dir.clone(), (Instant::now(), read));
            READING.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner()).remove(&dir);
        });
    }
    held.map(|(_, r)| r).unwrap_or_default()
}

/// One read of a repo: its origin's `owner/repo`, and its issues with their
/// bodies (`gh issue list --state all`, the newest 300: a blocker or subtask
/// older than that is not seen, and its rope or caravan crate is left out).
async fn read_repo(transport: &ServerTransport, dir: &Path) -> RepoRead {
    let slug = crate::github::origin_web_url(transport, dir).await.as_deref().and_then(slug_of);
    if slug.is_none() {
        return RepoRead::default();
    }
    let args: Vec<String> = ["issue", "list", "--state", "all", "--limit", "300", "--json", "number,title,state,stateReason,body"].map(String::from).to_vec();
    let mut issues = match crate::github::gh_repo_output(transport, dir, &args).await {
        Ok(out) if out.status.success() => serde_json::from_slice::<Vec<CrossIssue>>(&out.stdout).ok(),
        _ => None,
    };
    // GitHub's own relations (#532); a failed read leaves the bodies' alone.
    if let (Some(list), Some((owner, name))) = (issues.as_mut(), slug.as_deref().and_then(|s| s.split_once('/'))) {
        let args: Vec<String> =
            vec!["api".into(), "graphql".into(), "-f".into(), format!("query={NATIVE_QUERY}"), "-f".into(), format!("owner={owner}"), "-f".into(), format!("name={name}")];
        if let Ok(out) = crate::github::gh_repo_output(transport, dir, &args).await
            && out.status.success()
            && let Some(native) = parse_native(&out.stdout)
        {
            attach_native(list, &native);
        }
    }
    RepoRead { slug, issues }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(number: u32, title: &str, state: &str, reason: Option<&str>, body: &str) -> CrossIssue {
        CrossIssue { number, title: title.into(), state: state.into(), state_reason: reason.map(Into::into), body: Some(body.into()), ..Default::default() }
    }
    fn group() -> Vec<(String, String)> {
        vec![("lib-core".into(), "acme/lib-core".into()), ("api".into(), "acme/api".into()), ("web".into(), "acme/web".into())]
    }

    #[test]
    fn refs_read_every_link_form() {
        assert_eq!(refs("Depends on: acme/lib-core#12 and #3", "acme/api"), vec![("acme/lib-core".into(), 12), ("acme/api".into(), 3)]);
        assert_eq!(refs("Blocked by https://github.com/Acme/Lib-Core/issues/7.", "acme/api"), vec![("acme/lib-core".into(), 7)]);
        assert_eq!(refs("see a#4, x/#5 and owner/repo/sub#6", "acme/api"), Vec::<(String, u32)>::new());
        assert_eq!(slug_of("https://github.com/Acme/API"), Some("acme/api".into()));
        assert_eq!(slug_of("https://gitlab.com/acme/api"), None);
    }

    #[test]
    fn ropes_tie_open_issues_to_their_open_blockers_in_the_group() {
        let mut issues = HashMap::new();
        issues.insert("lib-core".to_string(), vec![issue(12, "Add the money type", "OPEN", None, ""), issue(13, "Old fix", "CLOSED", Some("COMPLETED"), "")]);
        issues.insert("api".to_string(), vec![
            issue(40, "Charge in cents", "OPEN", None, "Needs the money type.\n\n- **Depends on:** acme/lib-core#12\nBlocked by: acme/lib-core#13\nDepends on: elsewhere/other#1\nDepends on: #41"),
            issue(41, "Same repo", "OPEN", None, ""),
            issue(42, "Done already", "CLOSED", Some("COMPLETED"), "Depends on: acme/lib-core#12"),
        ]);
        issues.insert("web".to_string(), vec![issue(7, "Show cents", "OPEN", None, "Blocked by https://github.com/acme/api/issues/40")]);
        let (ropes, _) = links(&group(), &issues);
        let got: Vec<(String, u32, String, u32)> = ropes.iter().map(|r| (r.from.repo.clone(), r.from.number, r.to.repo.clone(), r.to.number)).collect();
        // Not: #13 (closed), elsewhere/other (outside the group), #41 (same repo), #42 (closed itself).
        assert_eq!(got, vec![("api".into(), 40, "lib-core".into(), 12), ("web".into(), 7, "api".into(), 40)]);
        assert_eq!(ropes[0].to.title, "Add the money type");
        assert_eq!(ropes[0].to.state, "open");
    }

    #[test]
    fn a_caravan_is_a_parent_with_subtasks_in_several_repos() {
        let mut issues = HashMap::new();
        issues.insert("lib-core".to_string(), vec![
            issue(1, "Money everywhere", "OPEN", None, "Tasks:\n- [x] acme/lib-core#2\n- [ ] acme/web#9"),
            issue(2, "Money type", "CLOSED", Some("COMPLETED"), ""),
            issue(5, "One-repo epic", "OPEN", None, "- [ ] #6"),
            issue(6, "Its only task", "OPEN", None, "Sub-task of #5"),
        ]);
        issues.insert("api".to_string(), vec![
            issue(30, "Cents in the API", "CLOSED", Some("COMPLETED"), "## Sub-task of acme/lib-core#1"),
            issue(31, "Cents in errors", "OPEN", None, "Parent: https://github.com/acme/lib-core/issues/1"),
        ]);
        issues.insert("web".to_string(), vec![issue(9, "Show cents", "OPEN", None, "")]);
        let (_, caravans) = links(&group(), &issues);
        assert_eq!(caravans.len(), 1, "the one-repo epic is no caravan");
        let c = &caravans[0];
        assert_eq!((c.parent.repo.as_str(), c.parent.number), ("lib-core", 1));
        let crates: Vec<(&str, Vec<u32>, bool)> = c.crates.iter().map(|k| (k.repo.as_str(), k.issues.clone(), k.loaded)).collect();
        assert_eq!(crates, vec![("lib-core", vec![2], true), ("api", vec![30, 31], false), ("web", vec![9], false)]);
        assert!(!c.arrived);

        // Every subtask landed: the caravan has arrived.
        for list in issues.values_mut() {
            for i in list.iter_mut().filter(|i| [30, 31, 9].contains(&i.number)) {
                i.state = "CLOSED".into();
                i.state_reason = Some("COMPLETED".into());
            }
        }
        let (_, caravans) = links(&group(), &issues);
        assert!(caravans[0].arrived && caravans[0].crates.iter().all(|k| k.loaded));
        // A listed subtask the read didn't reach: not known to have landed.
        issues.get_mut("lib-core").unwrap()[0].body = Some("- [x] acme/lib-core#2\n- [ ] acme/web#9\n- [ ] acme/web#3".into());
        let c = &links(&group(), &issues).1[0];
        assert_eq!(c.crates.last().map(|k| (k.issues.clone(), k.loaded)), Some((vec![3, 9], false)));
        assert!(!c.arrived);
        issues.get_mut("lib-core").unwrap()[0].body = Some("Tasks:\n- [x] acme/lib-core#2\n- [ ] acme/web#9".into());
        // Closed as not planned is not landed.
        issues.get_mut("web").unwrap()[0].state_reason = Some("NOT_PLANNED".into());
        assert!(!links(&group(), &issues).1[0].arrived);
    }

    /// [`NATIVE_QUERY`]'s answer, as GitHub gives it, cut to what matters.
    const NATIVE: &str = r#"{"data":{"repository":{"issues":{"nodes":[
      {"number":1,"subIssues":{"totalCount":2,"nodes":[{"number":2,"repository":{"nameWithOwner":"acme/lib-core"}},{"number":9,"repository":{"nameWithOwner":"Acme/Web"}}]},"blockedBy":{"nodes":[]}},
      {"number":5,"subIssues":{"nodes":[]},"blockedBy":{"nodes":[]}},
      {"number":12,"subIssues":{"nodes":[]},"blockedBy":{"nodes":[{"number":40,"repository":{"nameWithOwner":"acme/api"}}]}}
    ]}}}}"#;

    #[test]
    fn native_relations_are_parsed_by_issue() {
        let n = parse_native(NATIVE.as_bytes()).unwrap();
        assert_eq!(n.len(), 2, "an issue with no relations is left out");
        assert_eq!(n[&1].subs, vec![("acme/lib-core".into(), 2), ("acme/web".into(), 9)]);
        assert_eq!(n[&12].blocked_by, vec![("acme/api".into(), 40)]);
        // An error answer (a schema without these fields, a bad token) is not a read.
        assert_eq!(parse_native(br#"{"errors":[{"message":"Field 'blockedBy' doesn't exist"}]}"#), None);
        assert_eq!(parse_native(b"not json"), None);
    }

    #[test]
    fn native_sub_issues_load_caravans_and_native_blockers_tie_ropes() {
        let mut issues = HashMap::new();
        // Nothing in any body: only GitHub's own relations link them.
        let mut lib = vec![issue(1, "Money everywhere", "OPEN", None, ""), issue(2, "Money type", "CLOSED", Some("COMPLETED"), ""), issue(12, "Add the money type", "OPEN", None, "")];
        let api = vec![issue(40, "Charge in cents", "OPEN", None, "")];
        let web = vec![issue(9, "Show cents", "OPEN", None, "")];
        let native = parse_native(NATIVE.as_bytes()).unwrap();
        attach_native(&mut lib, &native);
        issues.insert("lib-core".to_string(), lib.clone());
        issues.insert("api".to_string(), api);
        issues.insert("web".to_string(), web);
        let (ropes, caravans) = links(&group(), &issues);
        let got: Vec<(String, u32, String, u32)> = ropes.iter().map(|r| (r.from.repo.clone(), r.from.number, r.to.repo.clone(), r.to.number)).collect();
        // lib-core#12 is blocked by api#40, natively: one rope, that way only.
        assert_eq!(got, vec![("lib-core".into(), 12, "api".into(), 40)]);
        assert_eq!(caravans.len(), 1);
        let crates: Vec<(&str, Vec<u32>, bool)> = caravans[0].crates.iter().map(|k| (k.repo.as_str(), k.issues.clone(), k.loaded)).collect();
        assert_eq!(crates, vec![("lib-core", vec![2], true), ("web", vec![9], false)]);

        // The same link in a body and natively is one rope, one crate entry.
        lib[2].body = Some("Blocked by: acme/api#40".into());
        lib[0].body = Some("- [ ] acme/web#9".into());
        issues.insert("lib-core".to_string(), lib);
        let (ropes, caravans) = links(&group(), &issues);
        assert_eq!(ropes.iter().filter(|r| r.from.number == 12).count(), 1);
        assert_eq!(caravans[0].crates.iter().find(|k| k.repo == "web").map(|k| k.issues.clone()), Some(vec![9]));
    }

    /// An epic with more native sub-issues than the read returns (50) never
    /// arrives on the ones it saw.
    #[test]
    fn a_cut_off_list_of_sub_issues_never_arrives() {
        let answer = r#"{"data":{"repository":{"issues":{"nodes":[{"number":1,"subIssues":{"totalCount":60,"nodes":[
          {"number":2,"repository":{"nameWithOwner":"acme/lib-core"}},{"number":9,"repository":{"nameWithOwner":"acme/web"}}]},"blockedBy":{"nodes":[]}}]}}}}"#;
        let native = parse_native(answer.as_bytes()).unwrap();
        assert!(native[&1].subs_cut);
        assert!(!parse_native(NATIVE.as_bytes()).unwrap()[&1].subs_cut, "all read: not cut");
        let mut lib = vec![issue(1, "Epic", "OPEN", None, ""), issue(2, "A", "CLOSED", Some("COMPLETED"), "")];
        attach_native(&mut lib, &native);
        let mut issues = HashMap::new();
        issues.insert("lib-core".to_string(), lib);
        issues.insert("web".to_string(), vec![issue(9, "B", "CLOSED", Some("COMPLETED"), "")]);
        let c = &links(&group(), &issues).1[0];
        assert!(c.crates.iter().all(|k| k.loaded), "every crate it saw has landed");
        assert!(!c.arrived, "but 58 sub-issues weren't read");
    }

    #[test]
    fn without_native_relations_the_bodies_alone_count() {
        // A failed GraphQL read attaches nothing: the issues are as the listing gave them.
        let mut lib = vec![issue(1, "Epic", "OPEN", None, "- [ ] acme/web#9")];
        attach_native(&mut lib, &HashMap::new());
        assert_eq!(lib[0].native, Native::default());
        let mut issues = HashMap::new();
        issues.insert("lib-core".to_string(), lib);
        issues.insert("web".to_string(), vec![issue(9, "Show cents", "OPEN", None, "")]);
        issues.insert("api".to_string(), vec![]);
        let (_, caravans) = links(&group(), &issues);
        // One repo's subtask alone is no caravan; the parent's own repo has none here.
        assert!(caravans.is_empty());
        issues.get_mut("lib-core").unwrap().push(issue(2, "Money type", "OPEN", None, "Sub-task of #1"));
        assert_eq!(links(&group(), &issues).1.len(), 1, "the bodies still make the caravan");
    }

    #[test]
    fn a_repo_not_read_is_left_out() {
        let mut issues = HashMap::new();
        issues.insert("api".to_string(), vec![issue(40, "x", "OPEN", None, "Depends on: acme/lib-core#12")]);
        let (ropes, caravans) = links(&group(), &issues);
        assert!(ropes.is_empty() && caravans.is_empty());
    }
}
