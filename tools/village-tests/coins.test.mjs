// Coins by the door (src/web/village/coins.js, #515), under node:
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
// The stack's height scale, the dialog's words, and each theme's stack
// painted onto a pixel grid and read back.
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= {}; // state.js reads window.matchMedia, when there is one
const V = '../../src/web/village/';
const { DETAIL, prepare } = await import(V + 'detail.js');
const { COIN_MAX, COIN_ROWS, coinCount, coins, tokenCount, tokenWords } = await import(V + 'coins.js');
const { THEMES } = await import(V + 'themes/registry.js');
await import(V + 'themes/medieval.js');
await import(V + 'themes/modern.js');
await import(V + 'themes/future.js');

/* A canvas of detail pixels: fillRect lands on a grid through the transform
 * (scale and translate only, as the village uses), recording every rect too. */
function grid(w = 40, h = 40) {
  let m = [1, 0, 0, 1, 0, 0];
  const stack = [], px = new Map(), rects = [];
  const c = {
    fillStyle: '#000', imageSmoothingEnabled: true,
    setTransform(a, b, cc, d, e, f) { m = [a, b, cc, d, e, f]; },
    save() { stack.push([...m]); }, restore() { m = stack.pop(); },
    translate(x, y) { m = [m[0], m[1], m[2], m[3], m[4] + m[0] * x + m[2] * y, m[5] + m[1] * x + m[3] * y]; },
    scale(sx, sy) { m = [m[0] * sx, m[1] * sx, m[2] * sy, m[3] * sy, m[4], m[5]]; },
    fillRect(x, y, rw, rh) {
      rects.push(m[0]);
      const x0 = m[0] * x + m[4], y0 = m[3] * y + m[5];
      for (let yy = Math.round(y0); yy < Math.round(y0 + m[3] * rh); yy++)
        for (let xx = Math.round(x0); xx < Math.round(x0 + m[0] * rw); xx++)
          if (xx >= 0 && yy >= 0 && xx < w && yy < h) px.set(`${xx},${yy}`, c.fillStyle);
    },
    at: (x, y) => px.get(`${x},${y}`),
    px, rects,
  };
  prepare(c, DETAIL);
  return c;
}

test('one coin up to two million tokens, then one per doubling, capped', () => {
  assert.equal(coinCount(undefined), 0, 'no field, no coins');
  assert.equal(coinCount(0), 0, 'nothing used today, no coins');
  assert.equal(coinCount(5000), 1);
  assert.equal(coinCount(1.99e6), 1);
  assert.equal(coinCount(2e6), 2);
  assert.equal(coinCount(16e6), 5);
  assert.equal(coinCount(31.9e6), 5);
  assert.equal(coinCount(90.4e6), 7);
  assert.equal(coinCount(128e6), COIN_MAX);
  assert.equal(coinCount(5e9), COIN_MAX);
});

test('token counts read short', () => {
  assert.deepEqual([262, 262271, 1701089, 90390636, 1.25e9].map(tokenCount), ['262', '262k', '1.7M', '90.4M', '1.3B']);
  // Just under a unit, the next unit up: never 1000k or 1000M.
  assert.deepEqual([999, 999960, 999.96e6, 99.95e6].map(tokenCount), ['999', '1M', '1B', '100M']);
});

test("the house dialog: today's use and the current issue's, with the breakdown", () => {
  const t = (total, output) => ({ input: 1200, output, cache_write: 34000, cache_read: total - 1200 - output - 34000, total });
  const h = { issue: 515, tokens: { today: t(90390636, 262271), issue: t(1701089, 40000) } };
  assert.deepEqual(tokenWords(h), [
    'Tokens today: 90.4M (262k out, 1.2k in, 34k cache write, 90.1M cache read)',
    'Tokens on #515: 1.7M (40k out, 1.2k in, 34k cache write, 1.6M cache read)',
  ]);
  assert.deepEqual(tokenWords({ tokens: { today: t(2e6, 1000) } }).length, 1, 'no issue, no issue line');
  assert.deepEqual(tokenWords({ issue: 7 }), [], 'no field, no words');
  assert.deepEqual(tokenWords(null), []);
});

/* The rows of the grid a stack painted at scene (5, 5) covers, in detail pixels. */
function rowsOf(c) {
  const ys = [...c.px.keys()].map(k => Number(k.split(',')[1]));
  return [Math.min(...ys), Math.max(...ys)];
}

test('every theme stands its stack on the same ground, taller with more coins', () => {
  const ground = 10 + COIN_MAX * 2; // scene (5, 5) is detail (10, 10); the stack stands COIN_MAX scene pixels lower
  for (const name of ['medieval', 'modern', 'future']) {
    const theme = THEMES[name];
    const one = grid(), all = grid();
    coins(one, 5, 5, 1, theme); coins(all, 5, 5, COIN_MAX, theme);
    assert.ok(one.px.size > 0, `${name}: a coin is drawn`);
    assert.equal(rowsOf(all)[1], ground - 1, `${name}: it stands on the ground`);
    assert.equal(rowsOf(one)[1], ground - 1, `${name}: one coin too`);
    // More lit: a taller stack, or a fuller meter window.
    const lit = c => [...c.px.values()].filter(v => v === '#f0c020' || v === '#40e0f0').length;
    assert.ok(lit(all) > lit(one), `${name}: more coins, more lit`);
    assert.ok(all.rects.every(s => s === 1), `${name}: drawn in detail pixels, one to a unit`);
    const empty = grid(); coins(empty, 5, 5, 0, theme);
    assert.equal(empty.px.size, 0, `${name}: no coins, nothing drawn`);
  }
});

test('a coin stack grows two detail rows a coin (gold and credits)', () => {
  for (const name of ['medieval', 'future']) {
    const [top, bottom] = rowsOf((() => { const c = grid(); coins(c, 5, 5, 3, THEMES[name]); return c; })());
    assert.equal(bottom - top + 1, 3 * COIN_ROWS, name);
  }
});
