//! The merge gate drawn as a gatehouse, and work it bounced back to its house.

use super::*;

#[test]
fn village_draws_the_merge_gate_as_a_gatehouse() {
    let body = &village_source();
    let layout = village_asset("layout.js").unwrap().1;
    // Layout: a band between the houses and the quay, one table per gate slot, only when the gate is known.
    assert!(layout.contains("export const GATE_H = 30, GATE_TABLES = 6;"));
    assert!(layout.contains("const tablesN = v.plane === 'runtime' || (!merge && !gates.length) ? 0"));
    assert!(layout.contains("const quayY = top + rows * LH + 4 + (gate ? GATE_H : 0);"));
    assert!(layout.contains("const lockStart = Math.max(roadX + 40, gate ? gate.x0 + gate.w + 10 : 0);"));
    assert!(layout.contains("decor, banners, devW, devH, envs, envCy, gate, slot };"));
    // Every theme has a gatehouse, named in its own words; busy slots are lit.
    for (file, name) in [("themes/medieval.js", "Gatehouse"), ("themes/modern.js", "QA checkpoint"), ("themes/future.js", "Scanner arch")] {
        let src = village_asset(file).unwrap().1;
        assert!(src.contains("gatehouse(c, g, t, busy) {"), "{file} draws no gatehouse");
        assert!(src.contains(&format!("gate: '{name}' }},")), "{file} does not name its gatehouse");
        assert!(src.contains("busy.has(k + 1)"), "{file} does not light its busy tables");
    }
    let gate = village_asset("gatehouse.js").unwrap().1;
    assert!(gate.contains("const busy = new Set([...(merge.busy || []), ...gs.units.filter(u => u.table !== undefined).map(u => u.table + 1)]);"));
    // Each state's place and picture: the numbered line, the rope to the master crate, the tables with a meter.
    assert!(gate.contains("export const IN_GATE = new Set(['queued', 'merging', 'gating']);"));
    assert!(gate.contains("u.x = g.rx - 16 - k * 16; u.y = g.y + 20;"));
    assert!(gate.contains("if (u.state === 'queued') numberTag(c, x + 2 - String(n).length * 2, y - 8, n);"));
    assert!(gate.contains("if (u.state === 'merging') rope(c, x + 2, y + 1, mx + 6, my + 6, e.attempt > 1);"));
    assert!(gate.contains("drawAction(c, x - 5, y - 8, 'searching', t + u.table * 97);"));
    assert!(gate.contains("const total = S.testTotal || 0, ran = e.tests_run || 0, mw = 9;"));
    assert!(gate.contains("if (!gateEntries(v).some(e => e.state === 'red-base')) return;"));
    assert!(gate.contains("if (!e || (e.state !== 'landed-unproven' && e.state !== 'passed')) return;"));
    // A merge train is one cart; handed-back work marks its house, one strike per attempt.
    assert!(gate.contains("tap(x - 1, y - 1, cw + 2, 10, { kind: 'train', id: e.train });"));
    assert!(gate.contains("if (!e || (e.state !== 'failed' && e.state !== 'conflict')) return;"));
    assert!(gate.contains("const n = Math.min(4, Math.max(1, e.attempt || 1));"));
    // A crate at the gate rests there: no house, board or quay draws it too.
    assert!(gate.contains("if (gs.spots.size) scene.bound = new Set([...scene.bound, ...gs.spots.keys()]);"));
    // Transitions: stamped out to the quay, or back to the board dented; no double trip from the landing.
    let trips = village_asset("trips.js").unwrap().1;
    assert!(trips.contains("kind: st === 'conflict' ? 'knot' : 'rejected' });"));
    assert!(trips.contains("[{ pts: [from, [lay.roadX + 3, from[1]], [lay.roadX + 3, ROAD], ...boardFromRoad(lay)], carry: true }], { dented: true }));"));
    assert!(trips.contains("if (oldQuay.has(n) || gated.has(n) || oldGs.spots.has(n)) return;"));
    assert!(body.contains("if (p.dented) dent(c, a.x - 2, a.y - 16);"));
    // Dialogs and the area label, which also counts today's failures (#405: no counter row).
    assert!(gate.contains("return { title: `${theme.labels.gate} · merge gate`, line: `${gs.queued} queued · ${gs.gating} gating`,"));
    assert!(gate.contains("link('What went wrong', e.failure_url)"));
    for kind in ["gatehouse", "train", "redbase", "gatecrate"] {
        assert!(gate.contains(&format!("registerHit({{ kind: '{kind}',")), "no dialog for {kind}");
    }
    assert!(gate.contains("text: `${theme.labels.gate} · merge gate`, sub: `${gs.queued} queued · ${gs.gating} gating${failed ? ` · ${failed} failed today` : ''}`"));
}

