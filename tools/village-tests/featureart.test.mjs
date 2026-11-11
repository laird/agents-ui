// Banners, barges and the fog stop redrawn at 2× (src/web/village/bannerart.js,
// featureart.js, #475), under node:
//   node --test tools/village-tests/
// Each is painted onto a fake context that follows the detail transform, at
// many moments of its animation, in every theme: it is all detail pixels, and
// it stays inside the scene box its 1× art covered (FOOTPRINT, recorded from
// the 1× art before #475). The clock is pinned (#450); the art reads only `t`.
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= {};
globalThis.performance.now = () => 0;
const V = '../../src/web/village/';
const { state } = await import(V + 'state.js');
const { THEMES } = await import(V + 'themes/registry.js');
await import(V + 'themes/medieval.js'); await import(V + 'themes/modern.js'); await import(V + 'themes/future.js');
const { blink } = await import(V + 'palette.js');
const { drawBanner } = await import(V + 'bannerart.js');
const { bargeArt, fogArt } = await import(V + 'featureart.js');

/* The 1× art's scene box, [x0, y0, x1, y1] (x1, y1 exclusive), for each
 * sprite drawn at (20, 20). The barge's is its hull, mast and flag (its
 * crates are the crates' own sprites). */
const FOOTPRINT = {"medieval.barge":[20,14,48,32],"medieval.banner.plain":[18,18,35,45],"medieval.banner.plain.on":[17,11,37,47],"medieval.banner.live":[18,15,35,45],"medieval.banner.live.on":[17,11,37,47],"medieval.banner.stuck":[18,12,35,45],"medieval.banner.stuck.on":[17,11,37,47],"medieval.banner.liveStuck":[18,10,35,45],"medieval.banner.liveStuck.on":[17,10,37,47],"modern.barge":[20,21,45,33],"modern.banner.plain":[18,18,35,45],"modern.banner.plain.on":[17,11,37,47],"modern.banner.live":[18,15,35,45],"modern.banner.live.on":[17,11,37,47],"modern.banner.stuck":[18,12,35,45],"modern.banner.stuck.on":[17,11,37,47],"modern.banner.liveStuck":[18,10,35,45],"modern.banner.liveStuck.on":[17,10,37,47],"future.barge":[20,21,44,32],"future.banner.plain":[18,18,35,45],"future.banner.plain.on":[17,11,37,47],"future.banner.live":[18,15,35,45],"future.banner.live.on":[17,11,37,47],"future.banner.stuck":[18,12,35,45],"future.banner.stuck.on":[17,11,37,47],"future.banner.liveStuck":[18,10,35,45],"future.banner.liveStuck.on":[17,10,37,47]};
const TIMES = [0, 100, 200, 260, 320, 400, 700, 1000, 1400, 2100, 3300];

/* A ×2 context recording each rect in scene pixels, whether it was drawn on
 * the detail grid, and its colour. */
function canvas() {
  let m = { s: 2, x: 0, y: 0 }; const st = [], rects = [];
  const c = { rects, fillStyle: '', globalAlpha: 1,
    save() { st.push({ ...m }); }, restore() { m = st.pop(); },
    translate(x, y) { m = { s: m.s, x: m.x + m.s * x, y: m.y + m.s * y }; },
    scale(s) { m = { s: m.s * s, x: m.x, y: m.y }; },
    fillRect(x, y, w, h) { rects.push({ x: (m.x + m.s * x) / 2, y: (m.y + m.s * y) / 2, w: m.s * w / 2, h: m.s * h / 2, fine: m.s === 1, col: c.fillStyle }); } };
  return c;
}
const boxOf = rs => {
  const b = rs.reduce((b, r) => [Math.min(b[0], r.x), Math.min(b[1], r.y), Math.max(b[2], r.x + r.w), Math.max(b[3], r.y + r.h)], [Infinity, Infinity, -Infinity, -Infinity]);
  return [Math.floor(b[0]), Math.floor(b[1]), Math.ceil(b[2]), Math.ceil(b[3])];
};
const within = (b, f) => b[0] >= f[0] && b[1] >= f[1] && b[2] <= f[2] && b[3] <= f[3];

