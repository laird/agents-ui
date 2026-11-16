//! The counting house (#516): the village's token use (#514, `usage`) at the
//! east edge, past the almanac. What it says is tested under node
//! (`tools/village-tests/countingtally.test.mjs`) and where it stands and
//! that it draws in every theme by `countingmap.test.mjs`.

use super::*;

#[test]
fn village_draws_the_counting_house_from_usage() {
    let main = village_asset("main.js").unwrap().1;
    assert!(main.contains("import './countinghouse.js';"));
    let layout = village_asset("layout.js").unwrap().1;
    // A slot of its own only when the village has `usage`, and the map widens for it.
    assert!(layout.contains("const counting = countingUsage(v) ?"));
    assert!(layout.contains("const devW = counting ? counting.x + counting.w + 8"));
    // No forest tree grows in its clearing.
    assert!(layout.contains("[lay.almanac, lay.counting].some("));
    let house = village_asset("countinghouse.js").unwrap().1;
    assert!(house.contains("tap(at.x, at.y, at.w, at.h, { kind: 'counting' });"));
    assert!(house.contains("registerHit({ kind: 'counting'"));
    // Tokens, never dollars (#496).
    let tally = village_asset("countingtally.js").unwrap().1;
    assert!(tally.contains("'Tokens, not dollars: what they cost depends on the plan'"));
    assert!(!tally.contains("price") && !tally.contains("rate:"));
}
