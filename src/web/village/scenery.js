/* Things and scenery: speech bubbles, "zzz", "+N" badges, grass, paths, cobbles, water, trees,
 * the castle and the board. The waterfront (lock, harbour, ship, boat) is
 * waterfront.js. */
import { state } from './state.js';
import { NOTECOL, P, ROPE, STATE, SHADE, hash } from './palette.js';
import { detail } from './detail.js';
import { disc, px } from './art2x.js';
import { bitmap, rounded, tail, write } from './glyphs2x.js';

/* ---------- People and things ---------- */
/* The quiet bubbles' icons (#456): 10 by 10 detail pixels, drawn in the
 * bubble's state colour. */
export const ICONS = {
  blocked: ['...####...', '...####...', '...####...', '....##....', '....##....', '....##....', '..........', '..........', '....##....', '....##....'],
  done: ['..........', '........##', '.......###', '......###.', '##...###..', '###.###...', '.#####....', '..###.....', '...#......', '..........'],
  unknown: ['...####...', '..##..##..', '......##..', '.....##...', '....##....', '....##....', '..........', '..........', '....##....', '....##....'],
  stalled: ['..........', '..........', '..........', '..........', '##..##..##', '##..##..##', '..........', '..........', '..........', '..........']
};
/* A speech bubble, 9 by 11 scene pixels from (x, y): a rounded box, its
 * tail pointing down at whoever it is over. Drawn at 2× (#456). */
export const SPEECH = { w: 18, h: 18, tail: [6, 12] };
export function speech(g, cols) {
  rounded(g, 0, 0, SPEECH.w, SPEECH.h, cols, SPEECH.tail);
  tail(g, SPEECH.tail[0], SPEECH.h, SPEECH.tail[1] - SPEECH.tail[0], 4, cols);
}
export function bubble(c, x, y, icon) {
  detail(c, x, y, g => {
    speech(g, { outline: P.o, fill: P.white, dark: '#d0d4e0' });
    bitmap(g, 4, 4, ICONS[icon], STATE[icon]);
  });
}
/* "zzz" drifting up from a house whose worker is asleep (#411): three z's,
 * small to large, rising and swaying, each outlined so it reads on any
 * ground. With reduced motion they hold still, stacked. Drawn at 2× (#456),
 * so they drift half a pixel at a time. */
const ZS = [['######', '....##', '...##.', '.##...', '##....', '######'],
  ['########', '......##', '.....##.', '....##..', '..##....', '.##.....', '##......', '########']];
/* A glyph's outline: every pixel next to its ink (diagonals too) that is not ink. */
export const halo = rows => rows.map((row, j) => [...row].map((ch, i) => {
  if (ch === '#') return '.';
  for (let dj = -1; dj <= 1; dj++) for (let di = -1; di <= 1; di++) if ((rows[j + dj] || '')[i + di] === '#') return '#';
  return '.';
}).join(''));
const pad = rows => ['', ...rows, ''].map(r => `.${r.padEnd(rows[0].length, '.')}.`);
const ZHALO = ZS.map(z => halo(pad(z)));
export function zzz(c, x, y, t) {
  const x0 = Math.round(x), y0 = Math.round(y);
  detail(c, x0, y0, g => {
    for (let k = 0; k < 3; k++) {
      const ph = state.reduce ? k / 3 : (t / 2400 + k / 3) % 1;
      if (ph > 0.92) continue; // gone, before it starts again at the roof
      const k2 = ph < 0.45 ? 0 : 1;
      const gx = Math.round((x - x0 + ph * 6 + (state.reduce ? 0 : Math.sin(ph * 12) * 1.2)) * 2), gy = Math.round((y - y0 - ph * 12) * 2);
      bitmap(g, gx - 1, gy - 1, ZHALO[k2], P.o);
      bitmap(g, gx, gy, ZS[k2], P.white);
    }
  });
}
/* A rope between a waiting crate and the crate it waits on, both at the
 * docks: dashed, drawn under the crates so only the gaps show it. At 2×
 * (#456) it is a twist of light rope over a darker strand. */
