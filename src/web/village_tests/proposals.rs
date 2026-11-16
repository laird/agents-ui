//! Deciding proposals from the board (#483): the buttons, the bulk approval,
//! and what a read-only server makes them say.

use super::*;

#[test]
fn the_board_offers_decisions_on_its_proposals() {
    let body = &village_source();
    // Only a development village's board decides; a runtime's lists failures.
    assert!(body.contains("decide: isRuntime(sc.v) ? null : sc.v.project }) });"));
    // Every dialog runs the registered parts; each listed issue is findable by number.
    assert!(body.contains("DIALOG_PARTS.forEach(part => part(info));"));
    assert!(body.contains("<li data-n=\"${esc(i.number)}\">"));
    assert!(village_asset("index.html").unwrap().1.contains("<div id=\"d-act\"></div>"));
    // Approve, Reject asked twice, and Ask for changes with a text box.
    assert!(body.contains("<button data-a=\"approve\">Approve</button><button data-a=\"reject\">Reject</button>"));
    assert!(body.contains("else if (a === 'reject' && reject.dataset.sure) run('reject');"));
    assert!(body.contains("<button data-a=\"changes\">Ask for changes</button>"));
    // The bulk approval asks first, listing what it would approve.
    assert!(body.contains("Approve all P3 and below (${picks.length})"));
    assert!(body.contains("picks.map(n => `<li>${esc(pickWords(n))}</li>`)"));
}

#[test]
fn a_decision_posts_to_the_swarms_issue_and_a_403_says_read_only() {
    let body = &village_source();
    assert!(body.contains("`/api/swarms/${encodeURIComponent(project)}/issues/${encodeURIComponent(n)}/decision`"));
    assert!(body.contains("if (status === 403) return 'This dashboard is read-only: decide on GitHub.';"));
}
