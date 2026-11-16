//! Token use, read from Claude Code's transcripts (#496, #514): the counting
//! house's numbers, each house's coins, and what a landed issue cost.
//!
//! Every assistant turn in `~/.claude/projects/<cwd>/*.jsonl` (and its
//! sessions' `subagents/*.jsonl`) carries `message.usage`. It is read here,
//! locally and read-only; nothing is sent anywhere. A village read through
//! `--server` has none (the transcripts are on the other host).
//!
//! - A streamed message repeats one `message.id`, with the same usage, on
//!   consecutive lines: each id is counted once.
//! - Each turn's `gitBranch` says what it was for: `feature/issue-N`,
//!   `enhancement/issue-N-auto` and `escalated/review-N` count toward issue N.
//! - Transcripts run to hundreds of MB, so each file is read once and then
//!   only what is appended ([`UsageCache`]), at most [`READ_BUDGET`] per
//!   refresh; until the history is read the village says it is catching up.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Bytes of transcript read per [`BUDGET_EVERY`], across every file, village
/// and request: the first pass over a long history is spread out. A single
/// line longer than this is skipped.
pub const READ_BUDGET: u64 = 32 * 1024 * 1024;

/// How often the read budget comes back.
pub const BUDGET_EVERY: Duration = Duration::from_secs(10);

/// Transcripts untouched for longer than this aren't read.
pub const KEEP: Duration = Duration::from_secs(30 * 24 * 3600);

/// Days in the village's `days`.
pub const DAYS: usize = 7;

/// Issues listed in the village's `issues`.
pub const ISSUES: usize = 50;

/// Tokens, by kind. `total` is all four: what the model processed.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_write: u64,
    pub cache_read: u64,
    pub total: u64,
}

impl Tokens {
    fn from_usage(u: &serde_json::Value) -> Option<Self> {
        let n = |k: &str| u.get(k).and_then(|v| v.as_u64()).unwrap_or(0);
        let mut t = Tokens { input: n("input_tokens"), output: n("output_tokens"), cache_write: n("cache_creation_input_tokens"), cache_read: n("cache_read_input_tokens"), total: 0 };
        t.total = t.input + t.output + t.cache_write + t.cache_read;
        (t.total > 0).then_some(t)
    }

    fn add(&mut self, o: &Tokens) {
        self.input += o.input;
        self.output += o.output;
        self.cache_write += o.cache_write;
        self.cache_read += o.cache_read;
        self.total += o.total;
    }
}

/// A house's tokens: today, and on the issue it is working (all of it).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HouseTokens {
    pub today: Tokens,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<Tokens>,
}

/// One day's tokens, the day in local time (`YYYY-MM-DD`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DayTokens {
    pub date: String,
    pub tokens: Tokens,
}

/// One issue's tokens: all of it in transcripts touched within [`KEEP`], and
/// this week's (the last [`DAYS`] days).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IssueTokens {
    pub issue: u32,
    pub total: Tokens,
    pub week: Tokens,
}

/// The village's tokens: the counting house.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Usage {
    pub today: Tokens,
    /// The last [`DAYS`] days, oldest first, today last.
    pub days: Vec<DayTokens>,
    /// Issues worked this week, the costliest this week first.
    pub issues: Vec<IssueTokens>,
    /// Some of the history is still to be read: the numbers will grow.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub catching_up: bool,
}

/// The issue a branch is for: `feature/issue-N`, `enhancement/issue-N-auto`,
/// `escalated/review-N`.
pub fn branch_issue(branch: &str) -> Option<u32> {
    let rest = ["feature/issue-", "enhancement/issue-", "escalated/review-"].iter().find_map(|p| branch.strip_prefix(p))?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    let after = &rest[digits.len()..];
    if digits.is_empty() || !(after.is_empty() || after.starts_with('-')) {
        return None;
    }
    digits.parse().ok()
}

/// (day, issue) → tokens.
type Tally = HashMap<(String, Option<u32>), Tokens>;

