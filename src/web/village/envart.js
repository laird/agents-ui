/* ---------- The environment row's art at 2× (#454, #474) ----------
 * No DOM, so node tests it (tools/village-tests/envart.test.mjs).
 *
 * Each environment village's warehouse, sign and keep, its health marks
 * (the failed flag, the incident's fire), its feature houses' flags and the
 * banners in its warehouse bay. All of it is drawn inside detail()
 * (detail.js) in detail pixels, half a scene pixel each, with art2x.js's
 * helpers, over the same scene pixels it always covered, so footprints, hit
 * areas and labels stay where they were. Each theme gets its own look: a
 * barn, a stone keep and a carved board in the village; a depot, a glass
 * tower and an LED panel downtown; a hangar, a spire and a holo-sign in the
 * future. Flags flutter, unless motion is reduced. */
import { state } from './state.js';
import { F, P, SHADE, SHINE, shadow } from './palette.js';
import { detail } from './detail.js';
import { bricks, disc, dither, planks, px } from './art2x.js';
import { bitmap, rounded } from './glyphs2x.js';
import { MODERN, M } from './themes/modern.js';
import { FUTURE } from './themes/future.js';

/* The row's colours in the theme shown. */
export const envLook = (T = state.T) => {
  const mod = T === MODERN, fut = T === FUTURE;
  return { roof: fut ? F.purple : mod ? M.steel : P.barn, roofD: fut ? F.purpleD : mod ? M.steelD : P.barnD,
    roofL: fut ? F.plasmaL : mod ? M.concrete : P.red,
    wall: fut ? F.hullD : mod ? M.concrete : P.wood, wallD: fut ? F.deckD : mod ? M.concreteD : P.woodD,
    bay: fut ? F.navy : mod ? M.asphaltD : P.woodD,
    keep: fut ? F.hull : mod ? M.glass : P.stone, keepD: fut ? F.hullD : mod ? M.glassD : P.stoneD,
    pole: fut ? F.hullD : mod ? M.steelD : P.woodD, poleL: fut ? F.hullL : mod ? M.steel : P.woodL,
    mod, fut };
};

/* ---------- Flags ---------- */

/* A flag's cloth, w×h detail pixels from (X, Y), hoisted on a pole at its
 * left: drawn a column at a time, with a ripple running along it that moves
 * on as `wave` (0 or 1) turns. The hoist end stays still; the fly end lifts
 * by a detail pixel at most. An outline, a light hem along the top and the
 * fold in shade where the cloth dips. */
function envCloth(g, X, Y, w, h, col, wave) {
  let prev = 0;
  for (let i = 0; i < w; i++) {
    const dy = i < 2 ? 0 : Math.round(0.5 + 0.5 * Math.sin((i / w) * 5 - wave * Math.PI));
    px(g, X + i, Y + dy, 1, h, P.o);
    if (i < w - 1) {
      px(g, X + i, Y + dy + 1, 1, h - 2, col);
      px(g, X + i, Y + dy + 1, 1, 1, 'rgba(255,255,240,.45)');
      if (dy > prev) px(g, X + i, Y + dy + 2, 1, h - 3, SHADE);
    }
    prev = dy;
  }
}
/* A pole a detail pixel and its highlight wide, `len` long from (X, Y), with a knob on top. */
function envPole(g, X, Y, len, k) {
  px(g, X, Y, 2, len, k.pole); px(g, X, Y + 1, 1, len - 1, k.poleL);
  px(g, X, Y, 2, 1, k.fut ? F.cyan : P.gold);
}
const envWave = (wave) => (state.reduce ? 0 : wave ? 1 : 0);

/* A flag on a pole `pole` tall at x, its cloth `w` wide in `col`, raised by
 * `wave` (0 or 1): a feature house's flag, a failed ship's red one. */
export function poleFlag(c, x, y, col, wave, pole = 9, w = 4) {
  const k = envLook(), up = envWave(wave);
  detail(c, x, y, g => {
    envPole(g, 0, 0, pole * 2, k);
    envCloth(g, 2, 2 * up, w * 2, 6, col, up);
  });
}

/* The red flag over a warehouse whose newest deployment failed, its pole at x. */
export function failFlag(c, x, y) {
  const k = envLook();
  detail(c, x, y, g => {
    envPole(g, 0, 0, 18, k);
    envCloth(g, 2, 0, 10, 6, P.p0, 1);
  });
}

/* An open incident: a flame at (x, y), flickering (`f` 0 or 1; held still
 * when motion is reduced). Red outside, orange, then a gold heart. */
