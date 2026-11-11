// The future theme's scenery at 2× (themes/future.js, #453): each sprite
// keeps its 1× footprint and gains detail finer than a scene pixel.
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window = { matchMedia: () => ({ matches: false }) };
performance.now = () => 1000; // the groves glow in turn: pin the clock (#450)
const { hash } = await import('../../src/web/village/palette.js');
const { FUTURE: T } = await import('../../src/web/village/themes/future.js');

/* A canvas that follows translate/scale, and records each rect in scene pixels. */
function recorder() {
  let m = [1, 0, 0, 1, 0, 0]; const stack = [];
  const c = { rects: [], fillStyle: null,
    save() { stack.push(m.slice()); }, restore() { m = stack.pop(); },
    setTransform(a, b, cc, d, e, f) { m = [a, b, cc, d, e, f]; },
    translate(x, y) { m = [m[0], m[1], m[2], m[3], m[4] + m[0] * x, m[5] + m[3] * y]; },
    scale(sx, sy) { m = [m[0] * sx, m[1], m[2], m[3] * sy, m[4], m[5]]; },
    fillRect(x, y, w, h) { this.rects.push({ x: m[4] + m[0] * x, y: m[5] + m[3] * y, w: m[0] * w, h: m[3] * h }); } };
  c.box = () => {
    const xs = c.rects.flatMap(r => [r.x, r.x + r.w]), ys = c.rects.flatMap(r => [r.y, r.y + r.h]);
    return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
  };
  return c;
}
/* Where a tree of each kind stands (hash picks the kind). */
const kit = k => { for (let x = 10; x < 400; x++) if (Math.floor(hash(x, 20) * 3) === k) return x; };

/* Each sprite's box at 1×, in scene pixels: [x0, y0, x1, y1]. */
const FOOTPRINTS = {
  'deck plates': [c => T.ground(c, 0, 0, 120, 80), [0, 0, 120, 80]],
  walkway: [c => T.road(c, 0, 0, 120, 6), [0, 0, 120, 6]],
  'upright walkway': [c => T.road(c, 0, 0, 6, 80), [0, 0, 6, 80]],
  yard: [c => T.yard(c, 0, 0, 48, 9), [0, 0, 48, 9]],
  'plasma canal': [c => T.channel(c, 0, 0, 200, 11, 1234, 1), [0, 0, 200, 11]],
  // The groves (#473) stand where the station kit they replaced stood.
  'crystal spire': [c => T.tree(c, kit(0), 20), [kit(0) + 3, 20, kit(0) + 10, 32]],
  'glow tree': [c => T.tree(c, kit(1), 20), [kit(1), 22, kit(1) + 12, 32]],
  'glowing mushrooms': [c => T.tree(c, kit(2), 20), [kit(2) + 1, 24, kit(2) + 11, 31]],
  spaceport: [c => T.destination(c, 50, 100, 1000, -1e9), [50, 43, 88, 116]],
  shuttle: [c => T.vessel(c, 50, 100, 1000, 0, 0), [65, 54, 94, 92]],
  'shuttle, launching': [c => T.vessel(c, 50, 100, 1000, 0, -20), [65, 34, 94, 76]],
  'cargo shuttle': [c => T.supply(c, 30, 1000, 2), [28, 0, 59, 9]],
  'airlock, shut': [c => T.gate(c, 10, 80, false, '#38c848'), [5, 60, 19, 98]],
  'airlock, open': [c => T.gate(c, 10, 80, true, '#38c848'), [5, 60, 19, 98]],
  drone: [c => T.decor(c, 40, 40, 3, 1000), [40, 41, 48, 48]],
};
for (const [name, [draw, box]] of Object.entries(FOOTPRINTS)) {
  test(`the 2× ${name} keeps its 1× footprint`, () => {
    const c = recorder();
    draw(c);
    assert.deepEqual(c.box(), box);
    assert.ok(c.rects.some(r => r.w % 1 || r.h % 1 || r.x % 1 || r.y % 1), 'and has detail finer than a scene pixel');
  });
}

test('the plasma canal flows with the clock, and holds still with reduced motion (#471)', async () => {
  const { state } = await import('../../src/web/village/state.js');
  const at = t => { const c = recorder(); T.channel(c, 0, 0, 200, 11, t, 1); return JSON.stringify(c.rects); };
  assert.notEqual(at(0), at(2500));
  state.reduce = true;
  try { assert.equal(at(0), at(2500)); } finally { state.reduce = false; }
});
