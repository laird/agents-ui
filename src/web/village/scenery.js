/* Things and scenery: crates, speech bubbles, "zzz", "+N" badges, grass, paths, water, trees,
 * the castle, the board, the locks, the harbour, the ship and the supply boat. */
import { state } from './state.js';
import { KINDCOL, NOTECOL, P, PRIOCOL, R, STATE, blink, glassOf, hash, shadow } from './palette.js';

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
const ROPE = '#d8b070';
/* A crate's epic: the outline in its feature's flag colour, like a ribbon
 * tied round it. Drawn over any theme's crate. */
export function ribbon(c, x, y, col) {
  x = Math.round(x); y = Math.round(y);
  R(c, x, y, 5, 1, col); R(c, x, y + 4, 5, 1, col); R(c, x, y, 1, 5, col); R(c, x + 4, y, 1, 5, col);
}
/* Ready to pick up: a glint now and then on the crate's corner. */
export function glint(c, x, y, t) {
  if ((t % 2400 + 2400) % 2400 >= 180) return;
  x = Math.round(x); y = Math.round(y);
  R(c, x + 4, y - 1, 1, 1, P.white); R(c, x + 5, y, 1, 1, 'rgba(248,248,248,.6)'); R(c, x + 3, y, 1, 1, 'rgba(248,248,248,.6)');
}
/* A rope between a waiting crate and the crate it waits on, both at the
 * docks: dotted, drawn under the crates so only the gaps show it. */
export function tether(c, a, b) {
  const x0 = Math.round(a.x) + 2, y0 = Math.round(a.y) + 2, x1 = Math.round(b.x) + 2, y1 = Math.round(b.y) + 2;
  const n = Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0));
  for (let k = 0; k <= n; k += 2) R(c, x0 + Math.round((x1 - x0) * k / (n || 1)), y0 + Math.round((y1 - y0) * k / (n || 1)), 1, 1, ROPE);
}
/* A crate tied shut: it waits on another issue (a "Depends on" line), so
 * nobody picks it up yet. Drawn over any theme's crate. */
export function roped(c, x, y) {
  x = Math.round(x); y = Math.round(y);
  // Tied across, knotted in the middle: the crate's kind and priority still show.
  R(c, x - 1, y + 2, 7, 1, ROPE); R(c, x + 2, y - 1, 1, 7, ROPE);
  R(c, x + 2, y + 2, 1, 1, P.woodD);
}
export function crate(c, x, y, kind, pr, sealed) {
  x = Math.round(x); y = Math.round(y);
  R(c, x, y, 5, 5, P.o); R(c, x + 1, y + 1, 3, 3, P.woodL);
  R(c, x + 1, y + 1, 3, 1, KINDCOL[kind] || P.woodD);
  // Its priority: a painted band along the foot (none when unprioritised).
  if (PRIOCOL[pr]) R(c, x + 1, y + 3, 3, 1, PRIOCOL[pr]);
  if (sealed) R(c, x + 2, y + 2, 1, 1, P.seal);
}

