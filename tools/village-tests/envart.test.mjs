// The environment row's art and the feature flag at 2× (src/web/village/envart.js,
// bannerart.js), #454, #474:
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= {};
const V = '../../src/web/village/';
const { state } = await import(V + 'state.js');
const { THEMES } = await import(V + 'themes/registry.js');
await import(V + 'themes/medieval.js'); await import(V + 'themes/modern.js'); await import(V + 'themes/future.js');
const { bayBanner, envKeep, envShed, envSign, failFlag, flame, poleFlag, signText } = await import(V + 'envart.js');
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
const inside = (r, b) => r.x >= b.x0 && r.y >= b.y0 && r.x + r.w <= b.x1 && r.y + r.h <= b.y1;

/* Each sprite, and the scene pixels its 1× drawing covered (#474 keeps them):
 * [x0, y0, x1, y1], end exclusive. Everything but the banner is drawn on the
 * detail grid now, so that is what the detail must stay inside. */
const SPRITES = {
  shed: [c => envShed(c, 10, 40, 60, 32), [10, 40, 72, 74]],
  keep: [c => envKeep(c, 100, 30, 32, '#38c848', 0), [100, 20, 114, 64]],
  keepWaving: [c => envKeep(c, 100, 30, 32, '#38c848', 700), [100, 20, 114, 64]],
  sign: [c => envSign(c, 5, 30, null, 'release'), [5, 30, 20, 50]],
  keySign: [c => envSign(c, 5, 30), [5, 30, 20, 50]],
  unknownSign: [c => envSign(c, 5, 30, '#c8c8d8'), [5, 30, 20, 50]],
  failFlag: [c => failFlag(c, 50, 30), [50, 30, 56, 39]],
  flame: [c => flame(c, 20, 20, 1), [20, 18, 23, 23]],
  flameLow: [c => flame(c, 20, 20, 0), [20, 18, 23, 23]],
  houseFlag: [c => poleFlag(c, 30, 20, '#40b8e8', 0), [30, 20, 35, 29]],
  houseFlagUp: [c => poleFlag(c, 30, 20, '#40b8e8', 1), [30, 20, 35, 29]],
  shipFlag: [c => poleFlag(c, 30, 20, '#e02828', 1, 7, 5), [30, 20, 36, 27]],
  bayBanner: [c => bayBanner(c, 30, 20, '#40b8e8', 0), [30, 20, 35, 26]],
  bayBannerUp: [c => bayBanner(c, 30, 20, '#40b8e8', 600), [30, 20, 35, 26]],
};
// The flag itself is all on the detail grid now (#475): featureart.test.mjs.
const ON_GRID = new Set(Object.keys(SPRITES));

for (const [name, T] of Object.entries(THEMES)) {
  test(`${name}: the environment row draws at 2× inside its 1× footprint`, () => {
    state.T = T; state.reduce = false;
    for (const [what, [draw, [x0, y0, x1, y1]]] of Object.entries(SPRITES)) {
      const c = canvas(); draw(c);
      const fine = c.rects.filter(r => r.fine), b = { x0: 2 * x0, y0: 2 * y0, x1: 2 * x1, y1: 2 * y1 };
      assert.ok(fine.length > 0, `${what} has no 2× detail`);
      fine.forEach(r => assert.ok(inside(r, b), `${what}'s detail at (${r.x}, ${r.y}) is outside its footprint`));
      // Only its shadow is left in scene pixels.
      if (ON_GRID.has(what)) c.rects.filter(r => !r.fine).forEach(r => assert.match(String(r.col), /rgba/, `${what} still paints ${r.col} in scene pixels`));
    }
  });
}

test('the flags flutter, and hold still when motion is reduced', () => {
  state.T = THEMES.medieval;
  const at = (draw) => { const c = canvas(); draw(c); return JSON.stringify(c.rects); };
  const keep = t => c => envKeep(c, 100, 30, 32, '#38c848', t), flag = w => c => poleFlag(c, 30, 20, '#40b8e8', w);
  state.reduce = false;
  assert.notEqual(at(keep(0)), at(keep(700)), 'the keep\'s flag moves');
  assert.notEqual(at(flag(0)), at(flag(1)), 'a house flag moves');
  state.reduce = true;
  assert.equal(at(keep(0)), at(keep(700)), 'reduced motion: the keep\'s flag is still');
  assert.equal(at(flag(0)), at(flag(1)), 'reduced motion: a house flag is still');
  state.reduce = false;
});

test("the sign letters its environment's name, seven letters at most", () => {
  assert.equal(signText('nuc-1'), 'NUC-1');
  assert.equal(signText('production'), 'PRODUCT');
  assert.equal(signText(undefined), '');
  state.T = THEMES.medieval;
  const ink = name => { const c = canvas(); envSign(c, 5, 30, null, name); return c.rects.filter(r => r.fine && r.col === '#683818' && r.y < 2 * 30 + 9).length; };
  assert.ok(ink('nuc-1') > 10, 'NUC-1 is on the board');
  assert.notEqual(ink('nuc-1'), ink('release'), 'a different name, different letters');
});

test('each theme draws the row its own way', () => {
  const look = T => { state.T = T; const c = canvas(); envShed(c, 10, 40, 60, 32); envKeep(c, 100, 30, 32, '#38c848', 0); envSign(c, 5, 30, null); return JSON.stringify(c.rects); };
  const all = Object.values(THEMES).map(look);
  assert.equal(new Set(all).size, all.length);
});

test("a flag's issue number is drawn on its cloth, in the 2× font (#456)", () => {
  state.T = THEMES.medieval;
  const c = canvas(); drawBanner(c, { x: 20, y: 20, col: '#40b8e8', f: { number: 417, stuck: [] } }, false, 0);
  const ink = c.rects.filter(r => r.fine && r.col === '#f8f8f8');
  assert.ok(ink.length > 10, 'the digits of 417 are on the cloth');
});
