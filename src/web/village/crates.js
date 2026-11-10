/* ---------- Crates at 2× (#420) ----------
 * No DOM, so node tests it (tools/village-tests/crates.test.mjs).
 *
 * Every crate, in every theme, is drawn in detail pixels (detail.js): 11
 * wide and 10 tall, on the 5×5 scene pixels a crate has always stood on plus
 * one detail column on the right, so that three digits fit, one to a plank
 * (the middle one reversed out, light on dark, so they read apart).
 * Its rows, top down, are the same in every theme so the marks drawn over
 * any theme's crate (rope, ribbon, seal, glint) land where they should:
 *
 *   0      outline
 *   1      the kind of work (a lid, tape or a band)
 *   2–6    the issue number, stencilled (STENCIL_ROW)
 *   7      bare (where a waiting crate's rope goes round)
 *   8      the priority mark, when it has one
 *   9      outline
 *
 * The number is the crate's own: no label is laid over it on the map. A
 * number of four digits or more does not fit and is left off (the crate's
 * dialog still names it). */
import { detail } from './detail.js';
import { DIGITS, KINDCOL, P, PRIOCOL, R, ROPE } from './palette.js';

export const CRATE_W = 11, CRATE_H = 10;
/* The first row of the stencilled number, and the face's first column. */
export const STENCIL_ROW = 2, FACE_X = 1, FACE_W = 9;

/* Where each digit of `n` starts on the face, or [] when it doesn't fit:
 * three digits one to a plank, fewer centred with a pixel between them. */
export function stencilColumns(n) {
  const s = n === undefined || n === null ? '' : String(n);
  if (!/^\d{1,3}$/.test(s)) return [];
  if (s.length === 3) return [0, 3, 6].map(k => FACE_X + k);
  const w = s.length * 4 - 1, x0 = FACE_X + ((FACE_W - w) >> 1);
  return [...s].map((_, j) => x0 + j * 4);
}
/* The issue number stencilled on a crate's face (`d` draws in detail pixels).
 * Three digits stand shoulder to shoulder with no pixel between them, so the
 * middle one is reversed out: light (`light`) on a dark plate (`plate`),
 * between two dark ones. Without that, "427" ran together into one blot. */
export function stencil(d, n, ink = P.o, plate = P.woodD, light = P.parch) {
  const at = stencilColumns(n);
  if (at.length === 3) R(d, at[1], STENCIL_ROW, 3, 5, plate);
  [...String(n ?? '')].slice(0, at.length).forEach((ch, j) => {
    const g = DIGITS[+ch], col = stencilInk(at.length, j, ink, light);
    for (let k = 0; k < 15; k++) if (g[k] === '1') R(d, at[j] + (k % 3), STENCIL_ROW + Math.floor(k / 3), 1, 1, col);
  });
}
/* The ink of digit j of a number `len` digits long: the reversed-out middle of three is light. */
export const stencilInk = (len, j, ink = P.o, light = P.parch) => (len === 3 && j === 1 ? light : ink);
/* Locked: a green wax seal on the lid's corner, standing proud of it. Finished
 * work (the quay, the stages, the ship) travels sealed. */
export function seal(d) {
  R(d, 7, -2, 4, 4, P.o); R(d, 8, -1, 2, 2, P.seal);
}

/* The medieval crate: three planks under a lid painted in the kind's colour,
 * and the priority painted along its foot. */
export function crate(c, x, y, kind, pr, sealed, n) {
  detail(c, x, y, d => {
    R(d, 0, 0, CRATE_W, CRATE_H, P.o);
    for (let k = 0; k < 3; k++) R(d, FACE_X + k * 3, 1, 3, 8, k % 2 ? P.plank : P.woodL);
    R(d, FACE_X, 1, FACE_W, 1, KINDCOL[kind] || P.woodD);
    if (PRIOCOL[pr]) R(d, FACE_X, 8, FACE_W, 1, PRIOCOL[pr]);
    stencil(d, n);
    if (sealed) seal(d);
  });
}

/* A crate's epic: its outline in the feature's flag colour, like a ribbon
 * tied round it. Drawn over any theme's crate. */
export function ribbon(c, x, y, col) {
  detail(c, x, y, d => {
    R(d, 0, 0, CRATE_W, 1, col); R(d, 0, CRATE_H - 1, CRATE_W, 1, col);
    R(d, 0, 0, 1, CRATE_H, col); R(d, CRATE_W - 1, 0, 1, CRATE_H, col);
  });
}
/* Ready to pick up: a glint now and then on the crate's top corner. */
export function glint(c, x, y, t) {
  if ((t % 2400 + 2400) % 2400 >= 180) return;
  detail(c, x, y, d => {
    const soft = 'rgba(248,248,248,.6)';
    R(d, 9, -3, 1, 1, soft); R(d, 9, -1, 1, 1, soft); R(d, 8, -2, 1, 1, soft); R(d, 10, -2, 1, 1, soft);
    R(d, 9, -2, 1, 1, P.white);
  });
}
/* A crate tied shut: it waits on another issue (a "Depends on" line), so
 * nobody picks it up yet. A twisted rope goes round it under the number,
 * knotted on its right side, so the kind, number and priority all still
 * show. Drawn over any theme's crate. */
export function roped(c, x, y) {
  detail(c, x, y, d => {
    for (let k = -1; k <= CRATE_W; k++) R(d, k, 7, 1, 1, k % 2 ? ROPE : P.woodD);
    R(d, CRATE_W - 1, 6, 3, 3, P.o); R(d, CRATE_W, 7, 1, 1, ROPE);
    R(d, CRATE_W + 1, 9, 1, 2, ROPE);
  });
}
/* Handed back by the merge gate: a dent in the lid's corner. */
export function dent(c, x, y) {
  detail(c, x, y, d => { R(d, 1, 1, 3, 1, P.o); R(d, 0, 0, 2, 1, P.woodD); });
}