export const ROPE_D = '#a07c48';
export function tether(c, a, b) {
  const x0 = 2 * (Math.round(a.x) + 2), y0 = 2 * (Math.round(a.y) + 2), x1 = 2 * (Math.round(b.x) + 2), y1 = 2 * (Math.round(b.y) + 2);
  const n = Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0));
  detail(c, 0, 0, g => {
    for (let k = 0; k <= n; k++) {
      if (k % 4 > 1) continue;
      const x = x0 + Math.round((x1 - x0) * k / (n || 1)), y = y0 + Math.round((y1 - y0) * k / (n || 1));
      px(g, x, y + 1, 2, 1, ROPE_D); px(g, x, y, 2, 1, ROPE);
    }
  });
}
/* ---------- Scenery ---------- */
export const FLOWERS = ['#f8f8f8', '#f8d040', '#e070d0', '#e05050', '#a8c8f8'];
/* The ground, the paths, the yards and the water are drawn wholly at 2×
 * (#451, #471): their fill and their detail in detail pixels inside detail(),
 * nothing in scene pixels. Each samples where it did at 1× and keeps its
 * footprint, so the map lays out and costs as before. */
export function grass(c, x0, y0, w, h) {
  detail(c, x0, y0, g => {
    const W = 2 * w, H = 2 * h;
    px(g, 0, 0, W, H, P.grass);
    // Soft, dithered variation: one detail pixel per sample, set off the scene grid.
    for (let y = 0; y < h; y += 2) for (let x = (y >> 1) % 2; x < w; x += 2) {
      const sx = x0 + x, sy = y0 + y, v = hash(sx >> 3, sy >> 3) * 0.6 + hash(sx >> 2, sy >> 2) * 0.4;
      if (v < 0.2 || v > 0.82) px(g, 2 * x + ((sx + sy) & 1), 2 * y + ((sx >> 1) & 1), 1, 1, v < 0.2 ? '#62a850' : '#4c9040');
    }
    // Tufts of three blades (the middle one tallest, its tip lit), dark specks,
    // clover, and the odd flower: four round petals about a centre, on a stem with a leaf.
    for (let y = 0; y < h; y += 3) for (let x = 0; x < w; x += 3) {
      const sx = x0 + x, sy = y0 + y, v = hash(sx, sy), X = 2 * x, Y = 2 * y;
      if (v < 0.05) {
        px(g, X, Y, 1, 2, P.grassL); px(g, X + 2, Y - 2, 1, 4, P.grassL); px(g, X + 4, Y, 1, 2, P.grassL);
        px(g, X + 2, Y - 2, 1, 1, '#b0e070'); px(g, X + 3, Y + 1, 1, 1, P.grassD);
      } else if (v < 0.1) px(g, X + 1, Y + 1, 1, 1, P.grassD);
      else if (v < 0.115 && X + 4 <= W && Y + 3 <= H) {
        px(g, X + 1, Y, 1, 1, '#3e8a36'); px(g, X, Y + 1, 3, 1, '#3e8a36'); px(g, X + 1, Y + 1, 1, 1, P.grassL); px(g, X + 3, Y + 2, 1, 1, '#3e8a36');
      } else if (v > 0.985 && X + 4 <= W && Y + 5 <= H) {
        const petal = FLOWERS[Math.floor(hash(sy, sx) * FLOWERS.length)];
        px(g, X + 1, Y + 3, 1, 2, P.leafD); px(g, X + 2, Y + 4, 1, 1, P.leaf);
        px(g, X + 1, Y, 1, 1, petal); px(g, X, Y + 1, 3, 1, petal); px(g, X + 1, Y + 2, 1, 1, petal);
        px(g, X + 1, Y + 1, 1, 1, petal === '#f8d040' ? '#c06020' : '#f8d040');
      }
    }
  });
}
export function path(c, x0, y0, w, h) {
  detail(c, x0, y0, g => {
    const W = 2 * w, H = 2 * h;
    px(g, 0, 0, W, H, P.path);
    // Wheel ruts worn along a lane wide enough for a cart, a little darker than the earth.
    if (w > h && h >= 5) { px(g, 0, Math.round(H * 0.3), W, 1, '#c8a868'); px(g, 0, Math.round(H * 0.7), W, 1, '#c8a868'); }
    if (h > w && w >= 5) { px(g, Math.round(W * 0.3), 0, 1, H, '#c8a868'); px(g, Math.round(W * 0.7), 0, 1, H, '#c8a868'); }
    // Pebbles: round, lit on top and shaded under; and light grit.
    for (let y = 0; y < h; y += 2) for (let x = 0; x < w; x += 2) {
      const v = hash(x0 + x + 7, y0 + y + 3), X = 2 * x + 1, Y = 2 * y + 1;
      if (v < 0.07) { px(g, X, Y, 2, 1, P.pathS); px(g, X, Y - 1, 1, 1, P.pathL); px(g, X + 1, Y + 1, 1, 1, P.stoneD); }
      else if (v < 0.11) px(g, X, Y, 1, 1, P.pathL);
    }
    // Worn edges one detail pixel wide where the path meets the grass, broken
    // here and there, and blades of grass creeping in over them.
    if (w > h) {
      px(g, 0, 0, W, 1, P.pathL); px(g, 0, H - 1, W, 1, P.pathS);
      for (let x = 0; x < W; x += 5) {
        const v = hash(x0 * 2 + x, y0);
        if (v < 0.5) px(g, x, 1, 2, 1, P.pathL);
        if (v > 0.86 && x + 3 <= W) { px(g, x, 0, 1, 2, P.grass); px(g, x + 2, 0, 1, 1, P.grass); }
      }
    } else {
      px(g, 0, 0, 1, H, P.pathL); px(g, W - 1, 0, 1, H, P.pathS);
      for (let y = 0; y < H; y += 5) {
        const v = hash(x0, y0 * 2 + y);
        if (v < 0.5) px(g, 1, y, 1, 2, P.pathL);
        if (v > 0.86 && y + 3 <= H) { px(g, 0, y, 2, 1, P.grass); px(g, 0, y + 2, 1, 1, P.grass); }
      }
    }
  });
}
/* A cobbled yard (#471): staggered courses of rounded sandstone setts, each
 * lit along its top and shaded along its foot, in mortar the colour of the
 * paths; some warmer, some paler, and a kerb lit along the top. */
