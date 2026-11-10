/* Things and scenery: speech bubbles, "zzz", "+N" badges, grass, paths, water, trees,
 * the castle, the board, the locks, the harbour, the ship and the supply boat. */
import { state } from './state.js';
import { DIGITS, NOTECOL, P, R, ROPE, STATE, blink, glassOf, hash, shadow } from './palette.js';
import { crate } from './crates.js';
import { detail } from './detail.js';
import { bricks, disc, planks, px } from './art2x.js';

/* ---------- People and things ---------- */
export const ICONS = {
  blocked: ['..#..', '..#..', '..#..', '.....', '..#..'],
  done: ['.....', '....#', '...#.', '#.#..', '.#...'],
  unknown: ['.###.', '#...#', '..##.', '.....', '..#..'],
  stalled: ['.....', '.....', '#.#.#', '.....', '.....']
};
export function bubble(c, x, y, icon) {
  const o = P.o, w = P.white, m = ICONS[icon], col = STATE[icon];
  R(c, x + 1, y, 7, 1, o); R(c, x + 1, y + 8, 7, 1, o); R(c, x, y + 1, 1, 7, o); R(c, x + 8, y + 1, 1, 7, o);
  R(c, x + 1, y + 1, 7, 7, w); R(c, x + 4, y + 8, 1, 2, w); R(c, x + 3, y + 9, 1, 1, o); R(c, x + 5, y + 9, 1, 1, o); R(c, x + 4, y + 10, 1, 1, o);
  for (let r = 0; r < 5; r++) for (let k = 0; k < 5; k++) if (m[r][k] === '#') R(c, x + 2 + k, y + 2 + r, 1, 1, col);
}
/* "zzz" drifting up from a house whose worker is asleep (#411): three z's,
 * small to large, rising and swaying, each outlined so it reads on any
 * ground. With reduced motion they hold still, stacked. */
const ZS = [['###', '.#.', '###'], ['####', '..#.', '.#..', '####']];
export function zzz(c, x, y, t) {
  for (let k = 0; k < 3; k++) {
    const ph = state.reduce ? k / 3 : (t / 2400 + k / 3) % 1;
    if (ph > 0.92) continue; // gone, before it starts again at the roof
    const g = ZS[ph < 0.45 ? 0 : 1];
    const gx = Math.round(x + ph * 6 + (state.reduce ? 0 : Math.sin(ph * 12) * 1.2)), gy = Math.round(y - ph * 12);
    g.forEach((row, r) => [...row].forEach((ch, i) => { if (ch === '#') R(c, gx + i + 1, gy + r + 1, 1, 1, P.o); }));
    g.forEach((row, r) => [...row].forEach((ch, i) => { if (ch === '#') R(c, gx + i, gy + r, 1, 1, P.white); }));
  }
}
/* A rope between a waiting crate and the crate it waits on, both at the
 * docks: dotted, drawn under the crates so only the gaps show it. */
export function tether(c, a, b) {
  const x0 = Math.round(a.x) + 2, y0 = Math.round(a.y) + 2, x1 = Math.round(b.x) + 2, y1 = Math.round(b.y) + 2;
  const n = Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0));
  for (let k = 0; k <= n; k += 2) R(c, x0 + Math.round((x1 - x0) * k / (n || 1)), y0 + Math.round((y1 - y0) * k / (n || 1)), 1, 1, ROPE);
}
/* ---------- Scenery ---------- */
export const FLOWERS = ['#f8f8f8', '#f8d040', '#e070d0', '#e05050', '#a8c8f8'];
/* The ground, the paths, the water and the trees are drawn at 2× (#451):
 * their colour at scene scale, then their detail in detail pixels inside
 * detail(). Each samples where it did at 1× and keeps its footprint, so the
 * map lays out and costs as before. */
