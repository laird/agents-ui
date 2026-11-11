/* ---------- Art helpers on the 2× detail grid ----------
 * No DOM, so node tests it (tools/village-tests/art2x.test.mjs).
 *
 * Shared by the redrawn 2× sprites (#422: scenery here, the themes next).
 * Everything takes detail pixels: call these inside detail() (detail.js),
 * whose origin is a scene pixel and whose unit is half a scene pixel. Shapes
 * are drawn as runs of whole rows, not pixel by pixel, so twice the detail
 * costs about the same number of fillRects as before. */

/* One run of `w` by `h` detail pixels. */
export function px(c, x, y, w, h, col) {
  c.fillStyle = col;
  c.fillRect(x, y, w, h);
}

/* Where a row of a disc filling a `size`-square box starts and ends (end
 * exclusive), for a disc of radius `r` about the box's centre; null when the
 * row misses it. */
export function discRow(size, j, r = size / 2) {
  const dy = j + 0.5 - size / 2;
  if (Math.abs(dy) >= r) return null;
  const hw = Math.sqrt(r * r - dy * dy), a = Math.round(size / 2 - hw), b = Math.round(size / 2 + hw);
  return b > a ? [a, b] : null;
}

/* A shaded disc (a tree's canopy) filling the `size`-square box at (x0, y0):
 * a one-pixel outline, then light towards the top left, dark towards the
 * bottom right and `fill` between. cols: { outline, fill, light, dark }. */
export function disc(c, x0, y0, size, cols, band = size * 0.3) {
  for (let j = 0; j < size; j++) {
    const out = discRow(size, j);
    if (!out) continue;
    const inn = discRow(size, j, size / 2 - 1);
    if (!inn) { px(c, x0 + out[0], y0 + j, out[1] - out[0], 1, cols.outline); continue; }
    px(c, x0 + out[0], y0 + j, inn[0] - out[0], 1, cols.outline);
    px(c, x0 + inn[1], y0 + j, out[1] - inn[1], 1, cols.outline);
    // Across the row, x + y (from the centre) splits light, fill and dark.
    const dy = j + 0.5 - size / 2, cx = size / 2 - 0.5;
    const l = Math.max(inn[0], Math.min(inn[1], Math.ceil(cx - band - dy)));
    const d = Math.max(l, Math.min(inn[1], Math.floor(cx + band - dy) + 1));
    if (l > inn[0]) px(c, x0 + inn[0], y0 + j, l - inn[0], 1, cols.light);
    if (d > l) px(c, x0 + l, y0 + j, d - l, 1, cols.fill);
    if (inn[1] > d) px(c, x0 + d, y0 + j, inn[1] - d, 1, cols.dark);
  }
}

/* Dither: one detail pixel at most every `step` pixels across the box,
 * coloured by pick(x, y) (null: none). x, y are the box's own coordinates. */
export function dither(c, x0, y0, w, h, step, pick) {
  for (let y = 0; y < h; y += step) for (let x = (y / step) % 2 ? step >> 1 : 0; x < w; x += step) {
    const col = pick(x, y);
    if (col) px(c, x0 + x, y0 + y, 1, 1, col);
  }
}

/* Plank seams: a `line` every `gap` rows down a w×h box (not on its edges). */
export function planks(c, x, y, w, h, gap, line) {
  for (let yy = y + gap; yy < y + h; yy += gap) px(c, x, yy, w, 1, line);
}

/* Brickwork over a w×h box: a mortar line under every course of `bh`, and
 * a joint every `bw` along it, staggered by half a brick on alternate courses. */
export function bricks(c, x, y, w, h, bw, bh, mortar) {
  for (let k = 0, yy = y; yy < y + h; k++, yy += bh) {
    if (yy + bh - 1 < y + h) px(c, x, yy + bh - 1, w, 1, mortar);
    for (let xx = x + (k % 2 ? bw >> 1 : 0) + bw - 1; xx < x + w - 1; xx += bw) px(c, xx, yy, 1, Math.min(bh - 1, y + h - yy), mortar);
  }
}

/* Overlapping shaded discs as one lumpy shape (a bush, a cluster of leaves):
 * every disc's outline first, then every disc's inside over them, so the
 * lobes share one outline. discs: [[x0, y0, size], ...]; cols as disc(). */
export function lobes(c, discs, cols) {
  const solid = { outline: cols.outline, fill: cols.outline, light: cols.outline, dark: cols.outline };
  for (const [x, y, s] of discs) disc(c, x, y, s, solid);
  for (const [x, y, s] of discs) disc(c, x + 1, y + 1, s - 2, { ...cols, outline: cols.fill });
}

/* Pixel art from rows of characters: each character is a colour from
 * `cols` ('.' or a character `cols` lacks draws nothing), drawn in runs. */
export function pixmap(c, x, y, rows, cols) {
  rows.forEach((row, j) => {
    for (let i = 0; i < row.length;) {
      let k = i;
      while (k < row.length && row[k] === row[i]) k++;
      if (cols[row[i]]) px(c, x + i, y + j, k - i, 1, cols[row[i]]);
      i = k;
    }
  });
}
