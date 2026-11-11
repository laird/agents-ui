// Bounce art at 2× (src/web/village/bounceart.js, #457), under node:
//   node --test tools/village-tests/
// The rework dressing and the broken boards are painted onto a pixel grid
// (a fake context that rasterises fillRect through the detail transform) and
// read back: drawn in detail pixels, inside the bounce's tap, and the crate's
// stencilled number still readable through them.
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= {}; // state.js reads window.matchMedia, when there is one
const V = '../../src/web/village/';
const { DETAIL, prepare } = await import(V + 'detail.js');
const { state } = await import(V + 'state.js');
const { DIGITS, P } = await import(V + 'palette.js');
const { STENCIL_ROW, stencilColumns, stencilInk } = await import(V + 'crates.js');
const { reworked, debris, boards, BOARDS } = await import(V + 'bounceart.js');
const { THEMES } = await import(V + 'themes/registry.js');
await import(V + 'themes/medieval.js');
await import(V + 'themes/modern.js');
await import(V + 'themes/future.js');

function grid(w = 60, h = 60) {
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
const keys = c => [...c.px.keys()].map(k => k.split(',').map(Number));
/* The crate at scene (10, 10) is at detail (20, 20) on the grid. */
const SX = 10, SY = 10, OX = 20, OY = 20;
const INK = { medieval: ['#202030', '#e8dcc0'], modern: ['#202030', '#f8f8f8'], future: ['#1c2140', '#40e0f0'] };
function readNumber(c, len, [ink, light]) {
  return stencilColumns('9'.repeat(len)).map((x0, j) => {
    const col = stencilInk(len, j, ink, light);
    let bits = '';
    for (let k = 0; k < 15; k++) bits += c.at(OX + x0 + (k % 3), OY + STENCIL_ROW + Math.floor(k / 3)) === col ? '1' : '0';
    const d = DIGITS.indexOf(bits);
    return d < 0 ? '?' : String(d);
  }).join('');
}

test('one broken board for each bounce past the first, up to four', () => {
  assert.deepEqual([0, 1, 2, 3, 4, 5, 9].map(boards), [0, 0, 1, 2, 3, 4, 4]);
  assert.equal(BOARDS.length, 4);
});

for (const [name, T] of Object.entries(THEMES)) {
  test(`${name}: the rework dressing draws in detail pixels, inside the bounce's tap`, () => {
    state.T = T; state.reduce = true;
    const c = grid();
    reworked(c, SX, SY, 0, T);
    assert.ok(c.rects.length > 0 && c.rects.every(s => s === 1));
    // bounces.js taps scene (x-1, y-2) 8×9: detail x-2..x+13, y-4..y+13.
    const xs = keys(c);
    assert.ok(Math.min(...xs.map(([x]) => x)) >= OX - 2);
    assert.ok(Math.max(...xs.map(([x]) => x)) <= OX + 13);
    assert.ok(Math.min(...xs.map(([, y]) => y)) >= OY - 4);
    assert.ok(Math.max(...xs.map(([, y]) => y)) <= OY + 13);
  });

  test(`${name}: the crate's number still reads through the rework dressing`, () => {
    state.T = T; state.reduce = true;
    const c = grid();
    T.crate(c, SX, SY, 'feature', 'P1', false, 433);
    reworked(c, SX, SY, 0, T);
    assert.equal(readNumber(c, 3, INK[name]), '433');
  });

  test(`${name}: broken boards draw in detail pixels, one per bounce past the first`, () => {
    state.T = T;
    const lot = { x: 0, y: 0 };
    for (const count of [1, 2, 3, 5]) {
      const c = grid();
      debris(c, lot, count, T);
      assert.ok(c.rects.every(s => s === 1));
      // Each board's lit top row starts at its corner.
      const light = name === 'future' ? P.stone : P.plank;
      const lit = BOARDS.filter(([bx, by]) => c.at(32 + bx, 36 + by) === light).length;
      assert.equal(lit, boards(count), `${count} bounces`);
      // The pile lies beside the house, at scene (x+16, y+18) and within three scene pixels of it.
      keys(c).forEach(([x, y]) => assert.ok(x >= 32 && x < 32 + 12 && y >= 35 && y < 36 + 10, `${x},${y}`));
    }
  });
}
