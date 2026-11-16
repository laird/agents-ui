/* ---------- The counting house (#516) ----------
 * The village's token use (#514, `usage`), read from Claude Code's
 * transcripts: a counting house at the east edge, past the almanac, with a
 * tally board of the last 7 days beside it. Its label says today's tokens;
 * clicking it lists today, each of the last 7 days and the costliest issues
 * this week. Tokens, never dollars: rates depend on the plan (#496). A stone
 * counting house in the village, a bank downtown, a credit vault in the
 * future. Absent when the village has no `usage`. Drawn at 2×, inside
 * detail(). What it says is decided in countingtally.js. */
import { state } from './state.js';
import { P, F } from './palette.js';
import { detail } from './detail.js';
import { px } from './art2x.js';
import { registerHit, registerKey, registerLabels, registerLayer, tap } from './registry.js';
import { MODERN } from './themes/modern.js';
import { FUTURE } from './themes/future.js';
import { COUNTING_SIZE, countingBars, countingFacts, countingSub, countingUsage } from './countingtally.js';

/* Each theme's materials: walls, their shade, the roof, the trim, the coin
 * on the sign, the tally board and its bars. */
const COUNTING_LOOKS = {
  medieval: { name: 'Counting house', wall: P.stoneL, wallD: P.stone, roof: P.slate, roofD: P.slateD, trim: P.woodD, glass: P.lamp, coin: P.gold, board: '#2c3a30', bar: P.parch },
  modern: { name: 'Bank', wall: '#d8d4c8', wallD: '#a8a498', roof: '#808890', roofD: '#585e68', trim: '#f0ece0', glass: P.glass, coin: P.gold, board: '#1c2430', bar: '#60e080' },
  future: { name: 'Credit vault', wall: '#262b4c', wallD: '#161a34', roof: '#3a4070', roofD: '#20244a', trim: F.cyan, glass: F.magenta, coin: F.cyan, board: '#10122a', bar: F.cyan },
};
export const countingLook = theme => (theme === FUTURE ? COUNTING_LOOKS.future : theme === MODERN ? COUNTING_LOOKS.modern : COUNTING_LOOKS.medieval);

/* The house, 24 × 22 scene pixels at (x, y), and its tally board beside it:
 * a bar per day, each as tall as its share of the busiest day. */
export function countingArt(c, x, y, bars, theme = state.T) {
  const k = countingLook(theme);
  detail(c, x, y, d => {
    // The roof: a gable, outlined, or a flat cornice downtown and in the future.
    if (k === COUNTING_LOOKS.medieval) {
      for (let r = 0; r < 8; r++) { px(d, 22 - 3 * r, r * 2, 4 + 6 * r, 2, P.o); px(d, 23 - 3 * r, r * 2, 2 + 6 * r, 2, r < 6 ? k.roof : k.roofD); }
      px(d, 0, 16, 48, 2, P.o);
    } else {
      px(d, 2, 8, 44, 10, P.o); px(d, 3, 9, 42, 7, k.roof); px(d, 3, 15, 42, 2, k.roofD);
      px(d, 14, 2, 20, 7, P.o); px(d, 15, 3, 18, 6, k.roof);
    }
    // The walls, shaded on the right.
    px(d, 2, 18, 44, 26, P.o); px(d, 3, 18, 42, 25, k.wall); px(d, 36, 18, 9, 25, k.wallD);
    // Beams across the walls in the village; columns downtown and in the future.
    if (k === COUNTING_LOOKS.medieval) { px(d, 3, 24, 42, 1, k.trim); px(d, 3, 30, 42, 1, k.trim); }
    else [6, 16, 30, 40].forEach(cx => { px(d, cx, 20, 3, 22, k.trim); px(d, cx + 2, 20, 1, 22, k.wallD); });
    // Two windows, lit, and the door.
    [[8, 26], [33, 26]].forEach(([wx, wy]) => { px(d, wx, wy, 7, 6, P.o); px(d, wx + 1, wy + 1, 5, 4, k.glass); });
    px(d, 20, 30, 8, 13, P.o); px(d, 21, 31, 6, 12, k === COUNTING_LOOKS.future ? '#0a0c1e' : P.woodD);
    // The sign over the door: a coin on a bracket.
    px(d, 21, 19, 6, 6, P.o); px(d, 22, 20, 4, 4, k.coin); px(d, 23, 21, 1, 2, P.white);
    // The step, and its shadow on the ground.
    px(d, 18, 43, 12, 1, P.o);
    // The tally board on two posts, a bar per day.
    const bx = 52, by = 12, bw = 16, bh = 22;
    px(d, bx + 2, by + bh, 2, 10, P.o); px(d, bx + bw - 4, by + bh, 2, 10, P.o);
    px(d, bx, by, bw, bh, P.o); px(d, bx + 1, by + 1, bw - 2, bh - 2, k.board);
    const week = bars.slice(-7);
    week.forEach((b, i) => {
      const h = Math.max(1, Math.round(b.share * (bh - 4)));
      px(d, bx + 1 + i * 2, by + bh - 2 - h, 1, h, i === week.length - 1 ? k.coin : k.bar);
    });
  });
}

registerLayer({ z: 46, draw(c, { v, lay }) {
  const at = lay.counting, u = at && countingUsage(v);
  if (!u) return;
  countingArt(c, at.x, at.y, countingBars(u), state.T);
  tap(at.x, at.y, at.w, at.h, { kind: 'counting' });
} });

registerHit({ kind: 'counting', dialog(o, sc, theme) {
  const facts = countingFacts(countingUsage(sc.v));
  if (!facts) return null;
  return { title: `${countingLook(theme).name}: tokens used`, line: facts.line, meta: facts.meta };
} });

/* Its name under it, with today's tokens; labels keep off it. */
registerLabels((sc, theme) => {
  const at = sc.L.counting, u = at && countingUsage(sc.v);
  if (!u) return [];
  return [{ avoid: { x: at.x, y: at.y, w: at.w, h: at.h } },
    { x: at.x + (at.w >> 1), y: at.y + at.h + 1, anchor: 'top', text: countingLook(theme).name, sub: countingSub(u) }];
}, 35);

/* In the Key: the house and a week on its board, today busiest. */
const COUNTING_SAMPLE = [0.3, 0.6, 0.4, 0.8, 0.5, 0.2, 1].map(share => ({ share }));
registerKey({ group: 'other', rows: theme => [
  [c => countingArt(c, 1, 1, COUNTING_SAMPLE, theme),
    `${countingLook(theme).name} at the east edge: the tokens the swarm used, read from Claude Code's transcripts; its board has a bar per day for the last week, today's in the sign's colour (click it for today, the week and the costliest issues)`, COUNTING_SIZE.w + 2],
] });