/// What one transcript has said so far.
#[derive(Default)]
struct FileTally {
    /// Bytes read: up to the end of the last whole line.
    offset: u64,
    /// The last message counted, so its repeats on later lines aren't.
    last_id: Option<String>,
    tally: Tally,
}

impl FileTally {
    /// Count whole lines of transcript. `day` names the day of a moment.
    fn count(&mut self, text: &str, day: &dyn Fn(DateTime<Utc>) -> String) {
        for line in text.lines() {
            // Most lines are tool output; only an assistant turn has usage.
            if !line.contains("\"usage\"") || !line.contains("\"assistant\"") {
                continue;
            }
            let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else { continue };
            if entry.get("type").and_then(|t| t.as_str()) != Some("assistant") {
                continue;
            }
            let Some(message) = entry.get("message") else { continue };
            let id = message.get("id").and_then(|i| i.as_str()).map(str::to_string);
            if id.is_some() && id == self.last_id {
                continue;
            }
            let Some(tokens) = message.get("usage").and_then(Tokens::from_usage) else { continue };
            let Some(at) = entry.get("timestamp").and_then(|t| t.as_str()).and_then(|t| DateTime::parse_from_rfc3339(t).ok()) else { continue };
            let issue = entry.get("gitBranch").and_then(|b| b.as_str()).and_then(branch_issue);
            self.tally.entry((day(at.with_timezone(&Utc)), issue)).or_default().add(&tokens);
            self.last_id = id;
        }
    }

    /// Read what was appended to `path` since last time, at most `budget`
    /// bytes. A file that shrank was rewritten: it is counted afresh.
    /// Returns the bytes read and whether any are left.
    ///
    /// The budget can be what another file left, less than a line: then
    /// nothing is read (the line waits for a fuller budget). Only a line
    /// longer than `longest` with all of it to spend is skipped.
    fn update(&mut self, path: &Path, budget: u64, longest: u64, day: &dyn Fn(DateTime<Utc>) -> String) -> (u64, bool) {
        let Ok(mut file) = std::fs::File::open(path) else { return (0, false) };
        let Ok(len) = file.metadata().map(|m| m.len()) else { return (0, false) };
        if len < self.offset {
            *self = FileTally::default();
        }
        if len == self.offset || budget == 0 {
            return (0, len > self.offset);
        }
        let want = (len - self.offset).min(budget);
        let mut buf = Vec::with_capacity(want as usize);
        if file.seek(SeekFrom::Start(self.offset)).is_err() || file.take(want).read_to_end(&mut buf).is_err() {
            return (0, false);
        }
        // Only whole lines: the rest is read again once it ends.
        let Some(end) = buf.iter().rposition(|b| *b == b'\n').map(|i| i + 1) else {
            // One line longer than the whole budget: skip it rather than stick on it.
            if want == budget && budget >= longest {
                self.offset += want;
                return (want, self.offset < len);
            }
            return (0, want == budget);
        };
        self.count(&String::from_utf8_lossy(&buf[..end]), day);
        self.offset += end as u64;
        (end as u64, self.offset < len)
    }
}

/// Every transcript read so far, kept between refreshes, and the read
/// budget spent since it last came back.
#[derive(Default)]
pub struct UsageCache {
    files: HashMap<PathBuf, FileTally>,
    budget_since: Option<std::time::Instant>,
    spent: u64,
}

/// The transcripts of one Claude project dir: its sessions, and their
/// sub-agents, touched within [`KEEP`].
fn transcripts(project_dir: &Path, now: SystemTime) -> Vec<PathBuf> {
    let recent = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok().is_some_and(|t| now.duration_since(t).map(|a| a <= KEEP).unwrap_or(true));
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(project_dir) else { return found };
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "jsonl") && recent(&p) {
            found.push(p);
        } else if p.is_dir()
            && let Ok(subs) = std::fs::read_dir(p.join("subagents"))
        {
            found.extend(subs.flatten().map(|s| s.path()).filter(|s| s.extension().is_some_and(|x| x == "jsonl") && recent(s)));
        }
    }
    found.sort();
    found
}

