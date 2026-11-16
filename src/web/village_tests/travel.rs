//! An issue's travel log in its dialogs (#495): a crate's and a house's
//! dialog offer it, every listed issue gets a "log" button, and the log is
//! read from the server on demand. The words are tested under node
//! (travelwords.test.mjs), the log itself in `travel.rs`.

use super::*;

#[test]
fn dialogs_offer_an_issues_travel_log() {
    assert!(village_asset("index.html").unwrap().1.contains("<div id=\"d-travel\"></div>"));
    assert!(village_asset("main.js").unwrap().1.contains("import './travel.js';"));
    // A crate names its issue; a house's is its claimed issue.
    assert!(village_asset("lifecycle.js").unwrap().1.contains("travel: o.n };"));
    let travel = village_asset("travel.js").unwrap().1;
    assert!(travel.contains("const n = info.travel || (info.house && info.house.issue) || null;"));
    // Landed work is listed in the quay, stop, ship and castle dialogs: each issue gets a button.
    assert!(travel.contains("document.querySelectorAll('#d-meta li[data-n]')"));
    // A platform's workloads aren't issues.
    assert!(travel.contains("if (!v || v.plane === 'runtime') return;"));
    // Asked for on a click, never on open; a slow answer for an earlier dialog is dropped.
    assert!(travel.contains("const res = await fetch(travelUrl(project, n));"));
    assert!(travel.contains("if (mine !== travelToken) return;"));
    // The footer's medians come from the cached GitHub summary.
    assert!(travel.contains("medians = g.stage_medians || {}"));
    // The dialog knows which village it is in.
    assert!(village_asset("dialogs.js").unwrap().1.contains("if (info && !info.project && sc && sc.v) info.project = sc.v.project;"));
}
