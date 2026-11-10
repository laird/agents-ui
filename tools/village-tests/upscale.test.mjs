// Buildings at 2× (src/web/village/upscale.js, #421), under node:
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { hd, opaque, record, scale2x } from '../../src/web/village/upscale.js';

test('record keeps what fillRect paints, by scene pixel, from its top-left', () => {
  const rec = record(c => { c.fillStyle = 'a'; c.fillRect(3, 4, 2, 1); c.fillStyle = 'b'; c.fillRect(4, 5, 1, 1); });
  assert.equal(rec.x0, 3); assert.equal(rec.y0, 4);
  assert.deepEqual(rec.grid, [['a', 'a'], [null, 'b']]);
  assert.deepEqual(record(() => {}).grid, []);
});

test('a see-through colour over another keeps both, to be laid down in order', () => {
  const rec = record(c => { c.fillStyle = '#804020'; c.fillRect(0, 0, 1, 1); c.fillStyle = 'rgba(255,255,240,.3)'; c.fillRect(0, 0, 2, 1); c.fillStyle = '#ffffff'; c.fillRect(0, 0, 1, 1); });
  // Over nothing it is itself; an opaque colour on top replaces the stack.
  assert.deepEqual(rec.grid, [['#ffffff', 'rgba(255,255,240,.3)']]);
  const lit = record(c => { c.fillStyle = '#804020'; c.fillRect(0, 0, 1, 1); c.fillStyle = 'rgba(255,255,240,.3)'; c.fillRect(0, 0, 1, 1); });
  assert.deepEqual(lit.grid, [['#804020|rgba(255,255,240,.3)']]);
  assert.ok(opaque('#123456') && opaque('rgb(1,2,3)') && opaque('rgba(1,2,3,1)'));
  assert.ok(!opaque('rgba(12,20,36,.28)'));
});

test('Scale2x keeps flat colour and straight edges, and smooths a diagonal', () => {
  // A flat block stays flat inside, and its edges straight; only its outer corners round off.
  const block = scale2x([['a', 'a', 'a'], ['a', 'a', 'a'], ['a', 'a', 'a']]);
  assert.ok(block.slice(1, 5).every(row => row.slice(1, 5).every(x => x === 'a')), 'flat inside');
  assert.deepEqual(block[0].slice(1, 5), ['a', 'a', 'a', 'a'], 'a straight top edge');
  assert.equal(block[0][0], null, 'a rounded corner');
  // A diagonal staircase gets its inner corners filled: smoother, still inside its cells.
  const out = scale2x([['a', null], [null, 'a']]);
  assert.equal(out.length, 4);
  assert.deepEqual(out[0], ['a', 'a', null, null]);
  assert.equal(out[1][1], 'a');
  // An outline corner rounds off: the outer corner pixel of a 3×3 square's corner cell is left empty.
  const sq = [[null, null, null, null], [null, 'o', 'o', 'o'], [null, 'o', 'f', 'o'], [null, 'o', 'o', 'o']];
  const big = scale2x(sq);
  assert.equal(big[2][2], null, 'the corner is rounded off');
  assert.deepEqual([big[2][3], big[3][2], big[3][3]], ['o', 'o', 'o'], 'the rest of the corner cell keeps its colour');
});

test('hd draws a slot at 2× once, even when it draws another wrapped slot', () => {
  let calls = 0;
  const inner = hd((c, x, y) => { calls++; c.fillStyle = 'b'; c.fillRect(x, y, 1, 1); });
  const outer = hd((c, x, y) => { c.fillStyle = 'a'; c.fillRect(x, y, 2, 2); inner(c, x + 1, y + 1); });
  assert.equal(hd(outer), outer, 'wrapping twice is wrapping once');
  const rects = [];
  let m = [1, 0, 0, 1, 0, 0];
  const c = {
    fillStyle: '', save() {}, restore() { m = [1, 0, 0, 1, 0, 0]; },
    translate(x, y) { m = [m[0], 0, 0, m[3], m[4] + m[0] * x, m[5] + m[3] * y]; },
    scale(sx, sy) { m = [m[0] * sx, 0, 0, m[3] * sy, m[4], m[5]]; },
    fillRect(x, y, w, h) { rects.push([m[0] * x + m[4], m[3] * y + m[5], m[0] * w, c.fillStyle]); },
  };
  outer(c, 10, 20);
  assert.equal(calls, 1, 'the inner slot ran once, inside the recording');
  // Painted in half scene pixels from (10, 20): 2×2 scene pixels → 4×4 detail rows.
  assert.ok(rects.length >= 4);
  assert.ok(rects.every(([x, y]) => x >= 10 && y >= 20 && x < 12 && y < 22));
  assert.ok(rects.some(([, , , col]) => col === 'b'), 'the inner slot is in it');
});

test('what a building draws at 2× itself (a note glyph, #443) is kept apart and painted on top', async () => {
  const { detail } = await import('../../src/web/village/detail.js');
  const rec = record(c => {
    c.fillStyle = 'w'; c.fillRect(10, 20, 3, 3);
    detail(c, 10, 20, d => { d.fillStyle = 'ink'; d.fillRect(1, 1, 1, 1); });
    c.fillStyle = 'x'; c.fillRect(13, 20, 1, 1); // the transform is back to scene pixels after
  });
  assert.deepEqual(rec.grid, [['w', 'w', 'w', 'x'], ['w', 'w', 'w', null], ['w', 'w', 'w', null]], 'the 2× drawing is not in the scaled grid');
  assert.deepEqual(rec.over, [[10.5, 20.5, 0.5, 0.5, 'ink']], 'it is kept, in scene units, where it was drawn');
  // And hd paints it after the building, one detail pixel at (21, 41).
  const rects = [];
  let m = [2, 0, 0, 2, 0, 0]; const st = [];
  const c = { fillStyle: '', save() { st.push([...m]); }, restore() { m = st.pop(); },
    translate(x, y) { m = [m[0], 0, 0, m[3], m[4] + m[0] * x, m[5] + m[3] * y]; }, scale(s) { m = [m[0] * s, 0, 0, m[3] * s, m[4], m[5]]; },
    fillRect(x, y, w, h) { rects.push([m[0] * x + m[4], m[3] * y + m[5], m[0] * w, m[3] * h, c.fillStyle]); } };
  hd((g, x, y) => { g.fillStyle = 'w'; g.fillRect(x, y, 3, 3); detail(g, x, y, d => { d.fillStyle = 'ink'; d.fillRect(1, 1, 1, 1); }); })(c, 10, 20);
  assert.deepEqual(rects[rects.length - 1], [21, 41, 1, 1, 'ink']);
});
