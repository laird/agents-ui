// The shadow under a sprite at 2× (palette.js's shadow(), #476): it covers
// the scene box it always did, on the detail grid, soft at its edges.
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';

const { SHADE, SHADE_SOFT, shadow } = await import('../../src/web/village/palette.js');

/* A canvas that follows translate/scale, and records each rect in scene pixels. */
function recorder() {
  let m = [1, 0, 0, 1, 0, 0]; const stack = [];
  return { rects: [], fillStyle: null,
    save() { stack.push(m.slice()); }, restore() { m = stack.pop(); },
    translate(x, y) { m = [m[0], m[1], m[2], m[3], m[4] + m[0] * x, m[5] + m[3] * y]; },
    scale(sx, sy) { m = [m[0] * sx, m[1], m[2], m[3] * sy, m[4], m[5]]; },
    fillRect(x, y, w, h) { this.rects.push({ x: m[4] + m[0] * x, y: m[5] + m[3] * y, w: m[0] * w, h: m[3] * h, col: this.fillStyle }); } };
}
const box = rs => [Math.min(...rs.map(r => r.x)), Math.min(...rs.map(r => r.y)), Math.max(...rs.map(r => r.x + r.w)), Math.max(...rs.map(r => r.y + r.h))];
const at = (rs, x, y) => rs.filter(r => x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h).map(r => r.col);

for (const [w, h] of [[6, 2], [7, 1], [42, 2], [3, 1]]) {
  test(`a ${w}×${h} shadow keeps its scene box, in detail pixels`, () => {
    const c = recorder();
    shadow(c, 10, 12, w, h);
    assert.deepEqual(box(c.rects), [10, 12, 10 + w, 12 + h]);
    assert.ok(c.rects.every(r => r.w % 0.5 === 0 && r.h === 0.5), 'drawn in rows of detail pixels');
    assert.ok(c.rects.some(r => r.col === SHADE), 'a SHADE core');
    // Paler at the ends, never doubled up: each detail pixel is painted once.
    assert.deepEqual(at(c.rects, 10.25, 12 + h / 2 - 0.25), [SHADE_SOFT], 'a soft end');
    for (const r of c.rects) for (const o of c.rects) if (r !== o) assert.ok(r.y !== o.y || r.x + r.w <= o.x || o.x + o.w <= r.x, 'no overlap');
  });
}

test('a shadow two scene pixels tall is rounded: its corners are left clear', () => {
  const c = recorder();
  shadow(c, 0, 0, 6, 2);
  assert.deepEqual(at(c.rects, 0.25, 0.25), [], 'top left');
  assert.deepEqual(at(c.rects, 5.75, 1.75), [], 'bottom right');
  assert.deepEqual(at(c.rects, 3, 0.25), [SHADE_SOFT], 'a soft top edge');
  assert.deepEqual(at(c.rects, 3, 1), [SHADE], 'a darker middle');
});

test('a shadow at a fractional position lands on the nearest detail pixel', () => {
  const c = recorder();
  shadow(c, 10.3, 4.6, 4, 1);
  assert.deepEqual(box(c.rects), [10.5, 4.5, 14.5, 5.5]);
});
