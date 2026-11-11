// The medieval decoration and hedge at 2× (themes/medieval.js, #462): each
// keeps its 1× footprint and gains detail finer than a scene pixel.
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= { matchMedia: () => ({ matches: false }) };
const { MEDIEVAL: T } = await import('../../src/web/village/themes/medieval.js');

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
const drawn = paint => { const c = recorder(); paint(c); return c.rects; };
const box = rects => [Math.min(...rects.map(r => r.x)), Math.min(...rects.map(r => r.y)), Math.max(...rects.map(r => r.x + r.w)), Math.max(...rects.map(r => r.y + r.h))];
const fine = r => r.w === 0.5 || r.h === 0.5 || r.x % 1 === 0.5 || r.y % 1 === 0.5;

// Each decoration's box as it was drawn at 1×, before #462, from (x, y) = (40, 30).
const DECOR = { bush: [0, [40, 30, 48, 36]], flowerbed: [1, [40, 30, 48, 35]], rocks: [2, [40, 31, 47, 35]],
  haystack: [3, [40, 30, 49, 37]], log: [4, [40, 30, 47, 35]], fence: [5, [40, 31, 48, 35]] };

for (const [name, [k, was]] of Object.entries(DECOR)) {
  test(`medieval ${name}: the same footprint as at 1×, with 2× detail`, () => {
    const rects = drawn(c => T.decor(c, 40, 30, k, 0));
    assert.deepEqual(box(rects), was, 'its box is unchanged');
    assert.ok(rects.some(fine), 'something finer than a scene pixel');
  });
}

test('every 2× detail pixel of a decoration lands on its 1× drawing', () => {
  for (const [name, [k]] of Object.entries(DECOR)) {
    const rects = drawn(c => T.decor(c, 40, 30, k, 0));
    const solid = rects.filter(r => !fine(r)), inside = (x, y) => solid.some(r => x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h);
    for (const r of rects.filter(fine)) assert.ok(inside(r.x, r.y) && inside(r.x + r.w - 0.01, r.y + r.h - 0.01), `${name}: ${JSON.stringify(r)} is over the sprite`);
  }
});

test('the hedge keeps to its strips, with single leaves at 2×', () => {
  const W = 120, H = 80, rects = drawn(c => T.border(c, W, H));
  assert.deepEqual(box(rects), [0, 11, W, H], 'its box is unchanged');
  const leaves = rects.filter(fine);
  assert.ok(leaves.length > 20, 'leaves drawn in detail pixels');
  const inStrip = r => r.y >= H - 3 || ((r.x + r.w <= 3 || r.x >= W - 3) && r.y >= 11);
  for (const r of leaves) assert.ok(inStrip(r), `${JSON.stringify(r)} is in the hedge`);
  // The inner outline stays unbroken: no leaf on it.
  for (const r of leaves) assert.ok(!(r.y < H - 2 && r.y + r.h > H - 3 && r.x >= 3 && r.x < W - 3), `${JSON.stringify(r)} clears the outline`);
});