export function grass(c, x0, y0, w, h) {
  R(c, x0, y0, w, h, P.grass);
  detail(c, x0, y0, g => {
    // Soft, dithered variation: one detail pixel per sample, set off the scene grid.
    for (let y = 0; y < h; y += 2) for (let x = (y >> 1) % 2; x < w; x += 2) {
      const sx = x0 + x, sy = y0 + y, v = hash(sx >> 3, sy >> 3) * 0.6 + hash(sx >> 2, sy >> 2) * 0.4;
      if (v < 0.2 || v > 0.82) px(g, 2 * x + ((sx + sy) & 1), 2 * y + ((sx >> 1) & 1), 1, 1, v < 0.2 ? '#62a850' : '#4c9040');
    }
    // Tufts of three blades (the middle one tallest), dark specks, and the odd flower with a centre.
    for (let y = 0; y < h; y += 3) for (let x = 0; x < w; x += 3) {
      const sx = x0 + x, sy = y0 + y, v = hash(sx, sy), X = 2 * x, Y = 2 * y;
      if (v < 0.05) { px(g, X, Y, 1, 2, P.grassL); px(g, X + 2, Y - 2, 1, 4, P.grassL); px(g, X + 4, Y, 1, 2, P.grassL); px(g, X + 3, Y + 1, 1, 1, P.grassD); }
      else if (v < 0.1) px(g, X + 1, Y + 1, 1, 1, P.grassD);
      else if (v > 0.985) {
        const petal = FLOWERS[Math.floor(hash(sy, sx) * FLOWERS.length)];
        px(g, X, Y, 2, 1, petal); px(g, X, Y + 1, 1, 1, petal); px(g, X + 1, Y + 1, 1, 1, petal === '#f8d040' ? '#c06020' : '#f8d040');
        px(g, X + 1, Y + 2, 1, 2, P.leafD);
      }
    }
  });
}
export function path(c, x0, y0, w, h) {
  R(c, x0, y0, w, h, P.path);
  detail(c, x0, y0, g => {
    // Pebbles with a shaded underside, and light grit.
    for (let y = 0; y < h; y += 2) for (let x = 0; x < w; x += 2) {
      const v = hash(x0 + x + 7, y0 + y + 3), X = 2 * x + 1, Y = 2 * y + 1;
      if (v < 0.07) { px(g, X, Y, 2, 1, P.pathS); px(g, X + 1, Y + 1, 1, 1, P.stoneD); }
      else if (v < 0.11) px(g, X, Y, 1, 1, P.pathL);
    }
    // Worn edges one detail pixel wide where the path meets the grass, broken here and there.
    const W = 2 * w, H = 2 * h;
    if (w > h) {
      px(g, 0, 0, W, 1, P.pathL); px(g, 0, H - 1, W, 1, P.pathS);
      for (let x = 0; x < W; x += 5) if (hash(x0 * 2 + x, y0) < 0.5) px(g, x, 1, 2, 1, P.pathL);
    } else {
      px(g, 0, 0, 1, H, P.pathL); px(g, W - 1, 0, 1, H, P.pathS);
      for (let y = 0; y < H; y += 5) if (hash(x0, y0 * 2 + y) < 0.5) px(g, 1, y, 1, 2, P.pathL);
    }
  });
}
export function water(c, x0, y0, w, h, t, flow) {
  R(c, x0, y0, w, h, P.water);
  R(c, x0, y0 + 1, w, 2, P.waterD);
  const off = Math.floor(t / 120 * flow);
  detail(c, x0, y0, g => {
    const W = 2 * w, H = 2 * h;
    // Banks: a dark lip on the far side, a sandy edge on the near one, each a detail pixel finer.
    px(g, 0, 0, W, 1, P.pathS); px(g, 0, 1, W, 1, P.stoneD);
    px(g, 0, H - 2, W, 1, P.path); px(g, 0, H - 1, W, 1, P.pathL);
    // Ripples drifting with the flow: a light streak with a brighter glint.
    for (let y = 3; y < h - 2; y += 3) for (let x = 0; x < w; x += 7) {
      if (hash(x0 + x, y0 + y) >= 0.35) continue;
      const X = 2 * ((((x + off) % w) + w) % w);
      px(g, X, 2 * y, Math.min(3, W - X), 1, P.waterL);
      if (X + 1 < W) px(g, X + 1, 2 * y, 1, 1, P.white);
    }
    // Thin reeds on the near bank, and the odd lily pad with a notch and a flower.
    for (let x = 2; x < w - 2; x += 5) {
      const v = hash(x0 + x, y0 + h), X = 2 * x;
      if (v < 0.22) { px(g, X, H - 6, 1, 4, P.leafD); px(g, X + 2, H - 8, 1, 6, P.leaf); px(g, X + 3, H - 5, 1, 3, P.leafD); px(g, X + 2, H - 9, 1, 1, P.woodD); }
      else if (v > 0.93 && h > 9) { const Y = 2 * (h >> 1); px(g, X, Y, 6, 2, P.leaf); px(g, X + 3, Y, 1, 1, P.water); px(g, X + 2, Y, 1, 1, '#e070d0'); }
    }
  });
}
export function tree(c, x, y) {
  shadow(c, x + 4, y + 11, 9, 2);
  detail(c, x, y, g => {
    // The trunk, with bark.
    px(g, 10, 16, 4, 8, P.woodD); px(g, 11, 16, 1, 8, P.wood); px(g, 12, 19, 1, 2, P.o); px(g, 11, 22, 1, 1, P.o);
    // A round canopy: outlined, lit from the top left.
    disc(g, 2, 0, 22, { outline: P.o, fill: P.leaf, light: P.leafL, dark: P.leafD });
    // Leaf clusters and, on some trees, fruit with a glint.
    [[7, 8], [13, 12], [15, 5], [9, 14]].forEach(([dx, dy]) => { px(g, dx, dy, 3, 1, P.leafL); px(g, dx + 1, dy + 1, 2, 1, P.leafD); });
    if (hash(x, y) < 0.3) [[8, 12], [17, 8]].forEach(([fx, fy]) => { px(g, fx, fy, 2, 2, '#e05050'); px(g, fx, fy, 1, 1, '#f8a0a0'); });
  });
}
/* The manager's castle: a keep with banners between two round-roofed
 * towers, behind a crenellated curtain wall. 40 wide, centred on the
 * 32-wide hall slot; the gate opens when the manager waits on you. */
