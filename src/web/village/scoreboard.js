/* ---------- The gate's scoreboard and hotspots sign ----------
 * The inefficiency view (#435), beside the gatehouse: a slate with today's
 * tally, landed in green and bounced in red, and a signpost with a red flame
 * for the files that conflict most. A click on either says it in words
 * (scoretally.js). Drawn only when the gate reports a tally
 * (`pipeline.merge_stats`, #431); the sign only when something conflicted. */
import { DIGITS, P, blink } from './palette.js';
import { detail } from './detail.js';
import { dither, px } from './art2x.js';
import { state } from './state.js';
import { numberTagSize } from './scenery.js';
import { registerHit, registerKey, registerLabels, registerLayer, tap } from './registry.js';
import { hotspotLines, scoreLine } from './scoretally.js';

const LANDED = '#38c848', BOUNCED = '#e02828', SLATE = '#283040';
/* Where the slate stands: east of the gatehouse, clear of its lamp and above the locks' towers. */
export const tallyAt = g => ({ x: g.x0 + g.w + 4, y: g.y });

/* A count chalked on a coloured chit, drawn in detail pixels from (x, y),
 * the chit's outer corner: the same 3×5 digits as numberTag, each stroke a
 * 2×2 block over a drop shadow, so the figures read embossed. */
function chit(d, x, y, n, bg, ink) {
  const glyphs = String(n).split('').map(k => DIGITS[+k]), w = 2 * numberTagSize(n).w;
  px(d, x, y, w, 18, P.o); px(d, x + 2, y + 2, w - 4, 14, bg); px(d, x + 2, y + 2, w - 4, 1, 'rgba(255,255,255,.3)');
  // A drop shadow under every stroke first, then the strokes over it.
  [['rgba(0,0,0,.3)', 1], [ink, 0]].forEach(([col, o]) => glyphs.forEach((g, j) => {
    for (let k = 0; k < 15; k++) if (g[k] === '1') px(d, x + 4 + j * 8 + 2 * (k % 3) + o, y + 4 + 2 * Math.floor(k / 3) + o, 2, 2, col);
  }));
}
/* The slate with its two counts; how wide it is (scene pixels, as before:
 * the chits' widths and five pixels of frame). Drawn at 2× (#458): a slate
 * in a wooden frame on two legs, chalk dust on it, the counts on chits. */
export function slate(c, x, y, landed, bounced) {
  const wl = numberTagSize(landed).w, wb = numberTagSize(bounced).w, w = wl + wb + 5;
  detail(c, x, y, d => {
    [2, 2 * w - 4].forEach(lx => { px(d, lx, 22, 2, 8, P.woodD); px(d, lx, 22, 1, 8, P.wood); });
    px(d, 0, 0, 2 * w, 24, P.o); px(d, 1, 1, 2 * w - 2, 22, P.wood); px(d, 1, 1, 2 * w - 2, 1, P.woodL);
    px(d, 2, 2, 2 * w - 4, 20, SLATE);
    // Chalk dust where the old counts were wiped.
    dither(d, 3, 3, 2 * w - 6, 18, 5, (dx, dy) => (dx * 7 + dy * 3) % 11 < 3 ? 'rgba(232,232,240,.16)' : null);
    chit(d, 2, 2, landed, LANDED, P.o);
    chit(d, 4 + 2 * wl, 2, bounced, BOUNCED, P.white);
  });
  return w;
}
/* The hotspots sign at 2× (#458): a post, a parchment board with a nail at
 * each corner, and a red flame on it, flickering, a gold heart in it. */
export function hotspotSign(c, x, y, t) {
  const f = !state.reduce && blink(t, 220) ? 1 : 0;
  detail(c, x, y, d => {
    px(d, 6, 14, 2, 16, P.woodD); px(d, 6, 14, 1, 16, P.wood);
    px(d, 0, 0, 16, 16, P.o); px(d, 2, 2, 12, 12, P.parch); px(d, 2, 13, 12, 1, P.mill);
    [[2, 2], [12, 2], [2, 12], [12, 12]].forEach(([nx, ny]) => px(d, nx, ny, 1, 1, P.stoneD));
    // The flame, a row at a time from its tip; it licks a pixel higher on the flicker.
    [[7, 3 - f, 2], [6, 5 - f, 3], [5, 7, 6], [4, 9, 8], [4, 11, 8], [5, 12, 6]].forEach(([fx, fy, fw]) => px(d, fx, fy, fw, 2, BOUNCED));
    px(d, 4, 7, 1, 2, BOUNCED); px(d, 11, 6, 1, 3, BOUNCED);
    px(d, 6, 8, 4, 4, '#f07020'); px(d, 7, 9, 2, 3, P.gold); px(d, 7, 9, 1, 1, P.white);
  });
}

registerLayer({ z: 76, draw(c, { v, lay }, t) {
  const g = lay.gate, ms = v.pipeline.merge_stats;
  if (!g || !ms) return;
  const { x, y } = tallyAt(g);
  const w = slate(c, x, y, ms.landed_today || 0, ms.bounced_today || 0);
  tap(x, y, w, 15, { kind: 'scoreboard' });
  if ((ms.hotspots || []).length) {
    hotspotSign(c, x + w + 3, y, t);
    tap(x + w + 2, y - 1, 10, 16, { kind: 'hotspots' });
  }
} });

registerHit({ kind: 'scoreboard', dialog(o, sc) {
  const ms = sc.v.pipeline.merge_stats || {}, top = hotspotLines(ms)[0];
  return { title: 'The merge gate today', line: `Work ${scoreLine(ms)}`,
    meta: ['Landed (green): merged onto the integration branch', 'Bounced (red): handed back to its worker, a conflict or a failed gate',
      top ? `Most conflicted: ${top}` : null] };
} });
registerHit({ kind: 'hotspots', dialog(o, sc) {
  const lines = hotspotLines(sc.v.pipeline.merge_stats);
  return { title: 'Conflict hotspots', line: 'The files that conflict most today: work that touches them bounces back from the gate.', meta: lines };
} });

/* How wide the slate is for these counts (slate() draws it so). */
const slateW = ms => numberTagSize(ms.landed_today || 0).w + numberTagSize(ms.bounced_today || 0).w + 5;
/* The tally in words under the slate; every label keeps clear of the slate and the sign. */
registerLabels(sc => {
  const g = sc.L.gate, ms = sc.v.pipeline.merge_stats, words = scoreLine(ms);
  if (!g || !words) return [];
  const { x, y } = tallyAt(g), w = slateW(ms), signed = (ms.hotspots || []).length;
  return [{ avoid: { x, y: y - 1, w: w + (signed ? 12 : 0), h: 17 } },
    { x: x + (w >> 1), y: y + 16, anchor: 'top', text: words }];
}, 32);

/* In the Key, beside the bounce marks. */
registerKey({ group: 'other', rows: () => [
  [c => slate(c, 1, 1, 14, 6), 'Slate by the gatehouse: work landed (green) and bounced (red) today', 26],
  [c => hotspotSign(c, 3, 0, 0), 'Sign with a red flame by the gatehouse: the files that conflict most (click it)', 16]] });

