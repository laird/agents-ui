//! An issue's own dialog in the village (#564): the board's rows (and any
//! other list naming an issue) get `#N` as a button, and a single-subject
//! dialog about one issue (a crate's, or a house's claimed one) gets an
//! "Open issue" button; either reads the issue in full from the server
//! (issue.rs) and shows it here. The words are tested under node
//! (issuewords.test.mjs, markdown.test.mjs), the server read in `issue.rs`.

use super::*;

#[test]
fn dialogs_offer_an_issues_own_dialog() {
    assert!(village_asset("index.html").unwrap().1.contains("<div id=\"d-issue\"></div>"));
    assert!(village_asset("modules.js").unwrap().1.contains("import './issuedialog.js';"));
    assert!(village_asset("modules.js").unwrap().1.contains("import './markdown.js';"));
    assert!(village_asset("modules.js").unwrap().1.contains("import './issuewords.js';"));
    // Every list item's #N is a button, not a direct GitHub link; the
    // GitHub link stays, now secondary.
    let dialogs = village_asset("dialogs.js").unwrap().1;
    assert!(dialogs.contains("<button class=\"issue-open\" type=\"button\" data-n=\"${esc(i.number)}\">#${i.number}</button>"));
    assert!(dialogs.contains("on GitHub ↗"));
    let issuedialog = village_asset("issuedialog.js").unwrap().1;
    // Any list row's button opens the issue dialog on the server's detail.
    assert!(issuedialog.contains("document.querySelectorAll('#d-meta li[data-n] button.issue-open')"));
    assert!(issuedialog.contains("const res = await fetch(issueUrl(project, n));"));
    // A single-subject dialog's own issue, the same number travel.js already finds.
    assert!(issuedialog.contains("const n = info.travel || (info.house && info.house.issue) || null;"));
    // A platform's workloads aren't issues.
    assert!(issuedialog.contains("if (!v || v.plane === 'runtime') return;"));
    // A slow answer for an earlier click is dropped, as travel.js's own fetch is.
    assert!(issuedialog.contains("if (mine !== issueToken) return;"));
}