export function hall(c, x, y, st) {
  const x0 = x - 4, gy = y + 22, lit = glassOf(st);
  R(c, x0 + 12, y - 2, 16, 20, P.o); R(c, x0 + 13, y - 1, 14, 18, P.stoneL);
  for (let k = 0; k < 16; k += 3) R(c, x0 + 12 + k, y - 4, 2, 2, P.o);
  R(c, x0 + 16, y + 3, 2, 4, lit); R(c, x0 + 22, y + 3, 2, 4, lit);
  [x0 + 14, x0 + 25].forEach(bx => { R(c, bx, y + 1, 2, 7, '#804098'); R(c, bx, y + 1, 2, 1, P.gold); R(c, bx, y + 8, 1, 1, '#804098'); });
  R(c, x0 + 20, y - 11, 1, 7, P.stoneD); R(c, x0 + 21, y - 11, 5, 3, P.o); R(c, x0 + 21, y - 10, 4, 1, P.gold);
  R(c, x0, gy - 12, 40, 12, P.o); R(c, x0 + 1, gy - 11, 38, 10, P.stone);
  for (let k = 0; k < 40; k += 4) R(c, x0 + k, gy - 14, 2, 2, P.o);
  for (let yy = gy - 9; yy < gy - 1; yy += 3) for (let xx = x0 + 2 + ((yy % 2) ? 2 : 0); xx < x0 + 38; xx += 5) R(c, xx, yy, 1, 1, P.stoneD);
  [x0, x0 + 32].forEach(tx => {
    R(c, tx, y - 4, 8, gy - y + 4, P.o); R(c, tx + 1, y - 3, 6, gy - y + 2, P.stoneL);
    R(c, tx + 3, y + 3, 2, 3, lit);
    for (let r = 0; r < 4; r++) { R(c, tx + 3 - r, y - 8 + r, 2 + 2 * r, 1, P.o); if (r) R(c, tx + 4 - r, y - 8 + r, 2 * r, 1, r % 2 ? P.red : P.redD); }
  });
  R(c, x0 + 16, gy - 9, 8, 9, P.o);
  if (st === 'blocked') R(c, x0 + 17, gy - 8, 6, 8, P.o);
  else { R(c, x0 + 17, gy - 8, 6, 8, P.woodD); for (let k = 0; k < 6; k += 2) R(c, x0 + 17 + k, gy - 8, 1, 8, P.stoneD); }
}
export function board(c, x, y, notes) {
  R(c, x + 1, y + 9, 1, 6, P.woodD); R(c, x + 16, y + 9, 1, 6, P.woodD);
  R(c, x, y, 18, 11, P.o); R(c, x + 1, y + 1, 16, 9, P.wood);
  notes.slice(0, 8).forEach((n, i) => R(c, x + 2 + (i % 4) * 4, y + 2 + Math.floor(i / 4) * 4, 3, 3, NOTECOL[n.label] || P.white));
}
export function lock(c, x, cy, open, flag) {
  R(c, x - 1, cy - 4, 8, 4, P.o); R(c, x, cy - 3, 6, 2, P.stone);
  R(c, x - 1, cy + 14, 8, 4, P.o); R(c, x, cy + 15, 6, 2, P.stone);
  if (!open) { R(c, x + 1, cy, 1, 14, P.woodD); R(c, x + 4, cy, 1, 14, P.woodD); R(c, x + 2, cy, 2, 14, P.wood); }
  R(c, x - 4, cy - 15, 12, 10, P.o); R(c, x - 3, cy - 13, 10, 7, P.stone);
  R(c, x - 5, cy - 18, 14, 5, P.o); R(c, x - 4, cy - 17, 12, 3, P.red);
  R(c, x, cy - 10, 3, 4, P.woodD);
  R(c, x + 8, cy - 25, 1, 10, P.stoneD); R(c, x + 9, cy - 25, 5, 4, P.o); R(c, x + 9, cy - 24, 4, 2, flag);
}
/* Where finished work ships out: a harbour with a lighthouse at the end of
 * the canal. Its lamp flares when a release goes out. The ship moored in it
 * is drawn by ship() below, so it can load cargo and sail. */
