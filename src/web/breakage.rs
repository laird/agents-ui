//! Breakage travelling upstream (#529, phase 4 of #494): a consumer in a
//! valley's group whose integration branch is red, and whose failure names a
//! library it depends on, sends a **barge** back up the river to it.
//! `/api/valley` carries them as each group's `barges`.
//!
//! The consumer's failure names a library when:
//!
//! - the log of one of its failing CI jobs mentions one of the library's
//!   package names (`pins::package_names`: its crate or npm name), as a whole
//!   word, in either spelling of a Rust crate (`lib-core`, `lib_core`); or
//! - one of its open issues names the library's repo (`owner/repo#N`, or a
//!   link into it).
//!
//! The build and its failing jobs are what the consumer's village already
//! read (`build.rs`, cached there). The failing jobs' logs are read here with
//! `gh`, once per job (a finished job's log doesn't change), in the
//! background: a poll never waits on GitHub. A consumer with no village, or
//! whose CI can't be read, sends nothing.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;

use super::crossrepo::CrossIssue;
use crate::transport::ServerTransport;

/// A red consumer's failure, sent back to a library it depends on.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Barge {
    /// The consumer, where the failure is.
    pub from: String,
    /// The library its failure names.
    pub to: String,
    /// Why it names it: "the log of <job> names <package>", or
    /// "issue #N names <owner/repo>".
    pub why: String,
    /// The failing CI run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
}

/// A library a consumer depends on: its name in the group, its `owner/repo`
/// (lowercased, when known) and its package names.
#[derive(Debug, Clone, Default)]
pub struct Library {
    pub name: String,
    pub slug: Option<String>,
    pub packages: Vec<String>,
}

/// Does `text` mention `word` as a whole word? `-` and `_` count as part of
/// a word, so `lib-core` is not found in `lib-core-extra`.
fn mentions(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let part = |c: char| c.is_alphanumeric() || c == '-' || c == '_';
    text.match_indices(word).any(|(k, _)| {
        let before = text[..k].chars().next_back();
        let after = text[k + word.len()..].chars().next();
        !before.is_some_and(part) && !after.is_some_and(part)
    })
}

/// The barges a red consumer sends: `logs` are its failing jobs' (name, log),
/// `issues` what was read of its issues, `libs` what it depends on in the
/// group. One barge per library, for the first reason found: a log first.
pub fn barges(consumer: &str, run: Option<&str>, logs: &[(String, String)], issues: &[CrossIssue], libs: &[Library]) -> Vec<Barge> {
    let mut out = Vec::new();
    for lib in libs.iter().filter(|l| l.name != consumer) {
        let mut names: Vec<String> = Vec::new();
        for p in &lib.packages {
            for n in [p.clone(), p.replace('-', "_"), p.replace('_', "-")] {
                if !n.is_empty() && !names.contains(&n) {
                    names.push(n);
                }
            }
        }
        let in_log = logs.iter().find_map(|(job, log)| names.iter().find(|n| mentions(log, n)).map(|n| format!("the log of {job} names {n}")));
        let in_issue = || {
            let slug = lib.slug.as_deref()?;
            issues.iter().filter(|i| i.state.eq_ignore_ascii_case("open")).find_map(|i| {
                let text = format!("{}\n{}", i.title, i.body.as_deref().unwrap_or(""));
                let lower = text.to_ascii_lowercase();
                let named = text.lines().any(|l| super::crossrepo::refs(l, "").iter().any(|(s, _)| s == slug))
                    || lower.contains(&format!("github.com/{slug}"));
                named.then(|| format!("issue #{} names {slug}", i.number))
            })
        };
        if let Some(why) = in_log.or_else(in_issue) {
            out.push(Barge { from: consumer.to_string(), to: lib.name.clone(), why, run: run.map(str::to_string) });
        }
    }
    out
}

/// A consumer's failing run and its failing jobs `(name, id)`, from the build
/// its village read; None unless the build is red.
pub fn failing(build: Option<&super::build::Build>) -> Option<(Option<String>, Vec<(String, u64)>)> {
    let b = build.filter(|b| b.state == super::build::BuildState::Red)?;
    let jobs = b.jobs.iter().filter(|j| j.state == super::build::BuildState::Red).filter_map(|j| Some((j.name.clone(), job_id(j.url.as_deref()?)?))).collect();
    Some((b.url.clone(), jobs))
}

/// The most of a job's log kept: its end, where a failure is reported.
const LOG_KEEP: usize = 512 * 1024;

type Logs = Mutex<HashMap<(PathBuf, u64), String>>;
static LOGS: OnceLock<Logs> = OnceLock::new();
static READING: OnceLock<Mutex<HashSet<(PathBuf, u64)>>> = OnceLock::new();

/// A job's id, from its URL (`…/actions/runs/R/job/J`).
pub fn job_id(url: &str) -> Option<u64> {
    url.rsplit("/job/").next().filter(|_| url.contains("/job/"))?.trim_end_matches('/').parse().ok()
}

