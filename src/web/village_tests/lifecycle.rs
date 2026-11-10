//! Each issue's life: the docks, the houses, the people and their marks.

use super::*;

#[test]
fn village_html_animates_each_issues_life_in_a_queue() {
    let body = &village_source();
    // The queue: in order, staggered, capped; timed so a background tab never replays.
    assert!(body.contains("const STAGGER = 700, QUEUE_CAP = 12, WALK = 0.05;"));
    assert!(body.contains("export const ARRIVE_IN = 1800, UNLOAD = 450, SAIL_OUT = 1500;"));
    assert!(body.contains("if (state.reduce || state.porters.filter(p => p.t0 > t).length >= QUEUE_CAP) return null;"));
    assert!(body.contains("trip.t0 = Math.max(t, sc.nextAt || 0);"));
    // No replay on load; reduced motion walks no one and says what changed.
    assert!(body.contains("if (!old || old.project !== now.project) return [];"));
    assert!(body.contains("if (state.reduce && said.length && shownScenes().some(([x]) => x === sc)) toast("));
    // Arrives: a supply vessel in every theme, its crew unloading one crate each.
    assert_eq!(body.matches("supply(c, x, t, load) {").count(), 2, "modern truck and future shuttle");
    assert!(body.contains("vessel: ship, supply: boat,"));
    assert!(body.contains("sc.supplies.push(job);"));
    assert!(body.contains("T.supply(c, x, t, load);"));
    // Claimed: walk out empty, then carry it home; the crate waits at the docks meanwhile.
    assert!(body.contains("enqueue(sc, t, trip(h.id, h.issue, old.backlog[slot], 'house', [{ pts: out }, { pts: back(out), carry: true }]));"));
    assert!(body.contains("if (!a.fetched && p.issue && !p.crew) {"));
    // Worked on: the crate at the door, sparking unless the action is a quiet one.
    assert!(body.contains("tap(lot.x + 11, lot.y + 16, 5, 5, { kind: 'crate', n: h.issue, at: 'house', i });"));
    assert!(body.contains("const QUIET_ACTIONS = new Set(['thinking', 'reading', 'searching', 'messaging']);"));
    // Board both ways, landing with the walk home, and closed issues carried off the map.
    assert!(body.contains("'board', steps));"));
    assert!(body.contains("said.push(`#${c.number} back in the backlog`);"));
    assert!(body.contains("[{ pts, carry: true }, { pts: back(pts) }], { sealed: true }));"));
    assert!(body.contains("'away', [{ pts: [...from, [-10, ROAD]], carry: true }]));"));
    // A crate in transit is hidden at rest and opens the crate dialog.
    assert!(body.contains("scene.bound = new Set(state.porters.filter(p => p.issue && at.get(p) && !(at.get(p).fetched && !at.get(p).carry)).map(p => p.issue));"));
    assert!(body.contains("tap(a.x - 2, a.y - 16, 5, 5, { kind: 'crate', n: p.issue, at: 'transit', to: p.to });"));
    assert!(body.contains("if (o.at === 'transit') return o.to === 'away' ? 'closed without landing: carried away' : `on its way to the ${tripPlace(o.to, theme)}`;"));
}