impl UsageCache {
    /// What is left of the read budget at `at`: it comes back in full every
    /// [`BUDGET_EVERY`], however many villages and requests ask.
    fn allowance(&mut self, at: std::time::Instant) -> u64 {
        if self.budget_since.is_none_or(|t| at.saturating_duration_since(t) >= BUDGET_EVERY) {
            self.budget_since = Some(at);
            self.spent = 0;
        }
        READ_BUDGET - self.spent
    }

    /// Bring every transcript under `dirs` up to date (within the budget),
    /// then sum them: per dir, and over all of them. Returns the per-dir
    /// tallies, the whole, and whether anything was left unread.
    fn refresh(&mut self, dirs: &[PathBuf], now: SystemTime, budget: u64, day: &dyn Fn(DateTime<Utc>) -> String) -> (HashMap<PathBuf, Tally>, Tally, bool) {
        let mut left = budget;
        let mut behind = false;
        let mut per_dir: HashMap<PathBuf, Tally> = HashMap::new();
        let mut all = Tally::new();
        for dir in dirs {
            if per_dir.contains_key(dir) {
                continue;
            }
            let mut sum = Tally::new();
            for path in transcripts(dir, now) {
                let file = self.files.entry(path.clone()).or_default();
                let (read, more) = file.update(&path, left, READ_BUDGET, day);
                left -= read;
                behind |= more;
                for (k, t) in &file.tally {
                    sum.entry(k.clone()).or_default().add(t);
                    all.entry(k.clone()).or_default().add(t);
                }
            }
            per_dir.insert(dir.clone(), sum);
        }
        self.spent += budget - left;
        (per_dir, all, behind)
    }
}

fn sum_where(tally: &Tally, keep: impl Fn(&str, Option<u32>) -> bool) -> Tokens {
    let mut t = Tokens::default();
    tally.iter().filter(|((d, i), _)| keep(d, *i)).for_each(|(_, v)| t.add(v));
    t
}

/// The last [`DAYS`] days, oldest first, ending on `today`.
fn last_days(today: &str) -> Vec<String> {
    let Ok(t) = chrono::NaiveDate::parse_from_str(today, "%Y-%m-%d") else { return vec![today.to_string()] };
    (0..DAYS as i64).rev().map(|k| (t - chrono::Duration::days(k)).to_string()).collect()
}

/// The village's usage from the whole tally.
fn village_usage(all: &Tally, today: &str, catching_up: bool) -> Usage {
    let days = last_days(today);
    let mut issues: HashMap<u32, IssueTokens> = HashMap::new();
    for ((d, i), t) in all {
        let Some(n) = i else { continue };
        let e = issues.entry(*n).or_insert(IssueTokens { issue: *n, total: Tokens::default(), week: Tokens::default() });
        e.total.add(t);
        if days.contains(d) {
            e.week.add(t);
        }
    }
    let mut issues: Vec<IssueTokens> = issues.into_values().filter(|i| i.week.total > 0).collect();
    issues.sort_by(|a, b| b.week.total.cmp(&a.week.total).then(a.issue.cmp(&b.issue)));
    issues.truncate(ISSUES);
    Usage {
        today: sum_where(all, |d, _| d == today),
        days: days.iter().map(|d| DayTokens { date: d.clone(), tokens: sum_where(all, |x, _| x == d) }).collect(),
        issues,
        catching_up,
    }
}

