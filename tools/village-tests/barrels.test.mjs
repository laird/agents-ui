// Context barrels at 2× (src/web/village/barrels.js, #457), under node:
//   node --test tools/village-tests/
// Each theme's barrel is painted onto a pixel grid (a fake context that
// rasterises fillRect through the detail transform) and read back: drawn in
// detail pixels, on the 4×8 scene pixels it always stood on, its level the
// context's colour and as tall as the context is full.
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= {}; // state.js reads window.matchMedia, when there is one
const V = '../../src/web/village/';
const { DETAIL, prepare } = await import(V + 'detail.js');
const { state } = await import(V + 'state.js');
const { P } = await import(V + 'palette.js');
const { barrel, barrelFill, contextColour, BARREL_W, BARREL_H, BARREL_ROWS } = await import(V + 'barrels.js');
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
/* The barrel at scene (5, 5) is at detail (10, 10) on the grid. */
const OX = 10, OY = 10;
const CELL = 'rgba(18,14,42,.35)';

test('the level fills twelve detail rows, in step with the context', () => {
  assert.equal(BARREL_ROWS, 12);
  assert.deepEqual([0, 40, 80, 97, 100, 140, -5].map(barrelFill), [0, 5, 10, 12, 12, 12, 0]);
});

for (const [name, T] of Object.entries(THEMES)) {
  test(`${name}: the barrel draws in detail pixels, not doubled scene pixels`, () => {
    state.T = T; state.reduce = true;
    const c = grid();
    barrel(c, 5, 5, 50, 0, T);
    assert.ok(c.rects.length > 0);
    assert.ok(c.rects.every(s => s === 1), 'every rect is one detail pixel to a unit');
  });

  test(`${name}: the barrel stands on its 4×8 scene pixels (a battery's cap above)`, () => {
    state.T = T; state.reduce = true;
    const c = grid();
    barrel(c, 5, 5, 60, 0, T);
    const xs = [...c.px.keys()].map(k => k.split(',').map(Number));
    assert.equal(Math.min(...xs.map(([x]) => x)), OX);
    assert.equal(Math.max(...xs.map(([x]) => x)), OX + BARREL_W - 1);
    // Its tap reaches one scene pixel (two detail rows) above: a cap may stand there, nothing higher.
    assert.ok(Math.min(...xs.map(([, y]) => y)) >= OY - 2);
    assert.equal(Math.max(...xs.map(([, y]) => y)), OY + BARREL_H - 1);
  });

  test(`${name}: the level shows the context's colour, as tall as it is full`, () => {
    state.T = T; state.reduce = true;
    for (const pct of [40, 80, 92]) {
      const c = grid();
      barrel(c, 5, 5, pct, 0, T);
      const col = contextColour(pct);
      // Up the window's middle column from the bottom, to the lit surface row
      // (a battery's dim cell lines lie over its level).
      let rows = 0;
      for (let y = OY + 13; y >= OY + 2 && [col, CELL].includes(c.at(OX + 3, y)); y--) rows++;
      assert.equal(rows, barrelFill(pct) - 1, `${pct}%`);
      assert.equal(c.at(OX + 3, OY + 13), col, `${pct}%: filled from the bottom`);
    }
  });

  test(`${name}: an empty barrel shows no level`, () => {
    state.T = T; state.reduce = true;
    const c = grid();
    barrel(c, 5, 5, 0, 0, T);
    assert.ok(![...c.px.values()].includes(contextColour(0)));
  });

  test(`${name}: from ${95}% the level flashes white, unless motion is reduced`, () => {
    state.T = T;
    state.reduce = false;
    const seen = new Set();
    for (let t = 0; t < 1200; t += 50) {
      const c = grid();
      barrel(c, 5, 5, 97, t, T);
      seen.add(c.at(OX + 3, OY + 13));
    }
    assert.ok(seen.has(P.white) && seen.has(contextColour(97)));
    state.reduce = true;
    const c = grid();
    barrel(c, 5, 5, 97, 300, T);
    assert.equal(c.at(OX + 3, OY + 13), contextColour(97));
  });
}
