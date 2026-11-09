/* The village page: layout, scenes, the ship, features, worlds, panels,
 * dialogs, labels, the key and the data feed. Drawing comes from the modules
 * imported here. */
import { state } from './state.js';
import { F, FEATURE_COLS, KINDCOL, NOTECOL, P, R, SPECIALTIES, STOPCOL, blink, hash, shadow, subHat, waveFrame } from './palette.js';
import { cobwebs, pennant, puffs } from './sprites/houses.js';
import { ICONS, bubble, roped } from './scenery.js';
import { QUIET_ACTIONS, actionSign, drawAction, person, wander } from './people.js';
import { MEDIEVAL } from './themes/medieval.js';
import { M, MODERN } from './themes/modern.js';
import { FUTURE } from './themes/future.js';
import { THEMES } from './themes/registry.js';

const themeFromUrl = new URLSearchParams(location.search).get('theme');
let themeName = THEMES[themeFromUrl] ? themeFromUrl : (() => {
  try { return THEMES[localStorage.getItem('village-theme')] ? localStorage.getItem('village-theme') : 'medieval'; } catch { return 'medieval'; }
})();
state.T = THEMES[themeName];

/* ---------- Layout ---------- */
const LW = 20, LH = 26, RD = 6, DOCK_W = 64;
function layout(v) {
  const n = v.houses.length;
  const per = Math.max(1, Math.ceil(Math.sqrt(Math.max(n, 1) / 2)));
  const rows = Math.max(1, Math.ceil(n / (2 * per)));
  const cx0 = DOCK_W + 8;
  const vw = Math.max(per * LW * 2 + RD, 72);
  const roadX = cx0 + Math.floor((vw - RD) / 2);
  const hallX = roadX + RD / 2 - 16;
  const top = 60;
  const lots = [];
  for (let i = 0; i < rows * 2 * per; i++) {
    const r = Math.floor(i / (2 * per)), j = i % (2 * per), side = j % 2, pos = Math.floor(j / 2);
    const x = side === 0 ? roadX - (pos + 1) * LW + 2 : roadX + RD + pos * LW + 2;
    lots.push({ x, y: top + r * LH, lane: top + r * LH + 18 });
  }
  const quayY = top + rows * LH + 4;
  const canalY = quayY + 8;
  // Every stop but the last is a lock; the last stop is the castle.
  const locks = Math.max(0, v.pipeline.stops.length - 1);
  const lockStart = roadX + 40;
  const castleX = lockStart + locks * 34 + 6;
  const forestX = Math.max(cx0 + vw + 6, hallX + 64);
  // Feature banners stand in a strip cleared out of the forest, east of the
  // houses; the map widens if they need the room. No features, no strip.
  const banners = (v.features || []).map((f, k) => ({ f, x: forestX + 12 + k * 30, y: 14, col: FEATURE_COLS[k % FEATURE_COLS.length] }));
  const W = Math.max(cx0 + vw + 40, roadX + 70, castleX + 46, banners.length ? forestX + 12 + banners.length * 30 : 0);
  const H = canalY + 34;
  // What's built on: river, docks, plaza, roads, house rows, canal and its
  // stations. Every free 9×8 cell may get a decoration.
  const taken = [
    [0, 0, W, 14], [4, 8, 62, 46], [4, 46, cx0 + vw, 14], [hallX - 10, 4, 74, 46],
    [roadX - 2, 46, RD + 4, quayY - 40], [roadX - 18, quayY - 12, W, H],
    [forestX - 2, 12, W, canalY - 46]
  ];
  for (let r = 0; r < rows; r++) {
    const left = roadX - per * LW, right = roadX + RD + per * LW;
    taken.push([left - 4, top + r * LH - 10, right - left + 8, LH + 4]);
  }
  const free = (x, y, w, h) => !taken.some(([tx, ty, tw, th]) => x < tx + tw && x + w > tx && y < ty + th && y + h > ty);
  // Big free ground grows small groves; what's left gets a few props. The
  // prop kind drifts slowly across the map, so neighbours form a garden
  // (flowerbeds together, rocks together) instead of random clutter.
  const decor = [];
  for (let y = 16; y < H - 18; y += 15) for (let x = 6; x < W - 16; x += 14) {
    if (!free(x, y, 14, 15) || hash(x * 7, y * 3) > 0.45) continue;
    decor.push({ x: x + Math.floor(hash(y, x) * 3), y: y + Math.floor(hash(x, y) * 2), tree: true });
    taken.push([x, y, 14, 15]);
  }
  for (let y = 16; y < H - 10; y += 9) for (let x = 6; x < W - 14; x += 11) {
    if (!free(x, y, 11, 9) || hash(x * 3, y * 5) > 0.2) continue;
    const zone = Math.floor(hash(x >> 5, y >> 5) * 6), k = hash(x + 11, y + 7) < 0.7 ? zone : Math.floor(hash(y, x + 3) * 6);
    decor.push({ x: x + Math.floor(hash(y, x) * 4), y: y + Math.floor(hash(x, y + 1) * 3), k });
  }
  return { per, rows, cx0, vw, roadX, hallX, top, lots, quayY, canalY, lockStart, castleX, W, H, decor, banners };
}

/* ---------- State ---------- */
const cv = document.getElementById('map'), ctx = cv.getContext('2d');
const pick = document.getElementById('pick');
let villages = [], worlds = [], view = null;
/* One scene per village: its model, layout and the animations in flight.
 * The one being drawn is in `state` while paint() runs (see state.js). */
const scenes = {};
/* Per world: the incidents and landing seen last poll, and crates afloat. */
const worldState = {};

const slotPos = i => ({ x: 10 + (i % 8) * 6, y: 22 + Math.floor(i / 8) * 6 });
/* What is clickable comes from what is drawn: paint() records a rectangle
 * (scene pixels) and the object behind it for every sprite it draws, later
 * ones on top. hit() and the keyboard targets read the list back, so a new
 * or moved sprite stays clickable without copying its coordinates. */
let hits = null;
const tap = (x, y, w, h, o) => { if (hits) hits.push({ x: Math.round(x), y: Math.round(y), w, h, o }); };

/* ---------- Each issue's life ----------
 * Every change seen between two polls plays as a trip: a worker, porter or
 * crew member walks a route in steps, carrying the issue's crate on some.
 *   arrives   a supply boat (truck, shuttle) brings new issues to the docks
 *             and its crew unloads one crate each into its slot;
 *   claimed   the worker walks from its house to the docks, picks up that
 *             crate and carries it home, then works on it at the door;
 *   blocked   the crate is carried from the docks or house to the board;
 *   unblocked it is carried from the board back to the docks;
 *   landed    the worker carries it, sealed, to the quay and walks home;
 *   closed    (not planned) it is carried off the map's west edge.
 * Trips queue: each starts STAGGER after the one before, so many changes in
 * one poll play in order rather than at once. Past QUEUE_CAP waiting trips
 * the rest are skipped, their crates simply where they now are. Trips are
 * timed, so a tab left in the background never replays what it missed. The
 * first poll after a load animates nothing, and with reduced motion no one
 * walks: a toast says what changed. */
