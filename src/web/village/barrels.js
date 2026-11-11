/* ---------- Context barrels ----------
 * How much of its context window each house's agent has used (`context_pct`,
 * from the status line under its input box): a barrel beside the house, and
 * beside the hall for the manager, filling as the context does. Green below
 * 70%, amber to 90%, red from 90%, flashing from 95%: an agent there is about
 * to wedge (Claude Code stops at about 97%). Downtown it is a fuel gauge, in
 * the future a battery. No barrel when the context is unknown. Drawn at 2×
 * (#457): tools/village-tests/barrels.test.mjs paints it under node. */
import { state } from './state.js';
import { P, R, blink } from './palette.js';
import { detail } from './detail.js';
import { registerKey, registerLayer, tap } from './registry.js';
import { MODERN, M } from './themes/modern.js';
import { FUTURE } from './themes/future.js';

export const CTX_WARN = 70, CTX_FULL = 90, CTX_WEDGE = 95;
/* The fill's colour at `pct`, and whether it flashes. */
export function contextColour(pct) {
  return pct >= CTX_FULL ? P.p0 : pct >= CTX_WARN ? '#f0a020' : '#38c848';
}
/* "context 82% of 1M", with what it means near the top. */
export function contextWords(h) {
  if (h.context_pct == null) return null;
  const of = h.context_window ? ` of ${h.context_window}` : '';
  const note = h.context_pct >= CTX_WEDGE ? ': about to run out and wedge' : h.context_pct >= CTX_FULL ? ': nearly full' : '';
  return `Context ${h.context_pct}%${of}${note}`;
}
/* A barrel's footprint, in detail pixels (detail.js): 4×8 scene pixels, as
 * it has always stood, redrawn at 2× (#457). Its level fills a window of
 * BARREL_ROWS detail rows, from the bottom. */
export const BARREL_W = 8, BARREL_H = 16, BARREL_ROWS = 12;
/* How many of the window's rows are filled at `pct`. */
export const barrelFill = pct => Math.round(BARREL_ROWS * Math.max(0, Math.min(100, pct)) / 100);
/* The level, in the window x 2–5 and rows 2–13, with a lighter surface on top. */
function barrelLevel(d, fill, col) {
  if (!fill) return;
  R(d, 2, 14 - fill, 4, fill, col);
  R(d, 2, 14 - fill, 4, 1, 'rgba(255,255,255,.35)');
}
/* A barrel 4 wide and 8 tall at scene (x, y), filled to `pct` from the bottom. */
export function barrel(c, x, y, pct, t, theme = state.T) {
  const fill = barrelFill(pct);
  const flash = pct >= CTX_WEDGE && !state.reduce && blink(t, 300);
  const col = flash ? P.white : contextColour(pct);
  detail(c, x, y, d => {
    if (theme === FUTURE) {
      // A battery: a cap on top, cells lit from the bottom, a glint down its side.
      R(d, 2, -2, 4, 2, P.o); R(d, 3, -1, 2, 1, '#40e0f0');
      R(d, 0, 0, BARREL_W, BARREL_H, P.o); R(d, 1, 1, 6, 14, '#262b4c'); R(d, 1, 1, 1, 14, '#3a4070');
      barrelLevel(d, fill, col);
      // A dim line between cells, as on a battery's charge meter.
      for (let k = 5; k < 14; k += 3) R(d, 2, k, 4, 1, 'rgba(18,14,42,.35)');
    } else if (theme === MODERN) {
      // A fuel gauge on a post: a steel frame, the level in its window, ticks up the side.
      R(d, 0, 0, BARREL_W, BARREL_H, P.o); R(d, 1, 1, 6, 14, M.steel); R(d, 2, 2, 4, 12, M.steelD);
      R(d, 1, 1, 6, 1, P.stoneL);
      barrelLevel(d, fill, col);
      [5, 8, 11].forEach(k => R(d, 6, k, 1, 1, P.white));
    } else {
      // A wooden barrel, bellied: the level showing through its staves, two iron hoops.
      R(d, 1, 0, 6, BARREL_H, P.o); R(d, 0, 2, BARREL_W, 12, P.o);
      R(d, 2, 1, 4, 14, P.wood); R(d, 1, 2, 6, 12, P.wood);
      R(d, 1, 2, 1, 12, P.woodD); R(d, 6, 2, 1, 12, P.woodD); R(d, 4, 2, 1, 12, P.woodD);
      barrelLevel(d, fill, col);
      [3, 11].forEach(hy => { R(d, 0, hy, 2, 2, P.stoneD); R(d, 6, hy, 2, 2, P.stoneD); R(d, 0, hy, 2, 1, P.stone); R(d, 6, hy, 2, 1, P.stone); });
      R(d, 2, 1, 4, 1, P.woodL);
    }
  });
}

registerLayer({ z: 74, draw(c, { v, lay }, t) {
  v.houses.forEach((h, i) => {
    const lot = lay.lots[i];
    if (!lot || h.context_pct == null) return;
    barrel(c, lot.x + 16, lot.y + 8, h.context_pct, t + i * 97);
    tap(lot.x + 16, lot.y + 7, 4, 9, { kind: 'house', i });
  });
  const m = v.manager;
  if (m && m.context_pct != null && m.status !== 'stopped') {
    barrel(c, lay.hallX + 34, 30, m.context_pct, t);
    tap(lay.hallX + 34, 29, 4, 9, { kind: 'hall' });
  }
} });

/* The Key: what a barrel's level and colour mean (registerKey, so key.js needs no edit). */
registerKey({ group: "Beside a house: its agent's context", rows: theme => [
  [c => barrel(c, 5, 3, 40, 0, theme), `Under ${CTX_WARN}% used: plenty left, a third full and green`],
  [c => barrel(c, 5, 3, 80, 0, theme), `${CTX_WARN}% to ${CTX_FULL}%: getting full, amber`],
  [c => barrel(c, 5, 3, 97, 0, theme), `${CTX_FULL}% or more, red; flashing from ${CTX_WEDGE}%: about to wedge`],
  [c => theme.B.cottage(c, 3, 5, 'idle', 0), 'No barrel: no status line to read the context from', 22]
] });
