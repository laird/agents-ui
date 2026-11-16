//! All swarms at a glance (#481): the overview map, its wiring into the
//! page, and the screensaver's `overview=1`. Its decisions are tested under
//! node (`tools/village-tests/overviewmodel.test.mjs`).

use super::*;

#[test]
fn the_overview_is_drawn_selected_and_picked() {
    let body = &village_source();
    // draw() hands the overview's frame to overview.js, before any village.
    assert!(body.contains("if (view.overview) { drawOverview(t); return; }"));
    // `#*` selects it, and the picker ends with it.
    assert!(body.contains("export const OVERVIEW_NAME = '*';"));
    assert!(body.contains("if (name === OVERVIEW_NAME && villages.length) { selectOverview(); return; }"));
    assert!(body.contains("const names = swarms.length ? [...swarms, OVERVIEW_NAME] : swarms;"));
    assert!(body.contains("n === OVERVIEW_NAME ? 'All swarms'"));
    // Re-selecting what is shown (a theme change, a poll) keeps the overview.
    assert!(body.contains("view.overview ? OVERVIEW_NAME : view.world || view.village"));
    assert!(!body.contains("select(view.world || view.village)"), "every re-select goes through viewName");
    // A click on an island opens it; the village's own click handler leaves the overview alone.
    assert!(body.contains("if (!state.view || state.view.overview || state.view.valley) return; // overview.js and valley.js answer their clicks"));
    assert!(body.contains("if (k !== null) openSwarm(islands[k].name);"));
    // Each island has a keyboard target that says how it is.
    assert!(body.contains("okeys.className = 'hitkeys ovkeys';"));
    assert!(body.contains("b.addEventListener('click', () => openSwarm(b.dataset.name));"));
}

#[test]
fn the_overview_is_not_a_village_scene() {
    // The village layers, labels and hit tests walk shownScenes(); the
    // overview must stay out of it, or every village layer would draw on it.
    let layout = village_asset("layout.js").unwrap().1;
    let shown = js_fn(layout, "shownScenes");
    assert!(!shown.contains("overview"), "shownScenes() knows only villages and worlds");
    let overview = village_asset("overview.js").unwrap().1;
    assert!(!overview.contains("registerLayer("), "the overview draws its own frame, not a village layer");
    assert!(!overview.contains("registerLabels("), "its names go in its own box, not the village's labels");
}

#[test]
fn the_screensaver_can_show_the_overview() {
    let body = &village_source();
    assert!(body.contains("overview: ['1', 'true', 'on'].includes(q.get('overview')),"));
    assert!(body.contains("if (saverOpts.overview && !saverOpts.pinned) history.replaceState(null, '', location.pathname + location.search + '#' + OVERVIEW_NAME + '&screensaver');"));
    assert!(body.contains("const next = nextSaverScene(names, current, lastSwarm, saverOpts.overview);"));
    assert!(body.contains("if (state.view && state.view.overview) { renderOverviewStrip(box, lastOk); return; }"));
}