/// Attach token use to a village: each house with a worktree gets its
/// tokens today and on its issue, the village its usage. The manager's
/// transcripts count in the village's usage; it has no house to carry its
/// own. `projects` is `~/.claude/projects`.
pub fn attach(cache: &mut UsageCache, village: &mut super::village::VillageSnapshot, projects: &Path, now: SystemTime, day: &dyn Fn(DateTime<Utc>) -> String) {
    let dir = |cwd: &str| projects.join(super::village::claude_project_dir_name(cwd));
    let mut dirs: Vec<PathBuf> = village.houses.iter().filter(|h| h.id.starts_with('/')).map(|h| dir(&h.id)).collect();
    if !village.repo_path.is_empty() {
        dirs.push(dir(&village.repo_path));
    }
    if dirs.is_empty() {
        return;
    }
    let budget = cache.allowance(std::time::Instant::now());
    let (per_dir, all, behind) = cache.refresh(&dirs, now, budget, day);
    let today = day(DateTime::<Utc>::from(now));
    for house in village.houses.iter_mut().filter(|h| h.id.starts_with('/')) {
        let Some(mine) = per_dir.get(&dir(&house.id)) else { continue };
        house.tokens = Some(HouseTokens {
            today: sum_where(mine, |d, _| d == today),
            issue: house.issue.map(|n| sum_where(&all, |_, i| i == Some(n))),
        });
    }
    village.usage = Some(village_usage(&all, &today, behind));
}

/// The day of a moment in this machine's local time.
pub fn local_day(at: DateTime<Utc>) -> String {
    at.with_timezone(&chrono::Local).date_naive().to_string()
}