export function flame(c, x, y, f) {
  const u = state.reduce ? 0 : f ? 1 : 0;
  detail(c, x, y, g => {
    // Rows from the tip down: [left, width] of the outer flame, in a 6-wide box.
    const rows = u ? [[2, 1], [2, 2], [1, 3], [1, 4], [0, 5], [0, 6], [0, 6], [1, 4]] : [[3, 1], [2, 2], [1, 4], [0, 5], [0, 6], [0, 6], [1, 4]];
    const top = 6 - rows.length;
    rows.forEach(([l, w], j) => px(g, l, top + j, w, 1, P.p0));
    rows.slice(2).forEach(([l, w], j) => { if (w > 2) px(g, l + 1, top + 2 + j, w - 2, 1, '#f07020'); });
    px(g, 2, 2, 2, 3, P.gold); px(g, 2, 3 + u, 1, 1, P.lamp);
  });
}

/* A feature's banner in the warehouse bay, opening its cargo, in the 5×6
 * slot at (x, y): a little pole and a pennant in the feature's colour. */
export function bayBanner(c, x, y, col, t) {
  const k = envLook(), up = envWave(Math.floor((t || 0) / 600) % 2);
  detail(c, x, y, g => {
    px(g, 1, 11, 5, 1, SHADE);
    envPole(g, 2, 0, 12, k);
    envCloth(g, 4, 0, 6, 6, col, up);
  });
}

/* ---------- The warehouse ---------- */

/* The warehouse, w×h scene pixels at (x, y): a roof with rounded eaves over
 * a wall round the open bay, where its cargo stands. A barn of red shingles
 * and planks in the village, a depot with a corrugated roof and a half-raised
 * shutter downtown, a hangar of hull plates with a lit eave in the future. */
export function envShed(c, x, y, w, h) {
  const k = envLook();
  shadow(c, x + 2, y + h, w, 2);
  detail(c, x, y, g => {
    const W = w * 2, H = h * 2;
    // The outline, rounded at the top corners.
    px(g, 1, 0, W - 2, 1, P.o); px(g, 0, 1, W, H - 1, P.o);
    // The roof.
    px(g, 2, 2, W - 4, 13, k.roof);
    if (k.fut) {
      for (let xx = 2 + 10; xx < W - 4; xx += 10) px(g, xx, 3, 1, 11, k.roofD);
      for (let xx = 6; xx < W - 4; xx += 10) { px(g, xx, 5, 1, 1, F.rivet); px(g, xx, 11, 1, 1, F.rivet); }
      px(g, 2, 2, W - 4, 1, k.roofL);
    } else if (k.mod) {
      for (let xx = 3; xx < W - 3; xx += 3) { px(g, xx, 3, 1, 11, k.roofD); px(g, xx + 1, 3, 1, 11, SHINE); }
      px(g, 2, 2, W - 4, 1, M.concreteL);
    } else {
      bricks(g, 2, 3, W - 4, 12, 6, 3, k.roofD);
      px(g, 2, 2, W - 4, 1, k.roofL);
      // A little moss on the shingles.
      dither(g, 3, 4, W - 6, 9, 7, (xx, yy) => ((xx * 3 + yy) % 5 === 0 ? P.leafL : null));
    }
    // The eave and its shadow.
    px(g, 2, 14, W - 4, 2, k.roofD);
    if (k.fut) px(g, 2, 15, W - 4, 1, F.cyan);
    // The wall round the bay.
    px(g, 2, 16, W - 4, H - 18, k.wall);
    if (k.fut) for (let xx = 10; xx < W - 4; xx += 12) px(g, xx, 16, 1, 2, k.wallD);
    else if (k.mod) for (let xx = 4; xx < W - 3; xx += 3) px(g, xx, 16, 1, 2, k.wallD);
    else planks(g, 2, 16, W - 4, 2, 1, k.wallD);
    px(g, 2, 16, 1, H - 18, SHINE); px(g, W - 3, 16, 1, H - 18, SHADE);
    // The bay, its back wall in shade, and its rails.
    px(g, 4, 18, W - 8, H - 20, k.bay);
    px(g, 4, 18, W - 8, 2, SHADE);
    if (k.fut) {
      // A faint force field across the opening.
      for (let yy = 24; yy < H - 2; yy += 8) px(g, 5, yy, W - 10, 1, 'rgba(64,224,240,.14)');
      px(g, 4, 18, 1, H - 20, F.cyanD); px(g, W - 5, 18, 1, H - 20, F.cyanD);
    } else if (k.mod) {
      // The roller shutter, rolled half up, and hazard stripes on the rails.
      px(g, 4, 18, W - 8, 4, M.steel); px(g, 4, 19, W - 8, 1, M.steelD); px(g, 4, 21, W - 8, 1, M.steelD);
      for (let yy = 22; yy < H - 2; yy += 4) { px(g, 4, yy, 1, 2, M.crane); px(g, W - 5, yy, 1, 2, M.crane); }
    } else {
      for (let xx = 9; xx < W - 5; xx += 8) px(g, xx, 20, 1, H - 22, 'rgba(12,20,36,.18)');
      px(g, 4, 18, 1, H - 20, k.wallD); px(g, W - 5, 18, 1, H - 20, k.wallD);
      // A lantern hanging in the doorway.
      px(g, (W >> 1) - 1, 18, 1, 2, P.o); px(g, (W >> 1) - 2, 20, 3, 2, P.o); px(g, (W >> 1) - 1, 20, 1, 1, P.lamp);
    }
  });
}