export function harbour(c, x, cy, t, fw) {
  R(c, x, cy - 20, 38, 20, P.o); R(c, x + 1, cy - 19, 36, 18, P.stone);
  [x + 3, x + 12, x + 21].forEach(bx => R(c, bx, cy - 4, 3, 3, P.o));
  const lx = x + 27;
  R(c, lx, cy - 50, 9, 32, P.o);
  R(c, lx - 1, cy - 56, 11, 7, P.o); R(c, lx, cy - 55, 9, 4, blink(t, 700) || t - fw < 1600 ? P.lamp : P.glass);
  R(c, lx + 1, cy - 59, 7, 3, P.o); R(c, lx + 2, cy - 58, 5, 1, P.red);
  detail(c, x, cy, g => {
    // The quay wall in brick, a light course along its top; bollards with a cap.
    bricks(g, 2, -38, 72, 34, 8, 4, P.stoneD); px(g, 2, -38, 72, 1, P.stoneL);
    [6, 24, 42].forEach(bx => { px(g, bx + 2, -6, 2, 2, P.stoneD); px(g, bx + 1, -7, 4, 1, P.stoneL); });
    // The lighthouse: red and white bands, a highlight down the lit side, a shade down the other.
    const X = 2 * (lx - x);
    for (let yy = -98; yy < -38; yy += 2) {
      const white = Math.floor((yy / 2) / 5) % 2 !== 0;
      px(g, X + 2, yy, 14, 2, white ? P.white : P.red);
      px(g, X + 3, yy, 1, 2, white ? '#ffffff' : '#f07070');
      px(g, X + 14, yy, 2, 2, white ? P.stoneL : '#a01818');
    }
    px(g, X + 6, -88, 3, 4, P.o); px(g, X + 6, -66, 3, 4, P.o); // windows
    // The lamp room's glazing bars.
    px(g, X + 4, -110, 1, 8, P.o); px(g, X + 13, -110, 1, 8, P.o);
  });
  const ft = t - fw;
  if (ft >= 0 && ft < 1600) {
    const p = ft / 1600;
    [[x + 8, P.gold], [x + 20, P.fizzA], [lx + 4, P.fizzB]].forEach(([fx, col], i) => {
      const fy = cy - 62 - p * 18 - i * 3;
      for (let k = 0; k < 8; k++) { const a = k / 8 * Math.PI * 2; R(c, fx + Math.cos(a) * p * 9, fy + Math.sin(a) * p * 9, 1, 1, col); }
    });
  }
}
/* The ship that carries the next release, moored at harbour x, moved by
 * (ox, oy) while it sails. Its cargo is drawn on deck at SHIP_HOLD. */
/* The supply boat that brings new issues down the river to the docks,
 * with `load` crates still on deck. The river is the map's top 11 rows. */
