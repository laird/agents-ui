// The modern theme's scenery at 2× (themes/modern.js, #452): each sprite keeps
// its 1× footprint and gains detail finer than a scene pixel.
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= { matchMedia: () => ({ matches: false }) };
const { MODERN } = await import('../../src/web/village/themes/modern.js');
const { state } = await import('../../src/web/village/state.js');
await import('../../src/web/village/themes/medieval.js');

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

// Each sprite's box as it was drawn at 1×, before #452.
const T = MODERN;
const SPRITES = {
  ground: [c => T.ground(c, 10, 20, 40, 16), [10, 20, 50, 36]],
  road: [c => T.road(c, 10, 20, 40, 8), [10, 20, 50, 28]],
  'road, running down': [c => T.road(c, 10, 20, 8, 40), [10, 20, 18, 60]],
  yard: [c => T.yard(c, 10, 20, 30, 20), [10, 20, 40, 40]],
  'channel (the road with traffic)': [c => T.channel(c, 10, 20, 60, 12, 1000, 1), [10, 20, 70, 32]],
  tree: [c => T.tree(c, 10, 20), [12, 21, 21, 32]],
  'destination (launch pad)': [c => T.destination(c, 10, 80, 0, -1e9), [10, 30, 48, 96]],
  'vessel (rocket)': [c => T.vessel(c, 10, 80, 0, 0, 0), [23, 30, 50, 72]],
  'vessel, climbing': [c => T.vessel(c, 10, 80, 0, 0, -10), [23, 20, 50, 67]],
  'supply (delivery truck)': [c => T.supply(c, 10, 0, 2), [10, 0, 36, 11]],
};

for (const [name, [paint, was]] of Object.entries(SPRITES)) {
  test(`modern ${name}: the same footprint as at 1×, with 2× detail`, () => {
    const rects = drawn(paint);
    assert.deepEqual(box(rects), was, 'its box is unchanged');
    assert.ok(rects.some(r => r.w === 0.5 || r.h === 0.5 || r.x % 1 === 0.5 || r.y % 1 === 0.5), 'something finer than a scene pixel');
  });
}

test('the traffic markings still move with the clock', () => {
  const at = t => drawn(c => T.channel(c, 0, 0, 60, 12, t, 1)).filter(r => r.col === '#f8f8f8').map(r => r.x).join();
  assert.notEqual(at(0), at(120));
});
