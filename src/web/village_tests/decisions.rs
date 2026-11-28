//! Deciding a blocked (non-proposal) issue from its own dialog (#565,
//! following #564): Approve/Reject or Answer/Reject, by which blocking
//! label the issue carries; a proposal keeps its own flow (proposals.rs,
//! proposals.js). The words are tested under node (decidewords.test.mjs),
//! the server read and write in `decisions.rs`.

use super::*;

#[test]
fn the_issue_dialog_offers_approve_answer_or_reject() {
    assert!(village_asset("index.html").unwrap().1.contains("<div id=\"d-decide\"></div>"));
    assert!(village_asset("modules.js").unwrap().1.contains("import './decisions.js';"));
    assert!(village_asset("modules.js").unwrap().1.contains("import './decidewords.js';"));
    assert!(village_asset("modules.js").unwrap().1.contains("import './readonly.js';"));
    let decisions = village_asset("decisions.js").unwrap().1;
    assert!(decisions.contains("`api/swarms/${encodeURIComponent(project)}/issues/${n}/decide`") || decisions.contains("decideUrl"));
    // Only a decidable blocking label gets buttons; a proposal's doesn't (decidewords.js).
    assert!(decisions.contains("const d = info.issueDetail; if (!d) return;"));
    assert!(decisions.contains("const group = decideGroup(d.labels);"));
    assert!(decisions.contains("if (!group) return;"));
    // Hidden entirely on a read-only server, not just refused after a click.
    assert!(decisions.contains("import { canWrite } from './readonly.js';"));
    assert!(decisions.contains("canWrite().then(writable => { if (writable && mine === decideToken) fillDecide(box, info.project, d.number, group); });"));
    // A slow answer for an earlier click is dropped, as the issue dialog's own fetch is (issuedialog.js).
    assert!(decisions.contains("mine === decideToken"));
    let words = village_asset("decidewords.js").unwrap().1;
    assert!(words.contains("export const decideUrl = (project, n) => `api/swarms/${encodeURIComponent(project)}/issues/${n}/decide`;"));
}