#[test]
fn village_html_dialogs_explain_unknown_stops_the_whole_quay_and_houses_by_branch() {
    let body = &village_source();
    // Facts and the list each get their own line: no "branchNothing here".
    assert!(body.contains("map(m => `<span class=\"fact\">${esc(m)}</span>`).join('')"));
    assert!(body.contains(".dialog .fact { display: block; }"));
    assert!(!body.contains("filter(Boolean).map(esc).join(' · ')"));
    // An unknown stop says why in plain words, and links to the docs.
    assert!(body.contains("function unknownWords(v, i)"));
    assert!(body.contains("so nothing has been verified green. Run autocoder's sentinel"));
    assert!(body.contains("there is no successful GitHub Deployment to it and no env/${st.name} branch"));
    assert!(body.contains("/docs/village.md#environments"));
    assert!(body.contains("unknown ? unknownWords(v, o.i) : null"));
    assert!(body.contains("last.source === 'unknown' ? unknownWords(v, stops.length - 1) : null"));
    // The quay: six crates, a "+N" for the rest, and every issue listed with its title.
    assert!(body.contains("if (atQuay.length > QUAY_CAP) { const more = atQuay.length - QUAY_CAP, { w, h } = badgeSize(more); badge(c, roadX + 25, quayY - 9, more);"));
    assert!(body.contains("items: issueItems(v, v.pipeline.quay)"));
    assert!(!body.contains("info.items.slice(0, 40)"), "the dialog lists every issue");
    assert!(body.contains("(v.pipeline.titles || {})[n]"), "landed issues' titles come from the pipeline");
    // Houses are named by worktree and branch, with a compare link and how long in this status.
    assert!(body.contains("title: `${houseName(v, h)} · ${h.branch || h.kind}`"));
    assert!(body.contains("link(`Compare ${h.branch} with ${br}`, `${repo}/compare/"));
    assert!(body.contains("since ${since}` : null"));
    assert!(!body.contains("h.action ? `Now: ${h.action}` : null],\n        links: [...(h.links || []), issue"), "no separate Now line in the house dialog");
}

#[test]
fn village_html_shows_working_agents_and_every_subagent() {
    let body = &village_source();
    // A working agent is one busy figure at its door, acting out its action.
    assert!(body.contains("if (st === 'working') { person(c, lot.x + 4, lot.y + 8, h.id, workFrame(tt), { status: st }); drawAction(c, lot.x + 4, lot.y + 8, h.action, tt); }"));
    assert!(body.contains("const workFrame = t => (!state.reduce && blink(t, 180) ? 'work2' : 'work');"));
    assert!(body.contains("if (kind === 'work' || kind === 'work2') {"));
    // One figure per sub-agent in the slots, then a "+N" badge: none dropped.
    assert!(body.contains("const SUB_SLOTS = [[-1, 12], [9, 12], [-4, 16], [4, 16], [12, 16]];"));
    assert!(body.contains(".slice(0, SUB_SLOTS.length).forEach((sa, k) => {"));
    assert!(body.contains("if (subs.length > SUB_SLOTS.length) { const more = subs.length - SUB_SLOTS.length, { w, h } = badgeSize(more); badge(c, lot.x + 12, lot.y + 4, more);"));
    assert!(body.contains("function badge(c, x, y, n)"));
    assert!(!body.contains("subs.length > 2"), "the old two-figure cap is gone");
    // The manager is drawn busy at the hall while working.
    assert!(body.contains("if (v.manager.status !== 'stopped') {"));
    assert!(body.contains("busy ? workFrame(t) : 'stand', { mayor: true, status: v.manager.status });"));
}

/// #411: a finished worker shows ✓ only for a while; idle (or done a
/// while ago) it sits on its doorstep under "zzz". The timing is tested
/// under node (rest.js); this is the drawing that follows it.
#[test]
fn village_idle_workers_sleep_instead_of_a_lingering_tick() {
    let body = &village_source();
    assert!(body.contains("const st = h.status, tt = t + i * 311, rest = restOf(h, wall);"));
    assert!(body.contains("if (rest === 'asleep') { person(c, lot.x + 4, lot.y + 9, h.id, 'sit', { status: 'asleep' }); sleepers.push("));
    assert!(body.contains("if (ICONS[st] && rest !== 'asleep') bubbles.push(["), "no ✓ over a sleeper");
    assert!(body.contains("(sleepers || []).forEach(([x, y, dt]) => zzz(c, x, y, t + dt));"));
    // Every theme: people sit, robots sit with dim eyes and no hover.
    assert!(body.contains("if (kind === 'sit') return ['........', '........', ...r.slice(0, 9), r[11]];"));
    assert!(body.contains("asleep: '#24606c'"));
    assert!(body.contains("const bob = st === 'idle' && kind === 'stand'"), "only an awake idle robot hovers");
    // The key shows the sleeper and how long the tick lasts.
    assert!(body.contains("done: `Done: just finished (for ${FRESH_DONE_MS / 60000} minutes)`"));
    assert!(body.contains("['idle', 'Idle: sitting on the doorstep under \"zzz\", nothing claimed']"));
    assert!(body.contains("['done', `Done: a ✓ for ${freshDoneMinutes} minutes after finishing`]"));
    assert!(body.contains("stateRows(FRESH_DONE_MS / 60000)"));
}

