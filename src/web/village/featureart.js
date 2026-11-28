/* ---------- A feature's barge, and the fog over a stop, at 2× ----------
 * No DOM, so node tests it (tools/village-tests/featureart.test.mjs).
 * features.js draws them on the canal (#475): each is drawn inside detail()
 * (detail.js), in detail pixels, on the scene pixels its 1× art covered. */
import { state } from './state.js';
import { F, P, ROPE, blink } from './palette.js';
import { detail } from './detail.js';
import { disc, px } from './art2x.js';
import { M, MODERN } from './themes/modern.js';
import { FUTURE } from './themes/future.js';
import { MLIT, MSHADE } from './themes/modernhouses.js';

const RIPPLE = 'rgba(248,250,255,.62)';

/* A wooden barge, in detail pixels from (x, y): a hull of planks rounded at
 * bow and stern, a canvas tarp tied down over the hold (the crates ride on
 * it), a mast with a swallowtail pennant in the feature's colour, and a
 * small wake lapping at each end. */
function bargeMedieval(g, col, t) {
  const still = state.reduce, flap = !still && blink(t, 360);
  // The hull: the gunwale on top, three strakes of planking, a dark keel line.
  for (let j = 12; j < 22; j++) {
    const a = j < 17 ? 0 : j - 16, b = 48 - a;
    px(g, a, j, b - a, 1, P.o);
    if (j > 12 && j < 21) px(g, a + 1, j, b - a - 2, 1, j === 13 ? P.woodL : j === 16 || j === 18 || j === 20 ? P.woodD : P.wood);
  }
  [[13, 14], [31, 14], [21, 17], [37, 19]].forEach(([jx, jy]) => px(g, jx, jy, 1, 2, P.woodD));
  [[6, 14], [26, 17], [42, 14]].forEach(([nx, ny]) => px(g, nx, ny, 1, 1, P.plank));
  // Stem and sternpost, raised a little.
  px(g, 0, 9, 4, 3, P.o); px(g, 1, 10, 2, 2, P.woodL);
  px(g, 46, 9, 2, 3, P.o);
  // The tarp, draped over the gunwale with a scalloped hem, roped down.
  px(g, 3, 11, 40, 4, P.o); px(g, 4, 11, 38, 3, P.sail); px(g, 4, 13, 38, 1, P.millD);
  for (let k = 0; k < 6; k++) { px(g, 5 + k * 6, 15, 4, 1, P.o); px(g, 6 + k * 6, 14, 2, 1, P.sail); }
  [10, 22, 34].forEach(rx => { px(g, rx, 11, 1, 6, ROPE); px(g, rx, 17, 1, 1, P.o); });
  // The mast, and its pennant fluttering.
  px(g, 44, -12, 2, 24, P.o); px(g, 45, -11, 1, 22, P.woodL);
  for (let j = 0; j < 8; j++) {
    const notch = j === 3 || j === 4, len = notch ? 7 : 10 - (flap && (j === 0 || j === 7) ? 1 : 0);
    px(g, 46, -12 + j, len, 1, P.o);
    if (j > 0 && j < 7) px(g, 46, -12 + j, len - 1, 1, col);
  }
  px(g, 46, -11, 8, 1, 'rgba(255,255,255,.4)');
  // A small wake at each end.
  const k = still ? 0 : Math.floor(t / 320) % 2;
  px(g, k, 19, 2, 1, RIPPLE); px(g, 3 - 2 * k, 21, 2, 1, RIPPLE);
  px(g, 46 - k, 19, 2, 1, RIPPLE); px(g, 43 + 2 * k, 21, 2, 1, RIPPLE);
}

/* A flatbed truck downtown: a steel bed with the feature's stripe, a green
 * tarp strapped over the load, a white cab with a glinting windscreen and a
 * headlamp, and three wheels whose hubs turn. */
function bargeModern(g, col, t) {
  px(g, 0, 12, 40, 8, P.o); px(g, 1, 13, 38, 6, M.steel); px(g, 1, 13, 38, 1, MLIT);
  px(g, 1, 15, 38, 2, col); px(g, 1, 17, 38, 1, M.steelD);
  px(g, 3, 11, 34, 4, P.o); px(g, 4, 11, 32, 3, M.tarp); px(g, 4, 13, 32, 1, M.tarpD);
  [10, 22, 33].forEach(sx => px(g, sx, 11, 1, 4, M.crane));
  // The cab, its top corner rounded.
  px(g, 38, 2, 10, 1, P.o); px(g, 38, 3, 12, 17, P.o); px(g, 39, 4, 10, 14, P.white);
  px(g, 41, 4, 8, 6, P.o); px(g, 42, 5, 6, 4, M.glassL); px(g, 43, 5, 2, 1, P.white); px(g, 45, 6, 1, 1, P.white);
  px(g, 40, 11, 1, 7, MSHADE); px(g, 42, 12, 2, 1, M.steelD); px(g, 39, 16, 10, 1, MSHADE);
  px(g, 48, 13, 1, 2, P.lamp); px(g, 38, 18, 12, 2, M.steelD);
  const turn = !state.reduce && blink(t, 300);
  [6, 28, 42].forEach(wx => {
    disc(g, wx, 18, 6, { outline: P.o, fill: M.asphaltD, light: M.steelD, dark: P.o });
    px(g, wx + 2, 20, 2, 2, M.concrete); px(g, wx + (turn ? 3 : 2), turn ? 21 : 20, 1, 1, M.concreteL);
  });
}

