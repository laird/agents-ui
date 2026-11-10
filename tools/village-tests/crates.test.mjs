// Crates at 2× (src/web/village/crates.js, #420), under node:
//   node --test tools/village-tests/
// Each theme's crate is painted onto a pixel grid (a fake context that
// rasterises fillRect through the detail transform), and the issue number is
// read back off it: the number is in the sprite, and nothing drawn over the
// crate (rope, ribbon, seal, glint) covers it.
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= {}; // state.js reads window.matchMedia, when there is one
const V = '../../src/web/village/';
const { DETAIL, prepare } = await import(V + 'detail.js');
const { state } = await import(V + 'state.js');
const { DIGITS, KINDCOL, PRIOCOL } = await import(V + 'palette.js');
const crates = await import(V + 'crates.js');
const { CRATE_W, CRATE_H, STENCIL_ROW, stencilColumns, stencilInk, ribbon, roped, glint, dent } = crates;
const { THEMES } = await import(V + 'themes/registry.js');
await import(V + 'themes/medieval.js');
await import(V + 'themes/modern.js');
await import(V + 'themes/future.js');

/* A canvas of detail pixels: fillRect lands on a grid through the transform
 * (scale and translate only, as the village uses). */
function grid(w = 40, h = 30) {
  let m = [1, 0, 0, 1, 0, 0];
  const stack = [], px = new Map();
  const c = {
    fillStyle: '#000', imageSmoothingEnabled: true,
    setTransform(a, b, cc, d, e, f) { m = [a, b, cc, d, e, f]; },
    save() { stack.push([...m]); }, restore() { m = stack.pop(); },
    translate(x, y) { m = [m[0], m[1], m[2], m[3], m[4] + m[0] * x + m[2] * y, m[5] + m[1] * x + m[3] * y]; },
    scale(sx, sy) { m = [m[0] * sx, m[1] * sx, m[2] * sy, m[3] * sy, m[4], m[5]]; },
    fillRect(x, y, rw, rh) {
      const x0 = m[0] * x + m[4], y0 = m[3] * y + m[5];
      for (let yy = Math.round(y0); yy < Math.round(y0 + m[3] * rh); yy++)
        for (let xx = Math.round(x0); xx < Math.round(x0 + m[0] * rw); xx++)
          if (xx >= 0 && yy >= 0 && xx < w && yy < h) px.set(`${xx},${yy}`, c.fillStyle);
    },
    at: (x, y) => px.get(`${x},${y}`),
    px,
  };
  prepare(c, DETAIL);
  return c;
}
/* The crate at scene (5, 5) is at detail (10, 10) on the grid. */
const OX = 10, OY = 10;
/* Read the stencil back: each digit's 3×5 cells, inked or not, matched against the font. */
function readNumber(c, len, [ink, light]) {
  const cols = stencilColumns('9'.repeat(len));
  return cols.map((x0, j) => {
    const col = stencilInk(len, j, ink, light);
    let bits = '';
    for (let k = 0; k < 15; k++) bits += c.at(OX + x0 + (k % 3), OY + STENCIL_ROW + Math.floor(k / 3)) === col ? '1' : '0';
    const d = DIGITS.indexOf(bits);
    return d < 0 ? '?' : String(d);
  }).join('');
}
/* Each theme's ink, and the light ink of the reversed-out middle digit. */
const INK = { medieval: ['#202030', '#e8dcc0'], modern: ['#202030', '#f8f8f8'], future: ['#1c2140', '#40e0f0'] };