#[test]
fn village_html_ropes_crates_that_wait_on_a_dependency() {
    let body = &village_source();
    assert!(body.contains("if ((cr.waits_on || []).length) roped(c, s.x, s.y);"));
    assert!(body.contains("function roped(c, x, y)"));
    // The crate dialog and the docks list say what it waits on.
    assert!(body.contains("return 'Waits on ' + cr.waits_on.map(d => { const w = whereWaitedOn(v, d, houseName); return `#${d}${w ? ` (${w})` : ''}`; }).join(', ');"));
    assert!(body.contains("(i.waits_on || []).length ? ' · waits on ' + i.waits_on.map(d => '#' + d).join(', ') : ''"));
    // And the key explains the rope.
    assert!(body.contains("'Roped crate: waits on another issue (\"Depends on\")'"));
}

#[test]
fn village_html_shows_each_crates_priority_epic_state_and_number() {
    let body = &village_source();
    // Every theme marks all four priorities, each its own way.
    assert!(body.contains("export const PRIOCOL = { P0: '#e02828', P1: '#f07020', P2: '#f8d040', P3: '#9898a8' };"));
    // (crates are 2× sprites, crates.js: tools/village-tests/crates.test.mjs reads each theme's marks back.)
    assert_eq!(body.matches("if (PRIOCOL[pr]) R(d, ").count() + body.matches("R(d, 1, 8, 9, 1, PRIOCOL[pr] || F.hullD);").count(), 3, "medieval, modern and future");
    // The docks stand in dock order; a crate carries its epic's flag colour and glints when ready.
    assert!(body.contains("v.backlog = arrangeDocks(v.backlog);"));
    assert!(body.contains("if (cols.has(cr.parent)) ribbon(c, s.x, s.y, cols.get(cr.parent));"));
    assert!(body.contains("roped(c, s.x, s.y); else if (!state.reduce) glint(c, s.x, s.y, t + cr.number * 263);"));
    assert!(body.contains("if (spot.has(d)) tether(c, s, spot.get(d));"));
    // The dialog names the priority, the epic and the state; hovering opens it.
    assert!(body.contains("registerHit({ kind: 'crate', hover: true, dialog(o, sc, theme) {"));
    assert!(body.contains("meta: [`Priority: ${it.priority || 'none'}`, epic ? `Epic: ${epic.words}` : null, cr ? stateWords(v, cr) : null],"));
    assert!(body.contains("if (crateState(cr) === 'ready') return 'Ready to pick up';"));
    assert!(body.contains("hoverTo(h && (HITS[h.o.kind] || {}).hover ? h : null);"));
    assert!(body.contains("cv.addEventListener('pointerleave', () => hoverTo(null));"));
    // Each crate's number is stencilled on it (#420), not laid over it as a label.
    assert!(body.contains("state.T.crate(c, s.x, s.y, cr.kind, cr.priority, false, cr.number);"));
    assert!(body.contains("export function stencil(d, n, ink = P.o, plate = P.woodD, light = P.parch)"));
    assert_eq!(["stencil(d, n);", "stencil(d, n, P.o, P.o, P.white);", "stencil(d, n, F.navy, F.navy, F.cyan);"].iter().map(|s| body.matches(s).count()).sum::<usize>(), 3, "every theme stencils the number");
    assert!(!body.contains("cls: 'num'"), "no #N label over the docks' crates");
    for cargo in ["'workshop', null, true, n); tap(x, y, 5, 5, { kind: 'crate', n, at: 'quay' });",
                  "'workshop', null, true, n); tap(x, y, 5, 5, { kind: 'crate', n, at: 'stop', i });",
                  "state.T.crate(c, x, y, 'workshop', null, true, n);\n      if (!at.leaving)",
                  "state.T.crate(c, x + 2 + k * 5, y + 1, 'workshop', null, true, n));"] {
        assert!(body.contains(cargo), "quay, stop, ship and barge cargo carry their numbers too: {cargo}");
    }
    assert!(body.contains("export const slotPos = (i, n = 40) => (n <= 24"));
    // And the key explains it all.
    assert!(body.contains("'Glinting crate: ready to pick up'"));
    assert!(body.contains("\"Crate edged in a flag's colour: part of that feature (its epic)\""));
    assert!(body.contains("`Number on a crate: its issue. Blocked issues are notes on the ${L.board}`"));
}

