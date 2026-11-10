// The environment row's art and the feature flag at 2× (src/web/village/envart.js,
// bannerart.js), #454:
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= {};
const V = '../../src/web/village/';
const { state } = await import(V + 'state.js');
const { THEMES } = await import(V + 'themes/registry.js');
await import(V + 'themes/medieval.js'); await import(V + 'themes/modern.js'); await import(V + 'themes/future.js');
const { envKeep, envShed, envSign, failFlag, flame, poleFlag } = await import(V + 'envart.js');
const { drawBanner } = await import(V + 'bannerart.js');

/* A 2× canvas that records each rect in canvas pixels, and whether it was
 * drawn on the detail grid (inside detail(), at half a scene pixel). */
function canvas() {
  let m = [2, 0, 0, 2, 0, 0]; const st = [], rects = [];
  const c = { rects, fillStyle: '', globalAlpha: 1,
    save() { st.push([...m]); }, restore() { m = st.pop(); },
    translate(x, y) { m = [m[0], 0, 0, m[3], m[4] + m[0] * x, m[5] + m[3] * y]; },
    scale(s) { m = [m[0] * s, 0, 0, m[3] * s, m[4], m[5]]; },
    fillRect(x, y, w, h) { rects.push({ x: m[0] * x + m[4], y: m[3] * y + m[5], w: m[0] * w, h: m[3] * h, fine: m[0] === 1, col: c.fillStyle }); } };
  return c;
}
const box = rs => rs.reduce((b, r) => ({ x0: Math.min(b.x0, r.x), y0: Math.min(b.y0, r.y), x1: Math.max(b.x1, r.x + r.w), y1: Math.max(b.y1, r.y + r.h) }), { x0: Infinity, y0: Infinity, x1: -Infinity, y1: -Infinity });
const inside = (r, b) => r.x >= b.x0 && r.y >= b.y0 && r.x + r.w <= b.x1 && r.y + r.h <= b.y1;

const SPRITES = {
  shed: c => envShed(c, 10, 40, 60, 32),
  keep: c => envKeep(c, 100, 30, 32, '#38c848', 0),
  sign: c => envSign(c, 5, 30, null),
  failFlag: c => failFlag(c, 50, 30),
  flame: c => flame(c, 20, 20, 1),
  houseFlag: c => poleFlag(c, 30, 20, '#40b8e8', 0),
  shipFlag: c => poleFlag(c, 30, 20, '#e02828', 1, 7, 5),
  banner: c => drawBanner(c, { x: 20, y: 20, col: '#40b8e8', f: { number: 417, stuck: [] } }, false, 0),
  liveStuckBanner: c => drawBanner(c, { x: 20, y: 20, col: '#40b8e8', f: { number: 9, stuck: [1], live: true } }, false, 0)
};

for (const [name, T] of Object.entries(THEMES)) {
  test(`${name}: each sprite gains detail at 2× and keeps its 1× footprint`, () => {
    state.T = T;
    for (const [what, draw] of Object.entries(SPRITES)) {
      const c = canvas(); draw(c);
      const coarse = c.rects.filter(r => !r.fine), fine = c.rects.filter(r => r.fine);
      assert.ok(fine.length > 0, `${what} has no 2× detail`);
      const b = box(coarse);
      fine.forEach(r => assert.ok(inside(r, b), `${what}'s detail at (${r.x}, ${r.y}) is outside its footprint`));
    }
  });
}

test("a flag's issue number is still drawn on its cloth, as the 1× stencil", () => {
  state.T = THEMES.medieval;
  const c = canvas(); drawBanner(c, { x: 20, y: 20, col: '#40b8e8', f: { number: 417, stuck: [] } }, false, 0);
  const ink = c.rects.filter(r => !r.fine && r.col === '#f8f8f8' && r.w === 2 && r.h === 2);
  assert.ok(ink.length > 10, 'the digits of 417 are on the cloth');
});