const STAGGER = 700, QUEUE_CAP = 12, WALK = 0.05, ARRIVE_IN = 1800, UNLOAD = 450, SAIL_OUT = 1500;
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
function diff(old, now, lay, t, sc) {
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
        const s = slotPos(i), dx = 22 + (k % 3) * 5;
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
    const s = slotPos(Math.min(slot, 39)), out = [...houseToRoad(lot, lay), ...slotFromRoad(s)];
    said.push(`#${h.issue} claimed`);
    enqueue(sc, t, trip(h.id, h.issue, old.backlog[slot], 'house', [{ pts: out }, { pts: back(out), carry: true }]));
  });
  // Blocked: from the docks or a house to the board, and the board back to the docks.
  now.board.forEach(b => {
    const n = b.number;
    if (oldBoard.has(n)) return;
    const slot = oldSlot(n), hi = oldHouse(n);
    let steps = null, seed = 'porter:' + n;
    if (slot >= 0) { const s = slotPos(Math.min(slot, 39)); steps = [{ pts: [...back(slotFromRoad(s)), ...boardFromRoad(lay)], carry: true }]; }
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
    enqueue(sc, t, trip('porter:' + c.number, c.number, c, 'backlog', [{ pts: [...back(boardFromRoad(lay)), ...slotFromRoad(slotPos(i))], carry: true }]));
  });
  // Landed: the worker carries the sealed crate to the quay, then walks home.
  const oldQuay = new Set(old.pipeline.quay);
  now.pipeline.quay.forEach(n => {
    if (oldQuay.has(n)) return;
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
    || now.pipeline.quay.includes(n) || now.pipeline.stops.some(st => st.issues.includes(n));
  [...old.backlog.map((c, i) => ({ n: c.number, c, s: i < 40 ? slotPos(i) : null })), ...old.board.map(b => ({ n: b.number, c: b, board: true }))].forEach(o => {
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
function tripAt(p, t) {
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
const pickup = p => (p.steps.find(st => st.carry) || p.steps[0]).pts[0];
function porterPos(p, t) {
  let d = (t - p.t0) * p.speed;
  for (let k = 0; k < p.pts.length - 1; k++) {
    const [ax, ay] = p.pts[k], [bx, by] = p.pts[k + 1], len = Math.hypot(bx - ax, by - ay);
    if (d <= len) return [ax + (bx - ax) * d / len, ay + (by - ay) * d / len];
    d -= len;
  }
  return null;
}

/* ---------- The ship ----------
 * Work waiting for the next release is loaded onto the ship in the harbour
 * (a rocket on the launch pad): the issues at the stop before the last, or
 * at the quay when there is only one stop. When the last stop's deployment
 * changes, the ship sails off east with what shipped, the fireworks fire,
 * and an empty ship comes in. The change is read from the last stop's sha
 * (or the released commit), never its issues, so a release of the same
 * issues still sails; a page loaded after a release just shows the new ship. */
const SAIL = 4200, ARRIVE = 3200, GATE_OPEN = 2400, SHIP_CAP = 6;
/* Crates drawn at the quay; the rest are a "+N". */
const QUAY_CAP = 6;
const cargoStop = v => v.pipeline.stops.length - 2; // -1: the quay
function cargoOf(v) {
  const i = cargoStop(v);
  return i < -1 ? [] : i < 0 ? v.pipeline.quay : v.pipeline.stops[i].issues;
}
const lastStop = v => v.pipeline.stops[v.pipeline.stops.length - 1] || null;
/* What the last stop runs, with what says so ("deployment:<sha>"). A poll
 * where GitHub didn't answer falls back to the branch or to nothing; that
 * is a different source, not a release, so it never sails the ship. */
function releaseOf(v) {
  const last = lastStop(v), id = last && (last.sha || last.tag || v.pipeline.release);
  return id ? `${last.source || 'branch'}:${id}` : null;
}
/* Where the ship is: moored, sailing out with the last release, or coming in. */
function shipAt(sc, t, lay) {
  const vo = sc.voyage, since = vo ? t - vo.t0 : Infinity, flies = !!state.T.flies, out = lay.W - lay.castleX + 4;
  if (since < SAIL) { const p = (since / SAIL) ** 2; return { leaving: true, ox: flies ? 0 : p * out, oy: flies ? -p * (lay.canalY + 64) : 0 }; }
  if (since < SAIL + ARRIVE) { const p = 1 - (since - SAIL) / ARRIVE; return { leaving: false, ox: out * p * p, oy: 0 }; }
  return { leaving: false, ox: 0, oy: 0 };
}
const holdPos = (lay, k, ox = 0, oy = 0) => [Math.round(lay.castleX + state.T.hold[k][0] + ox), Math.round(lay.canalY + state.T.hold[k][1] + oy)];
/* A crate's resting point at the quay (-1) or in stop i's basin. */
const placePoint = (i, lay) => (i < 0 ? [lay.roadX + 9, lay.quayY + 2] : [lay.lockStart + i * 34 + 10, lay.canalY + 22]);
/* A porter's walk from a point along the canal bank to the ship's side. */
function toShip([x, y], lay) {
  const bank = lay.canalY + 26, sx = lay.castleX + state.T.hold[0][0] + 4;
  return [[x, y], [x, bank], [sx, bank], [sx, lay.canalY + 18]];
}
function sail(sc, v, t) {
  const old = sc.v, last = lastStop(v), before = new Set((lastStop(old) || { issues: [] }).issues);
  const shipped = last ? last.issues.filter(n => !before.has(n)) : [];
  sc.fireworks = t;
  sc.voyage = state.reduce ? null : { t0: t, cargo: shipped };
  if (shownScenes().some(([x]) => x === sc)) toast(`${v.project}: shipped ${releaseName(v, last) || 'a release'}${shipped.length ? ` · ${shipped.length} ${shipped.length === 1 ? 'issue' : 'issues'}` : ''}`);
}
/* A release's name: its tag, else its commit. */
function releaseName(v, s) {
  if (!s) return null;
  const sha = s.sha || v.pipeline.release;
  return s.tag || (sha ? String(sha).slice(0, 7) : null);
}
let toastTimer = 0;
function toast(text) {
  const el = document.getElementById('toast');
  el.textContent = text; el.hidden = false;
  clearTimeout(toastTimer); toastTimer = setTimeout(() => { el.hidden = true; }, 5000);
}

/* Where a house's sub-agents stand: two beside the door, then a row on the
 * lane in front. Past that, a "+N" badge counts the rest, so every running
 * sub-agent is either drawn or counted, never dropped. */
const SUB_SLOTS = [[-1, 12], [9, 12], [-4, 16], [4, 16], [12, 16]];
const workFrame = t => (!state.reduce && blink(t, 180) ? 'work2' : 'work');
const DIGITS = ['111101101101111', '010110010010111', '111001111100111', '111001111001111', '101101111001001',
  '111100111001111', '111100111101111', '111001001001001', '111101111101111', '111101111001111'];
/* A gold tag reading "+N" in a 3x5 pixel font. */
function badge(c, x, y, n) {
  const glyphs = ['000010111010000', ...String(n).split('').map(d => DIGITS[+d])], w = glyphs.length * 4 + 1;
  R(c, x - 1, y - 1, w + 2, 7, P.o); R(c, x, y, w, 5, P.gold);
  glyphs.forEach((g, j) => { for (let k = 0; k < 15; k++) if (g[k] === '1') R(c, x + 1 + j * 4 + (k % 3), y + Math.floor(k / 3), 1, 1, P.o); });
}
function paint(c, t) {
  const v = state.current, lay = state.L;
  const { W, H, roadX, hallX, lots, quayY, canalY, cx0, vw } = lay;
  // A whole-feature barge carries its subtasks' crates: they are not drawn loose.
  const barges = bargesFor(v, lay), hid = new Set(barges.flatMap(b => b.issues));
  // A crate on a trip is not at rest anywhere until the trip has delivered it.
  const at = new Map(state.porters.map(p => [p, tripAt(p, t)]));
  const bound = new Set(state.porters.filter(p => p.issue && at.get(p) && !(at.get(p).fetched && !at.get(p).carry)).map(p => p.issue));
  state.T.ground(c, 0, 0, W, H);
  if (state.T.border) state.T.border(c, W, H);
  state.T.channel(c, 0, 0, W, 11, t, 1); tap(0, 0, W, 11, { kind: 'river' });
  state.T.dock(c, 14, 10, 40);
  state.T.yard(c, 6, 18, 56, 34); tap(6, 8, 56, 44, { kind: 'docks' });
  // The supply boat: in from the west, moored while its crew unloads, then away.
  state.S.supplies = state.S.supplies.filter(j => {
    if (t > j.leave + SAIL_OUT) return false;
    if (t < j.t0) return true;
    const x = t < j.t0 + ARRIVE_IN ? -32 + 46 * ((t - j.t0) / ARRIVE_IN) : t < j.leave ? 14 : 14 - 46 * ((t - j.leave) / SAIL_OUT);
    const load = j.crates.filter((_, k) => t < j.t0 + ARRIVE_IN + k * UNLOAD).length;
    state.T.supply(c, x, t, load);
    tap(x, 0, 30, 11, { kind: 'supply', n: j.crates.length, ns: j.crates });
    return true;
  });
  state.T.road(c, 4, 50, cx0 + vw - 4, 6);
  state.T.road(c, roadX, 50, RD, quayY - 50 + 2);
  state.T.yard(c, hallX - 8, 42, 48, 9);
  for (let r = 0; r < lay.rows; r++) {
    const left = roadX - lay.per * LW + 2, right = roadX + RD + lay.per * LW - 2;
    state.T.road(c, left - 1, lay.top + r * LH + 17, right - left + 2, 4);
  }
  state.T.channel(c, roadX - 16, canalY, W - roadX + 16, 14, t, 0.6); tap(roadX - 16, canalY, W - roadX + 16, 14, { kind: 'canal' });
  state.T.dock(c, roadX - 4, quayY + 1, 22); if (v.plane !== 'runtime') tap(roadX - 4, quayY - 16, 22, 24, { kind: 'quay' });

  for (let x = Math.max(cx0 + vw + 6, hallX + 64); x < W - 14; x += 15) for (let y = 16; y < canalY - 38; y += 16) if (hash(x, y) < 0.55 && !inBannerStrip(lay, x, y)) state.T.tree(c, x + Math.floor(hash(y, x) * 4), y);
  lay.decor.forEach(d => d.tree ? state.T.tree(c, d.x, d.y) : state.T.decor && state.T.decor(c, d.x, d.y, d.k, state.reduce ? 0 : t));
  shadow(c, hallX - 2, 40, 42, 2);

  state.T.hall(c, hallX, 18, v.manager.status); tap(hallX - 4, 6, 40, 36, { kind: 'hall' });
  state.T.board(c, hallX + 42, 28, v.board.filter(b => !bound.has(b.number))); tap(hallX + 42, 28, 18, 16, { kind: 'board' });
  if (v.manager.status !== 'stopped') {
    const busy = v.manager.status === 'working';
    person(c, hallX + 26, 30, 'manager:' + v.project, v.manager.status === 'blocked' ? waveFrame(t) : busy ? workFrame(t) : 'stand', { mayor: true, status: v.manager.status });
    if (busy) drawAction(c, hallX + 26, 30, v.manager.action, t);
    tap(hallX + 26, 30, 8, 12, { kind: 'hall' });
  }
  // The manager's sub-agents (a modernize coordinator's specialists) work in the plaza.
  (v.manager.subagents || []).slice(0, 4).forEach((sa, k) => {
    const bob = !state.reduce && blink(t + k * 170, 220) ? -1 : 0;
    const seed = `manager:${v.project}/${sa.description || k}`, px = hallX - 2 + k * 10 + (state.reduce ? 0 : wander(sa.action, t, seed));
    person(c, px, 39 + bob, seed, 'stand', { accent: subHat(sa), sub: true, status: sa.status });
    tap(px, 38, 8, 13, { kind: 'sub', house: -1, sa, id: sa.description || k });
    drawAction(c, px, 39 + bob, sa.action, t + k * 131);
  });

  v.backlog.slice(0, 40).forEach((cr, i) => {
    if (bound.has(cr.number)) return;
    const s = slotPos(i); state.T.crate(c, s.x, s.y, cr.kind, cr.priority); if ((cr.waits_on || []).length) roped(c, s.x, s.y); tap(s.x, s.y, 5, 5, { kind: 'crate', n: cr.number, at: 'backlog' });
  });

  const bubbles = [];
  if (['blocked', 'done', 'unknown', 'stalled'].includes(v.manager.status)) bubbles.push([hallX + 26, 18, v.manager.status]);
  v.houses.forEach((h, i) => {
    const lot = lots[i]; if (!lot) return;
    const st = h.status, tt = t + i * 311;
    shadow(c, lot.x + 2, lot.y + 16, 16, 2); shadow(c, lot.x + 16, lot.y + 4, 2, 12);
    tap(lot.x, lot.y - 4, 16, 25, { kind: 'house', i });
    if (st === 'building') state.T.scaffold(c, lot.x, lot.y, tt);
    else (state.T.B[h.kind] || state.T.B.workshop)(c, lot.x, lot.y, st, state.reduce ? 0 : tt);
    pennant(c, lot.x, lot.y, h.priority, tt);
    if (st === 'stalled') cobwebs(c, lot.x, lot.y);
    if (st === 'unknown') R(c, lot.x - 1, lot.y - 6, 18, 23, 'rgba(32,34,60,.6)');
    const away = state.porters.some(p => p.seed === h.id && t >= p.t0 && at.get(p));
    if (!away) {
      if (st === 'idle') person(c, lot.x + 4, lot.y + 9, h.id, 'stand', { status: st });
      if (st === 'done') person(c, lot.x + 4, lot.y + 8, h.id, 'stand', { status: st });
      if (st === 'blocked') person(c, lot.x + 4, lot.y + 10, h.id, waveFrame(t + i * 90), { status: st });
      // A working agent is one busy figure at its door, acting out what it is doing.
      if (st === 'working') { person(c, lot.x + 4, lot.y + 8, h.id, workFrame(tt), { status: st }); drawAction(c, lot.x + 4, lot.y + 8, h.action, tt); }
      // Robots show every status, so a stalled one sits slumped at its door.
      const slumped = state.T.robots && st === 'stalled';
      if (slumped) person(c, lot.x + 4, lot.y + 9, h.id, 'stand', { status: st });
      if (['idle', 'done', 'blocked', 'working'].includes(st) || slumped) tap(lot.x + 4, lot.y + 8, 8, 12, { kind: 'person', i, who: h.crew[0] });
    }
    if (h.crew.length > 1 && st !== 'working') { person(c, lot.x + 12, lot.y + 11, h.id + '#crew'); tap(lot.x + 12, lot.y + 11, 8, 12, { kind: 'person', i, who: h.crew[1] }); }
    if (h.crew.length > 1 && st === 'working' && !state.reduce) puffs(c, lot.x + 3, lot.y - 3, tt + 700, ...state.T.steam);
    // Sub-agents keep the house busy: one figure each out front (SUB_SLOTS),
    // and a "+N" badge for any that don't fit.
    const subs = h.subagents || [];
    if (subs.length && !state.reduce) puffs(c, lot.x + 3, lot.y - 3, tt + 400, ...state.T.steam);
    // Anyone waiting on a person is shown first, waving.
    [...subs].sort((a, b) => (b.status === 'blocked') - (a.status === 'blocked')).slice(0, SUB_SLOTS.length).forEach((sa, k) => {
      const bob = !state.reduce && blink(t + k * 190, 210) ? -1 : 0;
      const waiting = sa.status === 'blocked', seed = `${h.id}/${sa.description || k}`, [sx, sy] = SUB_SLOTS[k];
      const px = lot.x + sx + (state.reduce ? 0 : wander(sa.action, t, seed)), py = lot.y + sy + bob;
      person(c, px, waiting ? lot.y + sy : py, seed, waiting ? waveFrame(t + k * 110) : 'stand', { accent: subHat(sa), sub: true, status: sa.status });
      tap(px, lot.y + sy - 1, 8, 13, { kind: 'sub', house: i, sa, id: sa.description || k });
      if (!waiting) drawAction(c, px, py, sa.action, t + k * 97);
    });
    // The issue's crate at the door while it is worked on: hammered, sawn, sparking.
    if (st === 'working' && h.issue && !away && !bound.has(h.issue)) {
      state.T.crate(c, lot.x + 11, lot.y + 16, h.kind, h.priority);
      tap(lot.x + 11, lot.y + 16, 5, 5, { kind: 'crate', n: h.issue, at: 'house', i });
      if (!state.reduce && !QUIET_ACTIONS.has(h.action) && blink(tt, 150)) {
        const k = Math.floor(tt / 150) % 3;
        R(c, lot.x + 12 + k, lot.y + 14 - (k % 2), 1, 1, P.gold); R(c, lot.x + 15 - k, lot.y + 15, 1, 1, P.white);
      }
    }
    if (st === 'working' && h.action) actionSign(c, lot.x + 4, lot.y - 4, h.action);
    if (subs.length > SUB_SLOTS.length) { badge(c, lot.x + 12, lot.y + 4, subs.length - SUB_SLOTS.length); tap(lot.x + 11, lot.y + 3, 12, 7, { kind: 'house', i }); }
    if (ICONS[st]) bubbles.push([lot.x + 4, lot.y - (st === 'blocked' ? 1 : 3) + (st === 'blocked' && blink(t, 200) ? -1 : 0), st]);
  });

  const carried = new Set([...bound, ...state.floats.map(f => f.n)]);
  const cargo = cargoOf(v), onShip = new Set(cargo);
  const atQuay = v.pipeline.quay.filter(n => !carried.has(n) && !hid.has(n) && !onShip.has(n));
  atQuay.slice(0, QUAY_CAP).forEach((n, k) => {
    const x = roadX + 6 + (k % 3) * 6, y = quayY - 3 - Math.floor(k / 3) * 6;
    state.T.crate(c, x, y, 'workshop', null, true); tap(x, y, 5, 5, { kind: 'crate', n, at: 'quay' });
  });
  // Six crates fit; a "+N" says how many more wait, and the quay's dialog lists them all.
  if (atQuay.length > QUAY_CAP) { badge(c, roadX + 25, quayY - 9, atQuay.length - QUAY_CAP); tap(roadX + 24, quayY - 10, 14, 7, { kind: 'quay' }); }

  // The sentinel's flag is grey while it still has newer work to test. With
  // no sentinel the first lock is an environment like any other.
  const gated = v.pipeline.sentinel !== 'absent';
  const flagOf = i => i === 0 && gated && v.pipeline.sentinel !== 'green' ? P.stoneD : STOPCOL[i] || P.white;
  const stops = v.pipeline.stops;
  stops.slice(0, -1).forEach((s, i) => {
    const lx = lay.lockStart + i * 34;
    const open = !state.reduce && t - (state.S.gates[i] ?? -1e9) < GATE_OPEN;
    state.T.gate(c, lx, canalY, open, flagOf(i)); tap(lx - 7, canalY - 28, 32, H - canalY + 28, { kind: 'stop', i });
    s.issues.filter(n => !hid.has(n) && !onShip.has(n) && !carried.has(n)).slice(0, 4).forEach((n, k) => {
      const x = lx + 8 + (k % 2) * 6, y = canalY + 18 + Math.floor(k / 2) * 6;
      state.T.crate(c, x, y, 'workshop', null, true); tap(x, y, 5, 5, { kind: 'crate', n, at: 'stop', i });
    });
  });
  // Crates floating down to a lock's basin while its gate stands open.
  state.floats = state.floats.filter(f => {
    const pos = porterPos({ t0: f.t0, speed: 0.03, pts: f.pts }, t);
    if (!pos) return false;
    state.T.crate(c, pos[0] - 2, pos[1] - 2, 'workshop', null, true);
    return true;
  });
  state.T.destination(c, lay.castleX, canalY, t, state.fireworks); tap(lay.castleX - 4, canalY - 28, 46, H - canalY + 28, { kind: 'harbour' });
  // The ship in the harbour, loaded with the next release; or the last one
  // sailing off with what shipped, leaving a wake, then an empty one coming in.
  if (stops.length) {
    const at = shipAt(state.S, t, lay), bob = state.T.flies || state.reduce || !blink(t, 900) ? 0 : 1, oy = at.oy + bob;
    if (at.leaving && state.T.flies) puffs(c, lay.castleX + 18, canalY - 12, t, ...state.T.steam);
    else if (at.leaving) {
      for (let x = lay.castleX + 4; x < lay.castleX + at.ox; x += 5) R(c, x, canalY + 12 + ((x >> 2) % 2), 3, 1, P.white);
      tap(lay.castleX + 4, canalY + 8, Math.max(0, at.ox - 4), 8, { kind: 'harbour' });
    }
    const load = at.leaving ? state.S.voyage.cargo : cargo.filter(n => !carried.has(n) && !hid.has(n));
    state.T.vessel(c, lay.castleX, canalY, t, at.ox, oy);
    const [hx, hy, hw, hh] = state.T.hull;
    tap(lay.castleX + hx + at.ox, canalY + hy + at.oy, hw, hh, at.leaving ? { kind: 'ship', at: 'away' } : { kind: 'ship' });
    load.slice(0, SHIP_CAP).forEach((n, k) => {
      const [x, y] = holdPos(lay, k, at.ox, oy);
      state.T.crate(c, x, y, 'workshop', null, true);
      if (!at.leaving) tap(x, y, 5, 5, { kind: 'crate', n, at: 'ship' });
    });
    if (load.length > SHIP_CAP) badge(c, Math.round(lay.castleX + state.T.holdBadge[0] + at.ox), Math.round(canalY + state.T.holdBadge[1] + oy), load.length - SHIP_CAP);
  }

  // Trips: whoever is walking, with the crate carried overhead, or still
  // waiting where it will be picked up.
  state.porters = state.porters.filter(p => {
    const a = at.get(p);
    if (!a) return false;
    if (!a.fetched && p.issue && !p.crew) {
      const [px, py] = pickup(p);
      state.T.crate(c, px - 2, py - 6, p.kind, p.pr); tap(px - 2, py - 6, 5, 5, { kind: 'crate', n: p.issue, at: 'transit', to: p.to });
    }
    if (a.waiting) return true;
    person(c, a.x - 4, a.y - 11, p.seed, a.carry ? (blink(t, 140) ? 'carrystep' : 'carry') : 'stand');
    tap(a.x - 4, a.y - 11, 8, 12, { kind: 'porter', seed: p.seed, n: p.issue, to: p.to, carrying: a.carry });
    if (a.carry) { state.T.crate(c, a.x - 2, a.y - 16, p.kind, p.pr, p.sealed); tap(a.x - 2, a.y - 16, 5, 5, { kind: 'crate', n: p.issue, at: 'transit', to: p.to }); }
    return true;
  });
  bubbles.forEach(([x, y, s]) => bubble(c, x, y, s));
  featureLayer(c, t, v, lay, barges);
}

/* ---------- Features ----------
 * A feature (a parent issue split into subtasks) is a banner in the strip
 * cleared east of the houses, with its spec scroll at its foot. Following
 * one draws a marching trail to each counted subtask. In whole-feature
 * projects a feature's subtasks ride one barge (a truck downtown) once
 * they are all at the same stop; an environment nobody can answer for is
 * in fog. The server works out every count; the renderer only draws them. */
let followed = null; // { project, number }
const isFollowed = (v, f) => !!followed && followed.project === v.project && followed.number === f.number;
const counted = f => (f.subtasks || []).filter(s => !s.dropped);
function inBannerStrip(lay, x, y) {
  const b = lay.banners;
  return b.length > 0 && y < 48 && x >= b[0].x - 14 && x <= b[b.length - 1].x + 22;
}
/* Where a subtask is on the map: -1 the quay, i stop i, null elsewhere. */
function placeOf(n, v) {
  if (v.pipeline.quay.includes(n)) return -1;
  const i = v.pipeline.stops.findIndex(s => s.issues.includes(n));
  return i >= 0 ? i : null;
}
function bargesFor(v, lay) {
  const last = v.pipeline.stops.length - 1, queued = {};
  return lay.banners.map(g => {
    if (g.f.promotion !== 'feature') return null;
    const ns = counted(g.f).map(s => s.number), at = ns.map(n => placeOf(n, v));
    if (!ns.length || at.some(a => a === null || a !== at[0])) return null;
    // Barges at the same stop queue up: off the quay they line up downstream,
    // at a lock or the harbour they wait upstream of it.
    const i = at[0], k = queued[i] = (queued[i] ?? -1) + 1;
    const x = i === -1 ? lay.roadX - 14 + k * 26
      : Math.max(lay.roadX - 14, (i === last ? lay.castleX - 26 : lay.lockStart + i * 34 + 8) - k * 26);
    return { f: g.f, col: g.col, issues: ns, stop: i, x, y: lay.canalY + 1 };
  }).filter(Boolean);
}
/* The map point a trail leads to for subtask n: its house, its crate, or its barge. */
function whereIs(n, v, lay, barges) {
  const hi = v.houses.findIndex(h => h.issue === n);
  if (hi >= 0) { const lot = lay.lots[hi]; if (lot) return { x: lot.x + 8, y: lot.y + 18 }; }
  const bi = v.backlog.findIndex(c => c.number === n);
  if (bi >= 0) { const s = slotPos(Math.min(bi, 39)); return { x: s.x + 2, y: s.y + 2 }; }
  const ni = v.board.findIndex(b => b.number === n);
  if (ni >= 0) { const k = Math.min(ni, 7); return { x: lay.hallX + 45 + (k % 4) * 4, y: 31 + Math.floor(k / 4) * 4 }; }
  const b = barges.find(b => b.issues.includes(n));
  if (b) { const k = Math.min(b.issues.indexOf(n), 3); return { x: b.x + 4 + k * 5, y: b.y + 3 }; }
  const hid = new Set(barges.flatMap(b => b.issues));
  const ci = cargoOf(v).filter(m => !hid.has(m)).indexOf(n);
  if (ci >= 0) { const [x, y] = holdPos(lay, Math.min(ci, SHIP_CAP - 1)); return { x: x + 2, y: y + 2 }; }
  const qi = v.pipeline.quay.filter(m => !hid.has(m)).indexOf(n);
  if (qi >= 0) { const k = Math.min(qi, 5); return { x: lay.roadX + 8 + (k % 3) * 6, y: lay.quayY - 1 - Math.floor(k / 3) * 6 }; }
  const stops = v.pipeline.stops;
  for (let i = 0; i < stops.length; i++) {
    let k = stops[i].issues.filter(m => !hid.has(m)).indexOf(n);
    if (k < 0) continue;
    // Shipped work has sailed: the trail ends at the harbour's lighthouse.
    return i < stops.length - 1
      ? (k = Math.min(k, 3), { x: lay.lockStart + i * 34 + 10 + (k % 2) * 6, y: lay.canalY + 20 + Math.floor(k / 2) * 6 })
      : { x: lay.castleX + 31, y: lay.canalY - 24 };
  }
  return null;
}
/* A barge on the canal, or a flatbed truck on the district's road. */
function drawBarge(c, b, t) {
  const x = b.x, y = b.y + (!state.reduce && blink(t, 700) ? 1 : 0);
  if (state.T === FUTURE) {
    // A hover-barge: hull, cockpit, a glow under it.
    R(c, x, y + 6, 24, 4, P.o); R(c, x + 1, y + 6, 22, 2, F.hull); R(c, x + 1, y + 6, 22, 1, b.col);
    R(c, x + 18, y + 1, 6, 6, P.o); R(c, x + 19, y + 2, 4, 3, F.glass);
    R(c, x + 2, y + 10, 20, 1, !state.reduce && blink(t, 200) ? F.cyanL : F.cyan);
  } else if (state.T === MODERN) {
    R(c, x, y + 6, 20, 4, P.o); R(c, x + 1, y + 6, 18, 2, M.steel); R(c, x + 1, y + 6, 18, 1, b.col);
    R(c, x + 19, y + 1, 6, 9, P.o); R(c, x + 20, y + 2, 4, 6, P.white); R(c, x + 21, y + 3, 3, 2, M.glassL);
    [x + 3, x + 14, x + 21].forEach(wx => { R(c, wx, y + 9, 3, 3, P.o); R(c, wx + 1, y + 10, 1, 1, M.steel); });
  } else {
    R(c, x, y + 6, 24, 5, P.o); R(c, x + 1, y + 6, 22, 3, P.wood); R(c, x + 1, y + 6, 22, 1, P.woodL); R(c, x + 3, y + 9, 18, 1, P.woodD);
    R(c, x + 22, y - 6, 1, 8, P.o); R(c, x + 23, y - 6, 5, 4, P.o); R(c, x + 23, y - 5, 4, 2, b.col);
  }
  b.issues.slice(0, 4).forEach((n, k) => state.T.crate(c, x + 2 + k * 5, y + 1, 'workshop', null, true));
  tap(b.x, b.y - 6, 28, 18, { kind: 'barge', f: b.f ? b.f.number : null, issues: b.issues });
}
/* The fog wash per theme, shared with the key. */
const fogCol = th => th === MODERN ? 'rgba(170,176,196,.75)' : th === FUTURE ? 'rgba(60,52,110,.78)' : 'rgba(206,210,230,.72)';
/* Fog over a stop with neither a Deployment nor a branch: "?", never "not live". */
function fogStop(c, v, i, lay, t) {
  const last = v.pipeline.stops.length - 1, x = i === last ? lay.castleX - 4 : lay.lockStart + i * 34 - 7, w = i === last ? 46 : 32;
  R(c, x, lay.canalY - 28, w, lay.H - lay.canalY + 28, fogCol(state.T));
  if (!state.reduce) for (let k = 0; k < w / 6 - 1; k++) puffs(c, x + 4 + k * 6, lay.canalY - 18 + (k % 2) * 22, t + k * 330, '#e0e4f0', '#b8c0d8');
  bubble(c, x + (w >> 1) - 4, lay.canalY - 6, 'unknown');
}
/* A flag on a pole: swallowtail cloth in the feature's colour, a red pennant
 * when a subtask is stuck, gold and sparkling once live. The spec lies at
 * its foot: a scroll in the village, a clipboard downtown. */
function drawBanner(c, g, on, t) {
  const { x, y, f } = g, col = f.live ? P.gold : g.col, wave = !state.reduce && blink(t + f.number * 7, 320) ? 1 : 0;
  const stuck = (f.stuck || []).length > 0, top = stuck ? y - 7 : y - 1, modern = state.T !== MEDIEVAL;
  R(c, x - 2, y + 23, 5, 2, P.o); R(c, x - 1, y + 23, 3, 1, modern ? M.concrete : P.stoneD);
  R(c, x, top, 1, y + 23 - top, modern ? M.steelD : P.o); R(c, x, top - 1, 1, 1, P.gold);
  R(c, x + 1, y, 14, 10, P.o); R(c, x + 1, y + 1, 13, 8, col); R(c, x + 1, y + 1, 13, 1, P.white);
  if (!modern) R(c, x + 12, y + 4 + wave, 3, 2, P.o);
  R(c, x + 4, y + 4, 6, 1, f.live ? P.o : P.white); R(c, x + 4, y + 6, 4, 1, f.live ? P.o : P.white);
  if (stuck) { const w = !state.reduce && blink(t, 260) ? 1 : 0; R(c, x + 1, y - 7, 7 + w, 4, P.o); R(c, x + 1, y - 6, 6 + w, 2, P.p0); }
  if (modern) { R(c, x + 4, y + 18, 7, 7, P.o); R(c, x + 5, y + 19, 5, 5, P.white); R(c, x + 6, y + 18, 3, 1, M.steel); R(c, x + 6, y + 21, 3, 1, M.concreteD); R(c, x + 6, y + 23, 2, 1, M.concreteD); }
  else { R(c, x + 3, y + 19, 8, 5, P.o); R(c, x + 4, y + 20, 6, 3, P.parch); R(c, x + 5, y + 21, 4, 1, P.pathS); }
  if (f.live && !state.reduce) for (let k = 0; k < 3; k++) if (blink(t + k * 170, 220)) R(c, x + 3 + k * 5, y - 3 - (k % 2) * 2, 1, 1, P.gold);
  if (on) { const o = !state.reduce && blink(t, 400) ? col : P.white; R(c, x - 3, y - 9, 20, 1, o); R(c, x - 3, y + 26, 20, 1, o); R(c, x - 3, y - 9, 1, 36, o); R(c, x + 16, y - 9, 1, 36, o); }
}
function drawTrail(c, x0, y0, x1, y1, col, t) {
  const off = state.reduce ? 0 : Math.floor(t / 110) % 4, pts = [];
  x1 = Math.round(x1); y1 = Math.round(y1);
  for (let y = y0; y !== y1; y += Math.sign(y1 - y0)) pts.push([x0, y]);
  for (let x = x0; x !== x1; x += Math.sign(x1 - x0)) pts.push([x, y1]);
  pts.forEach(([px, py], i) => { if ((i + off) % 4 < 2) { R(c, px, py + 1, 1, 1, P.o); R(c, px, py, 1, 1, col); } });
  const o = !state.reduce && blink(t, 300) ? col : P.white;
  R(c, x1 - 4, y1 - 4, 9, 1, o); R(c, x1 - 4, y1 + 4, 9, 1, o); R(c, x1 - 4, y1 - 4, 1, 9, o); R(c, x1 + 4, y1 - 4, 1, 9, o);
}
function featureLayer(c, t, v, lay, barges) {
  if (!lay.banners.length && !v.pipeline.stops.some(s => s.source === 'unknown')) return;
  barges.forEach(b => drawBarge(c, b, t));
  v.pipeline.stops.forEach((s, i) => { if (s.source === 'unknown') fogStop(c, v, i, lay, t); });
  const fade = (scenes[v.project] || {}).fade || {};
  lay.banners.forEach(g => {
    const on = isFollowed(v, g.f), a = on ? 1 : (fade[g.f.number] ?? 1);
    if (on) counted(g.f).forEach(s => { const w = whereIs(s.number, v, lay, barges); if (w) drawTrail(c, g.x, g.y + 25, w.x, w.y, g.f.live ? P.gold : g.col, t); });
    c.globalAlpha = a; drawBanner(c, g, on, t); c.globalAlpha = 1;
    tap(g.x - 3, g.y - 9, 20, 36, { kind: 'banner', f: g.f.number });
  });
}

function drawVillage(c, sc, t) {
  state.current = sc.v; state.L = sc.L; state.porters = sc.porters; state.floats = sc.floats; state.fireworks = sc.fireworks; state.S = sc;
  hits = [];
  paint(c, t);
  sc.hits = hits; hits = null;
  sc.porters = state.porters; sc.floats = state.floats;
}

/* A live banner has done its job: it fades over the first hour after this
 * page saw it go live and is gone after LIVE_GONE, unless it is being
 * followed. (The server also stops listing a feature a day after it went live.) */
const LIVE_FADE = 10 * 60e3, LIVE_DIM = 60 * 60e3, LIVE_GONE = 2 * 60 * 60e3;
function fadeLive(sc, v) {
  const now = Date.now(), seen = {};
  sc.fade = {};
  v.features = (v.features || []).filter(f => {
    if (!f.live) return true;
    const since = seen[f.number] = sc.liveSince[f.number] ?? now, age = now - since;
    if (age > LIVE_GONE && !isFollowed(v, f)) return false;
    sc.fade[f.number] = age <= LIVE_FADE ? 1 : Math.max(0.35, 1 - 0.65 * (age - LIVE_FADE) / (LIVE_DIM - LIVE_FADE));
    return true;
  });
  sc.liveSince = seen;
}
/* Fold a new model into its village's scene, starting the animations its
 * changes call for. */
function updateScene(v, t) {
  const sc = scenes[v.project] || (scenes[v.project] = { v: null, porters: [], floats: [], gates: {}, voyage: null, supplies: [], nextAt: 0, fireworks: -1e9,
    lastDelivered: null, lastRelease: null, liveSince: {}, fade: {} });
  fadeLive(sc, v);
  const lay = layout(v);
  state.porters = sc.porters; state.floats = sc.floats;
  const said = diff(sc.v, v, lay, t, sc);
  sc.porters = state.porters; sc.floats = state.floats;
  // Reduced motion: nobody walks, so say what changed instead.
  if (state.reduce && said.length && shownScenes().some(([x]) => x === sc)) toast(`${v.project}: ${said.slice(0, 4).join(' · ')}${said.length > 4 ? ` · +${said.length - 4} more` : ''}`);
  // A new deployment at the last stop sails the ship. A village with no
  // release to compare (a runtime's stages) still lights up when work arrives.
  const rel = releaseOf(v), delivered = v.pipeline.stops.length ? v.pipeline.stops[v.pipeline.stops.length - 1].issues.join(',') : '';
  const was = sc.lastRelease, sameSource = rel && was && rel.split(':')[0] === was.split(':')[0];
  if (rel) { if (sc.v && sameSource && rel !== was) sail(sc, v, t); sc.lastRelease = rel; }
  else if (!was && sc.lastDelivered !== null && sc.v && delivered !== sc.lastDelivered && delivered) sc.fireworks = t;
  sc.lastDelivered = delivered;
  sc.v = v; sc.L = lay;
  return sc;
}

/* ---------- Worlds ----------
 * A world stacks the platform village (what runs) over the dev village
 * (what builds it). Issues the platform raises leave the building they
 * came from and float down the river on the left to the dev village's
 * docks; releases leave the dev village's harbour and climb the road on
 * the right to the platform, until its workers run them. The platform is
 * drawn in the other style, so the two read apart at a glance. */
const SIDE = 24, GAP = 28;
const otherTheme = () => THEMES[state.T.pair] || THEMES.medieval;
function worldGeom(Ps, Ds) {
  const W = Math.max(Ps.L.W, Ds.L.W) + 2 * SIDE, devTop = Ps.L.H + GAP;
  return { W, H: devTop + Ds.L.H, devTop, srcY: Ps.L.top, roadY: 53 };
}
function offscreen(sc) {
  if (!sc.canvas) { sc.canvas = document.createElement('canvas'); sc.cx = sc.canvas.getContext('2d'); }
  if (sc.canvas.width !== sc.L.W || sc.canvas.height !== sc.L.H) { sc.canvas.width = sc.L.W; sc.canvas.height = sc.L.H; }
  return sc.cx;
}
/* A channel that flows down: water in the village, a conveyor road in the district. */
function downstream(c, x, y, w, h, t) {
  const modern = state.T !== MEDIEVAL, fut = state.T === FUTURE;
  R(c, x, y, w, h, fut ? F.space : modern ? M.asphaltD : P.water);
  const edge = fut ? F.magentaD : modern ? M.concreteD : P.waterD;
  R(c, x, y, 1, h, edge); R(c, x + w - 1, y, 1, h, edge);
  const off = Math.floor(t / 70) % 8;
  for (let yy = y - 8 + off; yy < y + h; yy += 8) if (yy >= y) {
    if (modern) R(c, x + (w >> 1), yy, 1, 4, fut ? F.plasmaL : P.white);
    else { R(c, x + 3, yy, 1, 2, P.waterL); R(c, x + w - 5, yy + 4, 1, 2, P.waterL); }
  }
}
function cart(c, x, y, loads) {
  const modern = state.T !== MEDIEVAL;
  x = Math.round(x); y = Math.round(y);
  R(c, x, y, 14, 8, P.o); R(c, x + 1, y + 1, 12, 5, modern ? P.white : P.wood);
  if (modern) { R(c, x + 10, y - 3, 4, 4, P.o); R(c, x + 11, y - 2, 2, 2, M.glassL); }
  R(c, x + 2, y + 7, 3, 3, P.o); R(c, x + 9, y + 7, 3, 3, P.o);
  for (let k = 0; k < Math.min(loads, modern ? 1 : 2); k++) state.T.crate(c, x + 2 + k * 5, y - 4, 'workshop', null, true);
}
function updateWorld(w, t) {
  const st = worldState[w.name] || (worldState[w.name] = { seen: null, floaters: [], landed: undefined, upgradedAt: -1e9 });
  const now = new Set(w.incidents.map(i => i.number));
  if (st.seen) w.incidents.forEach(i => { if (!st.seen.has(i.number)) st.floaters.push({ n: i.number, origin: i.origin, t0: t }); });
  st.seen = now;
  if (st.landed === false && w.landed === true) {
    st.upgradedAt = t;
    if (scenes[w.platform]) scenes[w.platform].fireworks = t;
  }
  st.landed = w.landed;
}
function originPoint(Ps, id) {
  const i = Ps.v.houses.findIndex(h => h.id === id), lot = i >= 0 ? Ps.L.lots[i] : null;
  return lot ? [SIDE + lot.x + 8, lot.y + 8] : [12, Ps.L.top - 6];
}
function drawWorld(t, w) {
  const Ps = scenes[w.platform], Ds = scenes[w.dev];
  if (!Ps || !Ps.v || !Ds || !Ds.v) return;
  const g = worldGeom(Ps, Ds), st = worldState[w.name] || { floaters: [], upgradedAt: -1e9 };
  if (cv.width !== g.W || cv.height !== g.H) { cv.width = g.W; cv.height = g.H; fit(); }
  const devTheme = state.T;
  state.T.ground(ctx, 0, 0, g.W, g.H);

  state.T = otherTheme();
  state.T.ground(ctx, 0, 0, g.W, Ps.L.H);
  drawVillage(offscreen(Ps), Ps, t);
  ctx.drawImage(Ps.canvas, SIDE, 0);
  // Platform buildings with an open incident against them are on fire.
  w.damaged.forEach(id => {
    const i = Ps.v.houses.findIndex(h => h.id === id), lot = Ps.L.lots[i];
    if (!lot) return;
    const x = SIDE + lot.x, y = lot.y;
    R(ctx, x, y - 1, 16, 17, 'rgba(40,24,24,.35)');
    for (let k = 0; k < 3; k++) {
      puffs(ctx, x + 4 + k * 4, y - 6, t + k * 450, '#484858', '#282830');
      puffs(ctx, x + 5 + k * 4, y - 12, t + k * 450 + 900, '#585870', '#383848');
    }
    const f = blink(t, 140) ? 1 : 0;
    R(ctx, x + 4, y + 2 - f, 3, 3 + f, P.p0); R(ctx, x + 5, y + 1 - f, 1, 2, P.gold);
    R(ctx, x + 10, y + 3 + f, 3, 3 - f, P.p0); R(ctx, x + 11, y + 2 + f, 1, 2, P.gold);
  });
  if (t - st.upgradedAt < 2400) Ps.v.houses.forEach((h, i) => {
    const lot = Ps.L.lots[i]; if (lot && blink(t + i * 90, 160)) R(ctx, SIDE + lot.x + 2 + (i * 5) % 12, lot.y - 2, 1, 1, P.gold);
  });
  state.T = devTheme;

  drawVillage(offscreen(Ds), Ds, t);
  ctx.drawImage(Ds.canvas, SIDE, g.devTop);

  // The river down: from the platform's gate to the dev village's river.
  downstream(ctx, 5, g.srcY, 14, g.devTop + 11 - g.srcY, t);
  R(ctx, 2, g.srcY - 8, 20, 8, P.o); R(ctx, 3, g.srcY - 7, 18, 6, state.T === MODERN ? M.concrete : state.T === FUTURE ? F.hullD : P.stone); R(ctx, 9, g.srcY - 6, 6, 5, P.dark);
  // The road up: from the dev village's harbour to the platform's main road.
  const ry = g.devTop + Ds.L.canalY + 4;
  state.T.road(ctx, g.W - 20, g.roadY - 3, 16, ry - g.roadY + 9);
  state.T.road(ctx, SIDE + Ds.L.castleX + 36, ry, g.W - 20 - (SIDE + Ds.L.castleX + 36), 6);
  const roadEnd = SIDE + Ps.L.cx0 + Ps.L.vw - 4;
  state.T.road(ctx, roadEnd, g.roadY - 3, g.W - 20 - roadEnd, 6);

  // Released work on its way up, until the platform's workers run it.
  // The cart takes over from the ship: it sets off from the harbour as the
  // ship sails off the dev village's edge, and none while the ship is still in sight.
  const sailed = Ds.voyage ? Ds.voyage.t0 + SAIL : 0;
  if (w.shipping.length && t >= sailed) {
    const pts = [[SIDE + Ds.L.castleX + 34, ry], [g.W - 12, ry], [g.W - 12, g.roadY], [roadEnd, g.roadY]];
    const len = pts.slice(1).reduce((n, p, k) => n + Math.hypot(p[0] - pts[k][0], p[1] - pts[k][1]), 0);
    const d = state.reduce ? len / 2 : ((t - sailed) / 30) % len;
    const at = porterPos({ t0: 0, speed: 1, pts }, d) || pts[0];
    cart(ctx, at[0] - 7, at[1] - 6, w.shipping.length);
  }
  // New incidents leave their building and float down to the docks.
  st.floaters = st.floaters.filter(f => {
    const [ox, oy] = originPoint(Ps, f.origin);
    const pts = [[ox, oy], [12, oy], [12, g.devTop + 6], [SIDE + 30, g.devTop + 6]];
    const at = porterPos({ t0: f.t0, speed: 0.04, pts }, t);
    if (!at) return false;
    state.T.crate(ctx, at[0] - 2, at[1] - 2, 'bug', 'P0');
    return true;
  });
}

/* ---------- Panels ---------- */
function renderFlow(v) {
  if (v.plane === 'runtime') return renderRuntimeFlow(v);
  const stops = v.pipeline.stops;
  const working = v.houses.filter(h => h.status === 'working').length;
  const items = [
    [state.T.labels.docks, 'backlog', v.backlog.length],
    [state.T.labels.board, 'blocked · proposals', v.board.length],
    [state.T.labels.houses, `${working} of ${v.houses.length} working`, v.houses.filter(h => h.issue).length],
    ['Sub-agents', 'running in sessions', [v.manager, ...v.houses].reduce((n, h) => n + (h.subagents || []).length, 0)],
    [state.T.labels.quay, `pushed to ${v.pipeline.integration || 'integration'}`, v.pipeline.quay.length],
    ...stops.map((s, i) => [i === stops.length - 1 ? `${state.T.labels.castle} · ${s.name}` : s.name, i === 0 && v.pipeline.sentinel !== 'absent' ? `sentinel: ${v.pipeline.sentinel}` : 'environment', s.issues.length])
  ];
  document.getElementById('flow').innerHTML = items.map(([l, s, n], i) =>
    `<div class="step"><b>${n}</b><span>${esc(l)}<br>${esc(s)}</span></div>`).join('');
}
/* Runtime villages (Temporal, AgentEx, ...) count workstreams and agents, not issues and pushes. */
function renderRuntimeFlow(v) {
  const count = st => v.houses.filter(h => h.status === st).length;
  const shipped = v.pipeline.stops.reduce((n, s) => n + s.issues.length, 0);
  const items = [
    [state.T.labels.houses, 'top-level workstreams', v.houses.length],
    ['Working', 'workstreams busy', count('working')],
    ['Waiting on you', 'an agent asked a person', count('blocked')],
    ['Retrying', 'activity failing', count('stalled')],
    ['Agents', 'working for workstreams', v.houses.reduce((n, h) => n + (h.subagents || []).length, 0)],
    ['Workers', v.manager.status === 'stalled' ? 'a task queue has none' : 'polling task queues', (v.manager.subagents || []).length],
    [state.T.labels.board, 'failed in 24h', v.board.filter(n => n.label === 'failed').length],
    [state.T.labels.castle, 'completed in 24h', shipped]
  ];
  document.getElementById('flow').innerHTML = items.map(([l, s, n]) =>
    `<div class="step"><b>${n}</b><span>${esc(l)}<br>${esc(s)}</span></div>`).join('');
}
/* World counters follow the loop: raised, being fixed, released, running. */
function renderWorldFlow(w) {
  const Ds = scenes[w.dev], Ps = scenes[w.platform];
  const open = w.incidents.filter(i => !i.in_progress).length, fixing = w.incidents.length - open;
  const short = s => (s ? s.slice(0, 7) : '—');
  const landed = w.landed === null ? 'platform reports no build IDs' : w.landed ? 'running on the platform' : 'rolling out';
  const items = [
    ['Incidents', 'raised by the platform', open],
    ['Being fixed', `claimed in ${w.dev}`, fixing],
    ['Dev village', 'houses working', Ds && Ds.v ? Ds.v.houses.filter(h => h.status === 'working').length : 0],
    ['Shipping', 'released, not yet running', w.shipping.length],
    ['Release', landed, short(w.release)],
    ['Platform', 'workstreams busy', Ps && Ps.v ? Ps.v.houses.filter(h => h.status === 'working').length : 0],
    ['Platform builds', 'reported by workers', w.platform_builds.map(short).join(' ') || '—']
  ];
  document.getElementById('flow').innerHTML = items.map(([l, s, n]) =>
    `<div class="step"><b>${esc(n)}</b><span>${esc(l)}<br>${esc(s)}</span></div>`).join('');
}
const esc = s => String(s).replace(/[&<>"]/g, ch => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[ch]));

const dialog = document.getElementById('dialog');
document.getElementById('close').addEventListener('click', () => { dialog.hidden = true; });
/* Drill in: each link opens the tool behind the picture (the agent's session,
 * its issue, its workflow) in a new tab. */
function links(list) {
  document.getElementById('d-links').innerHTML = (list || [])
    .map(l => `<a href="${esc(l.url)}" target="_blank" rel="noopener">${esc(l.label)} ↗</a>`).join('');
}
/* A structured question the agent is waiting on (a numbered menu or y/n):
 * one button per option. The server checks the same question is still on
 * screen before it types the answer; anything open-ended goes through the
 * agent's session link instead. */
function ask(house) {
  const box = document.getElementById('d-q'), q = house.question;
  if (!q || !house.answer_url) { box.innerHTML = ''; return; }
  box.innerHTML = `<p class="q">${esc(q.prompt)}</p><div class="opts">${q.options.map((o, i) =>
      `<button data-opt="${i}">${q.kind === 'menu' ? esc(o.key) + '. ' : ''}${esc(o.label)}</button>`).join('')}</div><p class="dim" id="d-q-msg" role="status"></p>`;
  const buttons = [...box.querySelectorAll('[data-opt]')], msg = document.getElementById('d-q-msg');
  const busy = on => buttons.forEach(b => { b.disabled = on; });
  buttons.forEach(b => b.addEventListener('click', async () => {
    busy(true); msg.textContent = 'Sending…';
    try {
      const res = await fetch(house.answer_url, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ prompt: q.prompt, option: Number(b.dataset.opt) }) });
      const body = await res.json().catch(() => ({}));
      if (res.ok) { msg.textContent = `Answered: ${body.answered || b.textContent}`; setTimeout(() => { dialog.hidden = true; }, 1200); return; }
      msg.textContent = res.status === 403 ? 'This dashboard is read-only: answer in the agent session.' : (body.error || `Could not answer (${res.status})`);
    } catch (e) { msg.textContent = "Can't reach agents-tui."; }
    busy(false);
  }));
}
/* ---------- What each object is ----------
 * One dialog per kind of thing on the map: what it is in plain words, and
 * links to what is behind it. Links come from the report (house `links`,
 * issue `url`s); the repo's own pages (commits, deployments, releases) only
 * when a real issue URL shows which GitHub repo this is, never a guess. */
const ISSUE_URL = /^(https:\/\/github\.com\/[^/]+\/[^/]+)\/issues\/\d+$/;
function repoUrl(v) {
  const urls = [...v.backlog.map(c => c.url), ...v.board.map(n => n.url), ...(v.features || []).map(f => f.url),
    ...[v.manager, ...v.houses].flatMap(h => (h.links || []).map(l => l.url))];
  for (const u of urls) { const m = ISSUE_URL.exec(u || ''); if (m) return m[1]; }
  return null;
}
const link = (label, url) => (url ? { label, url } : null);
/* An issue's title, priority and URL from wherever the village knows it. */
function issueOf(v, n) {
  const c = v.backlog.find(c => c.number === n) || v.board.find(b => b.number === n);
  const h = v.houses.find(h => h.issue === n);
  const sub = (v.features || []).flatMap(f => f.subtasks || []).find(s => s.number === n);
  const own = h && (h.links || []).find(l => new RegExp(`/issues/${n}$`).test(l.url));
  const repo = repoUrl(v);
  return {
    title: (c && c.title) || (h && h.issue_title) || (sub && sub.title) || (v.pipeline.titles || {})[n] || '',
    priority: (c && c.priority) || (h && h.priority) || '',
    url: (c && c.url) || (own && own.url) || (repo ? `${repo}/issues/${n}` : null)
  };
}
const issueLabel = n => `GitHub issue #${n}`;
const issueItems = (v, ns) => ns.map(n => ({ number: n, ...issueOf(v, n) }));
const sessionOf = h => (h.links || []).find(l => /^\/#\/swarm\/[^/]+\/agent\//.test(l.url));
const isSha = s => /^[0-9a-f]{7,40}$/i.test(String(s || ''));
function stopLinks(v, s) {
  const repo = repoUrl(v);
  if (!repo || !s) return [];
  return [
    s.source === 'deployment' ? link(`${s.name} deployments`, `${repo}/deployments/${encodeURIComponent(s.name)}`) : null,
    isSha(s.sha) ? link(`Commit ${String(s.sha).slice(0, 7)}`, `${repo}/commit/${s.sha}`) : null
  ].filter(Boolean);
}
/* The GitHub Release a stop runs: the page its deployment links, else its tag's page. */
function releaseLink(v, s, words = 'Release') {
  if (!s || !s.tag && !s.url) return null;
  const repo = repoUrl(v), url = s.url || (repo ? `${repo}/releases/tag/${encodeURIComponent(s.tag)}` : null);
  return link(`${words}${s.tag ? ' ' + s.tag : ''}`, url);
}
/* Why nothing says what a stop runs, in plain words, and where the docs are. */
function unknownWords(v, i) {
  const st = v.pipeline.stops[i], br = v.pipeline.integration || 'the integration branch';
  if (i === 0 && v.pipeline.sentinel === 'unknown')
    return `No ${st.name} ref yet, so nothing has been verified green. Run autocoder's sentinel (main-sentinel.sh), or, with no sentinel, mark the first environment "requires": "integration" in .autocoder.json.`;
  return `Nothing says what ${st.name} runs: there is no successful GitHub Deployment to it and no env/${st.name} branch. Record a deployment (tools/deploy.sh records nuc-1), or configure the environment in .autocoder.json.`;
}
function docsLink(v) {
  const repo = repoUrl(v), br = v.pipeline.integration;
  return repo && br ? link('How environments work', `${repo}/blob/${encodeURIComponent(br)}/docs/village.md#environments`) : null;
}
/* "12:05 · 14m ago" from a house's `since`. */
function sinceWords(since) {
  const t = Date.parse(since || '');
  if (!isFinite(t)) return '';
  const m = Math.max(0, Math.round((Date.now() - t) / 60000));
  const ago = m < 1 ? 'just now' : m < 60 ? `${m}m` : `${Math.floor(m / 60)}h ${m % 60}m`;
  return `${new Date(t).toTimeString().slice(0, 5)} (${ago})`;
}
/* " · waits on #361, #362" for a backlog issue with open dependencies. */
function waitsWords(v, n) {
  const w = (v.backlog.find(c => c.number === n) || {}).waits_on || [];
  return w.length ? ` · waits on ${w.map(d => '#' + d).join(', ')}` : '';
}
function crateWhere(o, v, theme) {
  const stops = v.pipeline.stops;
  if (o.at === 'backlog') return `waiting at the ${theme.labels.docks.toLowerCase()}${waitsWords(v, o.n)}`;
  if (o.at === 'quay') return `landed on ${v.pipeline.integration || 'the integration branch'}, at the ${theme.labels.quay.toLowerCase()}`;
  if (o.at === 'ship') return `on the ${theme.labels.ship.toLowerCase()}, waiting for the next release`;
  if (o.at === 'stop') return o.i === stops.length - 1 ? `shipped to ${stops[o.i].name}` : `at ${stops[o.i].name}`;
  if (o.at === 'house') { const h = v.houses[o.i]; return `being worked on at ${h ? houseName(v, h) : 'its house'}`; }
  if (o.at === 'transit') return o.to === 'away' ? 'closed without landing: carried away' : `on its way to the ${tripPlace(o.to, theme)}`;
  return '';
}
/* Where a trip takes its crate, in the theme's words. */
function tripPlace(to, theme) {
  return { house: theme.labels.houses.toLowerCase().replace(/s$/, ''), backlog: theme.labels.docks.toLowerCase(), board: theme.labels.board.toLowerCase(),
    quay: theme.labels.quay.toLowerCase(), ship: theme.labels.ship.toLowerCase() }[to] || 'next place';
}
function subWords(sa, v) {
  return `${sa.agent_type}${sa.description ? ' · ' + sa.description : ''}` +
    (sa.status === 'blocked' ? ' · waiting on you' : sa.action ? ` · ${sa.action}` : '');
}
/* What an object on the map is: { title, line, meta (list items), items (issues), links, house }. */
function explain(o, sc, theme) {
  const v = sc.v, runtime = v.plane === 'runtime', stops = v.pipeline.stops, last = stops[stops.length - 1];
  const work = runtime ? 'workstream' : 'issue';
  switch (o.kind) {
    case 'river': case 'docks':
      return { title: theme.labels.docks, line: runtime ? 'Waiting work' : 'The backlog, highest priority first', items: v.backlog };
    case 'board':
      return { title: theme.labels.board, line: runtime ? 'Failed in the last 24 hours' : 'Blocked issues and proposals waiting on you', items: v.board };
    case 'hall': {
      const m = v.manager, s = sessionOf(m), swarm = s && s.url.replace(/\/agent\/[^/]+$/, '');
      return { title: `${theme.labels.hall} · manager`, line: m.issue ? `#${m.issue} ${m.issue_title || ''}` : (m.issue_title || 'The manager of this swarm'),
        meta: [`Status: ${m.status}`, `Crew: ${m.crew.join(', ') || '—'}`, ...(m.subagents || []).map(sa => `${runtime ? 'Worker' : 'Sub-agent'}: ${subWords(sa, v)}`), m.action ? `Now: ${m.action}` : null],
        links: [...(m.links || []), link('Swarm view', swarm)], house: m };
    }
    case 'house': {
      const h = v.houses[o.i]; if (!h) return null;
      const hasIssue = h.issue && (h.links || []).some(l => /\/issues\//.test(l.url));
      const issue = h.issue && !hasIssue ? link(issueLabel(h.issue), issueOf(v, h.issue).url) : null;
      // Named by worktree and branch ("wt-1 · feature/issue-7"), not by building.
      const repo = repoUrl(v), br = v.pipeline.integration, since = sinceWords(h.since);
      const compare = !runtime && repo && br && h.branch && h.branch !== br ? link(`Compare ${h.branch} with ${br}`, `${repo}/compare/${encodeURIComponent(br)}...${encodeURIComponent(h.branch)}`) : null;
      return { title: `${houseName(v, h)} · ${h.branch || h.kind}`, line: h.issue ? `#${h.issue} ${h.issue_title || ''}` : (h.issue_title || `No ${work} claimed`),
        meta: [`${runtime ? 'Workstream' : 'Worktree'}: ${h.id}`, h.branch ? `Branch: ${h.branch}` : null, `Agent: ${h.role}`,
          `Status: ${h.status}${h.action && h.action !== h.status ? ' · ' + h.action : ''}${h.priority ? ' · ' + h.priority : ''}`,
          since ? `${h.status.charAt(0).toUpperCase() + h.status.slice(1)} since ${since}` : null,
          `Crew: ${h.crew.join(', ') || '—'}`, ...(h.subagents || []).map(sa => `${runtime ? 'Agent' : 'Sub-agent'}: ${subWords(sa, v)}`)],
        links: [...(h.links || []), issue, compare], house: h };
    }
    case 'person': {
      const h = v.houses[o.i]; if (!h) return null;
      return { title: `${o.who || h.role} · agent`, line: h.issue ? `Working on #${h.issue} ${h.issue_title || ''}` : `Status: ${h.status}`,
        meta: [`Lives in ${houseName(v, h)}`, `Status: ${h.status}`, h.action ? `Now: ${h.action}` : null],
        links: [sessionOf(h), h.issue ? link(issueLabel(h.issue), issueOf(v, h.issue).url) : null] };
    }
    case 'sub': {
      const h = o.house < 0 ? v.manager : v.houses[o.house], sa = o.sa; if (!h) return null;
      return { title: `${sa.agent_type} · ${runtime ? (o.house < 0 ? 'worker' : 'agent') : 'sub-agent'}`, line: sa.description || '',
        meta: [`Working for ${o.house < 0 ? 'the manager' : houseName(v, h)}`, sa.status ? `Status: ${sa.status}` : null, sa.action ? `Now: ${sa.action}` : null],
        links: [sessionOf(h) && { label: 'Parent agent session', url: sessionOf(h).url }] };
    }
    case 'porter': {
      const h = v.houses.find(h => h.id === o.seed), n = o.n || (h && h.issue);
      const it = n ? issueOf(v, n) : {};
      return { title: `${h ? h.role : 'Porter'} · carrying`, line: n ? `#${n} ${it.title}` : '',
        meta: [!o.carrying && o.to === 'house' ? `Going to the ${theme.labels.docks.toLowerCase()} to fetch its issue`
          : !o.carrying ? 'Walking home'
          : o.to === 'ship' ? `Loading the finished work onto the ${theme.labels.ship.toLowerCase()}`
          : o.to === 'quay' ? `Taking the finished work to the ${theme.labels.quay.toLowerCase()}`
          : o.to === 'house' ? 'Carrying a claimed issue from the backlog to its house'
          : o.to === 'board' ? `Pinning a blocked issue to the ${theme.labels.board.toLowerCase()}`
          : o.to === 'backlog' ? `Bringing an issue to the ${theme.labels.docks.toLowerCase()}`
          : 'Carrying off an issue closed without landing'],
        links: [h && sessionOf(h), n ? link(issueLabel(n), it.url) : null] };
    }
    case 'supply':
      return { title: `Supply ${{ Ship: 'boat', Rocket: 'truck', Shuttle: 'shuttle' }[theme.labels.ship] || 'boat'}`, line: `Bringing ${o.n} new ${o.n === 1 ? work : work + 's'} to the ${theme.labels.docks.toLowerCase()}`,
        items: issueItems(v, o.ns || []), links: [] };
    case 'crate': {
      const it = issueOf(v, o.n);
      return { title: `#${o.n} ${it.title}`.trim(), line: crateWhere(o, v, theme),
        meta: [it.priority ? `Priority: ${it.priority}` : null], links: [link(issueLabel(o.n), it.url)] };
    }
    case 'quay': {
      const repo = repoUrl(v), br = v.pipeline.integration;
      return { title: theme.labels.quay, line: `Work landed on ${br || 'the integration branch'}, not yet past ${stops[0] ? stops[0].name : 'the gate'}`,
        items: issueItems(v, v.pipeline.quay), links: [repo && br ? link(`${br} commits`, `${repo}/commits/${encodeURIComponent(br)}`) : null] };
    }
    case 'stop': {
      const st = stops[o.i]; if (!st) return null;
      const sentinel = o.i === 0 && !runtime && v.pipeline.sentinel !== 'absent';
      const unknown = !runtime && (st.source === 'unknown' || (sentinel && v.pipeline.sentinel === 'unknown'));
      return { title: `${st.name} · ${runtime ? 'stage' : 'environment'}`, line: sentinel ? `Sentinel: ${v.pipeline.sentinel}` : sourceWords(st),
        meta: [sentinel ? `Runs: ${sourceWords(st)}` : null, o.i === 0 && v.pipeline.sentinel === 'absent' ? `No sentinel: takes work straight from ${v.pipeline.integration || 'the integration branch'}` : null,
          unknown ? unknownWords(v, o.i) : null],
        items: issueItems(v, st.issues), links: [...stopLinks(v, st), unknown ? docsLink(v) : null] };
    }
    case 'harbour': {
      const rel = v.pipeline.release, repo = repoUrl(v);
      const relLink = rel && repo ? link(`Release ${isSha(rel) ? rel.slice(0, 7) : rel}`, isSha(rel) ? `${repo}/commit/${rel}` : `${repo}/releases/tag/${encodeURIComponent(rel)}`) : null;
      return { title: `${theme.labels.castle}${last ? ' · ' + last.name : ''}`, line: last ? (runtime ? 'Completed in the last 24 hours' : `What shipped to ${last.name}`) : 'Nothing ships yet',
        meta: [last && last.tag ? `Last release: ${last.tag}` : rel ? `Release: ${rel}` : null, last && !runtime ? `Runs: ${sourceWords(last)}` : null,
          last && !runtime && last.source === 'unknown' ? unknownWords(v, stops.length - 1) : null],
        items: last ? issueItems(v, last.issues) : [], links: [releaseLink(v, last), relLink, ...stopLinks(v, last), last && !runtime && last.source === 'unknown' ? docsLink(v) : null] };
    }
    case 'ship': {
      if (!last) return null;
      const name = releaseName(v, last);
      if (o.at === 'away') {
        const gone = (sc.voyage || {}).cargo || [];
        return { title: `${theme.labels.ship} · ${theme.labels.sails}`, line: `Shipped ${name || 'a release'} to ${last.name}`, items: issueItems(v, gone), links: [releaseLink(v, last)] };
      }
      const cargo = cargoOf(v), n = cargo.length, repo = repoUrl(v), br = v.pipeline.integration;
      return { title: `${theme.labels.ship} · next release`, line: `Next release: ${n} ${n === 1 ? work : work + 's'}${name ? ' since ' + name : ''}`,
        meta: [`${theme.labels.sails.charAt(0).toUpperCase() + theme.labels.sails.slice(1)} when ${last.name} is next deployed`],
        items: issueItems(v, cargo),
        links: [releaseLink(v, last, 'Last release'), repo && last.tag && br ? link(`Changes since ${last.tag}`, `${repo}/compare/${encodeURIComponent(last.tag)}...${encodeURIComponent(br)}`) : null] };
    }
    case 'canal':
      return { title: runtime ? 'Pipeline' : 'Canal', line: `From ${v.pipeline.integration || 'integration'} through ${stops.map(s => s.name).join(', ') || 'no stages'}`,
        meta: stops.map(s => `${s.name}: ${s.issues.length} ${s.issues.length === 1 ? work : work + 's'}`), links: [] };
    case 'banner': case 'barge':
      return null;
  }
  return null;
}
/* The dialog for one object, or a list such as the board. */
function openInfo(info) {
  document.getElementById('d-title').textContent = info.title;
  document.getElementById('d-line').textContent = info.line || '';
  // Each fact on its own line, then the list: joined, they ran together
  // ("Runs: no Deployment and no branchNothing here.").
  const meta = (info.meta || []).filter(Boolean).map(m => `<span class="fact">${esc(m)}</span>`).join('');
  const items = info.items && (info.items.length
    ? `<ul>${info.items.map(i => `<li>${i.url ? `<a href="${esc(i.url)}" target="_blank" rel="noopener">#${i.number}</a>` : '#' + i.number} ${esc(i.title || '')}${i.label ? ' · ' + esc(i.label) : ''}${i.priority ? ' · ' + esc(i.priority) : ''}${(i.waits_on || []).length ? ' · waits on ' + i.waits_on.map(d => '#' + d).join(', ') : ''}</li>`).join('')}</ul>`
    : '<span class="fact">Nothing here.</span>');
  document.getElementById('d-meta').innerHTML = [meta, items].filter(Boolean).join('');
  ask(info.house || {});
  links((info.links || []).filter(Boolean));
  dialog.hidden = false;
}
/* Do what a click on `o` does; false when it does nothing (open ground). A
 * flag follows its feature (label and strip shown) or stops following it;
 * the strip's Details button opens the feature. */
function act(o, sc, theme) {
  const v = sc.v;
  if (o.kind === 'banner') { toggleFollow(v.project, o.f); return true; }
  if (o.kind === 'barge') { const f = (v.features || []).find(f => f.number === o.f); if (f) showFeature(f, v); return !!f; }
  const info = explain(o, sc, theme);
  if (info) openInfo(info);
  return !!info;
}
/* The topmost drawn object under a point, in scene pixels. */
function hitAt(sc, x, y) {
  const hs = sc.hits || [];
  for (let k = hs.length - 1; k >= 0; k--) { const r = hs[k]; if (x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h) return r.o; }
  return null;
}
/* What is under a pointer event: the object, its scene and theme; or null. */
function objAt(e) {
  if (!view) return null;
  const r = cv.getBoundingClientRect();
  const x = (e.clientX - r.left) / r.width * cv.width, y = (e.clientY - r.top) / r.height * cv.height;
  if (!view.world) { const sc = scenes[view.village], o = sc && sc.v && hitAt(sc, x, y); return o ? { o, sc, theme: state.T } : null; }
  const w = worlds.find(x => x.name === view.world), Ps = w && scenes[w.platform], Ds = w && scenes[w.dev];
  if (!Ps || !Ds || !Ps.v || !Ds.v) return null;
  const g = worldGeom(Ps, Ds);
  const [sc, theme, top] = y < Ps.L.H ? [Ps, otherTheme(), 0] : y >= g.devTop ? [Ds, state.T, g.devTop] : [];
  const o = sc && hitAt(sc, x - SIDE, y - top);
  return o ? { o, sc, theme } : null;
}
cv.addEventListener('click', e => {
  if (!view) return;
  const h = objAt(e);
  if (!(h && act(h.o, h.sc, h.theme))) groundHint(e);
});
/* Hovering a flag shows its feature's label while the pointer is on it. */
cv.addEventListener('mousemove', e => {
  const h = objAt(e);
  peek(h && h.o.kind === 'banner' ? fbuttons.get(`${h.sc.v.project}#${h.o.f}`) : null);
});
cv.addEventListener('mouseleave', () => peek(null));
/* Open ground does nothing, so say what does, where the click was. */
const hintEl = document.getElementById('hint');
let hintTimer = 0;
function groundHint(e) {
  const vp = document.getElementById('viewport'), vr = vp.getBoundingClientRect();
  hintEl.hidden = false;
  const half = hintEl.offsetWidth / 2 + 4, y = e.clientY - vr.top;
  const x = Math.max(vp.scrollLeft + half, Math.min(vp.scrollLeft + vp.clientWidth - half, e.clientX - vr.left + vp.scrollLeft));
  hintEl.classList.toggle('below', y < 40);
  hintEl.style.left = x + 'px'; hintEl.style.top = y + vp.scrollTop + 'px';
  clearTimeout(hintTimer);
  hintTimer = setTimeout(() => { hintEl.hidden = true; }, 2500);
}
document.addEventListener('keydown', e => { if (e.key === 'Escape') dialog.hidden = true; });

/* ---------- Features: banner labels, strip, dialog ---------- */
const fbox = document.getElementById('fbanners'), fstrip = document.getElementById('fstrip');
const STAGE_LABEL = { dev: 'Dev', development: 'Dev', uat: 'UAT', qa: 'QA', staging: 'Staging', prod: 'Prod', production: 'Prod' };
const stageLabel = name => STAGE_LABEL[String(name).toLowerCase()] || String(name).charAt(0).toUpperCase() + String(name).slice(1);
const blockedWords = b => typeof b === 'string' && b ? b.replace(/-/g, ' ') : 'blocked';
function stuckWords(f) {
  const n = f.stuck[0], s = (f.subtasks || []).find(s => s.number === n);
  return `#${n} ${blockedWords(s && s.blocked)}`;
}
/* The furthest state a feature has reached, as its banner label reads it. */
function featureSummary(f) {
  if (f.live) return 'Live in prod';
  if ((f.stuck || []).length) return `Stuck · ${stuckWords(f)}`;
  const b = f.build || { landed: 0, total: counted(f).length }, stages = f.stages || [];
  if (b.landed < b.total) return `Build ${b.landed}/${b.total}`;
  const s = stages.find(s => s.unknown || s.reached < s.total) || stages[stages.length - 1];
  if (!s) return `Build ${b.landed}/${b.total}`;
  return s.unknown ? `${stageLabel(s.name)}: unknown` : `${stageLabel(s.name)} ${s.reached}/${s.total}`;
}
/* The scenes on screen and where each sits on the canvas. */
function shownScenes() {
  if (!view) return [];
  if (!view.world) { const sc = scenes[view.village]; return sc && sc.v ? [[sc, 0, 0]] : []; }
  const w = worlds.find(x => x.name === view.world), Ps = w && scenes[w.platform], Ds = w && scenes[w.dev];
  if (!Ps || !Ds || !Ps.v || !Ds.v) return [];
  return [[Ps, SIDE, 0], [Ds, SIDE, worldGeom(Ps, Ds).devTop]];
}
/* One button per banner, kept across polls so keyboard focus survives a refresh. */
const fbuttons = new Map();
let peeked = null;
function peek(b) {
  if (peeked === b) return;
  if (peeked) peeked.classList.remove('peek');
  peeked = b || null;
  if (peeked) peeked.classList.add('peek');
}
function placeBanners() {
  const vp = document.getElementById('viewport'), r = cv.getBoundingClientRect(), vr = vp.getBoundingClientRect();
  const s = r.width / cv.width, keep = new Set();
  let strip = null;
  if (!cv.hidden) shownScenes().forEach(([sc, ox, oy]) => sc.L.banners.forEach((g, k) => {
    const f = g.f, key = `${sc.v.project}#${f.number}`, on = isFollowed(sc.v, f), col = f.live ? 'var(--gold)' : g.col;
    if (on) strip = { sc, g };
    keep.add(key);
    let b = fbuttons.get(key);
    if (!b) {
      b = document.createElement('button');
      b.addEventListener('click', () => toggleFollow(b.project, b.number));
      fbuttons.set(key, b); fbox.appendChild(b);
    }
    b.project = sc.v.project; b.number = f.number;
    b.className = 'fb' + (f.live ? ' live' : '') + ((f.stuck || []).length ? ' stuck' : '') + (b === peeked ? ' peek' : '');
    b.style.setProperty('--fc', col);
    b.setAttribute('aria-pressed', String(on));
    const sum = featureSummary(f);
    b.innerHTML = `#${f.number} ${esc(f.title)}<small>${esc(sum)}</small>`;
    b.setAttribute('aria-label', `Feature #${f.number} ${f.title}: ${sum}. ${on ? 'Following; press to stop' : 'Press to follow'}.`);
    b.style.setProperty('--fade', on ? 1 : (sc.fade[f.number] ?? 1));
    // Labels sit under their banners; when the map is drawn small they alternate rows.
    const lift = 30 * s < 120 && k % 2 ? 36 : 0;
    b.style.left = (r.left - vr.left + vp.scrollLeft + (ox + g.x + 7) * s) + 'px';
    b.style.top = (r.top - vr.top + vp.scrollTop + (oy + g.y + 28) * s + lift) + 'px';
  }));
  for (const [key, b] of fbuttons) if (!keep.has(key)) { if (b === peeked) peeked = null; b.remove(); fbuttons.delete(key); }
  renderStrip(strip);
  placeLabels();
  placeKeys();
}
/* One keyboard target per object drawn in the last frame, kept across polls
 * (by what it is, not where) so focus survives a refresh. Walkers and the
 * banners (which have their own buttons) are left to the mouse. */
const kbox = document.getElementById('hitkeys'), kbuttons = new Map();
const NO_KEY = new Set(['porter', 'banner', 'river', 'canal', 'supply']);
const keyOf = o => [o.kind, o.i ?? '', o.n ?? '', o.at ?? '', o.who ?? '', o.house ?? '', o.id ?? '', o.f ?? ''].join(':');
function placeKeys() {
  const vp = document.getElementById('viewport'), r = cv.getBoundingClientRect(), vr = vp.getBoundingClientRect();
  const s = r.width / cv.width, ox0 = r.left - vr.left + vp.scrollLeft, oy0 = r.top - vr.top + vp.scrollTop, keep = new Set();
  if (!cv.hidden) shownScenes().forEach(([sc, ox, oy]) => {
    const theme = view.world && oy === 0 ? otherTheme() : state.T;
    (sc.hits || []).forEach(h => {
      if (NO_KEY.has(h.o.kind) || h.o.at === 'transit') return;
      const key = `${sc.v.project}|${keyOf(h.o)}`;
      if (keep.has(key)) return;
      const info = explain(h.o, sc, theme);
      if (!info) return;
      keep.add(key);
      let b = kbuttons.get(key);
      if (!b) {
        b = document.createElement('button'); b.className = 'hk';
        b.addEventListener('click', () => { const sc2 = scenes[b.project]; if (sc2 && sc2.v) act(b.obj, sc2, b.theme); });
        kbuttons.set(key, b); kbox.appendChild(b);
      }
      b.project = sc.v.project; b.obj = h.o; b.theme = theme;
      b.setAttribute('aria-label', [info.title, info.line].filter(Boolean).join(': '));
      Object.assign(b.style, { left: ox0 + (ox + h.x) * s + 'px', top: oy0 + (oy + h.y) * s + 'px', width: h.w * s + 'px', height: h.h * s + 'px' });
    });
  });
  for (const [key, b] of kbuttons) if (!keep.has(key)) { b.remove(); kbuttons.delete(key); }
}
/* ---------- Area labels ----------
 * A name over each part of the map (docks, castle, board, quay, each stop)
 * and the worktree or workstream under each house. Each label is placed in
 * canvas pixels with an anchor: 'top' hangs below the point, 'bottom' sits
 * above it, 'right' ends at it. */
const abox = document.getElementById('alabels'), labelsOn = document.getElementById('labels-on');
try { labelsOn.checked = localStorage.getItem('village-labels') !== 'off'; } catch {}
labelsOn.addEventListener('change', () => {
  try { localStorage.setItem('village-labels', labelsOn.checked ? 'on' : 'off'); } catch {}
  placeLabels();
});
/* A house's name: its worktree's folder, less the project prefix (shop-wt-2 → wt-2). */
function houseName(v, h) {
  const id = String(h.id || '');
  if (!id.includes('/')) return id || h.role;
  const base = id.replace(/\/+$/, '').split('/').pop();
  const short = base.startsWith(v.project + '-') ? base.slice(v.project.length + 1) : base;
  return short || h.role;
}
function areaLabels(sc, theme) {
  const v = sc.v, lay = sc.L, runtime = v.plane === 'runtime', stops = v.pipeline.stops, out = [];
  const add = (x, y, anchor, text, sub, cls) => out.push({ x, y, anchor, text, sub, cls });
  add(34, 51, 'top', theme.labels.docks, runtime ? 'waiting' : `backlog · ${v.backlog.length}`);
  add(lay.hallX + 16, 43, 'top', theme.labels.hall, 'manager');
  // Under the board, not over it or the trees beside it.
  add(lay.hallX + 51, 45, 'top', theme.labels.board, runtime ? 'failed' : `blocked · ${v.board.length}`);
  v.houses.forEach((h, i) => {
    const lot = lay.lots[i]; if (!lot) return;
    const name = houseName(v, h);
    add(lot.x + 8, lot.y + 17, 'top', name, null, 'house ' + h.status);
  });
  if (!runtime) add(lay.roadX - 7, lay.quayY + 4, 'right', theme.labels.quay, `→ ${v.pipeline.integration || 'integration'}`);
  // Each stop is labelled with its own name (master-green, uat, prod), as the pipeline reports it.
  stops.slice(0, -1).forEach((s, i) => add(lay.lockStart + i * 34 + 3, lay.canalY - 21, 'bottom', s.name));
  const last = stops[stops.length - 1];
  // Over the harbour wall, left of the lighthouse.
  add(lay.castleX + 10, lay.canalY - 22, 'bottom', theme.labels.castle, last ? last.name : null);
  return out;
}
/* Sprites a label must not cover, in scene pixels: the board, the
 * destination's towers (the lighthouse, the masts downtown and in the
 * future), which each theme lists, and the ship (or rocket, or shuttle)
 * where it waits at the destination. */
function labelAvoid(sc, theme) {
  const lay = sc.L, board = (sc.hits || []).filter(h => h.o.kind === 'board');
  const at = ([dx, dy, w, h]) => ({ x: lay.castleX + dx, y: lay.canalY + dy, w, h });
  return [...board, ...theme.towers.map(at), at(theme.hull)];
}
/* Move labels that cover an avoided sprite or an earlier label: up for those
 * that sit above their point, down for those that hang below it. Measured on
 * screen, so it holds at any scale. */
function unclutter(els, blocked) {
  const hits = (a, b) => a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom;
  const placed = [...blocked];
  els.forEach(el => {
    const up = el.dataset.anchor !== 'top';
    for (let k = 0; k < 8; k++) {
      const r = el.getBoundingClientRect(), o = placed.find(p => hits(r, p));
      if (!o) break;
      el.style.top = parseFloat(el.style.top) + (up ? o.top - r.bottom - 1 : o.bottom - r.top + 1) + 'px';
    }
    placed.push(el.getBoundingClientRect());
  });
}
function placeLabels() {
  abox.hidden = !labelsOn.checked || cv.hidden;
  if (abox.hidden) return;
  const vp = document.getElementById('viewport'), r = cv.getBoundingClientRect(), vr = vp.getBoundingClientRect();
  const s = r.width / cv.width, ox0 = r.left - vr.left + vp.scrollLeft, oy0 = r.top - vr.top + vp.scrollTop;
  const shift = { top: 'translate(-50%, 0)', bottom: 'translate(-50%, -100%)', right: 'translate(-100%, -50%)' };
  // Text grows with the map; below 3x the subtitles go, so names stay readable.
  abox.style.setProperty('--lf', Math.round(Math.max(9, Math.min(14, 2 + 2.5 * s))) + 'px');
  abox.classList.toggle('compact', s < 3);
  let html = '';
  const blocked = [];
  shownScenes().forEach(([sc, ox, oy]) => {
    const theme = view.world && oy === 0 ? otherTheme() : state.T;
    labelAvoid(sc, theme).forEach(a => blocked.push({ left: r.left + (ox + a.x) * s, top: r.top + (oy + a.y) * s, right: r.left + (ox + a.x + a.w) * s, bottom: r.top + (oy + a.y + a.h) * s }));
    areaLabels(sc, theme).forEach(l => {
      // A house label may be no wider than its lot, so neighbours never overlap.
      const max = l.cls && l.cls.startsWith('house') ? `max-width:${Math.max(28, (LW - 1) * s)}px;` : '';
      html += `<div class="al${l.cls ? ' ' + esc(l.cls) : ''}" aria-hidden="true" data-anchor="${l.anchor}" style="left:${ox0 + (ox + l.x) * s}px;top:${oy0 + (oy + l.y) * s}px;transform:${shift[l.anchor]};${max}">` +
        `${esc(l.text)}${l.sub ? ` <small>${esc(l.sub)}</small>` : ''}</div>`;
    });
  });
  // Redo it when the labels or the sprites to avoid change (the board's rect is
  // only known once a frame has drawn it).
  const drawn = html + blocked.map(b => [b.left, b.top, b.right, b.bottom].map(Math.round).join(',')).join(';');
  if (abox.drawn === drawn) return;
  abox.innerHTML = html; abox.drawn = drawn;
  unclutter([...abox.children], blocked);
}
function renderStrip(on) {
  fstrip.hidden = !on;
  if (!on) return;
  const { sc, g } = on, f = g.f, b = f.build || { landed: 0, total: counted(f).length };
  fstrip.style.setProperty('--fc', f.live ? 'var(--gold)' : g.col);
  const chip = (cls, text) => `<span class="chip ${cls}">${esc(text)}</span>`;
  const build = (f.stuck || []).length ? chip('stuck', `Build ${b.landed}/${b.total}: ${stuckWords(f)}`)
    : chip(b.landed === b.total ? 'done' : b.landed ? 'part' : '', `Build ${b.landed}/${b.total}`);
  const stages = (f.stages || []).map(s => s.unknown ? chip('unknown', `${stageLabel(s.name)}: unknown`)
    : chip(s.reached === s.total ? 'done' : s.reached ? 'part' : '', `${stageLabel(s.name)} ${s.reached}/${s.total}`));
  document.getElementById('fs-title').textContent = `Following #${f.number} ${f.title}`;
  const chips = [chip('done', 'Spec ✓'), build, ...stages].join('<span class="sep">›</span>') + (f.live ? ' ' + chip('live', '★ Live in prod') : '');
  // Only touch the live region when the reading changes, so a poll is not re-announced.
  const box = document.getElementById('fs-chips');
  if (box.innerHTML !== chips) box.innerHTML = chips;
  document.getElementById('fs-mode').textContent = f.promotion === 'feature' ? 'Promotes as a whole feature' : 'Promotes per subtask';
  fstrip.feature = { project: sc.v.project, number: f.number };
}
function toggleFollow(project, number) {
  followed = followed && followed.project === project && followed.number === number ? null : { project, number };
  placeBanners();
  if (state.reduce) draw(0);
}
document.getElementById('fs-stop').addEventListener('click', () => { followed = null; placeBanners(); if (state.reduce) draw(0); });
document.getElementById('fs-details').addEventListener('click', () => {
  const k = fstrip.feature, sc = k && scenes[k.project], f = sc && sc.v && (sc.v.features || []).find(f => f.number === k.number);
  if (f) showFeature(f, sc.v);
});
/* Where an environment's answer comes from. */
function sourceWords(s) {
  const sha = s.sha ? ' · ' + String(s.sha).slice(0, 7) : '';
  if (s.source === 'deployment') return `GitHub Deployment${sha}`;
  if (s.source === 'release') return `${s.tag ? s.tag + ' ' : ''}(GitHub Release)${sha}`;
  if (s.source === 'tag') return `tag ${s.tag || ''}${sha}`;
  if (s.source === 'url') return `its version endpoint${sha}`;
  if (s.source === 'branch') return `${s.branch ? 'branch ' + s.branch : 'its branch ref'}${sha}`;
  if (s.source === 'unknown') return 'nothing says what runs here (no Deployment, Release or branch)';
  return 'not reported';
}
function whereWords(s, v, f) {
  const extra = s.blocked ? ` · ${blockedWords(s.blocked)}` : '', stops = v.pipeline.stops;
  switch (s.where) {
    case 'house': {
      const h = v.houses.find(h => h.issue === s.number);
      if (!h) return 'being built' + extra;
      return h.status === 'blocked' ? `blocked · ${h.role} is waiting on you` : `being built by ${h.role}${extra}`;
    }
    case 'backlog': return `waiting at the ${state.T.labels.docks.toLowerCase()}${extra}`;
    case 'board': return `on the ${state.T.labels.board.toLowerCase()}${extra}`;
    case 'quay': return f.promotion === 'feature' ? `landed · waiting at the ${state.T.labels.quay.toLowerCase()} for the rest` : `landed · waiting for ${stops.length ? stageLabel(stops[0].name) : 'release'}`;
    case 'stop': {
      const st = stops.find(x => x.name === s.stop);
      return st && st.source === 'unknown' ? `${stageLabel(s.stop)}: unknown` : `in ${stageLabel(s.stop || '?')}`;
    }
    case 'closed': return 'closed';
    default: return s.where || '';
  }
}
function showFeature(f, v) {
  const subs = f.subtasks || [], url = n => (f.url || '').replace(/\/issues\/\d+$/, '/issues/' + n);
  const issueLink = (n, cls) => f.url ? `<a href="${esc(url(n))}" target="_blank" rel="noopener"${cls ? ` class="${cls}"` : ''}>#${n}</a>` : `<span${cls ? ` class="${cls}"` : ''}>#${n}</span>`;
  document.getElementById('d-title').textContent = `#${f.number} ${f.title} · feature`;
  document.getElementById('d-line').textContent = `${featureSummary(f)} · ${counted(f).length} subtasks counted`;
  document.getElementById('d-meta').innerHTML =
    `<ul class="subs">${subs.map(s => s.dropped
        ? `<li>${issueLink(s.number, 'drop')}<span class="drop">${esc(s.title || '')}</span><span class="w">closed as not planned · not counted</span></li>`
        : `<li>${issueLink(s.number)}<span>${esc(s.title || '')}</span><span class="w">${esc(whereWords(s, v, f))}</span></li>`).join('')}</ul>
      <dl><dt>Subtasks from</dt><dd>${f.linked_by === 'body' ? `"Sub-task of #${f.number}" in each body` : 'GitHub sub-issues'}</dd>
      ${v.pipeline.stops.map(s => `<dt>${esc(stageLabel(s.name))}</dt><dd>${esc(sourceWords(s))}</dd>`).join('')}
      <dt>Promotion</dt><dd>${f.promotion === 'feature' ? 'as a whole feature' : 'per subtask'}</dd>
      <dt>Live means</dt><dd>every counted subtask's code runs in ${esc(v.pipeline.stops.length ? stageLabel(v.pipeline.stops[v.pipeline.stops.length - 1].name) : 'prod')}</dd></dl>`;
  ask({});
  links([
    f.spec_url ? { label: `Spec: ${f.spec_url.split('/').pop()}`, url: f.spec_url } : f.url && { label: `Spec: #${f.number} issue body`, url: f.url },
    f.url && { label: `GitHub issue #${f.number}`, url: f.url }
  ].filter(Boolean));
  dialog.hidden = false;
  document.getElementById('close').focus();
}

/* ---------- Key ----------
 * What every symbol means, in the current theme. Each swatch is drawn by the
 * renderer's own functions (T.crate, pennant, bubble, cobwebs, person) from
 * its own tables (KINDCOL, ICONS, STATE, SPECIALTIES, NOTECOL), and the rows
 * are listed from those tables, so the key cannot drift from the map. */
const KIND_WORDS = { bug: 'Bug', feature: 'Feature', tests: 'Tests', security: 'Security', research: 'Research', perf: 'Performance',
  refactor: 'Refactor', docs: 'Docs', chores: 'Chore', workshop: 'Finished work, sealed (quay and stages)', cottage: 'Other work' };
const ICON_WORDS = { blocked: 'Blocked: waiting on you', done: 'Done', unknown: 'Unknown: no word from it', stalled: 'Stalled: no progress' };
const PENNANT_WORDS = { P0: 'P0: urgent (waves)', P1: 'P1: high', P2: 'P2: normal' };
function swatch(paint, size = 14) {
  const c = document.createElement('canvas');
  c.width = size; c.height = size; c.setAttribute('aria-hidden', 'true');
  const g = c.getContext('2d'); g.imageSmoothingEnabled = false; paint(g);
  return c;
}
function renderLegend() {
  const L = state.T.labels, body = document.getElementById('legend-body');
  document.getElementById('legend-title').textContent = `Key · ${state.T.name}`;
  const groups = [
    [`Crates at the ${L.docks}: the kind of work`, Object.keys(KINDCOL).map(k => [c => state.T.crate(c, 1, 1, k, null, k === 'workshop'), KIND_WORDS[k] || k, 7])],
    ['Priority', [[c => state.T.crate(c, 1, 1, 'feature', 'P0'), 'Red mark on a crate: P0', 7],
      ...Object.keys(PENNANT_WORDS).map(p => [c => pennant(c, 5, 7, p, 0), `Pennant over a house: ${PENNANT_WORDS[p]}`]),
      [() => {}, 'No pennant: P3']]],
    ['Bubbles over people and stops', Object.keys(ICONS).map(k => [c => bubble(c, 2, 1, k), ICON_WORDS[k] || k])],
    ['Other marks', [[c => { state.T.crate(c, 1, 1, 'feature', null); roped(c, 1, 1); }, 'Roped crate: waits on another issue ("Depends on")', 7], [c => cobwebs(c, -1, -6), 'Cobwebs on a house: stalled'],
      [c => { R(c, 0, 0, 14, 14, fogCol(state.T)); bubble(c, 2, 1, 'unknown'); }, 'Grey fog over a stage: nothing says what runs there (no Deployment, no branch)']]],
    ['Sub-agent accents: their specialty', [...SPECIALTIES.map(([name, kind]) => [c => person(c, 3, 1, 'key:' + name, 'stand', { accent: KINDCOL[kind], sub: true }), name]),
      [c => person(c, 3, 1, 'key:other', 'stand', { accent: P.white, sub: true }), 'any other']]],
    [`Notes on the ${L.board}`, Object.keys(NOTECOL).map(k => [c => { R(c, 1, 1, 5, 5, P.o); R(c, 2, 2, 3, 3, NOTECOL[k]); }, k.replace(/-/g, ' '), 7])]
  ];
  body.textContent = '';
  groups.forEach(([title, rows]) => {
    const h = document.createElement('h4'), ul = document.createElement('ul');
    h.textContent = title;
    rows.forEach(([paint, words, size]) => {
      const li = document.createElement('li'), span = document.createElement('span');
      span.textContent = words;
      li.append(swatch(paint, size), span); ul.appendChild(li);
    });
    body.append(h, ul);
  });
}
const legendBtn = document.getElementById('legend-btn'), legend = document.getElementById('legend');
legendBtn.addEventListener('click', () => {
  legend.hidden = !legend.hidden;
  legendBtn.setAttribute('aria-expanded', String(!legend.hidden));
  if (!legend.hidden) renderLegend();
});

/* ---------- Data ---------- */
/* The map fills the panel's width; a tall map scrolls the page rather than
 * shrinking. Whole-number scales keep the pixels square; below 1× (a phone)
 * it shrinks to the panel so the page never scrolls sideways. +/− step the
 * scale from the fitted one, and the step is remembered. */
const ZOOM_MAX_SCALE = 16;
let zoom = 0;
try { zoom = Math.max(-ZOOM_MAX_SCALE, Math.min(ZOOM_MAX_SCALE, parseInt(localStorage.getItem('village-zoom'), 10) || 0)); } catch {}
function fitScale() {
  const room = (document.getElementById('viewport').clientWidth - 8) / cv.width;
  return room >= 1 ? Math.floor(room) : Math.max(0.1, room);
}
function mapScale() {
  const f = fitScale();
  return zoom ? Math.max(Math.min(1, f), Math.min(ZOOM_MAX_SCALE, Math.max(1, Math.floor(f)) + zoom)) : f;
}
function fit() {
  const W = cv.width, H = cv.height, s = mapScale();
  cv.style.width = W * s + 'px'; cv.style.height = H * s + 'px';
  const fitBtn = document.getElementById('zoom-fit');
  fitBtn.textContent = zoom ? (Number.isInteger(s) ? s : s.toFixed(1)) + '×' : 'Fit';
  fitBtn.disabled = !zoom;
  document.getElementById('zoom-out').disabled = s <= Math.min(1, fitScale());
  document.getElementById('zoom-in').disabled = s >= ZOOM_MAX_SCALE;
}
function stepZoom(d) {
  const before = mapScale();
  zoom = d ? zoom + d : 0;
  if (d && mapScale() === before) zoom -= d;  // already at the smallest or largest scale
  try { localStorage.setItem('village-zoom', String(zoom)); } catch {}
  fit(); placeBanners();
}
document.getElementById('zoom-out').addEventListener('click', () => stepZoom(-1));
document.getElementById('zoom-in').addEventListener('click', () => stepZoom(1));
document.getElementById('zoom-fit').addEventListener('click', () => stepZoom(0));
window.addEventListener('resize', () => { fit(); placeBanners(); });

/* Which village this is, always in the bar (the picker shows it only when
 * there are two or more), plus its GitHub page when a real issue URL shows
 * which repo it is (`repoUrl`), never a guess. */
function nameBar(name, v) {
  const vn = document.getElementById('vname'), a = document.getElementById('repo'), repo = v ? repoUrl(v) : null;
  vn.textContent = name;
  vn.hidden = !pick.hidden;
  a.hidden = !repo;
  if (repo) { a.href = repo; a.textContent = repo.replace(/^https:\/\/github\.com\//, '') + ' on GitHub'; }
}

/* `· autocoder · development`, `· agentex · runtime`: which framework drew it, and which half of the loop it is. */
function describe(v) {
  const fw = v.workflow || v.framework;
  return [fw, v.plane].filter(Boolean).map(x => '· ' + x).join(' ');
}
/* Show a village (`shop`) or a world (`world:shop`). */
function select(name) {
  const w = worlds.find(x => x.name === name);
  const v = villages.find(x => x.project === name);
  if (w) view = { world: w.name };
  else if (v || villages[0]) view = { village: (v || villages[0]).project };
  else return;
  if (view.world) {
    document.getElementById('workflow').textContent = `· ${w.dev} builds ${w.platform}`;
    nameBar(`${w.dev} \u21c4 ${w.platform}`, villages.find(x => x.project === w.dev));
    renderWorldFlow(w);
  } else {
    const sc = scenes[view.village];
    document.getElementById('workflow').textContent = describe(sc.v);
    nameBar(sc.v.project, sc.v);
    if (cv.width !== sc.L.W || cv.height !== sc.L.H) { cv.width = sc.L.W; cv.height = sc.L.H; }
    renderFlow(sc.v);
  }
  fit();
  if (state.reduce) draw(0);
  placeBanners();
}
function draw(t) {
  if (!view) return;
  ctx.imageSmoothingEnabled = false;
  if (view.world) { const w = worlds.find(x => x.name === view.world); if (w) drawWorld(t, w); return; }
  const sc = scenes[view.village];
  if (sc && sc.v) drawVillage(ctx, sc, t);
}
const themePick = document.getElementById('theme');
// One option per registered theme, in the order they registered.
for (const [key, th] of Object.entries(THEMES)) themePick.add(new Option(th.name, key));
themePick.value = themeName;
themePick.addEventListener('change', () => {
  themeName = themePick.value; state.T = THEMES[themeName];
  try { localStorage.setItem('village-theme', themeName); } catch {}
  if (!legend.hidden) renderLegend();
  if (view) select(view.world || view.village);
});
pick.addEventListener('change', () => { location.hash = pick.value; select(pick.value); });
window.addEventListener('hashchange', () => select(decodeURIComponent(location.hash.slice(1))));

async function poll() {
  try {
    const res = await fetch('/api/village');
    if (!res.ok) throw new Error(res.status);
    const data = await res.json();
    villages = data.villages || [];
    worlds = data.worlds || [];
    const t = performance.now();
    villages.forEach(v => updateScene(v, t));
    worlds.forEach(w => updateWorld(w, t));
    const names = [...worlds.map(w => w.name), ...villages.map(v => v.project)];
    const label = n => { const w = worlds.find(x => x.name === n); return w ? `${w.dev} \u21c4 ${w.platform}` : n; };
    pick.hidden = names.length < 2;
    if (pick.options.length !== names.length || [...pick.options].some((o, i) => o.value !== names[i])) {
      pick.innerHTML = names.map(n => `<option value="${esc(n)}">${esc(label(n))}</option>`).join('');
    }
    const want = decodeURIComponent(location.hash.slice(1)) || (view && (view.world || view.village)) || names[0];
    if (names.includes(want)) pick.value = want;
    document.getElementById('empty').hidden = villages.length > 0;
    cv.hidden = villages.length === 0;
    if (!villages.length) document.getElementById('empty').textContent = 'No swarms running. Start one from the dashboard or the agents-tui terminal.';
    else select(pick.value || names[0]);
    document.getElementById('status').textContent = 'Live · updated ' + new Date().toLocaleTimeString();
  } catch (e) {
    document.getElementById('status').textContent = "Can't reach agents-tui. Retrying…";
  }
  setTimeout(poll, 3000);
}
poll();

if (!state.reduce) {
  // The first frame records what is clickable; place the keyboard targets then, not a poll later.
  const loop = t => { draw(t); if (!kbuttons.size && kbox.dataset.placed !== String(view && (view.world || view.village))) { kbox.dataset.placed = String(view && (view.world || view.village)); placeKeys(); } requestAnimationFrame(loop); };
  requestAnimationFrame(loop);
}
