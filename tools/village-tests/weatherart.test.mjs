// The scoreboard and the weather at 2× (src/web/village/scoreboard.js and
// weatherart.js, #458), under node:
//   node --test tools/village-tests/
// Each sprite is painted onto a pixel grid (a fake context that rasterises
// fillRect through the detail transform) and read back: drawn in detail
// pixels, inside the scene pixels it always covered, in every theme's weather.
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= {}; // state.js reads window.matchMedia, when there is one
const V = '../../src/web/village/';
const { DETAIL, prepare } = await import(V + 'detail.js');
const { state } = await import(V + 'state.js');
const { DIGITS, P } = await import(V + 'palette.js');
const { numberTagSize } = await import(V + 'scenery.js');
const { slate, hotspotSign } = await import(V + 'scoreboard.js');
const W = await import(V + 'weatherart.js');
const { THEMES } = await import(V + 'themes/registry.js');
await import(V + 'themes/medieval.js');
await import(V + 'themes/modern.js');
await import(V + 'themes/future.js');

function grid(w = 200, h = 120) {
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
/* The painted pixels' bounds, in detail pixels. */
function bounds(c) {
  const xs = [...c.px.keys()].map(k => k.split(',').map(Number));
  return { x0: Math.min(...xs.map(([x]) => x)), x1: Math.max(...xs.map(([x]) => x)), y0: Math.min(...xs.map(([, y]) => y)), y1: Math.max(...xs.map(([, y]) => y)) };
}
/* Drawn entirely in detail pixels, inside scene box (x, y, w, h). */
function within(c, x, y, w, h, what) {
  assert.ok(c.rects.length > 0, `${what}: drew something`);
  assert.ok(c.rects.every(s => s === 1), `${what}: every rect is in detail pixels`);
  const b = bounds(c);
  assert.ok(b.x0 >= 2 * x && b.x1 < 2 * (x + w) && b.y0 >= 2 * y && b.y1 < 2 * (y + h), `${what}: ${JSON.stringify(b)} inside ${[x, y, w, h]}`);
}
/* Read a count back off a chit: each digit's 3×5 strokes, 2×2 blocks of `ink`. */
function readChit(c, x, y, len, ink) {
  let s = '';
  for (let j = 0; j < len; j++) {
    let bits = '';
    for (let k = 0; k < 15; k++) bits += c.at(x + 4 + j * 8 + 2 * (k % 3), y + 4 + 2 * Math.floor(k / 3)) === ink ? '1' : '0';
    const d = DIGITS.indexOf(bits);
    s += d < 0 ? '?' : String(d);
  }
  return s;
}

test('the slate keeps its width, draws at 2× on its 12 + legs, and its counts read back', () => {
  state.reduce = true;
  for (const [landed, bounced] of [[14, 6], [3, 120], [0, 0]]) {
    const c = grid();
    const w = slate(c, 10, 10, landed, bounced);
    const wl = numberTagSize(landed).w;
    assert.equal(w, wl + numberTagSize(bounced).w + 5);
    within(c, 10, 10, w, 15, 'slate');
    assert.equal(readChit(c, 22, 22, String(landed).length, P.o), String(landed));
    assert.equal(readChit(c, 24 + 2 * wl, 22, String(bounced).length, P.white), String(bounced));
  }
});

test('the hotspots sign stands in its 8 × 15, its flame flickering unless motion is reduced', () => {
  state.reduce = false;
  const frames = new Set();
  for (let t = 0; t < 900; t += 60) {
    const c = grid();
    hotspotSign(c, 10, 10, t);
    within(c, 10, 10, 8, 15, 'sign');
    frames.add(c.at(20 + 7, 20 + 2));
  }
  assert.equal(frames.size, 2, 'the flame licks higher on the flicker');
  state.reduce = true;
  const still = new Set([0, 220, 440].map(t => { const c = grid(); hotspotSign(c, 10, 10, t); return c.at(27, 22); }));
  assert.equal(still.size, 1);
});

for (const [name, T] of Object.entries(THEMES)) {
  const wx = T.weather;
  test(`${name}: rain, clouds and bolt draw in detail pixels inside their boxes`, () => {
    state.T = T; state.reduce = true;
    let c = grid(); W.rain(c, 10, 10, 40, 20, 30, wx.rain, 1234); within(c, 10, 10, 41, 23, 'rain');
    // A streak per drop, half of them half a scene pixel over: the rain is finer than the grid.
    assert.ok([...c.px.keys()].some(k => +k.split(',')[0] % 2 === 1));
    c = grid(); W.clouds(c, 10, 10, 50, wx, 0); within(c, 4, 8, 70, 9, 'clouds');
    assert.ok([...c.px.values()].includes(wx.cloud) && [...c.px.values()].includes(wx.cloudD));
    c = grid(); W.bolt(c, 20, 10); within(c, 17, 10, 4, 14, 'bolt');
    assert.ok([...c.px.values()].includes(P.gold) && [...c.px.values()].includes(P.white));
  });

  test(`${name}: an alarm lamp is 3 × 2, its lens lit in the theme's alarm or dimmed`, () => {
    state.T = T;
    let c = grid(); W.alarmLamp(c, 10, 10, true, wx.alarm); within(c, 10, 10, 3, 2, 'lamp');
    assert.equal(c.at(23, 21), wx.alarm);
    c = grid(); W.alarmLamp(c, 10, 10, false, wx.alarm);
    assert.equal(c.at(23, 21), P.redD);
  });
}

test('the BUILD BROKEN sign and the blame crate keep their footprints', () => {
  let c = grid(); W.brokenSign(c, 10, 10); within(c, 10, 10, W.SIGN_W, W.SIGN_H, 'sign');
  assert.equal(c.at(20 + 20, 20 + 5), '#d02828');
  // The words: white letter strokes across the board's middle.
  assert.ok([...Array(52).keys()].filter(x => c.at(20 + x, 20 + 8) === P.white).length >= 12);
  c = grid(); W.blameCrate(c, 10, 20); within(c, 10, 13, 7, 14, 'blame');
  assert.equal(c.at(20 + 5, 40 - 12), '#e02828');
});
