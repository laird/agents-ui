//! autocoder's issue lifecycle: `issue-events.jsonl` (laird/agents#190).
//!
//! The verbs that move an issue through its life (claim, release, the merge
//! job's first commit, each gate attempt, the landing, a deploy) append one
//! JSON line each, at the moment they act, to
//! `${AUTOCODER_ISSUE_EVENTS:-${XDG_STATE_HOME:-~/.local/state}/autocoder/issue-events.jsonl}`.
//! The format is a contract, documented in laird/agents `scripts/README.md`
//! ("Issue lifecycle"): field-set version 1, and a reader skips lines whose
//! `v` it does not know.
//!
//! The file is shared by every repo the user works in, and laird/agents' own
//! test suite has written to it (laird/agents#282), so everything here keeps
//! to one repo id ([`super::gate::repo_id`]).
//!
//! The travel log (`travel.rs`, #539) reads it. An autocoder too old to write
//! the file, or an issue from before it, leaves nothing here, and the travel
//! log works the stages out from comments and git as before.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

/// The field-set version this reader knows.
const VERSION: u32 = 1;

/// One line of `issue-events.jsonl`. Fields the village doesn't use are left out.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct IssueEvent {
    #[serde(default)]
    pub v: u32,
    /// When (ISO-8601 UTC). A timeline is ordered by this, not by line.
    #[serde(default)]
    pub ts: String,
    #[serde(default)]
    pub repo: String,
    /// The issue, or `None` for the repo-wide `deployed` and `released-tag`.
    #[serde(default)]
    pub issue: Option<u32>,
    /// `claimed`, `released`, `first-commit`, `gate-queued`, `gate-start`,
    /// `gate-end`, `landed`, `escalated`, `decomposed`, `deployed`,
    /// `released-tag`, `ctl`.
    #[serde(default)]
    pub event: String,
    #[serde(default)]
    pub detail: Value,
}

impl IssueEvent {
    /// A string from `detail`.
    pub fn detail_str(&self, key: &str) -> Option<&str> {
        self.detail.get(key).and_then(Value::as_str)
    }
}

/// Where autocoder writes the file, from the environment (`env` looks a variable up).
pub fn path(env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let set = |k: &str| env(k).filter(|v| !v.is_empty());
    if let Some(p) = set("AUTOCODER_ISSUE_EVENTS") {
        return Some(PathBuf::from(p));
    }
    // XDG_STATE_HOME counts only when absolute, as autocoder reads it.
    let state = set("XDG_STATE_HOME").filter(|s| s.starts_with('/')).map(PathBuf::from).or_else(|| set("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(state.join("autocoder/issue-events.jsonl"))
}

/// This repo's version-1 lines, oldest first by `ts`. A line that is not
/// JSON (a write cut short) or of another version is skipped.
pub fn parse(text: &str, repo_id: &str) -> Vec<IssueEvent> {
    let mut out: Vec<IssueEvent> = text
        .lines()
        .filter(|l| l.contains(repo_id))
        .filter_map(|l| serde_json::from_str::<IssueEvent>(l).ok())
        .filter(|e| e.v == VERSION && e.repo == repo_id)
        .collect();
    out.sort_by_cached_key(|e| chrono::DateTime::parse_from_rfc3339(e.ts.trim()).ok());
    out
}

/// This repo's events on this host. `None` when the file is missing (an
/// autocoder older than laird/agents#190) or can't be read. Blocking.
pub fn read(repo_id: &str) -> Option<Vec<IssueEvent>> {
    read_file(&path(&|k| std::env::var(k).ok())?, repo_id)
}

/// [`read`], from a given file.
pub fn read_file(file: &Path, repo_id: &str) -> Option<Vec<IssueEvent>> {
    Some(parse(&std::fs::read_to_string(file).ok()?, repo_id))
}

/// [`read`] for a local checkout, its repo id worked out from git. Blocking.
pub fn read_local(repo: &Path) -> Vec<IssueEvent> {
    super::gate::local_repo_id(repo).and_then(|id| read(&id)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPO: &str = "agents-ui-33dc9185";

    /// Lines as autocoder 4.44 writes them, with another repo's, the test
    /// suite's (laird/agents#282), a future version's and a torn write mixed in.
    const LOG: &str = r#"{"v":1,"ts":"2026-11-21T14:23:57Z","repo":"agents-ui-33dc9185","issue":537,"worker":"w","branch":"feature/issue-537","event":"first-commit","detail":{"sha":"ee8e41a"}}
{"v":1,"ts":"2026-11-21T14:15:56Z","repo":"agents-ui-33dc9185","issue":537,"worker":"w","branch":"feature/issue-537","event":"claimed","detail":{}}
{"v":1,"ts":"2026-11-21T14:16:00Z","repo":"agents-5ea0ca80","issue":537,"worker":"w","branch":"feature/issue-537","event":"claimed","detail":{}}
{"v":1,"ts":"2026-11-17T08:06:02Z","repo":"worker-3dfbb44d","issue":18401,"worker":"worker","branch":"feature/code","event":"first-commit","detail":{"sha":"dbd2115"}}
{"v":2,"ts":"2026-11-21T14:20:00Z","repo":"agents-ui-33dc9185","issue":537,"event":"claimed","detail":{}}
{"v":1,"ts":"2026-11-21T15:00:00Z","repo":"agents-ui-33dc9185","issue":null,"event":"deployed","detail":{"environment":"nuc-1","sha":"abc","deployment":7}}
{"v":1,"ts":"2026-11-21T15:00:01Z","repo":"agents-ui-33dc91"#;

    #[test]
    fn only_this_repos_known_lines_in_time_order() {
        let events = parse(LOG, REPO);
        let got: Vec<_> = events.iter().map(|e| (e.event.as_str(), e.issue)).collect();
        assert_eq!(got, vec![("claimed", Some(537)), ("first-commit", Some(537)), ("deployed", None)]);
        assert_eq!(events[2].detail_str("environment"), Some("nuc-1"));
    }

    #[test]
    fn the_file_is_found_as_autocoder_writes_it() {
        let env = |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
        assert_eq!(path(&env(&[("AUTOCODER_ISSUE_EVENTS", "/x/e.jsonl"), ("HOME", "/h")])), Some(PathBuf::from("/x/e.jsonl")));
        assert_eq!(path(&env(&[("XDG_STATE_HOME", "/s"), ("HOME", "/h")])), Some(PathBuf::from("/s/autocoder/issue-events.jsonl")));
        assert_eq!(path(&env(&[("XDG_STATE_HOME", "rel"), ("HOME", "/h")])), Some(PathBuf::from("/h/.local/state/autocoder/issue-events.jsonl")));
        assert_eq!(path(&env(&[])), None);
    }

    /// An autocoder older than laird/agents#190 writes no file: nothing to read.
    #[test]
    fn a_missing_file_reads_as_none() {
        let dir = std::env::temp_dir().join(format!("agents-ui-issue-events-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("issue-events.jsonl");
        let _ = std::fs::remove_file(&file);
        assert_eq!(read_file(&file, REPO), None);
        std::fs::write(&file, LOG).unwrap();
        assert_eq!(read_file(&file, REPO).map(|e| e.len()), Some(3));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
