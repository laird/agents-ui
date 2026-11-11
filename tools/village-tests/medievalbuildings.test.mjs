// The Medieval theme's buildings redrawn at 2× (src/web/village/sprites/houses.js,
// houseparts.js, themes/medievalbuildings.js and the gatehouse in
// themes/medieval.js, #463), under node:
//   node --test tools/village-tests/
// Each is painted onto a fake context that follows the detail transform, at
// many moments of its animation and in every status: it draws its own detail
// pixels, and it stays inside the scene
// box its 1× art covered (FOOTPRINT, recorded from the 1× art before #463).
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= {}; // state.js reads window.matchMedia, when there is one
const V = '../../src/web/village/';
const { state } = await import(V + 'state.js');
const { MEDIEVAL } = await import(V + 'themes/medieval.js');
const { B } = await import(V + 'sprites/houses.js');

/* The 1× art's scene box for each sprite, [x0, y0, x1, y1] (x1, y1 exclusive). */
const FOOTPRINT = {"B.cottage.working":[20,4,36,36],"B.cottage.idle":[20,18,36,36],"B.cottage.blocked":[20,18,36,36],"B.cottage.unknown":[20,18,36,36],"B.workshop.working":[20,4,36,36],"B.workshop.idle":[20,18,36,36],"B.workshop.blocked":[20,18,36,36],"B.workshop.unknown":[20,18,36,36],"B.feature.working":[20,21,36,36],"B.feature.idle":[20,21,36,36],"B.feature.blocked":[20,21,36,36],"B.feature.unknown":[20,21,36,36],"B.bug.working":[20,16,36,36],"B.bug.idle":[20,16,36,36],"B.bug.blocked":[20,16,36,36],"B.bug.unknown":[20,16,36,36],"B.security.working":[20,15,36,36],"B.security.idle":[20,16,36,36],"B.security.blocked":[20,16,36,36],"B.security.unknown":[20,16,36,36],"B.tests.working":[20,4,36,36],"B.tests.idle":[20,18,36,36],"B.tests.blocked":[20,18,36,36],"B.tests.unknown":[20,18,36,36],"B.research.working":[21,21,37,36],"B.research.idle":[21,21,36,36],"B.research.blocked":[21,21,36,36],"B.research.unknown":[21,21,36,36],"B.docs.working":[20,20,36,36],"B.docs.idle":[20,20,36,36],"B.docs.blocked":[20,20,36,36],"B.docs.unknown":[20,20,36,36],"B.refactor.working":[20,16,36,36],"B.refactor.idle":[20,16,36,36],"B.refactor.blocked":[20,16,36,36],"B.refactor.unknown":[20,16,36,36],"B.perf.working":[21,16,36,36],"B.perf.idle":[21,17,35,36],"B.perf.blocked":[21,17,35,36],"B.perf.unknown":[21,17,35,36],"B.chores.working":[20,21,36,36],"B.chores.idle":[20,21,36,36],"B.chores.blocked":[20,21,36,36],"B.chores.unknown":[20,21,36,36],"hall.working":[16,29,56,62],"hall.idle":[16,29,56,62],"hall.blocked":[16,29,56,62],"board":[20,20,38,35],"dock":[20,20,46,27],"scaffold":[21,22,35,36],"gatehouse":[20,20,73,43]};

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
const G = { y: 20, x0: 40, w: 32, rx: 24, rw: 12, tables: [[44, 30], [56, 30]] };
const SPRITES = {
  ...Object.fromEntries(Object.keys(MEDIEVAL.B).flatMap(k => ['working', 'idle', 'blocked', 'unknown'].map(st => [`B.${k}.${st}`, (c, t) => MEDIEVAL.B[k](c, 20, 20, st, t)]))),
  ...Object.fromEntries(['working', 'idle', 'blocked'].map(st => [`hall.${st}`, c => MEDIEVAL.hall(c, 20, 40, st)])),
  board: c => MEDIEVAL.board(c, 20, 20, [{ label: 'needs-design' }, { label: 'proposal' }, { label: 'conflict' }]),
  dock: c => MEDIEVAL.dock(c, 20, 20, 26),
  scaffold: (c, t) => MEDIEVAL.scaffold(c, 20, 20, t),
  gatehouse: (c, t) => { MEDIEVAL.gatehouse(c, G, t, new Set()); MEDIEVAL.gatehouse(c, G, t, new Set([1, 2])); },
};

test('every sprite has a recorded footprint, and every footprint a sprite', () => {
  assert.deepEqual(Object.keys(SPRITES).sort(), Object.keys(FOOTPRINT).sort());
});

for (const [name, paint] of Object.entries(SPRITES)) {
  test(`medieval ${name}: drawn at 2× inside its 1× footprint`, () => {
    state.T = MEDIEVAL; state.reduce = true;
    const { box, fine } = measure(paint);
    assert.ok(fine > 0, 'draws detail pixels');
    const [a, b, c, d] = FOOTPRINT[name];
    assert.ok(box[0] >= a && box[1] >= b && box[2] <= c && box[3] <= d, `${JSON.stringify(box)} inside ${JSON.stringify(FOOTPRINT[name])}`);
  });
}

test("registerTheme keeps the theme's own slots: nothing wraps them", () => {
  // The theme's houses are houses.js's own, unwrapped: the workshop still builds on the cottage.
  for (const kind of Object.keys(B)) assert.equal(MEDIEVAL.B[kind], B[kind], kind);
});

test('the buildings draw nothing in scene pixels, the notes on the board included', () => {
  state.T = MEDIEVAL; state.reduce = true;
  for (const [name, paint] of Object.entries(SPRITES)) assert.equal(measure(paint).coarse, 0, name);
});

test('a house shows its status: lit glass while working, a dark door when blocked', () => {
  state.T = MEDIEVAL; state.reduce = true;
  // What is painted, in order: the glass's colour differs, and a shut door paints no planks.
  const strokes = st => { const out = []; const c = { fillStyle: '', save() {}, restore() {}, translate() {}, scale() {}, fillRect(...r) { out.push([c.fillStyle, ...r].join()); } }; MEDIEVAL.B.cottage(c, 0, 0, st, 0); return out; };
  assert.ok(strokes('working').some(s => s.startsWith('#f8f8f8')), 'a glint on the glass while working');
  assert.notDeepEqual(strokes('working'), strokes('idle'));
  assert.ok(strokes('blocked').length < strokes('idle').length, 'the blocked door is shut: no planks, hinges or knob');
});