/* ---------- The keep ---------- */

/* The capital's keep (a tower downtown, a spire in the future), h tall at
 * (x, y), its flag in the health colour. Twelve scene pixels wide, with
 * merlons (or rooftop plant, or emitter fins) two over its top and the flag
 * on its pole ten over that. */
export function envKeep(c, x, y, h, flag, t) {
  const k = envLook(), wave = envWave(Math.floor((t || 0) / 700) % 2);
  shadow(c, x + 2, y + h, 12, 2);
  detail(c, x, y, g => {
    const H = h * 2;
    envPole(g, 10, -20, 16, k);
    envCloth(g, 12, -20 + 2 * wave, 10, 6, flag, wave);
    // The three merlons over the top, then the tower.
    [0, 8, 16].forEach(mx => {
      px(g, mx, -4, 4, 4, P.o);
      px(g, mx + 1, -3, 2, 3, k.fut ? F.hullD : k.mod ? M.concrete : k.keep);
      px(g, mx + 1, -3, 2, 1, k.fut ? F.cyan : k.mod ? M.concreteL : P.stoneL);
    });
    px(g, 0, 0, 24, H, P.o); px(g, 2, 2, 20, H - 4, k.keep);
    if (k.fut) {
      // Hull plates, a light seam down the middle, round ports up the spire.
      for (let yy = 6; yy < H - 2; yy += 10) px(g, 2, yy, 20, 1, k.keepD);
      px(g, 2, 2, 2, H - 4, F.hullL); px(g, 19, 2, 3, H - 4, SHADE);
      px(g, 11, 2, 2, H - 4, F.cyanD);
      for (let yy = 9; yy < H - 14; yy += 14) disc(g, 8, yy, 8, { outline: P.o, fill: F.glass, light: F.cyanL, dark: F.glassD });
      px(g, 8, H - 10, 8, 8, P.o); px(g, 9, H - 9, 6, 7, F.navy); px(g, 9, H - 9, 6, 1, F.cyan);
    } else if (k.mod) {
      // A glass curtain wall: steel mullions and floors, a streak of sky in it.
      for (let xx = 7; xx < 22; xx += 5) px(g, xx, 2, 1, H - 4, M.steelD);
      for (let yy = 7; yy < H - 2; yy += 6) px(g, 2, yy, 20, 1, M.steelD);
      for (let j = 0; j < H - 12; j++) px(g, 3 + ((j * 2) % 18), 3 + j, 2, 1, j % 18 < 6 ? M.glassL : 'rgba(168,208,240,.35)');
      [[3, 8], [13, 20], [8, 32], [18, 44]].forEach(([wx, wy]) => { if (wy < H - 12) px(g, wx, wy, 4, 5, P.lamp); });
      px(g, 19, 2, 3, H - 4, SHADE);
      px(g, 7, H - 10, 10, 8, P.o); px(g, 8, H - 9, 8, 7, M.glassD); px(g, 12, H - 9, 1, 7, M.steel);
    } else {
      // Ashlar courses, arrow slits with rounded heads, a door and some ivy.
      bricks(g, 2, 2, 20, H - 4, 6, 4, k.keepD);
      px(g, 2, 2, 1, H - 4, P.stoneL); px(g, 19, 2, 3, H - 4, SHADE);
      for (let yy = 10; yy < H - 12; yy += 14) {
        px(g, 10, yy, 4, 7, P.o); px(g, 11, yy - 1, 2, 1, P.o); px(g, 11, yy + 1, 2, 5, P.dark);
        px(g, 9, yy + 7, 6, 1, P.stoneL);
      }
      px(g, 7, H - 12, 10, 10, P.o); px(g, 8, H - 13, 8, 1, P.o);
      px(g, 8, H - 11, 8, 9, P.woodD); planks(g, 8, H - 11, 8, 9, 3, P.wood); px(g, 14, H - 7, 1, 1, P.gold);
      [[2, H - 6], [3, H - 9], [2, H - 12], [4, H - 14], [3, H - 17], [5, H - 4]].forEach(([lx, ly]) => {
        px(g, lx, ly, 2, 2, P.leaf); px(g, lx, ly, 1, 1, P.leafL);
      });
    }
  });
}