/// The server's one cache.
pub fn shared() -> &'static std::sync::Mutex<UsageCache> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<UsageCache>> = std::sync::OnceLock::new();
    CACHE.get_or_init(Default::default)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc_day(at: DateTime<Utc>) -> String {
        at.date_naive().to_string()
    }

    fn turn(id: &str, at: &str, branch: &str, input: u64, output: u64, cw: u64, cr: u64) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"{at}","gitBranch":"{branch}","message":{{"id":"{id}","usage":{{"input_tokens":{input},"output_tokens":{output},"cache_creation_input_tokens":{cw},"cache_read_input_tokens":{cr}}}}}}}"#
        )
    }

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("agents-ui-usage-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn branches_name_their_issue() {
        assert_eq!(branch_issue("feature/issue-514"), Some(514));
        assert_eq!(branch_issue("enhancement/issue-12-auto"), Some(12));
        assert_eq!(branch_issue("escalated/review-479"), Some(479));
        assert_eq!(branch_issue("master-wt-1"), None);
        assert_eq!(branch_issue("feature/issue-"), None);
        assert_eq!(branch_issue("feature/issue-12x"), None);
        assert_eq!(branch_issue("feature/issue-99999999999"), None, "too big for a u32");
    }

    #[test]
    fn a_message_is_counted_once_and_kept_by_day_and_issue() {
        let mut f = FileTally::default();
        let text = [
            turn("m1", "2026-11-15T23:00:00Z", "feature/issue-7", 1, 2, 3, 4),
            turn("m1", "2026-11-15T23:00:00Z", "feature/issue-7", 1, 2, 3, 4),
            r#"{"type":"user","message":{"content":"usage"}}"#.to_string(),
            turn("m2", "2026-11-16T01:00:00Z", "feature/issue-7", 10, 0, 0, 0),
            turn("m3", "2026-11-16T02:00:00Z", "master-wt-1", 0, 5, 0, 0),
            "not json \"usage\" \"assistant\"".to_string(),
        ]
        .join("\n");
        f.count(&text, &utc_day);
        assert_eq!(f.tally[&("2026-11-15".into(), Some(7))], Tokens { input: 1, output: 2, cache_write: 3, cache_read: 4, total: 10 });
        assert_eq!(f.tally[&("2026-11-16".into(), Some(7))].total, 10);
        assert_eq!(f.tally[&("2026-11-16".into(), None)].output, 5);
        assert_eq!(f.tally.len(), 3);
    }

    #[test]
    fn appended_lines_are_read_and_nothing_twice() {
        let d = temp("append");
        let path = d.join("s.jsonl");
        std::fs::write(&path, format!("{}\n{}", turn("a", "2026-11-16T01:00:00Z", "x", 1, 0, 0, 0), &turn("b", "2026-11-16T01:00:00Z", "x", 2, 0, 0, 0)[..20])).unwrap();
        let mut f = FileTally::default();
        f.update(&path, u64::MAX, u64::MAX, &utc_day);
        assert_eq!(f.tally[&("2026-11-16".into(), None)].input, 1, "a half-written line waits");
        // Nothing new: nothing read.
        assert_eq!(f.update(&path, u64::MAX, u64::MAX, &utc_day), (0, false));
        // The line ends, and the same message repeats on the next.
        let whole = format!("{}\n{}\n{}\n", turn("a", "2026-11-16T01:00:00Z", "x", 1, 0, 0, 0), turn("b", "2026-11-16T01:00:00Z", "x", 2, 0, 0, 0), turn("b", "2026-11-16T01:00:00Z", "x", 2, 0, 0, 0));
        std::fs::write(&path, &whole).unwrap();
        f.update(&path, u64::MAX, u64::MAX, &utc_day);
        assert_eq!(f.tally[&("2026-11-16".into(), None)].input, 3);
        // Rewritten shorter: counted afresh.
        std::fs::write(&path, format!("{}\n", turn("c", "2026-11-16T01:00:00Z", "x", 5, 0, 0, 0))).unwrap();
        f.update(&path, u64::MAX, u64::MAX, &utc_day);
        assert_eq!(f.tally[&("2026-11-16".into(), None)].input, 5);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_long_history_is_read_over_several_refreshes() {
        let d = temp("budget");
        let path = d.join("s.jsonl");
        let lines: Vec<String> = (0..10).map(|k| turn(&format!("m{k}"), "2026-11-16T01:00:00Z", "x", 1, 0, 0, 0)).collect();
        std::fs::write(&path, lines.join("\n") + "\n").unwrap();
        let line = lines[0].len() as u64 + 1;
        let mut f = FileTally::default();
        let (read, more) = f.update(&path, line * 4 + 3, line * 4 + 3, &utc_day);
        assert_eq!((read, more), (line * 4, true));
        assert_eq!(f.tally[&("2026-11-16".into(), None)].input, 4);
        while f.update(&path, line * 4 + 3, line * 4 + 3, &utc_day).1 {}
        assert_eq!(f.tally[&("2026-11-16".into(), None)].input, 10);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_budget_left_short_by_another_file_reads_nothing_and_loses_nothing() {
        let d = temp("short");
        let path = d.join("s.jsonl");
        let lines: Vec<String> = (0..3).map(|k| turn(&format!("m{k}"), "2026-11-16T01:00:00Z", "x", 1, 0, 0, 0)).collect();
        std::fs::write(&path, lines.join("\n") + "\n").unwrap();
        let mut f = FileTally::default();
        // Ten bytes left over from another file: less than a line.
        assert_eq!(f.update(&path, 10, READ_BUDGET, &utc_day), (0, true));
        assert_eq!(f.offset, 0, "the line waits, whole");
        f.update(&path, READ_BUDGET, READ_BUDGET, &utc_day);
        assert_eq!(f.tally[&("2026-11-16".into(), None)].input, 3);
        // A line longer than the whole budget is skipped, and the next one read.
        let budget = lines[0].len() as u64 + 10;
        let long = format!("{}\n{}\n", "x".repeat(3 * budget as usize), lines[0]);
        std::fs::write(&path, &long).unwrap();
        let mut f = FileTally::default();
        assert_eq!(f.update(&path, budget, budget, &utc_day), (budget, true));
        while f.update(&path, budget, budget, &utc_day).1 {}
        assert_eq!(f.tally[&("2026-11-16".into(), None)].input, 1);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn the_budget_comes_back_every_so_often_however_many_ask() {
        let mut c = UsageCache::default();
        let t0 = std::time::Instant::now();
        assert_eq!(c.allowance(t0), READ_BUDGET);
        c.spent = READ_BUDGET - 5;
        assert_eq!(c.allowance(t0 + Duration::from_secs(1)), 5);
        assert_eq!(c.allowance(t0 + BUDGET_EVERY), READ_BUDGET);
    }

    #[test]
    fn a_village_gets_house_and_counting_house_numbers() {
        use super::super::village::{build, Pipeline};
        use super::super::{AgentSnapshot, SwarmSnapshot};
        let home = temp("village");
        let projects = home.join("projects");
        let wt = |n: &str| projects.join(super::super::village::claude_project_dir_name(n));
        std::fs::create_dir_all(wt("/wt1").join("s1/subagents")).unwrap();
        std::fs::create_dir_all(wt("/wt2")).unwrap();
        std::fs::write(wt("/wt1").join("s1.jsonl"), [turn("a", "2026-11-16T01:00:00Z", "feature/issue-7", 100, 0, 0, 0), turn("b", "2026-11-10T01:00:00Z", "feature/issue-7", 1000, 0, 0, 0)].join("\n") + "\n").unwrap();
        std::fs::write(wt("/wt1").join("s1/subagents/agent-x.jsonl"), turn("c", "2026-11-16T02:00:00Z", "feature/issue-7", 0, 10, 0, 0) + "\n").unwrap();
        std::fs::write(wt("/wt2").join("s.jsonl"), [turn("d", "2026-11-16T03:00:00Z", "feature/issue-7", 1, 0, 0, 0), turn("e", "2026-11-15T03:00:00Z", "feature/issue-8", 50, 0, 0, 0)].join("\n") + "\n").unwrap();

        let agent = |role: &str, path: &str, issue: Option<u32>| AgentSnapshot {
            id: format!("shop/{role}"),
            role: role.into(),
            state: "Working".into(),
            is_manager: role == "manager",
            waiting_for_input: false,
            current_issue: issue,
            current_issue_title: None,
            pane_content: String::new(),
            tmux_target: String::new(),
            health: "Healthy".into(),
            completed_issue_count: 0,
            resurrection_attempts: 0,
            status_timestamp: None,
            worktree_path: path.into(),
            branch: None,
            mux: Default::default(),
            context_pct: None,
            context_window: None,
            limit: None,
        };
        let swarm = SwarmSnapshot {
            project_name: "shop".into(),
            repo_path: "/repos/shop".into(),
            repo_key: String::new(),
            manager_by: None,
            agent_type: "Claude".into(),
            workflow: None,
            tmux_session: "claude-shop".into(),
            stopped: false,
            busy_count: 0,
            idle_count: 0,
            attention_count: 0,
            manager: agent("manager", "/repos/shop", None),
            workers: vec![agent("worker-1", "/wt1", Some(7)), agent("worker-2", "/wt2", None)],
            issues: Vec::new(),
        };
        let mut v = build(&swarm, &[], Pipeline::default());
        let now = SystemTime::from(DateTime::parse_from_rfc3339("2026-11-16T12:00:00Z").unwrap().with_timezone(&Utc));
        // The files' mtimes are later than this fixed now: they count as recent.
        let mut cache = UsageCache::default();
        attach(&mut cache, &mut v, &projects, now, &utc_day);

        let w1 = v.houses.iter().find(|h| h.id == "/wt1").unwrap().tokens.clone().unwrap();
        assert_eq!(w1.today.total, 110, "its session and its sub-agent, today only");
        assert_eq!(w1.issue.unwrap().total, 1111, "issue 7 everywhere, every day");
        let w2 = v.houses.iter().find(|h| h.id == "/wt2").unwrap().tokens.clone().unwrap();
        assert_eq!((w2.today.total, w2.issue), (1, None));

        let u = v.usage.unwrap();
        assert_eq!(u.today.total, 111);
        assert_eq!(u.days.len(), DAYS);
        assert_eq!(u.days.last().unwrap().date, "2026-11-16");
        assert_eq!(u.days[0].date, "2026-11-10");
        assert_eq!(u.days[0].tokens.total, 1000);
        assert_eq!(u.issues.iter().map(|i| (i.issue, i.week.total)).collect::<Vec<_>>(), vec![(7, 1111), (8, 50)]);
        assert!(!u.catching_up);
        std::fs::remove_dir_all(&home).ok();
    }
}
