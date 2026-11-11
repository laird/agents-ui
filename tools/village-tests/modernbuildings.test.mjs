// The Modern theme's buildings redrawn at 2× (themes/modernhouses.js and
// themes/modern.js, #464): offices of every kind, headquarters, the
// whiteboard, the loading dock, the scaffold and the QA checkpoint. Each
// keeps the footprint it had at 1×, so layout, labels, taps and Fit are
// unchanged, and draws on the detail grid rather than in scene pixels.
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= { matchMedia: () => ({ matches: false }) };
const { MODERN } = await import('../../src/web/village/themes/modern.js');
const { state } = await import('../../src/web/village/state.js');
const { record } = await import('../../src/web/village/upscale.js');
const { NOTECOL } = await import('../../src/web/village/palette.js');

/* A canvas that follows translate/scale and records each rect in scene pixels. */
function recorder() {
  let m = [1, 0, 0, 1, 0, 0]; const stack = [];
  return { rects: [], fillStyle: null,
    save() { stack.push(m.slice()); }, restore() { m = stack.pop(); },
    setTransform(a, b, c, d, e, f) { m = [a, b, c, d, e, f]; },
    translate(x, y) { m = [m[0], m[1], m[2], m[3], m[4] + m[0] * x, m[5] + m[3] * y]; },
    scale(sx, sy) { m = [m[0] * sx, m[1], m[2], m[3] * sy, m[4], m[5]]; },
    fillRect(x, y, w, h) { this.rects.push({ x: m[4] + m[0] * x, y: m[5] + m[3] * y, w: m[0] * w, h: m[3] * h, col: this.fillStyle }); } };
}
const drawn = paint => { const c = recorder(); state.T = MODERN; paint(c); return c.rects; };
const box = rects => [Math.min(...rects.map(r => r.x)), Math.min(...rects.map(r => r.y)), Math.max(...rects.map(r => r.x + r.w)), Math.max(...rects.map(r => r.y + r.h))];
const T = MODERN;
const STATES = ['working', 'idle', 'blocked', 'stalled', 'unknown'];

// Each office's box at (10, 20), as drawn at 1× before #464: working (at
// t = 0 and t = 777, with its crane's hook or swung telescope), and in any
// other state. Working, the café and the lab also smoke and fizz (SMOKY).
const SMOKY = new Set(['cottage', 'tests']);
const OFFICES = {
  cottage: { working: [[10, 11, 26, 36], [10, 11, 26, 36]], other: [10, 23, 26, 36] },
  workshop: { working: [[10, 21, 26, 36], [10, 21, 26, 36]], other: [10, 21, 26, 36] },
  feature: { working: [[10, 16, 26, 36], [10, 16, 26, 36]], other: [10, 16, 26, 36] },
  bug: { working: [[10, 17, 26, 36], [10, 17, 26, 36]], other: [10, 17, 26, 36] },
  security: { working: [[10, 18, 26, 36], [10, 18, 26, 36]], other: [10, 18, 26, 36] },
  tests: { working: [[10, 6, 26, 36], [10, 6, 26, 36]], other: [10, 18, 26, 36] },
  research: { working: [[10, 18, 26, 36], [10, 17, 26, 36]], other: [10, 18, 26, 36] },
  docs: { working: [[10, 18, 26, 36], [10, 18, 26, 36]], other: [10, 18, 26, 36] },
  refactor: { working: [[10, 21, 26, 36], [10, 21, 26, 36]], other: [10, 21, 26, 36] },
  perf: { working: [[10, 19, 26, 36], [10, 19, 26, 36]], other: [10, 19, 26, 36] },
  chores: { working: [[10, 23, 26, 36], [10, 23, 26, 36]], other: [10, 23, 26, 36] },
};

test('every kind of house the theme draws has its office here', () => {
  assert.deepEqual(Object.keys(T.B).sort(), Object.keys(OFFICES).sort());
});

for (const [kind, was] of Object.entries(OFFICES)) {
  test(`modern ${kind} office: the 1× footprint in every state, drawn on the detail grid`, () => {
    STATES.forEach(st => [0, 777].forEach((t, k) => {
      const got = box(drawn(c => T.B[kind](c, 10, 20, st, t)));
      if (st !== 'working' || !SMOKY.has(kind)) return assert.deepEqual(got, st === 'working' ? was.working[k] : was.other, `${st} at t=${t}`);
      // Its smoke or fizz is drawn by sprites/houses.js (#461), and rises
      // above the roof however that draws it; the office's sides and ground
      // stay where they were.
      assert.deepEqual([got[0], got[2], got[3]], [was.other[0], was.other[2], was.other[3]], `${st} at t=${t}`);
      assert.ok(got[1] < was.other[1], `${st} at t=${t}: the smoke rises over the roof`);
    }));
    STATES.forEach(st => {
      const rec = record(r => T.B[kind].raw(r, 10, 20, st, 0));
      assert.deepEqual(rec.grid, [], `${st}: nothing drawn in scene pixels`);
      assert.ok(rec.over.length, `${st}: drawn at 2×`);
    });
  });
}

