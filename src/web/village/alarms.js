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
 * hold still but keep their size. Drawn at 2× (#456), inside detail(), and
 * measured by node (tools/village-tests/alarms.test.mjs). */
import { state } from './state.js';
import { P, blink } from './palette.js';
import { numberTag, speech } from './scenery.js';
import { detail } from './detail.js';
import { px } from './art2x.js';
import { bitmap, ellipseBand, rounded, tail } from './glyphs2x.js';

export const RED = '#e02828', RED_D = '#a01818', RED_L = '#f86868', RED_DD = '#701010', GOLD_L = '#f8e8a0';
/* The quiet status bubble's size (scenery.bubble), and the "!" sign's. */
export const BUBBLE = { w: 9, h: 11 }, ALARM = { w: 11, h: 19 };

/* The "!" sign: its top-left at x, y. It pulses between a lit and a dimmer
 * red with a gold halo; held lit with reduced motion. */
const BANG = ['.####.', '######', '######', '######', '.####.', '.####.', '.####.', '.####.', '..##..', '..##..', '..##..', '..##..', '..##..', '..##..', '......', '......', '......', '..##..', '.####.', '.####.', '..##..'];
export function alarm(c, x, y, t) {
  const lit = state.reduce || blink(t, 380), face = lit ? RED : RED_D;
  detail(c, x, y, g => {
    if (lit && !state.reduce) {
      px(g, -2, 5, 1, 22, GOLD_L); px(g, -1, 4, 1, 24, P.gold);
      px(g, 2 * ALARM.w, 4, 1, 24, P.gold); px(g, 2 * ALARM.w + 1, 5, 1, 22, GOLD_L);
      px(g, 5, -2, 12, 1, GOLD_L); px(g, 4, -1, 14, 1, P.gold);
    }
    const cols = { outline: P.o, fill: face, light: lit ? RED_L : RED, dark: lit ? RED_D : RED_DD };
    rounded(g, 0, 0, 22, 34, cols, [8, 14]);
    // The tail points down at whoever is waiting.
    tail(g, 8, 34, 6, 4, { outline: P.o, fill: face });
    bitmap(g, 8, 6, BANG, P.white);
  });
}
/* A ring on the ground round a door at cx, cy (an ellipse 22 by 7). */
export function ring(c, cx, cy, t) {
  const col = state.reduce || blink(t + 190, 380) ? RED : P.gold;
  detail(c, cx, cy, g => ellipseBand(g, 22, 7.5, 1.6, col));
}
/* A failed agent: a red bubble with a white ✗, the quiet bubble's size. */
export function failed(c, x, y) {
  detail(c, x, y, g => {
    speech(g, { outline: P.o, fill: RED, light: RED_L, dark: RED_D });
    for (let r = 0; r < 8; r++) { px(g, 4 + r, 5 + r, 2, 1, P.white); px(g, 12 - r, 5 + r, 2, 1, P.white); }
  });
}
/* The board's alert: how many notes it holds, or null when it is empty. */
export const boardAlert = notes => ((notes || []).length ? { n: notes.length } : null);
/* A red frame round the board (its 18 by 11 face at x, y) and its count:
 * a rounded rim with a darker inner edge. */
export function boardFrame(c, x, y, n, t) {
  const lit = state.reduce || blink(t, 500), col = lit ? RED : RED_D, inner = lit ? RED_D : RED_DD;
  detail(c, x - 2, y - 2, g => {
    px(g, 1, 0, 42, 1, col); px(g, 1, 29, 42, 1, col); px(g, 0, 1, 1, 28, col); px(g, 43, 1, 1, 28, col);
    px(g, 1, 1, 42, 1, inner); px(g, 1, 28, 42, 1, inner); px(g, 1, 2, 1, 26, inner); px(g, 42, 2, 1, 26, inner);
  });
  numberTag(c, x + 14, y - 6, n, RED, P.white);
}
/* The gatehouse's red lamp, at x, y, while a merge has failed: a lantern
 * on a post, its glass lit and dim in turn. */
export function lamp(c, x, y, t) {
  const lit = state.reduce || blink(t, 300);
  detail(c, x, y, g => {
    px(g, 0, 6, 2, 8, P.o); px(g, 1, 7, 1, 7, '#585870');
    px(g, -2, -3, 6, 1, P.o);
    rounded(g, -4, -2, 10, 10, { outline: P.o, fill: lit ? RED : RED_D, light: lit ? RED_L : RED, dark: lit ? RED_D : RED_DD });
    if (lit) px(g, -2, 0, 2, 2, '#ffd8d8');
  });
}