/// Trouble is the loudest thing on the map (#439): sizes are measured
/// under node (tools/village-tests/alarms.test.mjs); this checks each mark
/// is drawn where its trouble is.
#[test]
fn village_draws_trouble_loudest() {
    let body = &village_source();
    // Waiting on you: a "!" over the house or the hall and a ring at the door, drawn last.
    assert!(body.contains("if (st === 'blocked') alarms.push([lot.x + 8 - (ALARM.w >> 1), lot.y - ALARM.h - 2]);"));
    assert!(body.contains("if (v.manager.status === 'blocked') alarms.push([hallX + 25, 0]);"));
    assert!(body.contains("if (st === 'blocked') ring(c, lot.x + 8, lot.y + 19, tt);"));
    assert!(body.contains("(alarms || []).forEach(([x, y], k) => alarm(c, x, y, t + k * 120));"));
    // Failed is red and crossed; busy is smoke from the chimney.
    assert!(body.contains("bubbles.forEach(([x, y, s]) => (s === 'stalled' ? failed : bubble)(c, x, y, s));"));
    assert!(body.contains("if ((st === 'working' || (h.subagents || []).length) && !state.reduce) puffs("));
    // The board's alert follows its notes; the gatehouse lamps a failed merge.
    assert!(body.contains("const notes = v.board.filter(b => !bound.has(b.number)), alert = boardAlert(notes);"));
    assert!(body.contains("if (alert) boardFrame(c, hallX + 42, 28, alert.n, t);"));
    assert!(body.contains("if (gateEntries(v).some(e => e.state === 'failed' || e.state === 'conflict')) lamp(c, g.x0 + g.w - 2, g.y - 4, t);"));
    // And the Key says so.
    assert!(body.contains("['waiting', 'Waiting on you: a big red \"!\" and a ring at the door"));
}

/// #412: a barrel per house (and the hall) filling with the agent's context,
/// in each theme's form; the retry and failure markers are #433's.
#[test]
fn village_html_shows_context_barrels() {
    let body = &village_source();
    assert!(body.contains("export const CTX_WARN = 70, CTX_FULL = 90, CTX_WEDGE = 95;"));
    assert!(body.contains("return pct >= CTX_FULL ? P.p0 : pct >= CTX_WARN ? '#f0a020' : '#38c848';"));
    assert!(body.contains("const flash = pct >= CTX_WEDGE && !state.reduce && blink(t, 300);"));
    assert!(body.contains("if (theme === FUTURE) {") && body.contains("} else if (theme === MODERN) {"));
    assert!(body.contains("barrel(c, lot.x + 16, lot.y + 8, h.context_pct, t + i * 97);"));
    assert!(body.contains("barrel(c, lay.hallX + 34, 30, m.context_pct, t);"));
    // The house and hall dialogs say how full it is.
    assert!(body.contains("return `Context ${h.context_pct}%${of}${note}`;"));
    assert!(body.contains("contextWords(h),"));
    assert!(body.contains("meta: [`Status: ${m.status}`, contextWords(m),"));
    // And the Key explains it, through registerKey.
    assert!(body.contains("registerKey({ group: \"Beside a house: its agent's context\", rows: theme => ["));
    assert!(body.contains("`${CTX_FULL}% or more, red; flashing from ${CTX_WEDGE}%: about to wedge`"));
    assert!(!body.contains("retryTag(") && !body.contains("brokenCrate("), "the gate markers are #433's bounces");
}
