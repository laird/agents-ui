// The Future theme's buildings redrawn at 2× (src/web/village/themes/future.js,
// #465), under node:
//   node --test tools/village-tests/
// Each is painted onto a fake context that follows the detail transform, at
// many moments of its animation and in every status: it draws its own detail
// pixels, and it stays inside the scene
// box its 1× art covered (FOOTPRINT, recorded from the 1× art before #465).
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= {}; // state.js reads window.matchMedia, when there is one
const V = '../../src/web/village/';
const { state } = await import(V + 'state.js');
const { FUTURE } = await import(V + 'themes/future.js');
const { FB, futureBoard, futureDock, futureHall, futureScaffold } = await import(V + 'themes/futurebuildings.js');

/* The 1× art's scene box for each sprite, [x0, y0, x1, y1] (x1, y1 exclusive). */
const FOOTPRINT = {"B.cottage.working":[20,7,36,36],"B.cottage.idle":[20,16,36,36],"B.cottage.blocked":[20,16,36,36],"B.cottage.unknown":[20,16,36,36],"B.workshop.working":[20,16,36,36],"B.workshop.idle":[20,16,36,36],"B.workshop.blocked":[20,16,36,36],"B.workshop.unknown":[20,16,36,36],"B.feature.working":[20,16,36,36],"B.feature.idle":[20,16,36,36],"B.feature.blocked":[20,16,36,36],"B.feature.unknown":[20,16,36,36],"B.bug.working":[20,16,36,36],"B.bug.idle":[20,16,36,36],"B.bug.blocked":[20,16,36,36],"B.bug.unknown":[20,16,36,36],"B.security.working":[20,16,36,36],"B.security.idle":[20,18,36,36],"B.security.blocked":[20,18,36,36],"B.security.unknown":[20,18,36,36],"B.tests.working":[20,4,36,36],"B.tests.idle":[20,17,36,36],"B.tests.blocked":[20,17,36,36],"B.tests.unknown":[20,17,36,36],"B.research.working":[20,16,36,36],"B.research.idle":[20,17,36,36],"B.research.blocked":[20,17,36,36],"B.research.unknown":[20,17,36,36],"B.docs.working":[20,16,36,36],"B.docs.idle":[20,17,36,36],"B.docs.blocked":[20,17,36,36],"B.docs.unknown":[20,17,36,36],"B.refactor.working":[20,21,36,36],"B.refactor.idle":[20,21,36,36],"B.refactor.blocked":[20,21,36,36],"B.refactor.unknown":[20,21,36,36],"B.perf.working":[20,18,36,36],"B.perf.idle":[20,18,36,36],"B.perf.blocked":[20,18,36,36],"B.perf.unknown":[20,18,36,36],"B.chores.working":[20,23,36,36],"B.chores.idle":[20,23,36,36],"B.chores.blocked":[20,23,36,36],"B.chores.unknown":[20,23,36,36],"hall.working":[16,21,56,62],"hall.idle":[16,21,56,62],"hall.blocked":[16,21,56,62],"board":[20,20,38,35],"dock":[20,20,46,27],"scaffold":[21,18,36,36],"gatehouse":[20,20,72,43],"decor.0":[21,20,26,28],"decor.1":[20,20,28,27],"decor.2":[21,21,28,27],"decor.3":[20,20,28,28],"decor.4":[20,22,28,27],"decor.5":[20,21,28,26],"border":[0,11,120,81]};

/* Run fn against a context that tracks, in scene pixels, the box everything
 * drawn covers, and how many rects were drawn finer than a scene pixel. */
