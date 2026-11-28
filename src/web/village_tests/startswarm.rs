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
    assert!(page.contains("if (!(await canWrite())) { box.textContent = READ_ONLY; return; }"));
    // One build per empty spell: a 3s poll must never wipe out what someone is mid-typing.
    assert!(page.contains("if (filling) return;"));
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
