/* ---------- Trips ----------
 * Every change seen between two polls plays as a trip: a worker, porter or
 * crew member walks a route in steps, carrying the issue's crate on some.
 *   arrives   a supply boat (truck, shuttle) brings new issues to the docks
 *             and its crew unloads one crate each into its slot;
 *   claimed   the worker walks from its house to the docks, picks up that
 *             crate and carries it home, then works on it at the door;
 *   blocked   the crate is carried from the docks or house to the board;
 *   unblocked it is carried from the board back to the docks;
 *   landed    the worker carries it, sealed, to the quay and walks home;
 *   gated     with a merge gate (gatehouse.js), the worker carries it to the
 *             line instead, and the inspector moves it between the line, the
 *             arch and the tables, then stamps it and carries it out to the
 *             quay, or back up to the board, dented, when it is handed back;
 *   closed    (not planned) it is carried off the map's west edge.
 * Trips queue: each starts STAGGER after the one before, so many changes in
 * one poll play in order rather than at once. Past QUEUE_CAP waiting trips
 * the rest are skipped, their crates simply where they now are. Trips are
 * timed, so a tab left in the background never replays what it missed. The
 * first poll after a load animates nothing, and with reduced motion no one
 * walks: a toast says what changed. */
import { state } from './state.js';
import { GATE_H, porterPos, slotPos } from './layout.js';
import { cargoStop, placeOf, placePoint, toShip } from './ship.js';
import { gateEntries, gateOf, gateSpots } from './gatehouse.js';

const STAGGER = 700, QUEUE_CAP = 12, WALK = 0.05;
export const ARRIVE_IN = 1800, UNLOAD = 450, SAIL_OUT = 1500;
function enqueue(sc, t, trip, gap = STAGGER) {
  if (state.reduce || state.porters.filter(p => p.t0 > t).length >= QUEUE_CAP) return null;
  trip.t0 = Math.max(t, sc.nextAt || 0);
  sc.nextAt = trip.t0 + gap;
  state.porters.push(trip);
  return trip;
}
const back = pts => [...pts].reverse();
/* Routes on the map, from the lane in front of a house (lot) or a docks slot (s). */
const ROAD = 53;
const houseToRoad = (lot, lay) => [[lot.x + 8, lot.lane], [lay.roadX + 3, lot.lane], [lay.roadX + 3, ROAD]];
const slotFromRoad = s => [[s.x + 2, ROAD], [s.x + 2, s.y + 6]];
const boardFromRoad = lay => [[lay.hallX + 51, ROAD], [lay.hallX + 51, 46]];
/* What a trip does to the crate it carries, for the dialog of a crate in transit. */
function trip(seed, n, cr, to, steps, extra = {}) {
  return { seed, issue: n, kind: (cr && cr.kind) || 'workshop', pr: cr && cr.priority, to, steps, ...extra };
}
/* The trips (and floats down the canal) that a new model calls for, started
 * on `state.porters` and `state.floats`; returns what changed, in words. */
