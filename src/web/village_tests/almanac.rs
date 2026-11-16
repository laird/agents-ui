//! The almanac (#499): the last 14 days from the village's `github` summary
//! (#498), at the east edge. Its rows, marks and words are tested under node
//! (`tools/village-tests/almanac.test.mjs`, and `almanacmap.test.mjs` on the
//! map); this checks how the page wires it up.

use super::*;

#[test]
fn village_draws_the_almanac_from_the_github_summary() {
    let main = village_asset("modules.js").unwrap().1;
    assert!(main.contains("import './almanac.js';"));
    let tally = village_asset("almanactally.js").unwrap().1;
    assert!(!tally.contains("document"), "almanactally.js stays DOM-free, so node can test it");
    assert!(tally.contains("export const ALMANAC_DAYS = 14;"));
    // A slot at the east edge, only when the summary came with the village.
    let layout = village_asset("layout.js").unwrap().1;
    assert!(layout.contains("const almanac = almanacDays(v.github) ? { x: eastW + 2, y: 18, ...ALMANAC_SIZE } : null;"));
    assert!(layout.contains("!inAlmanacClearing(lay, x, y)"), "no forest tree grows in its clearing");
    let alm = village_asset("almanac.js").unwrap().1;
    // Drawn at 2×, nothing without rows; clickable, and a keyboard target (no `key: false`).
    assert!(alm.contains("detail(c, x, y, d => {"));
    assert!(alm.contains("if (!rows.length) return;"));
    assert!(alm.contains("tap(a.x, a.y, a.w, a.h, { kind: 'almanac' });"));
    assert!(alm.contains("registerHit({ kind: 'almanac', dialog(o, sc, theme) {"));
    assert!(!alm.contains("key: false"));
    // In each theme's own words, with a label and a Key row.
    for name in ["Almanac field", "Throughput chart", "Holo-graph"] {
        assert!(alm.contains(&format!("name: '{name}'")), "no {name}");
    }
    assert!(alm.contains("registerLabels((sc, theme) => {"));
    assert!(alm.contains("registerKey({ group: 'other', order: 60, rows: theme => ["));
}

/// The numbers dialog says where its numbers come from and when: `as_of`.
#[test]
fn almanac_dialog_carries_the_numbers_and_as_of() {
    let tally = village_asset("almanactally.js").unwrap().1;
    for fact in ["`Landed: ", "`Opened: ", "`Bounced: ", "`Reverted: ", "`Deploys: ", "Median from claimed to landed: ", "`As of ${almanacAsOf(gh.as_of)}`"] {
        assert!(tally.contains(fact), "the dialog lacks {fact}");
    }
}
