//! Every object on the map links to its GitHub page (#491): which dialogs
//! use the links `ghlinks.js` builds (its own tests are under node).

use super::*;

#[test]
fn village_dialogs_link_branches_prs_runs_and_history() {
    let gh = village_asset("ghlinks.js").unwrap().1;
    assert!(!gh.contains("document"), "ghlinks.js stays DOM-free, so node can test it");
    // A worker house: its branch and its pull request.
    let life = village_asset("lifecycle.js").unwrap().1;
    assert!(life.contains("...(runtime ? [] : branchLinks(repo, h.branch, h.links, br))"));
    // The hall: the integration branch's commit history.
    assert!(life.contains("commitsLink(repoUrl(v), v.pipeline.integration)"));
    // The board: every note links to its issue.
    assert!(life.contains("items: sc.v.board.map(n => (n.url ? n : { ...n, url: issueOf(sc.v, n.number).url }))"));
    // The gate: each entry's pull request and failure beside its issue, in every gate dialog.
    let gate = village_asset("gatehouse.js").unwrap().1;
    assert!(gate.contains("return [gatePrLink(repoUrl(v), e.issue, branchOf(v, e.issue)), failed];"));
    assert_eq!(gate.matches("flatMap(e => entryLinks(v, e))").count(), 3, "gatehouse, train and storm dialogs");
    assert!(!gate.contains("links: [] }"), "no gate dialog without links");
    assert!(gate.contains("e ? gatePrLink(repoUrl(v), o.n, branchOf(v, o.n)) : null"));
    // The build: the run when there is one, else the repo's workflow runs.
    let weather = village_asset("weather.js").unwrap().1;
    assert!(weather.contains("b.url ? null : actionsLink(repoUrl(v))"));
}