/* A hover-barge: a rounded hull with a stripe of the feature's colour, a
 * glass cockpit dome, a glowing cargo field under the crates, and thrusters
 * that pulse, trailing a shimmer. */
function bargeFuture(g, col, t) {
  const still = state.reduce, pulse = !still && blink(t, 200);
  disc(g, 36, 2, 12, { outline: P.o, fill: F.glass, light: F.cyanL, dark: F.glassD });
  px(g, 39, 5, 2, 1, P.white);
  px(g, 4, 10, 32, 1, 'rgba(64,224,240,.35)'); px(g, 4, 11, 32, 1, F.cyanD);
  [4, 35].forEach(cx => px(g, cx, 9, 1, 3, F.cyanL));
  for (let j = 12; j < 20; j++) {
    const a = j === 12 || j === 19 ? 3 : j === 13 || j === 18 ? 1 : 0, b = 48 - a;
    px(g, a, j, b - a, 1, P.o);
    if (j > 12 && j < 19) px(g, a + 1, j, b - a - 2, 1, j === 13 ? F.hullL : j === 15 ? col : j > 16 ? F.hullD : F.hull);
  }
  [14, 28].forEach(sx => px(g, sx, 13, 1, 5, 'rgba(0,0,0,.18)'));
  [[4, 14], [43, 14], [20, 17]].forEach(([rx, ry]) => px(g, rx, ry, 1, 1, F.rivet));
  [6, 20, 34].forEach(tx => px(g, tx, 20, 8, 1, pulse ? F.cyanL : F.cyan));
  px(g, 4, 21, 40, 1, 'rgba(64,224,240,.35)');
  if (!still) for (let k = 0; k < 3; k++) px(g, Math.floor(t / 90 + k * 2) % 6, 20 + (k % 2), 1, 1, F.cyanL);
}

/* The barge (a truck downtown) for feature colour `col`, at scene (x, y),
 * which already carries its bob. The crates it carries are drawn over it. */
export function bargeArt(c, x, y, col, t) {
  const draw = state.T === FUTURE ? bargeFuture : state.T === MODERN ? bargeModern : bargeMedieval;
  detail(c, x, y, g => draw(g, col, t));
}

/* The fog over a stop nothing reports (#569): translucent, so the ship, the
 * river and the lighthouse show through it, and soft at its edges. The wash
 * is FOG_EDGE boxes, each inset a detail pixel inside the last and each
 * FOG_STEP opaque: one at the very edge, all of them over the middle. */
export const FOG_EDGE = 8, FOG_STEP = 0.06;
/* How opaque the wash is `d` detail pixels in from its edge. */
export const fogAlpha = d => 1 - (1 - FOG_STEP) ** Math.min(FOG_EDGE, Math.max(0, Math.floor(d) + 1));
const FOG_RGB = th => th === MODERN ? '170,176,196' : th === FUTURE ? '60,52,110' : '206,210,230';
/* One of the wash's boxes, in theme `th`. */
export const fogWash = th => `rgba(${FOG_RGB(th)},${FOG_STEP})`;
const WISP = th => th === MODERN ? ['rgba(226,228,238,.32)', 'rgba(150,156,176,.22)']
  : th === FUTURE ? ['rgba(150,128,232,.3)', 'rgba(30,24,70,.22)'] : ['rgba(250,251,255,.36)', 'rgba(176,182,210,.26)'];

/* Fog over a w×h scene box at (x, y): the soft-edged wash, and layers of
 * faint wisps drifting slowly across it, each its own way, kept clear of
 * the box's edge so it stays soft. In theme `th` (the key draws it for the
 * theme it shows). */
export function fogArt(c, x, y, w, h, t, th = state.T) {
  const W = 2 * w, H = 2 * h, [light, dark] = WISP(th), wash = fogWash(th);
  const m = Math.min(FOG_EDGE >> 1, W >> 2, H >> 2);
  const run = (g, a, b, j, col) => { a = Math.max(m, a); b = Math.min(W - m, b); if (b > a && j >= m && j < H - m) px(g, a, j, b - a, 1, col); };
  detail(c, x, y, g => {
    for (let k = 0; k < FOG_EDGE && 2 * k < W && 2 * k < H; k++) px(g, k, k, W - 2 * k, H - 2 * k, wash);
    for (let i = 0, Y = 6; Y + 4 <= H; i++, Y += 14) for (let n = 0; n < 2; n++) {
      const L = 16 + ((i * 7 + n * 11) % 3) * 6, span = W + L;
      const drift = state.reduce ? 0 : (i % 2 ? -1 : 1) * t / (170 + i * 50);
      const X = ((((n * W) >> 1) + i * 13 + drift) % span + span) % span - L, col = (i + n) % 2 ? dark : light;
      run(g, X + 3, X + L - 3, Y, col); run(g, X, X + L, Y + 1, col); run(g, X, X + L, Y + 2, col); run(g, X + 3, X + L - 3, Y + 3, col);
    }
  });
}