/// The logs of the jobs (`(name, id)`) of the repo at `dir`, as far as they
/// have been read: a job's log is read once, in the background, and the
/// poll after has it. A log that can't be read is kept as empty.
pub fn job_logs(transport: &ServerTransport, dir: &Path, jobs: &[(String, u64)]) -> Vec<(String, String)> {
    let logs = LOGS.get_or_init(Default::default);
    let mut out = Vec::new();
    for (name, id) in jobs {
        let key = (dir.to_path_buf(), *id);
        if let Some(log) = logs.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
            out.push((name.clone(), log.clone()));
            continue;
        }
        if tokio::runtime::Handle::try_current().is_err() || !READING.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner()).insert(key.clone()) {
            continue;
        }
        let transport = transport.clone();
        tokio::spawn(async move {
            let args = vec!["api".to_string(), format!("repos/{{owner}}/{{repo}}/actions/jobs/{}/logs", key.1)];
            let log = match transport.output("gh", &args, Some(&key.0)).await {
                Ok(o) if o.status.success() => {
                    let text = String::from_utf8_lossy(&o.stdout).into_owned();
                    let mut cut = text.len().saturating_sub(LOG_KEEP);
                    while !text.is_char_boundary(cut) {
                        cut += 1;
                    }
                    text[cut..].to_string()
                }
                _ => String::new(),
            };
            let logs = LOGS.get_or_init(Default::default);
            let mut held = logs.lock().unwrap_or_else(|e| e.into_inner());
            // Old runs' logs are not needed again: keep the map small.
            if held.len() > 64 {
                held.clear();
            }
            held.insert(key.clone(), log);
            drop(held);
            READING.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner()).remove(&key);
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib(name: &str, slug: &str, packages: &[&str]) -> Library {
        Library { name: name.into(), slug: Some(slug.into()), packages: packages.iter().map(|s| s.to_string()).collect() }
    }
    fn issue(number: u32, state: &str, body: &str) -> CrossIssue {
        CrossIssue { number, title: "t".into(), state: state.into(), state_reason: None, body: Some(body.into()) }
    }
    const RUN: Option<&str> = Some("https://github.com/acme/api/actions/runs/9");

    #[test]
    fn a_failing_log_naming_the_library_sends_a_barge_with_the_run() {
        let logs = vec![("test (ubuntu)".to_string(), "error[E0432]: unresolved import `lib_core::Money`\n  --> src/charge.rs:3".to_string())];
        let b = barges("api", RUN, &logs, &[], &[lib("lib-core", "acme/lib-core", &["lib-core"]), lib("lib-ui", "acme/lib-ui", &["lib-ui"])]);
        assert_eq!(b, vec![Barge { from: "api".into(), to: "lib-core".into(), why: "the log of test (ubuntu) names lib_core".into(), run: RUN.map(Into::into) }]);
    }

    #[test]
    fn a_log_not_naming_it_sends_nothing() {
        let logs = vec![("test".to_string(), "thread 'main' panicked: lib-core-extra exploded; see mylib-core".to_string())];
        assert!(barges("api", RUN, &logs, &[], &[lib("lib-core", "acme/lib-core", &["lib-core"])]).is_empty());
        assert!(barges("api", RUN, &[], &[], &[lib("lib-core", "acme/lib-core", &["lib-core"])]).is_empty(), "no logs read yet");
    }

    #[test]
    fn an_open_issue_naming_the_library_repo_sends_a_barge() {
        let issues = vec![
            issue(7, "CLOSED", "Broken by acme/lib-core#12"),
            issue(8, "OPEN", "CI red since https://github.com/Acme/Lib-Core/pull/3 landed"),
        ];
        let b = barges("api", RUN, &[], &issues, &[lib("lib-core", "acme/lib-core", &[])]);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].why, "issue #8 names acme/lib-core");
        let issues = vec![issue(9, "OPEN", "Waits on acme/lib-core#12")];
        assert_eq!(barges("api", RUN, &[], &issues, &[lib("lib-core", "acme/lib-core", &[])])[0].why, "issue #9 names acme/lib-core");
    }

    #[test]
    fn only_a_red_build_fails() {
        use crate::web::build::{Build, BuildJob, BuildState};
        let job = |state, id: u32| BuildJob { name: format!("j{id}"), state, url: Some(format!("https://github.com/acme/api/actions/runs/9/job/{id}")), ..Default::default() };
        let red = Build { state: BuildState::Red, url: RUN.map(Into::into), jobs: vec![job(BuildState::Red, 1), job(BuildState::Running, 2)], ..Default::default() };
        assert_eq!(failing(Some(&red)), Some((RUN.map(Into::into), vec![("j1".to_string(), 1)])));
        let green = Build { state: BuildState::Green, ..Default::default() };
        assert_eq!(failing(Some(&green)), None, "a passing run sends nothing");
        assert_eq!(failing(None), None, "no build read");
    }

    #[test]
    fn job_ids_come_from_job_urls() {
        assert_eq!(job_id("https://github.com/acme/api/actions/runs/9/job/123"), Some(123));
        assert_eq!(job_id("https://github.com/acme/api/actions/runs/9"), None);
    }
}