/* ---------- Scenery ---------- */
export const FLOWERS = ['#f8f8f8', '#f8d040', '#e070d0', '#e05050', '#a8c8f8'];
export function grass(c, x0, y0, w, h) {
  R(c, x0, y0, w, h, P.grass);
  // Soft, dithered variation in the grass, then tufts and the odd flower.
  for (let y = y0; y < y0 + h; y += 2) for (let x = x0 + (y >> 1) % 2; x < x0 + w; x += 2) {
    const v = hash(x >> 3, y >> 3) * 0.6 + hash(x >> 2, y >> 2) * 0.4;
    if (v < 0.2) R(c, x, y, 1, 1, '#62a850'); else if (v > 0.82) R(c, x, y, 1, 1, '#4c9040');
  }
  for (let y = y0; y < y0 + h; y += 3) for (let x = x0; x < x0 + w; x += 3) {
    const v = hash(x, y);
    if (v < 0.05) { R(c, x, y, 1, 1, P.grassL); R(c, x + 2, y, 1, 1, P.grassL); R(c, x + 1, y - 1, 1, 1, P.grassL); }
    else if (v < 0.1) R(c, x, y, 1, 1, P.grassD);
    else if (v > 0.985) { R(c, x, y + 1, 1, 1, P.leafD); R(c, x, y, 1, 1, FLOWERS[Math.floor(hash(y, x) * FLOWERS.length)]); }
  }
}
export function path(c, x0, y0, w, h) {
  R(c, x0, y0, w, h, P.path);
  for (let y = y0; y < y0 + h; y += 2) for (let x = x0; x < x0 + w; x += 2) {
    const v = hash(x + 7, y + 3);
    if (v < 0.07) R(c, x, y, 1, 1, P.pathS); else if (v < 0.11) R(c, x, y, 1, 1, P.pathL);
  }
  // Worn edges where the path meets the grass.
  if (w > h) { R(c, x0, y0, w, 1, P.pathL); R(c, x0, y0 + h - 1, w, 1, P.pathS); }
  else { R(c, x0, y0, 1, h, P.pathL); R(c, x0 + w - 1, y0, 1, h, P.pathS); }
}
export function water(c, x0, y0, w, h, t, flow) {
  R(c, x0, y0, w, h, P.water);
  R(c, x0, y0 + 1, w, 2, P.waterD);
  R(c, x0, y0, w, 1, P.pathS); R(c, x0, y0 + h - 1, w, 1, P.pathL);
  const off = Math.floor(t / 120 * flow);
  for (let y = y0 + 3; y < y0 + h - 2; y += 3) for (let x = x0; x < x0 + w; x += 7) {
    if (hash(x, y) < 0.35) R(c, x0 + (((x - x0 + off) % w) + w) % w, y, 2, 1, P.waterL);
  }
  // Reeds on the near bank and the odd lily pad.
  for (let x = x0 + 2; x < x0 + w - 2; x += 5) {
    const v = hash(x, y0 + h);
    if (v < 0.22) { R(c, x, y0 + h - 3, 1, 2, P.leafD); R(c, x + 1, y0 + h - 4, 1, 3, P.leaf); }
    else if (v > 0.93 && h > 9) { R(c, x, y0 + (h >> 1), 3, 1, P.leaf); R(c, x + 1, y0 + (h >> 1), 1, 1, '#e070d0'); }
  }
}
export function tree(c, x, y) {
  shadow(c, x + 4, y + 11, 9, 2);
  R(c, x + 5, y + 8, 2, 4, P.woodD); R(c, x + 5, y + 8, 1, 4, P.wood);
  const cx = x + 6, cy = y + 5, r = 5;
  for (let dy = -r; dy <= r; dy++) for (let dx = -r; dx <= r; dx++) {
    const d = dx * dx + dy * dy; if (d > r * r) continue;
    R(c, cx + dx, cy + dy, 1, 1, d > (r - 1) * (r - 1) ? P.o : dx + dy > 3 ? P.leafD : dx + dy < -3 ? P.leafL : P.leaf);
  }
  // Leaf clusters and, on some trees, fruit.
  [[-2, -1], [1, 1], [2, -2]].forEach(([dx, dy]) => { R(c, cx + dx, cy + dy, 2, 1, P.leafL); R(c, cx + dx + 1, cy + dy + 1, 1, 1, P.leafD); });
  if (hash(x, y) < 0.3) { R(c, cx - 2, cy + 1, 1, 1, '#e05050'); R(c, cx + 2, cy - 1, 1, 1, '#e05050'); }
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
  for (let yy = cy - 17; yy < cy - 2; yy += 3) for (let xx = x + 2 + ((yy % 2) ? 2 : 0); xx < x + 36; xx += 5) R(c, xx, yy, 1, 1, P.stoneD);
  [x + 3, x + 12, x + 21].forEach(bx => { R(c, bx, cy - 4, 3, 3, P.o); R(c, bx + 1, cy - 3, 1, 1, P.stoneD); });
  const lx = x + 27;
  R(c, lx, cy - 50, 9, 32, P.o);
  for (let yy = cy - 49; yy < cy - 19; yy++) R(c, lx + 1, yy, 7, 1, Math.floor((yy - cy) / 5) % 2 ? P.white : P.red);
  R(c, lx - 1, cy - 56, 11, 7, P.o); R(c, lx, cy - 55, 9, 4, blink(t, 700) || t - fw < 1600 ? P.lamp : P.glass);
  R(c, lx + 1, cy - 59, 7, 3, P.o); R(c, lx + 2, cy - 58, 5, 1, P.red);
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
  R(c, x, y + 5, 28, 5, P.o); R(c, x + 1, y + 5, 26, 3, P.wood); R(c, x + 1, y + 5, 26, 1, P.woodL); R(c, x + 26, y + 4, 3, 3, P.o);
  R(c, x + 22, 0, 1, y + 5, P.woodD); R(c, x + 17, 1, 5, 4, P.o); R(c, x + 18, 1, 4, 3, P.sail);
  for (let k = 0; k < Math.min(load, 3); k++) crate(c, x + 2 + k * 5, y, 'workshop', null);
}
export function ship(c, x, cy, t, ox, oy) {
  x = Math.round(x + ox); cy = Math.round(cy + oy);
  R(c, x + 2, cy + 3, 22, 9, P.o); R(c, x + 3, cy + 4, 20, 6, P.wood); R(c, x + 3, cy + 4, 20, 1, P.woodL); R(c, x + 5, cy + 10, 16, 1, P.woodD);
  R(c, x + 12, cy - 14, 1, 18, P.woodD);
  R(c, x + 6, cy - 12, 13, 12, P.o); R(c, x + 7, cy - 11, 11, 10, P.sail); R(c, x + 12, cy - 11, 1, 10, P.millD);
  R(c, x + 13, cy - 17, 5, 3, P.o); R(c, x + 13, cy - 16, 4, 1, '#804098');
}

export const DIGITS = ['111101101101111', '010110010010111', '111001111100111', '111001111001111', '101101111001001',
  '111100111001111', '111100111101111', '111001001001001', '111101111101111', '111101111001111'];
const PLUS = '000010111010000';
/* A tag of 3x5 glyphs whose ink starts at (x + 1, y): an outline, and inside
 * it a pixel of `bg` all round the ink (#436). Without that pixel above and
 * below, a digit's top and bottom strokes merged into the outline and only the
 * gaps were left to read: "+31" came out as "+=fl". */
function glyphTag(c, x, y, glyphs, bg) {
  const { w, h } = tagSize(glyphs.length);
  R(c, x - 1, y - 2, w, h, P.o); R(c, x, y - 1, w - 2, h - 2, bg);
  glyphs.forEach((g, j) => { for (let k = 0; k < 15; k++) if (g[k] === '1') R(c, x + 1 + j * 4 + (k % 3), y + Math.floor(k / 3), 1, 1, P.o); });
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
/* A number on a parchment tag (a crate's issue in a line), in the same font. */
export function numberTag(c, x, y, n) {
  glyphTag(c, x, y, String(n).split('').map(d => DIGITS[+d]), P.parch);
}