/// A crate the gate handed back (#433): dressed for rework at its house,
/// a red tag saying why and how often, debris from the second bounce,
/// all gone once the issue lands. What the marks say is tested under node
/// (bouncemarks.test.mjs).
#[test]
fn village_html_marks_bounced_crates_at_their_house() {
    let body = &village_source();
    assert!(body.contains("import './bounces.js';"));
    // Drawn from #431's pipeline.bounces, one mark per house that hasn't landed.
    assert!(body.contains("export const bouncesOf = (v, n) => ((v && v.pipeline && v.pipeline.bounces) || {})[n] || [];"));
    assert!(body.contains("if (!bs.length || landed(v, h.issue)) return [];"));
    assert!(body.contains("debris: bs.length >= 2"));
    // The tag over the house, red; each theme its own art; a click lists every attempt.
    assert!(body.contains("text: m.tag, cls: 'bounce', fixed: true"));
    assert!(body.contains(".al.bounce { color: #fff; background: #b02020;"));
    assert!(body.contains("if (state.T === FUTURE) {") && body.contains("} else if (state.T === MODERN) {"));
    assert!(body.contains("registerHit({ kind: 'bounce', dialog(o, sc) {"));
    assert!(body.contains("meta: bounceLines(bs),"));
}

/// The bounce itself (#434): first seen between two polls, the crate is
/// carried to the gate, jams there under a red ✗ (a knot for a conflict)
/// and is carried home again, once. Which bounces are new is tested under
/// node (bouncemarks.test.mjs).
#[test]
fn village_html_plays_a_new_bounce_from_the_gate_back_to_the_house() {
    let body = &village_source();
    assert!(body.contains("import { newBounces } from './bouncemarks.js';"));
    assert!(body.contains("newBounces(old, now).forEach(b => {"));
    assert!(body.contains("if (gated.has(b.n) || !lot || !h) return;"), "the gate's own entries already carried it");
    assert!(body.contains("[{ pts: there, carry: true }, { pts: back(there), carry: true }], { dented: true, bounce: b.kind }));"));
    assert!(body.contains("kind: b.kind === 'conflict' ? 'knot' : 'rejected' });"));
    // Played once: only bounces `old` didn't have.
    assert!(body.contains(".filter(m => m.count > bouncesOf(old, m.n).length)"));
}

/// The inefficiency view (#435): a slate by the gatehouse with today's
/// landed and bounced counts, and a hotspots sign whose dialog lists the
/// files that conflict most, in order. The words and the order are tested
/// under node (scoretally.test.mjs); this checks what is drawn and said.
#[test]
fn village_html_shows_the_gate_scoreboard_and_hotspots() {
    let body = &village_source();
    // The slate shows the totals, landed in green and bounced in red, when the gate reports a tally.
    assert!(body.contains("const g = lay.gate, ms = v.pipeline.merge_stats;\n  if (!g || !ms) return;"));
    assert!(body.contains("const w = slate(c, x, y, ms.landed_today || 0, ms.bounced_today || 0);"));
    // At 2× (#458) the counts are chits drawn in detail pixels where numberTag put them.
    assert!(body.contains("chit(d, 2, 2, landed, LANDED, P.o);"));
    assert!(body.contains("chit(d, 4 + 2 * wl, 2, bounced, BOUNCED, P.white);"));
    assert!(body.contains("return `landed ${ms.landed_today || 0} · bounced ${ms.bounced_today || 0} today`;"));
    // The sign stands only when something conflicted, and its dialog lists them in order.
    assert!(body.contains("if ((ms.hotspots || []).length) {"));
    assert!(body.contains("return { title: 'Conflict hotspots', line: 'The files that conflict most today: work that touches them bounces back from the gate.', meta: lines };"));
    assert!(body.contains(".sort((a, b) => b.count - a.count || (a.file < b.file ? -1 : a.file > b.file ? 1 : 0))"));
    // The Key names the slate and the sign (bounces.js names the rework crate).
    assert!(body.contains("registerKey({ group: 'other', rows: () => ["));
    assert!(body.contains("[c => slate(c, 1, 1, 14, 6), 'Slate by the gatehouse: work landed (green) and bounced (red) today', 26],"));
}