export const COBBLE = { w: 6, h: 4, mortar: P.pathS, stone: ['#d0b888', '#c4aa7c', '#dcc8a0'], lit: '#ecdcb8', foot: '#a88a60' };
export function cobbles(c, x0, y0, w, h) {
  detail(c, x0, y0, g => {
    const W = 2 * w, H = 2 * h, { w: cw, h: ch } = COBBLE;
    px(g, 0, 0, W, H, COBBLE.mortar);
    for (let k = 0, y = 1; y < H - 1; k++, y += ch) {
      const rows = Math.min(ch - 1, H - 1 - y);
      for (let x = (k % 2 ? -(cw >> 1) : 0) + 1; x < W - 1; x += cw) {
        const a = Math.max(1, x), b = Math.min(W - 1, x + cw - 1);
        if (b - a < 2) continue;
        const v = hash(x0 * 2 + x, y0 * 2 + y), body = COBBLE.stone[v < 0.2 ? 1 : v > 0.85 ? 2 : 0];
        // Round the corners: the top and bottom rows a pixel in at each end.
        for (let j = 0; j < rows; j++) {
          const edge = j === 0 || j === rows - 1, l = a + (edge ? 1 : 0), r = b - (edge ? 1 : 0);
          if (r > l) px(g, l, y + j, r - l, 1, j === 0 ? COBBLE.lit : j === rows - 1 && rows > 2 ? COBBLE.foot : body);
        }
      }
    }
    px(g, 0, 0, W, 1, P.pathL); px(g, 0, H - 1, W, 1, P.woodD);
  });
}
/* Water: a stream with a dark lip under the far bank and a sandy near one.
 * Its ripples drift with the flow; soft highlights swell and fade slowly
 * (still, with reduced motion). */
