//! Returned goods (#497): landed work that came back, read from what the
//! GitHub summary (`github.rs`) already fetches, so it costs no extra call.
//!
//! An issue came back when, inside the summary's window:
//!
//! - it is open again with GitHub's `REOPENED` state reason (after landing);
//! - a `Revert "…#N…"` commit on the integration branch undid its landing,
//!   and nothing after the revert landed #N again;
//! - an open `bug` issue says "regression" and names #N, an issue that landed
//!   in the window.
//!
//! It stays returned until it is claimed again: a reopened issue until it is
//! labelled `working` (or is on its way to the gate), a revert until #N lands
//! again, a regression until its report is closed. Over the window, the share
//! of landed issues that came back is a quality figure for the swarm.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, FixedOffset, NaiveDate};
use serde::{Deserialize, Serialize};

/// Labels that say an issue has been claimed again.
const CLAIMED: [&str; 3] = ["working", "merging", "awaiting-integration"];

/// One issue that came back.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Returned {
    /// The issue that landed and came back.
    pub number: u32,
    pub title: String,
    /// `reopened`, `reverted` or `regression`.
    pub how: String,
    /// When it came back (RFC 3339): the revert's commit, the report's
    /// opening; absent for a reopening, whose time `gh issue list` doesn't give.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// The regression's report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<u32>,
    /// The revert's subject.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revert: Option<String>,
}

/// What came back, and the share of the window's landings it is.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Goods {
    pub items: Vec<Returned>,
    /// Issues that landed in the window, those reopened since included: what
    /// the share is out of.
    pub landed: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Issue {
    #[serde(default)]
    number: u32,
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    state: String,
    #[serde(default, rename = "stateReason")]
    state_reason: Option<String>,
    #[serde(default, rename = "createdAt")]
    created_at: String,
    #[serde(default, rename = "closedAt")]
    closed_at: Option<String>,
    #[serde(default)]
    labels: Vec<Label>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Label {
    #[serde(default)]
    name: String,
}

fn reason(i: &Issue) -> String {
    i.state_reason.as_deref().unwrap_or("").to_ascii_uppercase()
}
fn has_label(i: &Issue, name: &str) -> bool {
    i.labels.iter().any(|l| l.name.eq_ignore_ascii_case(name))
}
fn in_window(at: &str, first: NaiveDate, offset: &FixedOffset) -> bool {
    DateTime::parse_from_rfc3339(at.trim()).map(|t| t.with_timezone(offset).date_naive() >= first).unwrap_or(false)
}

/// Every `#N` in a text, in order.
pub fn issue_refs(text: &str) -> Vec<u32> {
    refs_with_lead(text).into_iter().map(|(n, _)| n).collect()
}

/// Every `#N` in a text with the few words of its clause just before it, lowercased.
fn refs_with_lead(text: &str) -> Vec<(u32, String)> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    for (k, _) in text.match_indices('#') {
        // `#N` on its own: not inside a word (`a#3`) or a URL fragment.
        if k > 0 && (b[k - 1].is_ascii_alphanumeric() || b[k - 1] == b'/') {
            continue;
        }
        let digits: String = text[k + 1..].chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = digits.parse() {
            // Only this clause: back to the last `#`, punctuation or line break.
            let start = text[..k].rfind(|c: char| matches!(c, '#' | '.' | ';' | ',' | '\n' | '(')).map_or(0, |i| i + 1);
            let before = &text[start..k];
            let words: Vec<&str> = before.split_whitespace().rev().take(4).collect();
            out.push((n, words.into_iter().rev().collect::<Vec<_>>().join(" ").to_lowercase()));
        }
    }
    out
}

/// The issues a commit subject lands: `Implement #N`, `Fix #N`, `Closes #N`
/// and the like. A `Review of #N` or a mention lands nothing.
pub fn landing_refs(subject: &str) -> Vec<u32> {
    const VERBS: [&str; 9] = ["implement", "fix", "fixes", "fixed", "close", "closes", "closed", "resolve", "resolves"];
    refs_with_lead(subject)
        .into_iter()
        .filter(|(_, lead)| lead.split_whitespace().last().is_some_and(|w| VERBS.contains(&w.trim_matches(|c: char| !c.is_alphanumeric()))))
        .map(|(n, _)| n)
        .collect()
}

/// The issues a report names as what regressed: `#N` with "regression" or
/// "regressed" just before it ("a regression from #N", "regressed in #N"),
/// not "a regression test for #N".
pub fn regression_refs(text: &str) -> Vec<u32> {
    refs_with_lead(text).into_iter().filter(|(_, lead)| lead.contains("regress") && !lead.contains("test")).map(|(n, _)| n).collect()
}