function measure(fn) {
  let m = { s: 1, x: 0, y: 0 }; const st = [];
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity, fine = 0, coarse = 0;
  const c = {
    fillStyle: '', globalAlpha: 1,
    setTransform(a, b, cc, d, e, f) { m = { s: a, x: e, y: f }; },
    save() { st.push({ ...m }); }, restore() { m = st.pop(); },
    translate(x, y) { m = { s: m.s, x: m.x + m.s * x, y: m.y + m.s * y }; },
    scale(s) { m = { s: m.s * s, x: m.x, y: m.y }; },
    beginPath() {}, ellipse() {}, fill() {}, arc() {},
    fillRect(x, y, w, h) {
      if (m.s < 1) fine++; else coarse++;
      const X = m.x + m.s * x, Y = m.y + m.s * y;
      x0 = Math.min(x0, X); y0 = Math.min(y0, Y); x1 = Math.max(x1, X + m.s * w); y1 = Math.max(y1, Y + m.s * h);
    },
  };
  for (const t of [0, 90, 120, 150, 240, 300, 400, 500, 700, 1300, 2600]) fn(c, t);
  return { box: [Math.floor(x0), Math.floor(y0), Math.ceil(x1), Math.ceil(y1)], fine, coarse };
}
/* The houses that smoke while working (their puffs() is sprites/houses.js's). */
const SMOKY = new Set(['B.cottage.working', 'B.tests.working']);
const G = { y: 20, x0: 40, w: 32, rx: 24, rw: 12, tables: [[44, 30], [56, 30]] };
const SPRITES = {
  ...Object.fromEntries(Object.keys(FUTURE.B).flatMap(k => ['working', 'idle', 'blocked', 'unknown'].map(st => [`B.${k}.${st}`, (c, t) => FUTURE.B[k](c, 20, 20, st, t)]))),
  ...Object.fromEntries(['working', 'idle', 'blocked'].map(st => [`hall.${st}`, c => FUTURE.hall(c, 20, 40, st)])),
  board: c => FUTURE.board(c, 20, 20, [{ label: 'needs-design' }, { label: 'proposal' }, { label: 'conflict' }]),
  dock: c => FUTURE.dock(c, 20, 20, 26),
  scaffold: (c, t) => FUTURE.scaffold(c, 20, 20, t),
  gatehouse: (c, t) => { FUTURE.gatehouse(c, G, t, new Set()); FUTURE.gatehouse(c, G, t, new Set([1, 2])); },
  ...Object.fromEntries([0, 1, 2, 3, 4, 5].map(k => [`decor.${k}`, (c, t) => FUTURE.decor(c, 20, 20, k, t)])),
  border: c => FUTURE.border(c, 120, 80),
};

test('every sprite has a recorded footprint, and every footprint a sprite', () => {
  assert.deepEqual(Object.keys(SPRITES).sort(), Object.keys(FOOTPRINT).sort());
});

for (const [name, paint] of Object.entries(SPRITES)) {
  test(`future ${name}: drawn at 2× inside its 1× footprint`, () => {
    state.T = FUTURE; state.reduce = true;
    const { box, fine } = measure(paint);
    assert.ok(fine > 0, 'draws detail pixels');
    const [a, b, c, d] = FOOTPRINT[name];
    // A working house's chimney smoke or fizz is drawn by sprites/houses.js
    // (#461), and rises above the roof however that draws it (as the Modern
    // test allows, #464); the building's sides and foot stay inside its box.
    const top = SMOKY.has(name) ? -Infinity : b;
    assert.ok(box[0] >= a && box[1] >= top && box[2] <= c && box[3] <= d, `${JSON.stringify(box)} inside ${JSON.stringify(FOOTPRINT[name])}`);
  });
}

test("registerTheme keeps the theme's own slots: nothing wraps them", () => {
  for (const [kind, fn] of Object.entries(FB)) assert.equal(FUTURE.B[kind], fn, kind);
  assert.equal(FUTURE.hall, futureHall); assert.equal(FUTURE.board, futureBoard);
  assert.equal(FUTURE.dock, futureDock); assert.equal(FUTURE.scaffold, futureScaffold);
});

test('the buildings draw nothing in scene pixels but the shadows and steam under them', () => {
  state.T = FUTURE; state.reduce = true;
  for (const [name, paint] of Object.entries(SPRITES)) {
    if (/^decor\.[023]$/.test(name) || name === 'B.cottage.working' || name === 'B.tests.working') continue;
    assert.equal(measure(paint).coarse, 0, name);
  }
});
