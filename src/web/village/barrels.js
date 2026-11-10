/* ---------- Context barrels ----------
 * How much of its context window each house's agent has used (`context_pct`,
 * from the status line under its input box): a barrel beside the house, and
 * beside the hall for the manager, filling as the context does. Green below
 * 70%, amber to 90%, red from 90%, flashing from 95%: an agent there is about
 * to wedge (Claude Code stops at about 97%). Downtown it is a fuel gauge, in
 * the future a battery. No barrel when the context is unknown. */
import { state } from './state.js';
import { P, R, blink } from './palette.js';
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
/* A barrel 4 wide and 8 tall at (x, y), filled to `pct` from the bottom. */
export function barrel(c, x, y, pct, t, theme = state.T) {
  x = Math.round(x); y = Math.round(y);
  const fill = Math.round(6 * Math.max(0, Math.min(100, pct)) / 100);
  const flash = pct >= CTX_WEDGE && !state.reduce && blink(t, 300);
  const col = flash ? P.white : contextColour(pct);
  if (theme === FUTURE) {
    // A battery: a cap on top, cells lit from the bottom.
    R(c, x + 1, y - 1, 2, 1, P.o); R(c, x, y, 4, 8, P.o); R(c, x + 1, y + 1, 2, 6, '#262b4c');
    if (fill) R(c, x + 1, y + 7 - fill, 2, fill, col);
    // A dim line between cells, as on a battery's charge meter.
    for (let k = y + 3; k < y + 7; k += 2) R(c, x + 1, k, 2, 1, 'rgba(18,14,42,.35)');
  } else if (theme === MODERN) {
    // A fuel gauge on a post: a steel frame, the level inside.
    R(c, x, y, 4, 8, P.o); R(c, x + 1, y + 1, 2, 6, M.steelD);
    if (fill) R(c, x + 1, y + 7 - fill, 2, fill, col);
    R(c, x + 3, y + 2, 1, 1, P.white);
  } else {
    // A wooden barrel, the level showing through its staves; two hoops on its sides.
    R(c, x, y, 4, 8, P.o); R(c, x + 1, y + 1, 2, 6, P.wood);
    if (fill) R(c, x + 1, y + 7 - fill, 2, fill, col);
    [y + 2, y + 5].forEach(hy => { R(c, x, hy, 1, 1, P.stone); R(c, x + 3, hy, 1, 1, P.stone); });
    R(c, x + 1, y, 2, 1, P.woodL);
  }
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