/// What came back, from `issues` (`gh issue list --json number,title,body,
/// state,stateReason,createdAt,closedAt,labels`) and `log` (the integration
/// branch's `git log --format=%cI%x09%s`, newest first), over the window from
/// `first`. None when the issues can't be read.
pub fn find(issues: &[u8], log: Option<&str>, first: NaiveDate, offset: FixedOffset) -> Option<Goods> {
    let issues: Vec<Issue> = serde_json::from_slice(issues).ok()?;
    let by_number: BTreeMap<u32, &Issue> = issues.iter().map(|i| (i.number, i)).collect();
    let title = |n: u32| by_number.get(&n).map(|i| i.title.clone()).unwrap_or_default();
    // Landed in the window: closed as completed, or reopened since (its
    // landing is what came back). A reopened issue's close isn't given, so
    // it counts when it was touched in the window, which the read already is.
    let landed: BTreeSet<u32> = issues
        .iter()
        .filter(|i| {
            let closed = i.state.eq_ignore_ascii_case("closed") && !matches!(reason(i).as_str(), "NOT_PLANNED" | "DUPLICATE");
            (closed && i.closed_at.as_deref().is_some_and(|at| in_window(at, first, &offset))) || reason(i) == "REOPENED"
        })
        .map(|i| i.number)
        .collect();
    let mut items: Vec<Returned> = Vec::new();
    let mut seen = BTreeSet::new();
    for i in &issues {
        let open = i.state.eq_ignore_ascii_case("open");
        if open && reason(i) == "REOPENED" && !CLAIMED.iter().any(|l| has_label(i, l)) && seen.insert(i.number) {
            items.push(Returned { number: i.number, title: i.title.clone(), how: "reopened".into(), ..Returned::default() });
        }
    }
    if let Some(log) = log {
        // Newest first: what landed again after a revert is seen before it.
        let mut relanded = BTreeSet::new();
        for line in log.lines() {
            let Some((at, subject)) = line.split_once('\t') else { continue };
            if !in_window(at, first, &offset) {
                continue;
            }
            let Some(rest) = subject.strip_prefix("Revert \"") else {
                relanded.extend(landing_refs(subject));
                continue;
            };
            // A revert of a revert lands the work again.
            if let Some(again) = rest.strip_prefix("Revert \"") {
                relanded.extend(landing_refs(again));
                continue;
            }
            for n in issue_refs(rest).into_iter().take(1) {
                if !relanded.contains(&n) && seen.insert(n) {
                    items.push(Returned { number: n, title: title(n), how: "reverted".into(), at: Some(at.trim().into()), revert: Some(subject.into()), ..Returned::default() });
                }
            }
        }
    }
    for r in &issues {
        let open = r.state.eq_ignore_ascii_case("open");
        if !open || !has_label(r, "bug") || !in_window(&r.created_at, first, &offset) {
            continue;
        }
        let text = format!("{}\n{}", r.title, r.body.as_deref().unwrap_or(""));
        for n in regression_refs(&text) {
            if n != r.number && landed.contains(&n) && seen.insert(n) {
                items.push(Returned { number: n, title: title(n), how: "regression".into(), at: Some(r.created_at.clone()), report: Some(r.number), ..Returned::default() });
            }
        }
    }
    let landed = landed.len().max(items.len()) as u32;
    Some(Goods { items, landed })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 3).unwrap()
    }
    fn utc() -> FixedOffset {
        FixedOffset::east_opt(0).unwrap()
    }
    const ISSUES: &str = r#"[
      {"number": 10, "title": "Saved carts", "state": "OPEN", "stateReason": "REOPENED", "createdAt": "2026-11-01T09:00:00Z", "labels": []},
      {"number": 11, "title": "Claimed again", "state": "OPEN", "stateReason": "REOPENED", "createdAt": "2026-11-01T09:00:00Z", "labels": [{"name": "working"}]},
      {"number": 12, "title": "Landed fine", "state": "CLOSED", "stateReason": "COMPLETED", "createdAt": "2026-11-05T09:00:00Z", "closedAt": "2026-11-06T09:00:00Z", "labels": []},
      {"number": 13, "title": "Reverted one", "state": "CLOSED", "stateReason": "COMPLETED", "createdAt": "2026-11-05T09:00:00Z", "closedAt": "2026-11-07T09:00:00Z", "labels": []},
      {"number": 14, "title": "Reverted, landed again", "state": "CLOSED", "stateReason": "COMPLETED", "createdAt": "2026-11-05T09:00:00Z", "closedAt": "2026-11-07T09:00:00Z", "labels": []},
      {"number": 15, "title": "Not planned", "state": "CLOSED", "stateReason": "NOT_PLANNED", "createdAt": "2026-11-05T09:00:00Z", "closedAt": "2026-11-07T09:00:00Z", "labels": []},
      {"number": 20, "title": "Checkout crashes, a regression from #12", "body": "Since #12 landed; see also #14", "state": "OPEN", "createdAt": "2026-11-10T09:00:00Z", "labels": [{"name": "bug"}]},
      {"number": 21, "title": "Regression in #15?", "body": "", "state": "OPEN", "createdAt": "2026-11-10T09:00:00Z", "labels": [{"name": "bug"}]},
      {"number": 22, "title": "Fixed regression from #13", "body": "", "state": "CLOSED", "stateReason": "COMPLETED", "createdAt": "2026-11-10T09:00:00Z", "closedAt": "2026-11-11T09:00:00Z", "labels": [{"name": "bug"}]},
      {"number": 23, "title": "Add a regression test for #14", "body": "", "state": "OPEN", "createdAt": "2026-11-10T09:00:00Z", "labels": [{"name": "bug"}]}
    ]"#;
    const LOG: &str = "2026-11-13T10:00:00Z\tReview of #13: a review lands nothing\n\
                       2026-11-12T10:00:00Z\tImplement #14: again, fixed\n\
                       2026-11-09T10:00:00Z\tRevert \"Implement #14: first try\"\n\
                       2026-11-08T10:00:00Z\tRevert \"Fix #13: reverted one\"\n\
                       2026-11-07T10:00:00Z\tFix #13: reverted one\n\
                       2026-10-01T10:00:00Z\tRevert \"Fix #9: too old\"\n";

    #[test]
    fn reopened_reverted_and_regressions_come_back_until_claimed_again() {
        let g = find(ISSUES.as_bytes(), Some(LOG), first(), utc()).unwrap();
        let got: Vec<(u32, &str)> = g.items.iter().map(|r| (r.number, r.how.as_str())).collect();
        assert_eq!(got, vec![(10, "reopened"), (13, "reverted"), (12, "regression")]);
        let rev = &g.items[1];
        assert_eq!(rev.title, "Reverted one");
        assert_eq!(rev.at.as_deref(), Some("2026-11-08T10:00:00Z"));
        assert_eq!(rev.revert.as_deref(), Some("Revert \"Fix #13: reverted one\""));
        assert_eq!(g.items[2].report, Some(20));
        // Landed in the window: 12, 13, 14, the fixed report 22, and the
        // reopened 10 and 11; not 15 (not planned).
        assert_eq!(g.landed, 6);
    }

    #[test]
    fn without_the_log_only_the_tracker_says_what_came_back() {
        let g = find(ISSUES.as_bytes(), None, first(), utc()).unwrap();
        assert_eq!(g.items.iter().map(|r| r.number).collect::<Vec<_>>(), vec![10, 12]);
        assert_eq!(find(b"not json", None, first(), utc()), None);
        assert_eq!(find(b"[]", Some(LOG), first(), utc()).unwrap().items.len(), 1, "reverts need no issue read: #13, not the re-landed #14");
    }

    #[test]
    fn a_revert_of_a_revert_lands_the_work_again() {
        let log = "2026-11-10T10:00:00Z\tRevert \"Revert \"Fix #13: x\"\"\n2026-11-08T10:00:00Z\tRevert \"Fix #13: x\"\n";
        assert!(find(b"[]", Some(log), first(), utc()).unwrap().items.is_empty());
        let log = "2026-11-10T10:00:00Z\tReview of #13: still reverted\n2026-11-08T10:00:00Z\tRevert \"Fix #13: x\"\n";
        assert_eq!(find(b"[]", Some(log), first(), utc()).unwrap().items.len(), 1, "a review lands nothing");
        let log = "2026-11-10T10:00:00Z\tFix #13: properly this time\n2026-11-08T10:00:00Z\tRevert \"Fix #13: x\"\n";
        assert!(find(b"[]", Some(log), first(), utc()).unwrap().items.is_empty());
    }

    #[test]
    fn issue_refs_reads_hashes_on_their_own() {
        assert_eq!(issue_refs("a regression from #12, see #3 and a#4 or /x#5"), vec![12, 3]);
        assert_eq!(issue_refs("#"), Vec::<u32>::new());
        assert_eq!(issue_refs("é#1 ü #2"), vec![1, 2], "non-ASCII before a hash is not a panic");
    }

    #[test]
    fn landings_and_regressions_are_read_from_the_words_before_the_hash() {
        assert_eq!(landing_refs("Implement #497: returned goods"), vec![497]);
        assert_eq!(landing_refs("Fix #12, closes #13"), vec![12, 13]);
        assert_eq!(landing_refs("Review of #497: words"), Vec::<u32>::new());
        assert_eq!(landing_refs("Re-sync origin/master into feature/issue-508"), Vec::<u32>::new());
        assert_eq!(regression_refs("Checkout crashes, a regression from #12; see #14"), vec![12]);
        assert_eq!(regression_refs("Regressed in #12"), vec![12]);
        assert_eq!(regression_refs("Add a regression test for #14"), Vec::<u32>::new());
    }
}
