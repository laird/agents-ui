/* ---------- Alarms ----------
 * Trouble is the loudest thing on the map (#439). Loudest first:
 *
 *   waiting on you  a big red "!" sign over the house (or the hall), pulsing,
 *                   and a ring on the ground at its door: one per agent the
 *                   strip counts as "waiting on you";
 *   failed          a red ✗ in a red bubble (a stalled agent), the same size
 *                   as the quiet bubbles but red and crossed;
 *   the board       a red frame and its count while it holds notes (blocked,
 *                   needs-*, failed); an empty board stays plain;
 *   the gatehouse   a red lamp while any merge failed or conflicted.
 *
 * Busy reads as activity (chimney smoke, the worker at work: lifecycle.js);
 * idle is "zzz" and done a brief ✓ (#411). With reduced motion the marks
 * hold still but keep their size. Drawn with palette primitives only, so
 * their sizes can be measured (tests/alarms.test.mjs). */
import { state } from './state.js';
import { P, R, blink } from './palette.js';
import { numberTag } from './scenery.js';

export const RED = '#e02828', RED_D = '#a01818';
/* The quiet status bubble's size (scenery.bubble), and the "!" sign's. */
export const BUBBLE = { w: 9, h: 11 }, ALARM = { w: 11, h: 19 };

/* The "!" sign: its top-left at x, y. It pulses between a lit and a dimmer
 * red with a gold halo; held lit with reduced motion. */
export function alarm(c, x, y, t) {
  const lit = state.reduce || blink(t, 380), face = lit ? RED : RED_D;
  if (lit && !state.reduce) { R(c, x - 1, y + 2, 1, 12, P.gold); R(c, x + ALARM.w, y + 2, 1, 12, P.gold); R(c, x + 2, y - 1, 7, 1, P.gold); }
  R(c, x + 1, y, 9, 1, P.o); R(c, x + 1, y + 16, 9, 1, P.o); R(c, x, y + 1, 1, 15, P.o); R(c, x + 10, y + 1, 1, 15, P.o);
  R(c, x + 1, y + 1, 9, 15, face);
  R(c, x + 4, y + 3, 3, 8, P.white); R(c, x + 4, y + 12, 3, 2, P.white);
  // The tail points down at whoever is waiting.
  R(c, x + 4, y + 16, 3, 1, face); R(c, x + 4, y + 17, 3, 1, P.o); R(c, x + 5, y + 18, 1, 1, P.o);
}
/* A ring on the ground round a door at cx, cy (an ellipse 22 by 7). */
export function ring(c, cx, cy, t) {
  const col = state.reduce || blink(t + 190, 380) ? RED : P.gold;
  for (let a = 0; a < 40; a++) {
    const th = (a / 40) * Math.PI * 2;
    R(c, cx + Math.round(Math.cos(th) * 11), cy + Math.round(Math.sin(th) * 3.5), 1, 1, col);
  }
}
/* A failed agent: a red bubble with a white ✗, the quiet bubble's size. */
export function failed(c, x, y) {
  R(c, x + 1, y, 7, 1, P.o); R(c, x + 1, y + 8, 7, 1, P.o); R(c, x, y + 1, 1, 7, P.o); R(c, x + 8, y + 1, 1, 7, P.o);
  R(c, x + 1, y + 1, 7, 7, RED); R(c, x + 4, y + 8, 1, 2, RED); R(c, x + 3, y + 9, 1, 1, P.o); R(c, x + 5, y + 9, 1, 1, P.o); R(c, x + 4, y + 10, 1, 1, P.o);
  for (let k = 0; k < 5; k++) { R(c, x + 2 + k, y + 2 + k, 1, 1, P.white); R(c, x + 6 - k, y + 2 + k, 1, 1, P.white); }
}
/* The board's alert: how many notes it holds, or null when it is empty. */
export const boardAlert = notes => ((notes || []).length ? { n: notes.length } : null);
/* A red frame round the board (its 18 by 11 face at x, y) and its count. */
export function boardFrame(c, x, y, n, t) {
  const col = state.reduce || blink(t, 500) ? RED : RED_D;
  R(c, x - 2, y - 2, 22, 1, col); R(c, x - 2, y + 12, 22, 1, col); R(c, x - 2, y - 2, 1, 15, col); R(c, x + 19, y - 2, 1, 15, col);
  numberTag(c, x + 14, y - 6, n, RED, P.white);
}
/* The gatehouse's red lamp, at x, y, while a merge has failed. */
export function lamp(c, x, y, t) {
  R(c, x, y + 3, 1, 4, P.o);
  R(c, x - 2, y - 1, 5, 5, P.o); R(c, x - 1, y, 3, 3, state.reduce || blink(t, 300) ? RED : RED_D);
}
