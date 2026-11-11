/* ---------- Bounce art at 2× (#457) ----------
 * No DOM, so node tests it (tools/village-tests/bounceart.test.mjs).
 *
 * How a crate the gate handed back is dressed for rework, and the broken
 * boards beside its house, drawn in detail pixels (detail.js) around the
 * crate's own 11×10 (crates.js). bounces.js puts them on the map. */
import { state } from './state.js';
import { P, R, blink } from './palette.js';
import { detail } from './detail.js';
import { CRATE_W, CRATE_H } from './crates.js';
import { MODERN } from './themes/modern.js';
import { FUTURE } from './themes/future.js';

/* The crate dressed for rework, over the crate at scene (x, y):
 *   future    a red force field round it, pulsing, sparking at its corners;
 *   modern    a red REJECTED stamp boxed round its number, a cross on its lid;
 *   medieval  back on its cart: two spoked wheels on an axle, a red rag on a pole.
 * Everything stays inside the bounce's tap (scene x-1..x+7, y-2..y+7). */
export function reworked(c, x, y, t, theme = state.T) {
  detail(c, x, y, d => {
    if (theme === FUTURE) {
      const on = state.reduce || blink(t, 400);
      const a = on ? 'rgba(255,60,60,.85)' : 'rgba(255,60,60,.45)', b = on ? 'rgba(255,150,150,.9)' : 'rgba(255,90,90,.55)';
      const w = CRATE_W + 4, h = CRATE_H + 4;
      R(d, -2, -2, w, 1, a); R(d, -2, h - 3, w, 1, a); R(d, -2, -1, 1, h - 2, a); R(d, w - 3, -1, 1, h - 2, a);
      // A faint inner shimmer a pixel in, and the field's corners sparking brighter.
      for (let k = 0; k < CRATE_W; k += 2) { R(d, k, -1, 1, 1, 'rgba(255,60,60,.3)'); R(d, k, CRATE_H, 1, 1, 'rgba(255,60,60,.3)'); }
      [[-2, -2], [w - 3, -2], [-2, h - 3], [w - 3, h - 3]].forEach(([cx, cy]) => R(d, cx, cy, 1, 1, b));
    } else if (theme === MODERN) {
      // A red REJECTED stamp: a box round the stencil, inked a touch unevenly, and a cross on the lid.
      R(d, 0, 1, CRATE_W, 1, P.p0); R(d, 0, 7, CRATE_W, 1, P.p0);
      R(d, 0, 1, 1, 7, P.p0); R(d, CRATE_W - 1, 1, 1, 7, P.p0);
      R(d, 3, 7, 2, 1, '#f06060'); R(d, 8, 1, 2, 1, '#f06060');
      [[CRATE_W, -3], [CRATE_W + 1, -2], [CRATE_W + 2, -1], [CRATE_W + 2, -3], [CRATE_W, -1]].forEach(([sx, sy]) => R(d, sx, sy, 1, 1, P.p0));
    } else {
      // Back on its cart: an axle under it, two spoked wheels, and a red rag on a pole.
      R(d, -2, CRATE_H, CRATE_W + 4, 1, P.woodD);
      [0, 7].forEach(wx => {
        R(d, wx + 1, CRATE_H, 2, 4, P.o); R(d, wx, CRATE_H + 1, 4, 2, P.o);
        R(d, wx + 1, CRATE_H + 1, 2, 2, P.woodL); R(d, wx + 1, CRATE_H + 1, 1, 1, P.woodD);
      });
      R(d, 9, -4, 1, 4, P.woodD);
      R(d, 10, -4, 4, 2, P.p0); R(d, 10, -2, 3, 1, P.p0); R(d, 10, -4, 4, 1, '#f06050'); R(d, 12, -2, 1, 1, P.redD);
    }
  });
}

/* How many broken boards lie by a house whose crate bounced `count` times:
 * one more for each bounce past the first, up to four. */
export const boards = count => Math.max(0, Math.min(4, count - 1));
/* Where each board lies, in detail pixels from the pile's corner: [x, y, length]. */
export const BOARDS = [[0, 0, 6], [2, 4, 4], [0, 6, 6], [4, 2, 4]];

/* Broken boards beside the house at `lot` (scene x+16, y+18): each a plank
 * two detail pixels deep, lit along the top, splintered at one end. Future
 * houses leave scrap plating instead. */
export function debris(c, lot, count, theme = state.T) {
  const [light, mid, dark] = theme === FUTURE ? [P.stone, P.stoneD, P.dark] : [P.plank, P.woodL, P.woodD];
  detail(c, lot.x + 16, lot.y + 18, d => BOARDS.slice(0, boards(count)).forEach(([bx, by, w]) => {
    R(d, bx, by, w, 1, light); R(d, bx, by + 1, w - 1, 1, mid);
    // The splintered end: a jag up, a dark break under it.
    R(d, bx + w - 1, by - 1, 1, 1, light); R(d, bx + w - 1, by + 1, 1, 2, dark); R(d, bx + 1, by + 2, w - 2, 1, 'rgba(32,32,48,.3)');
  }));
}