export function boat(c, x, t, load) {
  x = Math.round(x);
  const y = !state.reduce && blink(t, 600) ? 1 : 0;
  R(c, x, y + 5, 28, 5, P.o); R(c, x + 1, y + 5, 26, 3, P.wood); R(c, x + 26, y + 4, 3, 3, P.o);
  R(c, x + 17, 1, 5, 4, P.o); R(c, x + 18, 1, 4, 3, P.sail);
  detail(c, x, y, g => {
    // Planked hull: a light gunwale, seams, and nails.
    px(g, 2, 10, 52, 1, P.woodL); planks(g, 2, 10, 52, 6, 2, P.woodD);
    for (let k = 6; k < 52; k += 10) px(g, k, 12, 1, 1, P.o);
    // The mast, its stay to the bow, and the sail's seam.
    px(g, 44, -2 * y, 1, 10 + 2 * y, P.woodD);
    for (let k = 0; k < 6; k++) px(g, 45 + k, 2 + k, 1, 1, P.woodD);
    px(g, 36, 4 - 2 * y, 8, 1, P.millD);
  });
  for (let k = 0; k < Math.min(load, 3); k++) crate(c, x + 2 + k * 5, y, 'workshop', null);
}
export function ship(c, x, cy, t, ox, oy) {
  x = Math.round(x + ox); cy = Math.round(cy + oy);
  R(c, x + 2, cy + 3, 22, 9, P.o); R(c, x + 3, cy + 4, 20, 6, P.wood);
  R(c, x + 6, cy - 12, 13, 12, P.o); R(c, x + 7, cy - 11, 11, 10, P.sail);
  R(c, x + 13, cy - 17, 5, 3, P.o); R(c, x + 13, cy - 16, 4, 1, '#804098');
  detail(c, x, cy, g => {
    // Planked hull: a light rail, seams, a dark keel line and portholes.
    px(g, 6, 8, 40, 2, P.woodL); planks(g, 6, 9, 40, 10, 3, P.woodD); px(g, 10, 20, 32, 2, P.woodD);
    [12, 22, 32].forEach(k => { px(g, k, 13, 3, 2, P.o); px(g, k + 1, 13, 1, 1, P.glass); });
    // Rigging: stays from the masthead to bow and stern.
    for (let k = 0; k < 14; k++) { px(g, 24 - k, -26 + 2 * k, 1, 1, P.woodD); px(g, 26 + k, -26 + 2 * k, 1, 1, P.woodD); }
    // The mast, then the sail's seams over it: a vertical seam and two across.
    px(g, 24, -28, 2, 36, P.woodD);
    px(g, 25, -22, 1, 20, P.millD); px(g, 14, -16, 22, 1, P.millD); px(g, 14, -9, 22, 1, P.millD);
  });
}

const PLUS = '000010111010000';
/* A tag of 3x5 glyphs whose ink starts at (x + 1, y): an outline, and inside
 * it a pixel of `bg` all round the ink (#436). Without that pixel above and
 * below, a digit's top and bottom strokes merged into the outline and only the
 * gaps were left to read: "+31" came out as "+=fl". */
function glyphTag(c, x, y, glyphs, bg, ink = P.o) {
  const { w, h } = tagSize(glyphs.length);
  R(c, x - 1, y - 2, w, h, P.o); R(c, x, y - 1, w - 2, h - 2, bg);
  glyphs.forEach((g, j) => { for (let k = 0; k < 15; k++) if (g[k] === '1') R(c, x + 1 + j * 4 + (k % 3), y + Math.floor(k / 3), 1, 1, ink); });
}
/* A tag's outer size for `glyphs` glyphs: 3-wide glyphs a pixel apart, a
 * pixel of padding and a pixel of outline on every side. */
const tagSize = glyphs => ({ w: glyphs * 4 + 3, h: 9 });
/* The outer size of badge(n), and of numberTag(n), from (x - 1, y - 2). */
export const badgeSize = n => tagSize(String(n).length + 1);
export const numberTagSize = n => tagSize(String(n).length);
/* A gold tag reading "+N" in a 3x5 pixel font. */
export function badge(c, x, y, n) {
  glyphTag(c, x, y, [PLUS, ...String(n).split('').map(d => DIGITS[+d])], P.gold);
}
/* A number in the 3x5 font, straight onto whatever is under it (a flag's cloth). */
export function digits(c, x, y, n, col) {
  String(n).split('').forEach((d, j) => { const g = DIGITS[+d]; for (let k = 0; k < 15; k++) if (g[k] === '1') R(c, x + j * 4 + (k % 3), y + Math.floor(k / 3), 1, 1, col); });
}
/* A number on a parchment tag (a crate's issue in a line), in the same font;
 * `bg` and `ink` recolour it (the board's red count). */
export function numberTag(c, x, y, n, bg = P.parch, ink = P.o) {
  glyphTag(c, x, y, String(n).split('').map(d => DIGITS[+d]), bg, ink);
}
