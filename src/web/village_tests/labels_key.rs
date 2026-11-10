//! Area and house labels, and the Key drawn with the renderer's own functions.

use super::*;

#[test]
fn village_html_labels_each_area_and_house() {
    let body = &village_source();
    // One overlay of area names, switchable, placed with the map.
    assert!(body.contains(r#"<div class="alabels" id="alabels"></div>"#));
    assert!(body.contains(r#"id="labels-on" checked"#));
    assert!(body.contains("LABELS.forEach(({ fn }) => fn(sc, theme).forEach(l => {"));
    assert!(body.contains("registerOverlay(placeLabels, 20);"));
    assert!(body.contains("const l = houseLabel(houseName(v, h), h, h.issue && (h.issue_title || issueOf(v, h.issue).title));"));
    assert!(body.contains("parts: l.parts, sub: l.sub, cls: 'house ' + h.status, width: LW - 1"));
    // The hall names the manager's issue when it has one.
    assert!(body.contains("theme.labels.hall, managerSub(v.manager)"));
    // Houses are named for their worktree, less the project prefix.
    assert!(body.contains("function houseName(v, h)"));
    assert!(body.contains("base.startsWith(v.project + '-')"));
}

#[test]
fn village_html_hides_feature_labels_until_their_flag_is_clicked() {
    let body = &village_source();
    // Hidden and click-through until followed; shown while followed, focused or hovered.
    assert!(body.contains(".fb { opacity: 0; pointer-events: none; }"));
    assert!(body.contains(r#".fb[aria-pressed="true"] { opacity: 1; pointer-events: auto; }"#));
    assert!(body.contains(".fb:focus-visible, .fb.peek { opacity: var(--fade, 1); }"));
    // The fade no longer sets the opacity inline, which would show a hidden label.
    assert!(body.contains("b.style.setProperty('--fade', on ? 1 : (sc.fade[f.number] ?? 1));"));
    assert!(!body.contains("b.style.opacity ="));
    // Still real buttons in the Tab order: Enter/Space toggle as a click does.
    assert!(body.contains("b.addEventListener('click', () => toggleFollow(b.project, b.number));"));
    // Hovering a flag peeks at its label.
    assert!(body.contains("peek(h && h.o.kind === 'banner' ? fbuttons.get(`${h.sc.v.project}#${h.o.f}`) : null);"));
    assert!(body.contains("cv.addEventListener('mouseleave', () => peek(null));"));
}

#[test]
fn village_html_keeps_labels_clear_of_the_art_and_each_other() {
    let body = &village_source();
    // Size follows the map; subtitles drop below 3x.
    assert!(body.contains("abox.style.setProperty('--lf', Math.round(Math.max(9, Math.min(14, 2 + 2.5 * s))) + 'px');"));
    assert!(body.contains("abox.classList.toggle('compact', s < 3);"));
    assert!(body.contains(".alabels.compact small { display: none; }"));
    // The board label sits under the board; the harbour's left of the lighthouse.
    assert!(body.contains("add(lay.hallX + 51, 45, 'top', theme.labels.board,"));
    assert!(body.contains("add(lay.castleX + 10, lay.canalY - 22, 'bottom', theme.labels.castle,"));
    // And any label still covering the board, the tower or another label is moved.
    assert!(body.contains("(sc.hits || []).filter(h => h.o.kind === 'board').forEach(h => out.push({ avoid: h }));"));
    assert!(body.contains("out.push(...theme.towers.map(at), at(theme.hull));"));
    assert!(body.contains("towers: [[26, -60, 12, 42]],"));
    assert!(body.contains("function unclutter(els, blocked)"));
    assert!(body.contains("unclutter([...abox.children], blocked);"));
}

#[test]
fn village_html_has_a_key_drawn_from_the_renderer() {
    let body = &village_source();
    assert!(body.contains(r#"<button id="legend-btn" class="legend-btn" aria-expanded="false" aria-controls="legend""#));
    assert!(body.contains(r#"<div class="legend" id="legend" hidden role="region" aria-labelledby="legend-title">"#));
    // The rows come from the renderer's own tables, so a new kind or icon appears in the key.
    assert!(body.contains("Object.keys(KINDCOL).map(k => [c => state.T.crate(c, 1, 1, k, null, k === 'workshop')"));
    // Waiting on you, done and failed are States (#439, #442); the rest stay bubbles.
    assert!(body.contains("const IN_STATES = new Set(['blocked', 'done', 'stalled']);"));
    assert!(body.contains("Object.keys(ICONS).filter(k => !IN_STATES.has(k)).map(k => [c => bubble(c, 2, 1, k)"));
    assert!(body.contains("...SPECIALTIES.map(([name, kind]) => [c => person(c, 3, 1, 'key:' + name"));
    assert!(body.contains("Object.keys(NOTECOL).map(k =>"));
    assert!(body.contains("c => pennant(c, 5, 7, p, 0)"));
    assert!(body.contains("c => cobwebs(c, -1, -6)"));
    // The fog swatch (features.js's row) uses the same colour as the map's fog.
    // A feature adds its own rows where it draws them (registerKey), into a
    // group the key has or a new one, so a new feature needs no edit to key.js.
    assert!(body.contains("export function registerKey(entry) {"));
    assert!(body.contains("KEYS.forEach(k => {"));
    assert!(body.contains("registerKey({ group: 'Weather: the build', rows: theme => {"));
    assert!(body.contains("registerKey({ group: 'other', rows: theme => [\n  [c => { theme.crate(c, 3, 4, 'feature', null); reworked(c, 3, 4, 0); },"));
    assert!(body.contains("R(c, 0, 0, 14, 14, fogCol(theme));"));
    assert!(body.contains("lay.devH - lay.canalY + 28, fogCol(state.T));"));
    // Redrawn in the theme being shown.
    assert!(body.contains("if (!legend.hidden) renderLegend();\n    if (view) select("));
    assert!(body.contains("`Key · ${state.T.name}`"));
    // Places lead (#442): a sprite and a line for each part of the map, drawn
    // by the theme, then States, then the marks.
    assert!(body.contains("export const SECTIONS = ['Places', 'States', 'Marks'];"));
    assert!(body.contains("['places', 'What each part of the map is', placeRows(L)"));
    // Each place framed to fit its swatch, whatever the theme draws.
    assert!(body.contains("const [paint, size] = framed(PLACE_ART[r.id]);"));
    assert!(body.contains("['states', 'What a worker is doing, trouble loudest', stateRows(FRESH_DONE_MS / 60000)"));
    // A heading over each section; every other group (registerKey's too) is a mark.
    assert!(body.contains("const SECTION_OF = { places: 'Places', states: 'States' };"));
    assert!(body.contains("const now = SECTION_OF[id] || 'Marks';"));
    for place in ["house", "hall", "docks", "board", "gatehouse", "quay", "lock", "harbour", "env", "ship", "flag"] {
        assert!(body.contains(&format!("  {place}: c => ")), "no sprite for the {place} in the key");
    }
    // No blank swatches: every row draws something.
    assert!(!body.contains("[() => {}, "), "a key row with a blank swatch");
    // Priority crates drawn twice their size, the P-number first.
    assert!(body.contains("const bigCrate = (c, pr) => { c.save(); c.scale(2, 2); state.T.crate(c, 1, 1, 'feature', pr); c.restore(); };"));
    assert!(body.contains("...PRIO_ROWS.map(({ p, words }) => [c => bigCrate(c, p), words, 12])"));
    // Every kind, icon and pennant the map draws has words in the key.
    for table in ["KINDCOL", "ICONS"] {
        let words = if table == "KINDCOL" { "KIND_WORDS" } else { "ICON_WORDS" };
        let keys = js_keys(body, table);
        let have = js_keys(body, words);
        assert!(!keys.is_empty(), "no keys in {table}");
        for k in keys {
            assert!(have.contains(&k), "{words} has no words for {table}.{k}");
        }
    }
}
