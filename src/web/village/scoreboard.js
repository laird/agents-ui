/* ---------- The gate's scoreboard and hotspots sign ----------
 * The inefficiency view (#435), beside the gatehouse: a slate with today's
 * tally, landed in green and bounced in red, and a signpost with a red flame
 * for the files that conflict most. A click on either says it in words
 * (scoretally.js). Drawn only when the gate reports a tally
 * (`pipeline.merge_stats`, #431); the sign only when something conflicted. */
import { P, R, blink } from './palette.js';
import { state } from './state.js';
import { numberTag, numberTagSize } from './scenery.js';
import { registerHit, registerKey, registerLabels, registerLayer, tap } from './registry.js';
import { hotspotLines, scoreLine } from './scoretally.js';

const LANDED = '#38c848', BOUNCED = '#e02828', SLATE = '#283040';
/* Where the slate stands: east of the gatehouse, clear of its lamp and above the locks' towers. */
export const tallyAt = g => ({ x: g.x0 + g.w + 4, y: g.y });

/* The slate with its two counts; how wide it is. */
export function slate(c, x, y, landed, bounced) {
  const wl = numberTagSize(landed).w, wb = numberTagSize(bounced).w, w = wl + wb + 5;
  R(c, x + 1, y + 11, 1, 4, P.woodD); R(c, x + w - 2, y + 11, 1, 4, P.woodD);
  R(c, x, y, w, 12, P.o); R(c, x + 1, y + 1, w - 2, 10, SLATE);
  numberTag(c, x + 2, y + 3, landed, LANDED, P.o);
  numberTag(c, x + 3 + wl, y + 3, bounced, BOUNCED, P.white);
  return w;
}
/* The hotspots sign: a post, a board, and a red flame on it. */
export function hotspotSign(c, x, y, t) {
  R(c, x + 3, y + 7, 1, 8, P.woodD);
  R(c, x, y, 8, 8, P.o); R(c, x + 1, y + 1, 6, 6, P.parch);
  const f = !state.reduce && blink(t, 220) ? 1 : 0;
  R(c, x + 3, y + 2 + f, 2, 4 - f, BOUNCED); R(c, x + 2, y + 4, 1, 2, BOUNCED); R(c, x + 5, y + 3, 1, 3, BOUNCED);
  R(c, x + 3, y + 4, 2, 2, P.gold);
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

