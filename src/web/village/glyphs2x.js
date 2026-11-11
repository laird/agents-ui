/* ---------- Marks on the 2× detail grid (#456) ----------
 * No DOM, so node tests it (tools/village-tests/glyphs2x.test.mjs).
 *
 * The 6x10 font the tags, badges and flags write in, and the outlines the
 * bubbles and the "!" sign are built from. Everything is in detail pixels:
 * call these inside detail() (detail.js). A glyph keeps the 3x5 font's cell
 * (4 scene pixels across, 3 of them ink), so a tag is the size it was, but
 * its strokes now have rounded corners and its outline is half as thick. */
import { px } from './art2x.js';

/* Digits 0-9 and "+", each 10 rows of 6, '#' for ink. */
export const FONT2X = {
  0: ['.####.', '##..##', '##..##', '##..##', '##..##', '##..##', '##..##', '##..##', '##..##', '.####.'],
  1: ['..##..', '.###..', '####..', '..##..', '..##..', '..##..', '..##..', '..##..', '..##..', '######'],
  2: ['.####.', '##..##', '....##', '....##', '...##.', '..##..', '.##...', '##....', '##....', '######'],
  3: ['.####.', '##..##', '....##', '....##', '..###.', '....##', '....##', '....##', '##..##', '.####.'],
  4: ['##..##', '##..##', '##..##', '##..##', '######', '######', '....##', '....##', '....##', '....##'],
  5: ['######', '##....', '##....', '#####.', '....##', '....##', '....##', '....##', '##..##', '.####.'],
  6: ['.####.', '##..##', '##....', '##....', '#####.', '##..##', '##..##', '##..##', '##..##', '.####.'],
  7: ['######', '....##', '....##', '...##.', '...##.', '..##..', '..##..', '..##..', '..##..', '..##..'],
  8: ['.####.', '##..##', '##..##', '##..##', '.####.', '##..##', '##..##', '##..##', '##..##', '.####.'],
  9: ['.####.', '##..##', '##..##', '##..##', '.#####', '....##', '....##', '....##', '##..##', '.####.'],
  '+': ['......', '......', '..##..', '..##..', '######', '######', '..##..', '..##..', '......', '......'],
};
/* One cell of the font: 6 pixels of ink and 2 of gap. */
export const CELL = 8;

/* Draw a bitmap (rows of '#') at (x, y), one run of fillRect per stretch of ink. */
export function bitmap(g, x, y, rows, col) {
  rows.forEach((row, j) => {
    for (let i = 0; i < row.length;) {
      if (row[i] !== '#') { i++; continue; }
      let e = i;
      while (row[e] === '#') e++;
      px(g, x + i, y + j, e - i, 1, col);
      i = e;
    }
  });
}

/* Write `text` (digits and "+") from (x, y), a cell per character. */
export function write(g, x, y, text, col) {
  [...String(text)].forEach((ch, j) => { if (FONT2X[ch]) bitmap(g, x + j * CELL, y, FONT2X[ch], col); });
}

/* How far a box with rounded corners is inset on row j of `h`. */
const CORNER = [3, 1, 1];
export const inset = (j, h) => CORNER[Math.min(j, h - 1 - j)] ?? 0;

/* A box `w` by `h` with rounded corners: a 1-pixel `outline` round `fill`.
 * `light` and `dark` (optional) shade the inside edge, top and left, bottom
 * and right. A gap in the bottom outline, [gap0, gap1), is left filled, for
 * a bubble's tail. */
export function rounded(g, x0, y0, w, h, cols, gap = null) {
  const { outline, fill, light, dark } = cols;
  for (let j = 0; j < h; j++) {
    const a = inset(j, h);
    if (j === 0 || j === h - 1) {
      px(g, x0 + a, y0 + j, w - 2 * a, 1, outline);
      if (j === h - 1 && gap) px(g, x0 + gap[0], y0 + j, gap[1] - gap[0], 1, fill);
      continue;
    }
    // On a corner row the outline runs in to meet the row nearer the edge.
    const edge = Math.max(1, inset(j < h / 2 ? j - 1 : j + 1, h) - a);
    px(g, x0 + a, y0 + j, edge, 1, outline);
    px(g, x0 + w - a - edge, y0 + j, edge, 1, outline);
    const l = x0 + a + edge, r = x0 + w - a - edge;
    px(g, l, y0 + j, r - l, 1, fill);
    if (light) px(g, l, y0 + j, j === 1 ? r - l : 1, 1, light);
    if (dark) px(g, j === h - 2 ? l : r - 1, y0 + j, j === h - 2 ? r - l : 1, 1, dark);
  }
}

/* A tail under a box, pointing down from the gap [x, x + w) in its bottom
 * row (at y - 1): `rows` rows, each a pixel narrower on both sides. */
export function tail(g, x, y, w, rows, cols) {
  for (let k = 0; k < rows; k++) {
    const l = x + k, r = x + w - k;
    if (r - l <= 0 || k === rows - 1) { px(g, Math.min(l, r) - 1, y + k, Math.max(2, r - l + 2), 1, cols.outline); break; }
    px(g, l - 1, y + k, 1, 1, cols.outline); px(g, r, y + k, 1, 1, cols.outline);
    px(g, l, y + k, r - l, 1, cols.fill);
  }
}

/* A filled ellipse band: the pixels between an ellipse of radii (a, b) and
 * one `thick` pixels inside it, centred on (0, 0). */
export function ellipseBand(g, a, b, thick, col) {
  const h = Math.ceil(b);
  for (let y = -h; y < h; y++) {
    const yc = y + 0.5, out = 1 - (yc * yc) / (b * b);
    if (out <= 0) continue;
    const xo = Math.round(a * Math.sqrt(out)), bi = b - thick, ai = a - thick;
    const inn = bi > 0 ? 1 - (yc * yc) / (bi * bi) : -1, xi = inn > 0 ? Math.round(ai * Math.sqrt(inn)) : 0;
    if (xi <= 0) { px(g, -xo, y, 2 * xo, 1, col); continue; }
    px(g, -xo, y, xo - xi, 1, col); px(g, xi, y, xo - xi, 1, col);
  }
}
