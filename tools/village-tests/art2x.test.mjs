// The 2× art helpers (src/web/village/art2x.js) and the medieval scenery drawn
// with them (scenery.js, #451): each 2× sprite keeps its 1× footprint.
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window = { matchMedia: () => ({ matches: false }) };
const { bricks, disc, discRow, dither, lobes, pixmap, planks, px } = await import('../../src/web/village/art2x.js');
const S = await import('../../src/web/village/scenery.js');
const W = await import('../../src/web/village/waterfront.js');

/* A canvas that follows translate/scale, and records each rect in scene pixels. */
function recorder() {
  let m = [1, 0, 0, 1, 0, 0]; const stack = [];
  const c = { rects: [], fillStyle: null,
    save() { stack.push(m.slice()); }, restore() { m = stack.pop(); },
    setTransform(a, b, cc, d, e, f) { m = [a, b, cc, d, e, f]; },
    translate(x, y) { m = [m[0], m[1], m[2], m[3], m[4] + m[0] * x, m[5] + m[3] * y]; },
    scale(sx, sy) { m = [m[0] * sx, m[1], m[2], m[3] * sy, m[4], m[5]]; },
    fillRect(x, y, w, h) { this.rects.push({ x: m[4] + m[0] * x, y: m[5] + m[3] * y, w: m[0] * w, h: m[3] * h, col: this.fillStyle }); } };
  c.box = () => {
    const xs = c.rects.flatMap(r => [r.x, r.x + r.w]), ys = c.rects.flatMap(r => [r.y, r.y + r.h]);
    return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
  };
  return c;
}

test('a disc row is symmetric and inside its box', () => {
  for (const size of [6, 11, 22]) for (let j = 0; j < size; j++) {
    const r = discRow(size, j);
    if (!r) continue;
    assert.ok(r[0] >= 0 && r[1] <= size && r[0] < r[1], `row ${j} of ${size}`);
    assert.equal(r[0], size - r[1], `row ${j} of ${size} is centred`);
  }
  assert.ok(discRow(22, 11)[1] - discRow(22, 11)[0] >= 20, 'the middle row spans the box');
});

test('a disc fills its box with an outline, lit top left and dark bottom right', () => {
  const c = recorder();
  disc(c, 2, 0, 22, { outline: 'O', fill: 'F', light: 'L', dark: 'D' });
  assert.deepEqual(c.box(), [2, 0, 24, 22]);
  const at = (x, y) => c.rects.filter(r => x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h).map(r => r.col).pop();
  assert.equal(at(13, 0), 'O', 'the top edge is outline');
  assert.equal(at(2, 11), 'O', 'so is the left edge');
  assert.equal(at(7, 6), 'L');
  assert.equal(at(18, 17), 'D');
  assert.equal(at(13, 11), 'F');
  assert.ok(c.rects.length < 22 * 5 + 1, 'drawn in runs: at most five per row');
});

test('dither, planks and bricks stay in their boxes', () => {
  const c = recorder();
  dither(c, 10, 10, 20, 8, 2, (x, y) => ((x + y) % 4 === 0 ? 'X' : null));
  planks(c, 10, 30, 20, 6, 2, 'P');
  bricks(c, 40, 0, 32, 16, 8, 4, 'M');
  px(c, 0, 50, 3, 2, 'Q');
  const inside = (r, [x0, y0, x1, y1]) => r.x >= x0 && r.y >= y0 && r.x + r.w <= x1 && r.y + r.h <= y1;
  assert.ok(c.rects.filter(r => r.col === 'X').every(r => inside(r, [10, 10, 30, 18])));
  assert.deepEqual(c.rects.filter(r => r.col === 'P').map(r => r.y), [32, 34], 'seams inside, not on the edges');
  assert.ok(c.rects.filter(r => r.col === 'M').every(r => inside(r, [40, 0, 72, 16])));
});

test('pixmap draws each run of a colour once, and nothing for a dot', () => {
  const c = recorder();
  pixmap(c, 10, 20, ['.aab', 'a..a'], { a: 'A', b: 'B' });
  assert.deepEqual(c.rects.map(r => [r.x, r.y, r.w, r.h, r.col]), [[11, 20, 2, 1, 'A'], [13, 20, 1, 1, 'B'], [10, 21, 1, 1, 'A'], [13, 21, 1, 1, 'A']]);
});