const GATE = { x0: 30, y: 40, w: 32, rx: 20, rw: 6, tables: [[34, 50], [46, 50]] };
const SITES = {
  'headquarters, working': [c => T.hall(c, 10, 30, 'working'), r => T.hall.raw(r, 10, 30, 'working'), [4, 13, 51, 52]],
  'headquarters, blocked': [c => T.hall(c, 10, 30, 'blocked'), r => T.hall.raw(r, 10, 30, 'blocked'), [4, 13, 51, 52]],
  whiteboard: [c => T.board(c, 10, 20, []), r => T.board.raw(r, 10, 20, []), [10, 20, 28, 35]],
  'loading dock': [c => T.dock(c, 10, 20, 40), r => T.dock.raw(r, 10, 20, 40), [10, 20, 50, 27]],
  scaffold: [c => T.scaffold(c, 10, 20, 90), r => T.scaffold.raw(r, 10, 20, 90), [11, 16, 25, 36]],
  'QA checkpoint, idle': [c => T.gatehouse(c, GATE, 0, new Set()), r => T.gatehouse.raw(r, GATE, 0, new Set()), [16, 40, 62, 63]],
  'QA checkpoint, busy': [c => T.gatehouse(c, GATE, 300, new Set([1, 2])), r => T.gatehouse.raw(r, GATE, 300, new Set([1, 2])), [16, 40, 62, 63]],
};
for (const [name, [paint, raw, was]] of Object.entries(SITES)) {
  test(`modern ${name}: the 1× footprint, drawn on the detail grid`, () => {
    assert.deepEqual(box(drawn(paint)), was);
    const rec = record(raw);
    assert.deepEqual(rec.grid, [], 'nothing drawn in scene pixels');
    assert.ok(rec.over.length > 10, 'drawn at 2×, with detail');
  });
}

test('the whiteboard shows each note in its colour, over the board', () => {
  const rects = drawn(c => T.board(c, 10, 20, [{ label: 'needs-design' }, { label: 'failed' }]));
  const board = rects.findIndex(r => r.col === '#f8f8f8' && r.w === 16);
  [[12, 22, NOTECOL['needs-design']], [16, 22, NOTECOL.failed]].forEach(([x, y, col]) => {
    const at = rects.findIndex((r, i) => i > board && r.col === col && r.x === x && r.y === y && r.w === 3 && r.h === 3);
    assert.ok(at > board, `a ${col} note at (${x}, ${y}), painted after the board`);
  });
});

test('a busy QA table lights up and runs its belt; an idle one stays dark and still', () => {
  const at = (t, busy) => drawn(c => T.gatehouse(c, GATE, t, busy));
  const led = '#58e878';
  assert.ok(at(0, new Set([1])).some(r => r.col === led && r.x >= 34 && r.x < 44), 'table 1 lit');
  assert.ok(!at(0, new Set([1])).some(r => r.col === led && r.x >= 46 && r.x < 56 && r.y > 41), 'table 2 dark');
  const belt = (t, busy) => at(t, busy).filter(r => r.col === '#8890a0' && r.y >= 55 && r.x >= 34 && r.x < 44).map(r => r.x).join();
  assert.notEqual(belt(0, new Set([1])), belt(120, new Set([1])), 'the busy belt moves');
  assert.equal(belt(0, new Set()), belt(120, new Set()), 'the idle belt does not');
});

test('the offices keep the status language: lit windows working, an open door blocked, dark glass unknown', () => {
  const cols = st => new Set(drawn(c => T.B.workshop(c, 10, 20, st, 0)).map(r => r.col));
  assert.ok(cols('working').has('#f8e080'), 'lamp-lit windows');
  assert.ok(!cols('idle').has('#f8e080'));
  assert.ok(cols('unknown').has('#383850'), 'dark glass');
  // Blocked, the door's glass is gone: the doorway is dark.
  const door = st => drawn(c => T.B.workshop(c, 10, 20, st, 0)).some(r => r.col === '#386898' && r.x === 16.5 && r.w === 3);
  assert.ok(door('idle') && !door('blocked'));
});
