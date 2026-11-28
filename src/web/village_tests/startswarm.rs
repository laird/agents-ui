//! Start swarm from the empty state (#578): when /api/village shows no
//! villages at all, data.js hands `#empty` to startswarm.js instead of
//! just setting its text, so a writable server can launch one without
//! leaving the village page. The words are tested under node
//! (startswarmwords.test.mjs).

use super::*;

#[test]
fn data_js_hands_the_empty_state_to_startswarm() {
    let data = village_asset("data.js").unwrap().1;
    assert!(data.contains("import { fillEmpty, resetEmpty } from './startswarm.js';"));
    assert!(data.contains("if (!villages.length) fillEmpty(document.getElementById('empty'));"));
    // Villages back: the next empty spell gets a fresh build, not a stale one.
    assert!(data.contains("else { resetEmpty(); select("));
    assert!(village_asset("modules.js").unwrap().1.contains("import './startswarm.js';"));
}

#[test]
fn the_form_is_hidden_on_a_read_only_server_and_reuses_the_dashboards_routes() {
    let page = village_asset("startswarm.js").unwrap().1;
    assert!(page.contains("import { canWrite } from './readonly.js';"));
    assert!(page.contains("if (mine === emptyToken) box.textContent = READ_ONLY;"));
    // One build per empty spell (`emptyStarted`); a build still in flight from
    // a spell that already ended never lands on top of a fresh one or on top
    // of what someone is mid-typing (`emptyToken`, #578 review). Prefixed so
    // it can't collide with swarmcontrols.js's own `token` in the replay
    // bundle, which flattens every module into one script (#578 review).
    assert!(page.contains("if (emptyStarted) return;"));
    assert!(page.contains("if (mine !== emptyToken) return;"));
    assert!(page.contains("emptyStarted = false;\n  emptyToken++;"));
    // The agent-type fallback is imported, not re-declared, so it can't drift
    // from swarmcontrols.js's (#578 review, after #567's MANAGER_CONTROLS finding).
    assert!(page.contains("import { DEFAULT_AGENT_TYPES } from './swarmwords.js';"));
    let words = village_asset("startswarmwords.js").unwrap().1;
    assert!(words.contains("export const LAUNCH_URL = 'api/swarms';"));
    assert!(words.contains("export const AGENT_TYPES_URL = 'api/agent-types';"));
    assert!(words.contains("export const REPOS_URL = 'api/repos';"));
}

#[test]
fn village_css_styles_the_form_left_aligned_against_the_centered_empty_state() {
    let css = village_asset("village.css").unwrap().1;
    assert!(css.contains(".empty .start-swarm { display: grid;"));
    assert!(css.contains("text-align: left;"));
}