export function water(c, x0, y0, w, h, t, flow) {
  t = state.reduce ? 0 : t;
  const off = Math.floor(t / 120 * flow);
  detail(c, x0, y0, g => {
    const W = 2 * w, H = 2 * h;
    px(g, 0, 0, W, H, P.water);
    px(g, 0, 2, W, 4, P.waterD);
    // The shallows by the near bank, a soft dither of light.
    for (let x = (y0 & 1); x < W; x += 2) if (hash(x0 * 2 + x, y0) < 0.5) px(g, x, H - 4 - (x & 2 ? 1 : 0), 1, 1, '#5890d8');
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
    // Soft highlights that stay put: a pale curve that swells and fades, each in its own time.
    for (let y = 4; y < h - 2; y += 3) for (let x = 3; x < w - 3; x += 11) {
      const v = hash(x0 + x + 5, y0 + y + 9);
      if (v >= 0.3) continue;
      const s = Math.sin(t / 1400 + v * 40), n = s > 0.6 ? 3 : s > -0.2 ? 2 : 0, X = 2 * x, Y = 2 * y + 1;
      if (n) { px(g, X - n, Y + 1, 1, 1, P.waterL); px(g, X - n + 1, Y, 2 * n - 1, 1, '#a0ccf4'); px(g, X + n, Y + 1, 1, 1, P.waterL); }
    }
    // Thin reeds on the near bank, and the odd lily pad with a notch and a flower.
    for (let x = 2; x < w - 2; x += 5) {
      const v = hash(x0 + x, y0 + h), X = 2 * x;
      if (v < 0.22) { px(g, X, H - 6, 1, 4, P.leafD); px(g, X + 2, H - 8, 1, 6, P.leaf); px(g, X + 3, H - 5, 1, 3, P.leafD); px(g, X + 2, H - 9, 1, 1, P.woodD); }
      else if (v > 0.93 && h > 9) {
        const Y = 2 * (h >> 1);
        px(g, X + 1, Y - 1, 4, 1, P.leafL); px(g, X, Y, 6, 2, P.leaf); px(g, X + 1, Y + 2, 4, 1, P.leafD);
        px(g, X + 3, Y, 1, 1, P.water); px(g, X + 2, Y - 1, 1, 1, '#e070d0'); px(g, X + 2, Y - 2, 1, 1, '#f8b0e8');
      }
    }
  });
}
export function tree(c, x, y) {
  detail(c, x, y, g => {
    // A soft oval shadow, falling right and down (#473), where it fell at 1×.
    px(g, 10, 22, 14, 1, SHADE); px(g, 8, 23, 18, 2, SHADE); px(g, 10, 25, 14, 1, SHADE);
    // The trunk, with bark and a root either side.
    px(g, 9, 23, 2, 1, P.woodD); px(g, 14, 23, 2, 1, P.woodD);
    px(g, 10, 16, 4, 8, P.woodD); px(g, 11, 16, 1, 8, P.wood); px(g, 12, 19, 1, 2, P.o); px(g, 11, 22, 1, 1, P.o);
    // A round canopy: outlined, lit from the top left.
    disc(g, 2, 0, 22, { outline: P.o, fill: P.leaf, light: P.leafL, dark: P.leafD });
    // Leaf clusters and, on some trees, fruit with a glint.
    [[7, 8], [13, 12], [15, 5], [9, 14]].forEach(([dx, dy]) => { px(g, dx, dy, 3, 1, P.leafL); px(g, dx + 1, dy + 1, 2, 1, P.leafD); });
    if (hash(x, y) < 0.3) [[8, 12], [17, 8]].forEach(([fx, fy]) => { px(g, fx, fy, 2, 2, '#e05050'); px(g, fx, fy, 1, 1, '#f8a0a0'); });
  });
}
/* A tag whose ink starts at (x + 1, y): a rounded outline, and inside it
 * `bg` with a margin all round the ink (#436). Without the margin a digit's
 * top and bottom strokes merged into the outline and only the gaps were
 * left to read: "+31" came out as "+=fl". Written at 2× (#456) in the 6x10
 * font (glyphs2x.js), in the same cells as the 3x5 one, so it is the size
 * it was. */
function glyphTag(c, x, y, text, bg, ink = P.o) {
  const { w, h } = tagSize(text.length);
  detail(c, x - 1, y - 2, g => {
    rounded(g, 0, 0, 2 * w, 2 * h, { outline: P.o, fill: bg });
    write(g, 4, 4, text, ink);
  });
}
const tagSize = glyphs => ({ w: glyphs * 4 + 3, h: 9 });
/* The outer size of badge(n), and of numberTag(n), from (x - 1, y - 2). */
export const badgeSize = n => tagSize(String(n).length + 1);
export const numberTagSize = n => tagSize(String(n).length);
/* A gold tag reading "+N" in a 3x5 pixel font. */
export function badge(c, x, y, n) {
  glyphTag(c, x, y, `+${n}`, P.gold);
}
/* A number in the 3x5 font, straight onto whatever is under it (a flag's cloth). */
export function digits(c, x, y, n, col) {
  detail(c, x, y, g => write(g, 0, 0, n, col));
}
/* A number on a parchment tag (a crate's issue in a line), in the same font;
 * `bg` and `ink` recolour it (the board's red count). */
export function numberTag(c, x, y, n, bg = P.parch, ink = P.o) {
  glyphTag(c, x, y, String(n), bg, ink);
}