/* ---------- The sign ---------- */

/* The sign's lettering: capitals, digits and - . _ in 3×5 detail pixels,
 * each as 15 cells row by row, '#' for ink. */
const ENV_FONT = Object.fromEntries(Object.entries({
  A: '.#.#.#####.##.#', B: '##.#.###.#.###.', C: '.###..#..#...##', D: '##.#.##.##.###.', E: '####..##.#..###',
  F: '####..##.#..#..', G: '.###..#.##.#.##', H: '#.##.#####.##.#', I: '###.#..#..#.###', J: '..#..#..##.#.#.',
  K: '#.##.###.#.##.#', L: '#..#..#..#..###', M: '#.########.##.#', N: '##.#.##.##.##.#', O: '.#.#.##.##.#.#.',
  P: '##.#.###.#..#..', Q: '.#.#.##.###..##', R: '##.#.###.#.##.#', S: '.###...#...###.', T: '###.#..#..#..#.',
  U: '#.##.##.##.####', V: '#.##.##.##.#.#.', W: '#.##.########.#', X: '#.##.#.#.#.##.#', Y: '#.##.#.#..#..#.',
  Z: '###..#.#.#..###', 0: '####.##.##.####', 1: '.#.##..#..#.###', 2: '##...#.#.#..###', 3: '##...#.#...###.',
  4: '#.##.####..#..#', 5: '####..##...###.', 6: '.###..####.####', 7: '###..#.#..#..#.', 8: '####.#####.####',
  9: '####.####..###.', '-': '......###......', '.': '.............#.', _: '............###',
}).map(([ch, cells]) => [ch, cells.padEnd(15, '.').match(/.../g)]));
/* At most seven letters fit across the board. */
export const signText = name => String(name || '').toUpperCase().slice(0, 7);

/* The sign by the road, at (x, y), its board in `face` (grey when nothing
 * says what runs there): a framed board on a post, the environment's `name`
 * lettered across it (its first seven letters) over a line of small print. A carved board on a wooden post in
 * the village, an LED panel on a steel one downtown, a holo-sign on a
 * pylon in the future. */
export function envSign(c, x, y, face, name) {
  const k = envLook(), [frame, board, ink] = state.T.sign || [P.woodD, P.parch, P.woodD];
  detail(c, x, y, g => {
    // The post, from under the board to the ground.
    px(g, 12, 16, 4, 24, P.o); px(g, 13, 16, 2, 24, k.fut ? F.hullD : k.mod ? M.steel : P.wood);
    px(g, 13, 16, 1, 24, k.fut ? F.hullL : k.mod ? M.concreteL : P.woodL);
    if (k.fut) px(g, 13, 30, 2, 2, F.cyan);
    // The board, with rounded corners and its face lit along the top.
    rounded(g, 0, 0, 30, 18, { outline: P.o, fill: face || board, light: k.fut || k.mod ? null : 'rgba(255,255,240,.35)', dark: SHADE });
    if (!k.fut && !k.mod) { px(g, 2, 2, 26, 1, frame); px(g, 3, 3, 1, 1, P.o); px(g, 26, 3, 1, 1, P.o); }
    if (k.fut) px(g, 3, 2, 24, 1, ink);
    // The name, centred, then the small print; a sign with no name (the
    // Key's) has a heading's worth of strokes instead.
    const text = signText(name), X = (30 - (4 * text.length - 1)) >> 1;
    if (text) [...text].forEach((ch, i) => { if (ENV_FONT[ch]) bitmap(g, X + i * 4, 4, ENV_FONT[ch], ink); });
    else { px(g, 5, 5, 8, 3, ink); px(g, 15, 5, 10, 3, ink); }
    px(g, 4, 11, 10, 1, ink); px(g, 16, 11, 6, 1, ink); px(g, 4, 13, 7, 1, ink); px(g, 13, 13, 9, 1, ink);
    if (k.fut) for (let yy = 4; yy < 16; yy += 2) px(g, 2, yy, 26, 1, 'rgba(64,224,240,.12)');
    if (k.mod) px(g, 24, 2, 3, 1, 'rgba(255,255,255,.5)');
  });
}