/* Each sprite's box at 1×, in scene pixels: [x0, y0, x1, y1]. The 2× art must not move it. */
const FOOTPRINTS = {
  grass: [c => S.grass(c, 0, 0, 120, 80), [0, -1, 120, 80]],
  path: [c => S.path(c, 0, 0, 120, 6), [0, 0, 120, 6]],
  'path, upright': [c => S.path(c, 0, 0, 6, 80), [0, 0, 6, 80]],
  water: [c => S.water(c, 0, 0, 200, 11, 1234, 1), [0, 0, 200, 11]],
  'cobbled yard': [c => S.cobbles(c, 0, 0, 48, 9), [0, 0, 48, 9]],
  'the docks\' cobbled yard': [c => S.cobbles(c, 6, 18, 56, 34), [6, 18, 62, 52]],
  tree: [c => S.tree(c, 10, 10), [11, 10, 23, 23]],
  'another tree': [c => S.tree(c, 37, 51), [38, 51, 50, 64]],
  harbour: [c => W.harbour(c, 50, 100, 1000, -1e9), [50, 41, 88, 100]],
  boat: [c => W.boat(c, 30, 1000, 2), [30, 0, 59, 11]],
  ship: [c => W.ship(c, 50, 100, 1000, 0, 0), [52, 83, 74, 112]],
  'lock, shut': [c => W.lock(c, 40, 60, false, '#38c848'), [35, 35, 54, 78]],
  'lock, open': [c => W.lock(c, 40, 60, true, '#38c848'), [35, 35, 54, 78]],
};
for (const [name, [draw, box]] of Object.entries(FOOTPRINTS)) {
  test(`the 2× ${name} keeps its 1× footprint`, () => {
    const c = recorder();
    draw(c);
    assert.deepEqual(c.box(), box);
    assert.ok(c.rects.some(r => r.w < 1 || r.h < 1 || r.x % 1 || r.y % 1), 'and has detail finer than a scene pixel');
  });
}

test('lobes share one outline: no outline pixel inside where two discs overlap', () => {
  const c = recorder(), cols = { outline: 'O', fill: 'F', light: 'L', dark: 'D' };
  c.scale(1, 1);
  lobes(c, [[0, 0, 10], [6, 0, 10]], cols);
  // The last colour painted at each pixel.
  const at = new Map();
  for (const r of c.rects) for (let y = r.y; y < r.y + r.h; y++) for (let x = r.x; x < r.x + r.w; x++) at.set(`${x},${y}`, r.col);
  assert.notEqual(at.get('8,5'), 'O', 'the overlap is filled');
  assert.equal(at.get('0,5'), 'O', 'the left edge is outlined');
  assert.equal(at.get('15,5'), 'O', 'the right edge is outlined');
  assert.equal(Math.max(...[...at.keys()].map(k => +k.split(',')[0])), 15, 'it spans both discs');
});

/* The ground, paths, yards and water are wholly at 2× (#471): every rect is
 * drawn inside detail(), in half scene pixels. */
test('the ground, paths, yards and water draw nothing in scene pixels', () => {
  for (const draw of [c => S.grass(c, 0, 0, 60, 40), c => S.path(c, 0, 0, 60, 6), c => S.cobbles(c, 0, 0, 48, 9), c => S.water(c, 0, 0, 100, 11, 900, 1)]) {
    const c = recorder(), scales = [];
    let k = 1; const { scale, save, restore } = c, ks = [];
    c.scale = function (sx, sy) { k *= sx; return scale.call(this, sx, sy); };
    c.save = function () { ks.push(k); return save.call(this); };
    c.restore = function () { k = ks.pop(); return restore.call(this); };
    const fill = c.fillRect; c.fillRect = function (...a) { scales.push(k); return fill.apply(this, a); };
    draw(c);
    assert.ok(scales.length && scales.every(s => s === 0.5), 'every rect inside detail()');
  }
});
test('the cobbles are stones in mortar, lit along their tops', () => {
  const c = recorder();
  S.cobbles(c, 0, 0, 48, 9);
  const cols = new Set(c.rects.map(r => r.col));
  for (const col of [S.COBBLE.mortar, S.COBBLE.stone[0], S.COBBLE.lit, S.COBBLE.foot]) assert.ok(cols.has(col), col);
  assert.ok(c.rects.length > 100, 'one stone at a time, not one fill');
});
test('the stream ripples with the clock, and holds still with reduced motion', async () => {
  const { state } = await import('../../src/web/village/state.js');
  const at = t => { const c = recorder(); S.water(c, 0, 0, 200, 11, t, 1); return JSON.stringify(c.rects); };
  state.reduce = false;
  assert.notEqual(at(0), at(2500));
  state.reduce = true;
  assert.equal(at(0), at(2500));
  state.reduce = false;
});
