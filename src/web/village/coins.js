/* ---------- Coins by the door: each house's token use (#515, for #496) ----------
 * How many tokens each house's agent has used today (`tokens.today` on the
 * house, read from Claude Code's transcripts, usage.rs, #514): a stack of
 * coins at the house's front corner, one more coin each time the day's use
 * doubles past a million, up to COIN_MAX. Downtown it is a parking meter whose
 * window fills, in the future a stack of glowing credit chips. No stack when
 * the field is absent (a remote village, a non-Claude runtime) or nothing was
 * used today. The house's dialog says today's use and its current issue's,
 * with the breakdown (tokenWords). A house spending fast with no commits is
 * worth a look; it pairs with the context barrel (barrels.js). Drawn at 2×:
 * tools/village-tests/coins.test.mjs paints it under node. */
import { state } from './state.js';
import { P, R } from './palette.js';
import { detail } from './detail.js';
import { registerKey, registerLayer, tap } from './registry.js';
import { MODERN, M } from './themes/modern.js';
import { FUTURE } from './themes/future.js';

/* The tallest stack, and the use that earns the first coin. */
export const COIN_MAX = 8, COIN_FIRST = 1e6;
/* Coins for `total` tokens: none for nothing, one up to two million, then one
 * more per doubling, capped at COIN_MAX. Token use runs from thousands to
 * hundreds of millions a day, so a straight scale would hide all but the top. */
export function coinCount(total) {
  if (!Number.isFinite(total) || total <= 0) return 0;
  return Math.max(1, Math.min(COIN_MAX, 1 + Math.floor(Math.log2(total / COIN_FIRST))));
}

/* 262, 262k, 90.4M, 1.2B. */
export function tokenCount(n) {
  if (!Number.isFinite(n)) return '?';
  const unit = (d, s) => { const x = n / d; return `${x >= 100 ? Math.round(x) : Math.round(x * 10) / 10}${s}`; };
  return n >= 1e9 ? unit(1e9, 'B') : n >= 1e6 ? unit(1e6, 'M') : n >= 1e3 ? unit(1e3, 'k') : String(n);
}

const breakdown = t => `${tokenCount(t.output)} out, ${tokenCount(t.input)} in, ${tokenCount(t.cache_write)} cache write, ${tokenCount(t.cache_read)} cache read`;

/* The dialog's lines: today's use, then the current issue's (all of it), each
 * with its breakdown. Nothing when the house has no token count. */
export function tokenWords(h) {
  const tk = h && h.tokens;
  if (!tk || !tk.today) return [];
  const out = [`Tokens today: ${tokenCount(tk.today.total)} (${breakdown(tk.today)})`];
  if (h.issue && tk.issue) out.push(`Tokens on #${h.issue}: ${tokenCount(tk.issue.total)} (${breakdown(tk.issue)})`);
  return out;
}

/* A stack's footprint, in detail pixels: 6 wide, 2 rows per coin, standing
 * on scene (x, y + COIN_MAX) so a taller stack grows up, not down. */
export const COIN_W = 6, COIN_ROWS = 2;

/* `n` coins at scene (x, y): the stack's base is COIN_MAX scene pixels down. */
export function coins(c, x, y, n, theme = state.T) {
  if (!n) return;
  detail(c, x, y, d => {
    const base = COIN_MAX * 2; // detail rows to the ground
    if (theme === MODERN) {
      // A parking meter: a post, a head whose window fills with the count.
      R(d, 2, base - 6, 2, 6, M.steelD);
      R(d, 0, base - 6 - COIN_MAX * COIN_ROWS / 2 - 3, COIN_W, COIN_MAX * COIN_ROWS / 2 + 3, P.o);
      const top = base - 6 - COIN_MAX * COIN_ROWS / 2 - 2;
      R(d, 1, top, 4, COIN_MAX, M.steel);
      R(d, 1, top + COIN_MAX - n, 4, n, '#f0c020');
      R(d, 1, top + COIN_MAX - n, 4, 1, 'rgba(255,255,255,.45)');
      return;
    }
    for (let k = 0; k < n; k++) {
      const y0 = base - (k + 1) * COIN_ROWS, wob = k % 2; // a hand-made stack leans a little
      if (theme === FUTURE) {
        // A credit chip: dark rim, a cyan core lit along its top.
        R(d, wob, y0, COIN_W - 1, COIN_ROWS, P.o);
        R(d, wob + 1, y0, COIN_W - 3, 1, '#40e0f0');
      } else {
        // A gold coin, edge-on: a dark rim under a bright face.
        R(d, wob, y0, COIN_W - 1, COIN_ROWS, '#8a5a10');
        R(d, wob + 1, y0, COIN_W - 3, 1, '#f0c020');
      }
    }
  });
}

registerLayer({ z: 74.5, draw(c, { v, lay }) {
  v.houses.forEach((h, i) => {
    const lot = lay.lots[i], n = coinCount(h.tokens && h.tokens.today && h.tokens.today.total);
    if (!lot || !n) return;
    // At the house's front left corner, inside its lot: the house to the
    // left's barrel stands just past the lot's edge (barrels.js).
    coins(c, lot.x, lot.y + 13, n);
    tap(lot.x, lot.y + 13 + COIN_MAX - n, 3, n, { kind: 'house', i });
  });
} });

/* The Key: what a stack's height means (registerKey, so key.js needs no edit). */
registerKey({ group: "Beside a house: its token use", order: 72, rows: theme => [
  [c => coins(c, 5, 3, 1, theme), 'One coin: up to 2 million tokens today'],
  [c => coins(c, 5, 3, 5, theme), 'One more each time the day\'s use doubles: 5 coins, 16–32 million'],
  [c => coins(c, 5, 3, COIN_MAX, theme), `${COIN_MAX} coins: 128 million or more`],
] });