for (const [name, T] of Object.entries(THEMES)) {
  test(`${name}: the issue number is stencilled on the crate and reads back`, () => {
    state.T = T;
    for (const n of [7, 42, 420, 999, 108]) {
      const c = grid();
      T.crate(c, 5, 5, 'feature', 'P1', false, n);
      assert.equal(readNumber(c, String(n).length, INK[name]), String(n), `#${n}`);
    }
  });

  test(`${name}: a crate is ${CRATE_W}×${CRATE_H} detail pixels from where it stands`, () => {
    state.T = T;
    const c = grid();
    T.crate(c, 5, 5, 'bug', 'P0', false, 420);
    const xs = [...c.px.keys()].map(k => k.split(',').map(Number));
    // The hover-pad's glow (future) may hang a row under it; nothing goes left, above or right.
    assert.equal(Math.min(...xs.map(([x]) => x)), OX);
    assert.equal(Math.max(...xs.map(([x]) => x)), OX + CRATE_W - 1);
    assert.equal(Math.min(...xs.map(([, y]) => y)), OY);
    assert.ok(Math.max(...xs.map(([, y]) => y)) <= OY + CRATE_H);
    // Its kind and its priority both show.
    assert.ok([...c.px.values()].includes(KINDCOL.bug), 'kind colour');
    assert.ok([...c.px.values()].includes(PRIOCOL.P0), 'priority colour');
  });

  test(`${name}: rope, ribbon, seal, glint and a dent leave the number readable`, () => {
    state.T = T;
    const c = grid();
    T.crate(c, 5, 5, 'feature', 'P2', true, 427);
    ribbon(c, 5, 5, '#e070d0'); roped(c, 5, 5); glint(c, 5, 5, 0); dent(c, 5, 5);
    assert.equal(readNumber(c, 3, INK[name]), '427');
  });

  test(`${name}: four digits don't fit, and are left off rather than cut short`, () => {
    state.T = T;
    const bare = grid(), big = grid();
    T.crate(bare, 5, 5, 'feature', null, false);
    T.crate(big, 5, 5, 'feature', null, false, 1234);
    assert.deepEqual([...big.px], [...bare.px]);
  });
}

for (const [name, T] of Object.entries(THEMES)) test(`${name}: three digits read apart: the middle one is reversed out, light on a dark plate`, () => {
  state.T = T;
  const c = grid();
  T.crate(c, 5, 5, 'feature', null, false, 888);
  const [ink, light] = INK[name], x = stencilColumns(888);
  // The 8s' columns, top row: dark, light, dark, so their edges never merge into one bar.
  assert.equal(c.at(OX + x[0] + 2, OY + STENCIL_ROW), ink);
  assert.equal(c.at(OX + x[1], OY + STENCIL_ROW), light);
  assert.equal(c.at(OX + x[2], OY + STENCIL_ROW), ink);
  // And the middle digit's holes are its plate, not the face: dark round a light digit.
  const hole = c.at(OX + x[1] + 1, OY + STENCIL_ROW + 1);
  assert.notEqual(hole, light);
  assert.notEqual(hole, c.at(OX + x[0] + 1, OY + STENCIL_ROW + 1), 'the plate differs from the face');
});

test('digits sit one to a plank when there are three, centred when fewer', () => {
  assert.deepEqual(stencilColumns(420), [1, 4, 7]);
  assert.deepEqual(stencilColumns(42), [2, 6]);
  assert.deepEqual(stencilColumns(7), [4]);
  assert.deepEqual(stencilColumns(1234), []);
  assert.deepEqual(stencilColumns(undefined), []);
  assert.deepEqual(stencilColumns('x1'), []);
  // Every digit stays on the face, inside the outline.
  for (const n of [1, 12, 123]) for (const x of stencilColumns(n)) assert.ok(x >= 1 && x + 3 <= CRATE_W - 1, `#${n} at ${x}`);
});

test('a waiting crate is visibly roped, and a ribbon edges it in the epic colour', () => {
  state.T = THEMES.medieval;
  const c = grid();
  state.T.crate(c, 5, 5, 'feature', null, false, 42);
  roped(c, 5, 5);
  assert.equal(c.at(OX + 1, OY + 7), '#d8b070', 'rope round it');
  assert.equal(c.at(OX - 1, OY + 7), '#d8b070', 'and past its sides');
  const e = grid();
  ribbon(e, 5, 5, '#e070d0');
  assert.equal(e.at(OX, OY), '#e070d0');
  assert.equal(e.at(OX + CRATE_W - 1, OY + CRATE_H - 1), '#e070d0');
});

test('at the docks a crate fills its slot: neighbours 6 scene pixels apart never overlap', () => {
  // 6 scene pixels = 12 detail pixels; a crate is 11 wide, so a pixel of ground stays between.
  assert.ok(CRATE_W < 6 * DETAIL);
  assert.ok(CRATE_H < 6 * DETAIL);
});
