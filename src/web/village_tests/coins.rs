//! Coins by each house's door for its token use (#515, for #496): drawn from
//! `tokens.today` on the house (usage.rs, #514), and said in the house's
//! dialog. The scale and the drawing are tested under node (coins.test.mjs).

use super::*;

#[test]
fn houses_show_their_token_use() {
    assert!(village_asset("modules.js").unwrap().1.contains("import './coins.js';"));
    let coins = village_asset("coins.js").unwrap().1;
    // No field or nothing today: no stack.
    assert!(coins.contains("const lot = lay.lots[i], n = coinCount(h.tokens && h.tokens.today && h.tokens.today.total);"));
    assert!(coins.contains("if (!lot || !n) return;"));
    // A click on the stack opens its house.
    assert!(coins.contains("{ kind: 'house', i }"));
    // Every theme draws it.
    assert!(coins.contains("if (theme === MODERN) {") && coins.contains("if (theme === FUTURE) {"));
    // The house dialog says today's use and its issue's.
    let lifecycle = village_asset("lifecycle.js").unwrap().1;
    assert!(lifecycle.contains("import { tokenWords } from './coins.js';"));
    assert!(lifecycle.contains("contextWords(h), ...tokenWords(h),"));
    // The key explains the scale.
    assert!(coins.contains("registerKey({ group: \"Beside a house: its token use\", order: 72,"));
}
