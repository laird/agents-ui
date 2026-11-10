// The detail grid (src/web/village/detail.js, #418), under node:
//   node --test tools/village-tests/
// A canvas twice as fine draws every 1× sprite doubled, exactly; a 2× sprite
// draws in half scene pixels. Checked on the draw calls themselves: a fake
// context records each rectangle where it lands on the canvas.
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= {}; // state.js reads window.matchMedia, when there is one
const V = '../../src/web/village/';
const { DETAIL, prepare, detail, sceneScale, toScene } = await import(V + 'detail.js');
const { state } = await import(V + 'state.js');
const scenery = await import(V + 'scenery.js');
const { person } = await import(V + 'people.js');
const { SHADE } = await import(V + 'palette.js');
const houses = await import(V + 'sprites/houses.js');
const { THEMES } = await import(V + 'themes/registry.js');
await import(V + 'themes/medieval.js');
await import(V + 'themes/modern.js');
await import(V + 'themes/future.js');

/* A 2D context that keeps only what reaches the canvas: each fillRect as
 * device-pixel rectangles, through the current transform (scale and
 * translate only, as the village uses; a mirror is recorded too). */
function fakeContext() {
  let m = [1, 0, 0, 1, 0, 0];
  const stack = [], rects = [];
  const c = {
    rects, fillStyle: '#000', globalAlpha: 1, imageSmoothingEnabled: true,
    setTransform(a, b, cc, d, e, f) { m = [a, b, cc, d, e, f]; },
    save() { stack.push([...m]); }, restore() { m = stack.pop(); },
    translate(x, y) { m = [m[0], m[1], m[2], m[3], m[4] + m[0] * x + m[2] * y, m[5] + m[1] * x + m[3] * y]; },
    scale(sx, sy) { m = [m[0] * sx, m[1] * sx, m[2] * sy, m[3] * sy, m[4], m[5]]; },
    fillRect(x, y, w, h) {
      const x0 = m[0] * x + m[4], y0 = m[3] * y + m[5], x1 = m[0] * (x + w) + m[4], y1 = m[3] * (y + h) + m[5];
      rects.push([Math.min(x0, x1), Math.min(y0, y1), Math.abs(x1 - x0), Math.abs(y1 - y0), c.fillStyle, c.globalAlpha]);
    },
  };
  return c;
}
/* Every rectangle `paint` draws, on a canvas `d` times finer. */
function drawn(paint, d) {
  const c = fakeContext();
  prepare(c, d);
  paint(c);
  return c.rects;
}
const doubled = rects => rects.map(([x, y, w, h, col, a]) => [x * DETAIL, y * DETAIL, w * DETAIL, h * DETAIL, col, a]);

/* 1×-only sprites from every corner of the village: crates and their marks,
 * tags, houses and each theme's buildings. (People are drawn at 2× since
 * #419: their test is below and in people2x.test.mjs.) */
const SCENE = c => {
  state.T = THEMES.medieval;
  scenery.crate(c, 3, 4, 'bug', 'P1', true);
  scenery.roped(c, 10, 4);
  scenery.badge(c, 20, 3, 12);
  scenery.numberTag(c, 30, 3, 404);
  houses.pennant(c, 60, 20, 'P0', 0);
  houses.cobwebs(c, 70, 20);
  for (const T of Object.values(THEMES)) {
    state.T = T;
    T.crate(c, 80, 4, 'feature', 'P2', false);
    for (const kind of Object.keys(T.B)) T.B[kind](c, 90, 20, 'working', 0);
    T.hall(c, 120, 18, 'working');
    T.board(c, 160, 28, [{ label: 'needs-design' }, { label: 'proposal' }]);
  }
};

test('a 1×-only scene drawn on the 2× grid is the same scene, doubled', () => {
  const one = drawn(SCENE, 1), two = drawn(SCENE, DETAIL);
  assert.ok(one.length > 200, `the scene draws plenty (${one.length} rectangles)`);
  assert.deepEqual(two, doubled(one));
});

test('a 2× sprite draws in half scene pixels, and leaves the grid as it was', () => {
  const rects = drawn(c => {
    detail(c, 3, 4, d => { d.fillStyle = 'red'; d.fillRect(1, 1, 1, 1); d.fillRect(0, 0, 10, 10); });
    c.fillStyle = 'blue'; c.fillRect(0, 0, 1, 1);
  }, DETAIL);
  assert.deepEqual(rects.map(r => r.slice(0, 5)), [
    [7, 9, 1, 1, 'red'], // one detail pixel, half a scene pixel in from (3, 4)
    [6, 8, 10, 10, 'red'], // a 10×10 detail sprite covers 5×5 scene pixels, as a crate does
    [0, 0, 2, 2, 'blue'], // and a 1× sprite after it is doubled again
  ]);
});

test('a 1× sprite and a 2× sprite of the same thing cover the same canvas', () => {
  const one = drawn(c => c.fillRect(3, 4, 5, 5), DETAIL);
  const two = drawn(c => detail(c, 3, 4, d => d.fillRect(0, 0, 10, 10)), DETAIL);
  assert.deepEqual(two, one);
});

test('the screen maps to scene pixels through the finer canvas', () => {
  // A 200-pixel scene on a 400-pixel canvas shown 800 CSS pixels wide: 4 CSS pixels a scene pixel.
  assert.equal(sceneScale(800, 200 * DETAIL), 4);
  assert.equal(toScene(400), 200);
  assert.equal(sceneScale(800, 200, 1), 4, 'and at 1×, as before');
});

test('people are drawn at 2× (#419): in half scene pixels, inside their 8×12 footprint', () => {
  for (const T of Object.values(THEMES)) {
    state.T = T;
    for (const [seed, kind, opts] of [['worker-1', 'stand', { status: 'idle' }], ['manager:shop', 'stand', { mayor: true, status: 'idle' }]]) {
      const rects = drawn(c => person(c, 40, 10, seed, kind, opts), DETAIL);
      // Some detail pixels are single canvas pixels: finer than a doubled 1× sprite can draw.
      assert.ok(rects.some(([x, y, w, h]) => h === 1 && (x % 2 === 1 || y % 2 === 1)), `${T.name}: ${seed} has 2× detail`);
      // The body stays where the 1× one stood: scene (40, 10) to (48, 22), less the shadow under it.
      const body = rects.filter(([, , , , col]) => col !== SHADE);
      const xs = body.map(([x, , w]) => [x, x + w]).flat(), ys = body.map(([, y, , h]) => [y, y + h]).flat();
      assert.ok(Math.min(...xs) >= 40 * DETAIL - 2 && Math.max(...xs) <= 48 * DETAIL + 2, `${T.name}: ${seed} keeps its width`);
      // (A robot manager's crown stands up to 4 scene pixels over it, as at 1×.)
      assert.ok(Math.min(...ys) >= (10 - 4) * DETAIL && Math.max(...ys) <= 22 * DETAIL + 2, `${T.name}: ${seed} keeps its height`);
    }
  }
});