const FEATURES = { plain: { number: 417, stuck: [] }, live: { number: 9, stuck: [], live: true }, stuck: { number: 12, stuck: [3] }, liveStuck: { number: 1234, stuck: [1], live: true } };
const SPRITES = {
  // drawBarge's bob (features.js) moves the barge a scene pixel; the art draws at it.
  barge: (c, t) => bargeArt(c, 20, 20 + (!state.reduce && blink(t, 700) ? 1 : 0), '#40b8e8', t),
  ...Object.fromEntries(Object.entries(FEATURES).flatMap(([k, f]) => [false, true].map(on =>
    [`banner.${k}${on ? '.on' : ''}`, (c, t) => drawBanner(c, { x: 20, y: 20, col: '#40b8e8', f }, on, t)]))),
};

for (const [name, T] of Object.entries(THEMES)) {
  for (const [what, paint] of Object.entries(SPRITES)) {
    test(`${name} ${what}: drawn at 2× inside its 1× footprint`, () => {
      state.T = T; state.reduce = false;
      const c = canvas();
      TIMES.forEach(t => paint(c, t));
      assert.deepEqual(c.rects.filter(r => !r.fine), [], 'drawn in scene pixels, not in detail()');
      const box = boxOf(c.rects), fp = FOOTPRINT[`${name}.${what}`];
      assert.ok(within(box, fp), `${JSON.stringify(box)} is outside its footprint ${JSON.stringify(fp)}`);
    });
  }
  test(`${name}: the barge and the flag hold still under reduced motion`, () => {
    state.T = T; state.reduce = true;
    for (const paint of Object.values(SPRITES)) {
      const a = canvas(), b = canvas(); paint(a, 0); paint(b, 1234);
      assert.deepEqual(a.rects, b.rects);
    }
    state.reduce = false;
  });
  test(`${name}: the barge's tarp, the flag's ripple and the fog's wisps move`, () => {
    state.T = T; state.reduce = false;
    for (const paint of [SPRITES.barge, SPRITES['banner.plain'], (c, t) => fogArt(c, 100, 50, 32, 60, t)]) {
      const frames = TIMES.map(t => { const c = canvas(); paint(c, t); return JSON.stringify(c.rects); });
      assert.ok(new Set(frames).size > 1, 'the same at every moment');
    }
  });
  test(`${name}: the fog stays inside its stop and is drawn at 2×`, () => {
    state.T = T; state.reduce = false;
    for (const [x, y, w, h] of [[100, 50, 32, 60], [10, 4, 46, 90], [0, 0, 14, 14]]) {
      const c = canvas();
      TIMES.forEach(t => fogArt(c, x, y, w, h, t));
      assert.ok(c.rects.every(r => r.fine), 'drawn in scene pixels');
      assert.deepEqual(boxOf(c.rects), [x, y, x + w, y + h]);
      // Layered: more than the wash alone.
      assert.ok(c.rects.length / TIMES.length > 6, 'no wisps over the wash');
    }
  });
}

test('the cloth carries the feature colour, the number stays in the 2× font', () => {
  state.T = THEMES.medieval; state.reduce = false;
  const c = canvas(); drawBanner(c, { x: 20, y: 20, col: '#40b8e8', f: { number: 417, stuck: [] } }, false, 0);
  assert.ok(c.rects.filter(r => r.col === '#40b8e8').length > 20, 'the cloth is in the feature colour');
  // The 2× font draws its digits below the cloth's own top highlight row.
  assert.ok(c.rects.filter(r => r.col === '#f8f8f8' && r.y >= 22).length > 10, 'the digits of 417 are on the cloth');
});