export function diff(old, now, lay, t, sc) {
  if (!old || old.project !== now.project) return [];
  const cs = cargoStop(now), said = [];
  const oldSlot = n => old.backlog.findIndex(c => c.number === n), newSlot = n => now.backlog.findIndex(c => c.number === n);
  const oldBoard = new Set(old.board.map(b => b.number)), oldHouse = n => old.houses.findIndex(h => h.issue === n);
  const lotOf = id => lay.lots[now.houses.findIndex(h => h.id === id)];

  // Arrives: new issues come by supply boat, one crate each.
  const fresh = now.backlog.map((c, i) => ({ c, i })).filter(({ c }) => oldSlot(c.number) < 0 && !oldBoard.has(c.number) && oldHouse(c.number) < 0 && c.number);
  if (fresh.length) {
    said.push(`${fresh.length} new ${fresh.length === 1 ? 'issue' : 'issues'}`);
    if (!state.reduce && state.porters.filter(p => p.t0 > t).length < QUEUE_CAP) {
      const t0 = Math.max(t, sc.nextAt || 0), shown = fresh.filter(f => f.i < 40);
      const job = { t0, crates: shown.map(f => f.c.number), leave: t0 + ARRIVE_IN + shown.length * UNLOAD + 500 };
      sc.supplies.push(job);
      shown.forEach(({ c, i }, k) => {
        const s = slotPos(i, now.backlog.length), dx = 22 + (k % 3) * 5;
        state.porters.push({ ...trip('crew:' + now.project + k, c.number, c, 'backlog', [
          { pts: [[dx, 13], [s.x + 2, 13], [s.x + 2, s.y + 6]], carry: true },
          { pts: [[s.x + 2, s.y + 6], [s.x + 2, 13], [dx, 13]] }]), t0: t0 + ARRIVE_IN + k * UNLOAD, crew: job });
      });
      sc.nextAt = job.leave;
    }
  }
  // Claimed: the worker fetches its crate from the docks.
  now.houses.forEach((h, i) => {
    const before = old.houses.find(o => o.id === h.id);
    if (!h.issue || (before && before.issue === h.issue)) return;
    const slot = oldSlot(h.issue), lot = lay.lots[i];
    if (slot < 0 || !lot) return;
    const s = slotPos(Math.min(slot, 39), old.backlog.length), out = [...houseToRoad(lot, lay), ...slotFromRoad(s)];
    said.push(`#${h.issue} claimed`);
    enqueue(sc, t, trip(h.id, h.issue, old.backlog[slot], 'house', [{ pts: out }, { pts: back(out), carry: true }]));
  });
  // The merge gate: into the line or onto a table, between its places, and
  // out again: stamped and sent on to the quay, or handed back to the board.
  const gated = new Set(), oldGs = gateSpots(old, lay), newGs = gateSpots(now, lay);
  const at = sp => [sp.x + 2, sp.y + 5], inspector = 'inspector:' + now.project;
  newGs.spots.forEach((sp, n) => {
    const was = oldGs.spots.get(n), gap = sp.lead && sp.unit.members.length === 1 ? STAGGER : 0;
    if (was && was.state === sp.state && was.x === sp.x && was.y === sp.y) return;
    const to = at(sp), cr = { kind: 'workshop' };
    if (was) {
      if (was.state !== sp.state) said.push(`#${n} ${sp.state}`);
      enqueue(sc, t, trip(inspector, n, cr, 'gate', [{ pts: [at(was), [at(was)[0], to[1]], to], carry: true }]), gap);
      return;
    }
    said.push(`#${n} ${sp.state}`);
    const hi = oldHouse(n), lot = hi >= 0 && lotOf(old.houses[hi].id);
    const down = [[lay.roadX + 3, to[1]], to];
    if (lot) { const there = [...houseToRoad(lot, lay), ...down]; enqueue(sc, t, trip(old.houses[hi].id, n, cr, 'gate', [{ pts: there, carry: true }, { pts: back(there) }]), gap); }
    else enqueue(sc, t, trip('porter:' + n, n, cr, 'gate', [{ pts: [[lay.roadX + 3, ROAD], ...down], carry: true }]), gap);
  });
  oldGs.spots.forEach((was, n) => {
    if (newGs.spots.has(n)) return;
    const e = gateOf(now, n), st = e ? e.state : now.pipeline.quay.includes(n) || now.pipeline.stops.some(s => s.issues.includes(n)) ? 'passed' : null;
    if (!st) return;
    const from = at(was), before = gateOf(old, n), t0 = Math.max(t, sc.nextAt || 0);
    // A finished run's test count is what later meters fill towards.
    if (before && before.tests_run && ['passed', 'landed-unproven', 'failed'].includes(st)) sc.testTotal = Math.max(sc.testTotal || 0, before.tests_run);
    gated.add(n);
    sc.stamps = sc.stamps || [];
    if (st === 'failed' || st === 'conflict') {
      sc.stamps.push({ x: was.x - 1, y: was.y - 7, t0, kind: st === 'conflict' ? 'knot' : 'rejected' });
      said.push(`#${n} ${st === 'conflict' ? 'conflict' : 'failed the gate'}`);
      enqueue(sc, t, trip(inspector, n, { kind: 'workshop' }, 'board',
        [{ pts: [from, [lay.roadX + 3, from[1]], [lay.roadX + 3, ROAD], ...boardFromRoad(lay)], carry: true }], { dented: true }));
    } else {
      sc.stamps.push({ x: was.x - 1, y: was.y - 7, t0, kind: 'ok' });
      said.push(`#${n} passed the gate`);
      const out = lay.gate ? lay.gate.y + GATE_H - 3 : lay.quayY;
      enqueue(sc, t, trip(inspector, n, { kind: 'workshop' }, 'quay',
        [{ pts: [from, [from[0], out], [lay.roadX + 3, out], [lay.roadX + 3, lay.quayY]], carry: true }], { sealed: true }));
    }
  });
  // Blocked: from the docks or a house to the board, and the board back to the docks.
  now.board.forEach(b => {
    const n = b.number;
    if (oldBoard.has(n) || gated.has(n)) return;
    const slot = oldSlot(n), hi = oldHouse(n);
    let steps = null, seed = 'porter:' + n;
    if (slot >= 0) { const s = slotPos(Math.min(slot, 39), old.backlog.length); steps = [{ pts: [...back(slotFromRoad(s)), ...boardFromRoad(lay)], carry: true }]; }
    else if (hi >= 0 && lotOf(old.houses[hi].id)) {
      const lot = lotOf(old.houses[hi].id), there = [...houseToRoad(lot, lay), ...boardFromRoad(lay)];
      seed = old.houses[hi].id; steps = [{ pts: there, carry: true }, { pts: back(there) }];
    }
    if (!steps) return;
    said.push(`#${n} blocked`);
    enqueue(sc, t, trip(seed, n, { kind: 'workshop', priority: b.priority }, 'board', steps));
  });
  now.backlog.forEach((c, i) => {
    if (!oldBoard.has(c.number) || i >= 40) return;
    said.push(`#${c.number} back in the backlog`);
    enqueue(sc, t, trip('porter:' + c.number, c.number, c, 'backlog', [{ pts: [...back(boardFromRoad(lay)), ...slotFromRoad(slotPos(i, now.backlog.length))], carry: true }]));
  });
  // Landed: the worker carries the sealed crate to the quay, then walks home.
  const oldQuay = new Set(old.pipeline.quay);
  now.pipeline.quay.forEach(n => {
    if (oldQuay.has(n) || gated.has(n) || oldGs.spots.has(n)) return;
    const i = oldHouse(n);
    if (i < 0) return;
    const lot = lotOf(old.houses[i].id) || lay.lots[i];
    if (!lot) return;
    const pts = [[lot.x + 8, lot.lane], [lay.roadX + 3, lot.lane], [lay.roadX + 3, lay.quayY]];
    // With one stop the quay is where the next release waits: on to the ship.
    if (cs === -1) pts.push(...toShip([lay.roadX + 3, lay.quayY], lay).slice(1));
    said.push(`#${n} landed`);
    enqueue(sc, t, trip(old.houses[i].id, n, { kind: old.houses[i].kind, priority: old.houses[i].priority }, cs === -1 ? 'ship' : 'quay',
      [{ pts, carry: true }, { pts: back(pts) }], { sealed: true }));
  });
  // Closed without landing: carried off the map's west edge.
  const placed = n => now.backlog.some(c => c.number === n) || now.board.some(b => b.number === n) || now.houses.some(h => h.issue === n)
    || now.pipeline.quay.includes(n) || now.pipeline.stops.some(st => st.issues.includes(n)) || gateEntries(now).some(e => e.issue === n);
  [...old.backlog.map((c, i) => ({ n: c.number, c, s: i < 40 ? slotPos(i, old.backlog.length) : null })), ...old.board.map(b => ({ n: b.number, c: b, board: true }))].forEach(o => {
    if (placed(o.n) || (!o.s && !o.board)) return;
    const from = o.board ? back(boardFromRoad(lay)) : back(slotFromRoad(o.s));
    said.push(`#${o.n} closed`);
    enqueue(sc, t, trip('porter:' + o.n, o.n, o.c, 'away', [{ pts: [...from, [-10, ROAD]], carry: true }]));
  });
  // Work moving down the canal opens each lock it passes. The stop before
  // the harbour loads it onto the ship; any other stop docks it in its basin.
  now.pipeline.stops.slice(0, -1).forEach((st, i) => st.issues.forEach(n => {
    const was = placeOf(n, old) ?? -1;
    if (was >= i) return;
    for (let g = was + 1; g <= i; g++) sc.gates[g] = t;
    const from = placePoint(was, lay);
    if (i === cs) enqueue(sc, t, trip('porter:' + n, n, null, 'ship', [{ pts: toShip(from, lay), carry: true }], { sealed: true, speed: 0.04 }), 300);
    else if (!state.reduce) state.floats.push({ n, t0: t, pts: [from, [from[0], lay.canalY + 4], [lay.lockStart + i * 34 + 10, lay.canalY + 4], placePoint(i, lay)] });
  }));
  return said;
}
/* Where a trip is: { x, y, carry } on its route, `waiting` before it starts
 * (at the route's first point), or null once it is over. */
export function tripAt(p, t) {
  const first = p.steps[0].pts[0];
  if (t < p.t0) return { x: first[0], y: first[1], carry: false, waiting: true, fetched: false };
  let d = (t - p.t0) * (p.speed || WALK), fetched = false;
  for (const st of p.steps) {
    const at = porterPos({ t0: 0, speed: 1, pts: st.pts }, d);
    if (at) return { x: at[0], y: at[1], carry: !!st.carry, fetched: fetched || !!st.carry };
    fetched = fetched || !!st.carry;
    d -= st.pts.slice(1).reduce((n, q, k) => n + Math.hypot(q[0] - st.pts[k][0], q[1] - st.pts[k][1]), 0);
  }
  return null;
}
/* The point a trip picks its crate up from: the start of its first carrying step. */
export const pickup = p => (p.steps.find(st => st.carry) || p.steps[0]).pts[0];
