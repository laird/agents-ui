/* ---------- The parts Medieval houses are built from, at 2× (#463) ----------
 * No DOM, so node tests it (tools/village-tests/medievalbuildings.test.mjs).
 *
 * Every part takes detail pixels (detail.js) from the house's own origin:
 * two to a scene pixel, so a part at (X, Y) covers the scene pixels its 1×
 * version did at (X / 2, Y / 2). Outlines are one detail pixel: half the
 * weight they had at 1×, which is most of what makes the redrawn art finer. */
import { P, SHADE, SHINE, glassOf } from '../palette.js';
import { px } from '../art2x.js';

/* Marks a slot as drawn at 2× already, so registerTheme's hd() (the Scale2x
 * wrap, upscale.js) passes it through untouched. */
export const drawn2x = fn => { fn.hd = true; return fn; };

/* A pitched roof filling W by H at (X, Y), seen from the front: a sunlit
 * ridge, courses of staggered shingles (or straw, for thatch), the far
 * slope in shade and the eaves' shadow along its foot. */
export function roof2(g, X, Y, W, H, a, d) {
  px(g, X, Y, W, H, P.o);
  const iw = W - 2, lw = Math.ceil(iw * 0.62);
  px(g, X + 1, Y + 1, iw, H - 2, a);
  if (a === P.thatch) {
    // Straw: short strokes in rows, and a ragged fringe at the eaves.
    for (let j = 3; j < H - 3; j += 3) for (let k = X + 2 + (j % 2) * 2; k < X + W - 2; k += 4) px(g, k, Y + j, 1, 2, d);
    for (let k = X + 1; k < X + W - 1; k += 3) px(g, k, Y + H - 3, 2, 1, d);
    for (let k = X + 3; k < X + W - 3; k += 6) px(g, k, Y + 4, 2, 1, P.gold);
  } else {
    for (let j = 4, n = 0; j < H - 1; j += 4, n++) {
      px(g, X + 1, Y + j, iw, 1, d);
      for (let k = X + 2 + (n % 2) * 3; k < X + W - 2; k += 6) px(g, k, Y + j - 3, 1, 3, d);
    }
  }
  px(g, X + 1, Y + 1, iw, 1, d); px(g, X + 1, Y + 2, lw, 1, SHINE);
  px(g, X + 1 + lw, Y + 2, iw - lw, H - 3, SHADE);
  px(g, X + 1, Y + H - 2, iw, 1, SHADE);
}

/* A wall W by H at (X, Y): outlined at the sides and foot, a stone plinth
 * with its joints, the right-hand end in shade. Timber walls get a beam
 * under the eaves and two posts. */
export function wall2(g, X, Y, W, H, col) {
  px(g, X, Y, W, H, P.o); px(g, X + 1, Y, W - 2, H - 1, col);
  px(g, X + 1, Y + H - 3, W - 2, 2, P.stoneD);
  for (let k = X + 3; k < X + W - 2; k += 5) px(g, k, Y + H - 3, 1, 2, P.stone);
  if (col === P.wood) {
    px(g, X + 1, Y, W - 2, 2, P.woodD); px(g, X + 1, Y + 2, W - 2, 1, P.woodL);
    [X + 1, X + W - 3].forEach(k => px(g, k, Y + 2, 2, H - 5, P.woodD));
  }
  px(g, X + W - 5, Y, 4, H - 3, SHADE);
}

/* A window, 8 by 8 at (X, Y), over a flower box: a frame with a lit top,
 * four panes in the status's glass, a glint while working. */
export function win2(g, X, Y, st) {
  px(g, X, Y, 8, 8, P.woodD); px(g, X, Y, 8, 1, P.woodL);
  px(g, X + 1, Y + 1, 6, 5, glassOf(st));
  px(g, X + 4, Y + 1, 1, 5, P.woodD); px(g, X + 1, Y + 3, 6, 1, P.woodD);
  if (st === 'working') { px(g, X + 2, Y + 1, 1, 2, P.white); px(g, X + 5, Y + 4, 1, 1, P.white); }
  px(g, X, Y + 8, 8, 2, P.woodL); px(g, X, Y + 9, 8, 1, P.woodD);
  [[1, '#e05050'], [2, P.leafL], [3, P.gold], [5, P.leaf], [6, '#e05050']].forEach(([k, col]) => px(g, X + k, Y + 7, 1, 1, col));
}

/* A door, 8 by 10 at (X, Y): a round-topped plank door with iron hinges
 * and a brass knob; shut and dark when the agent waits on you. */
export function door2(g, X, Y, st) {
  px(g, X + 1, Y, 6, 1, P.o); px(g, X, Y + 1, 8, 9, P.o);
  if (st === 'blocked') return;
  px(g, X + 2, Y + 1, 4, 1, P.woodD); px(g, X + 1, Y + 2, 6, 8, P.woodD);
  [X + 2, X + 5].forEach(k => px(g, k, Y + 2, 1, 8, P.wood));
  [Y + 3, Y + 7].forEach(k => px(g, X + 1, k, 3, 1, P.stoneD));
  px(g, X + 5, Y + 5, 1, 1, P.gold); px(g, X + 1, Y + 9, 6, 1, P.wood);
}

/* A stone chimney W by H at (X, Y): brick courses and a lit cap. */
export function chimney(g, X, Y, W, H) {
  px(g, X, Y, W, H, P.o); px(g, X + 1, Y + 1, W - 2, H - 1, P.stone);
  for (let j = Y + 3, n = 0; j < Y + H; j += 3, n++) { px(g, X + 1, j, W - 2, 1, P.stoneD); px(g, X + 2 + (n % 2) * 2, j - 2, 1, 2, P.stoneD); }
  px(g, X + 1, Y + 1, W - 2, 1, P.stoneL);
}

/* A line of 1-pixel dots from (x0, y0) to (x1, y1), in `col`. */
export function line2(g, x0, y0, x1, y1, col, w = 1) {
  const n = Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0));
  for (let i = 0; i <= n; i++) px(g, Math.round(x0 + (x1 - x0) * i / (n || 1)), Math.round(y0 + (y1 - y0) * i / (n || 1)), w, w, col);
}
