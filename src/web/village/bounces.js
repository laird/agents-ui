/* ---------- Bounces ----------
 * A crate the gate handed back (`pipeline.bounces`, #431) sits at its
 * house dressed for rework (#433), each theme in its own way:
 *   medieval  back on a cart, a red rag tied to it;
 *   modern    a rejected parcel with a red stamp across it;
 *   future    a red force field round it.
 * Over the house a red tag says why and how often ("↺ conflict",
 * "↺2 gate failed"); from the second bounce a little pile of broken-crate
 * debris lies beside it, gone once the issue lands. A click lists every
 * attempt: when, why, and which files conflicted. What the marks say is in
 * bouncemarks.js. */
import { state } from './state.js';
import { P, R, blink } from './palette.js';
import { registerHit, registerKey, registerLabels, registerLayer, tap } from './registry.js';
import { issueLabel, issueOf, link } from './dialogs.js';
import { MODERN } from './themes/modern.js';
import { FUTURE } from './themes/future.js';
import { bounceLines, bounceMarks, bouncesOf, tagLevels } from './bouncemarks.js';

/* Where the house's crate sits (lifecycle.js draws it there). */
const crateAt = lot => [lot.x + 11, lot.y + 16];

function reworked(c, x, y, t) {
  if (state.T === FUTURE) {
    const a = state.reduce || blink(t, 400) ? 'rgba(255,60,60,.85)' : 'rgba(255,60,60,.45)';
    R(c, x - 1, y - 1, 7, 1, a); R(c, x - 1, y + 5, 7, 1, a); R(c, x - 1, y, 1, 5, a); R(c, x + 5, y, 1, 5, a);
  } else if (state.T === MODERN) {
    // A red REJECTED stamp: a box and its diagonal across the parcel.
    R(c, x, y + 1, 5, 1, P.p0); R(c, x, y + 3, 5, 1, P.p0); R(c, x + 1, y + 2, 1, 1, P.p0); R(c, x + 3, y + 2, 1, 1, P.p0);
  } else {
    // Back on its cart: two wheels and an axle, and a red rag on the corner.
    R(c, x - 1, y + 5, 7, 1, P.woodD); R(c, x, y + 5, 2, 2, P.o); R(c, x + 3, y + 5, 2, 2, P.o);
    R(c, x + 4, y - 2, 1, 2, P.woodD); R(c, x + 5, y - 2, 2, 2, P.p0);
  }
}

/* Broken boards beside the house: one more for each bounce past the first, up to four. */
function debris(c, lot, count) {
  const x = lot.x + 16, y = lot.y + 18, col = state.T === FUTURE ? P.stoneD : P.woodL;
  [[0, 0, 3], [1, 2, 2], [0, 3, 3], [2, 1, 2]].slice(0, Math.min(4, count - 1)).forEach(([dx, dy, w]) => {
    R(c, x + dx, y + dy, w, 1, col); R(c, x + dx + w - 1, y + dy + 1, 1, 1, P.woodD);
  });
}

registerLayer({ z: 75, draw(c, { v, lay }, t) {
  bounceMarks(v).forEach(m => {
    const lot = lay.lots[m.i]; if (!lot) return;
    const [x, y] = crateAt(lot);
    reworked(c, x, y, t);
    if (m.debris) debris(c, lot, m.count);
    tap(x - 1, y - 2, 8, 9, { kind: 'bounce', n: m.n, i: m.i });
  });
} });

registerHit({ kind: 'bounce', dialog(o, sc) {
  const v = sc.v, it = issueOf(v, o.n), bs = bouncesOf(v, o.n), last = bs[bs.length - 1] || {};
  return {
    title: `#${o.n} ${it.title || ''}`.trim(),
    line: `Back for rework: the gate handed it back ${bs.length === 1 ? 'once' : bs.length + ' times'}${last.kind === 'conflict' ? ', on a merge conflict' : last.kind === 'gate-failed' ? ', its tests failing on the integrated tree' : ''}`,
    meta: bounceLines(bs),
    links: [link(issueLabel(o.n), it.url)],
  };
} });

/* The red tag on each house's roof: why it came back, and how often. Fixed
 * there, so it can't drift towards a neighbour's; other labels keep clear.
 * A tag is wider than a house, so one whose neighbour in the row has a tag
 * too stands a step higher (tagLevels in bouncemarks.js). */
registerLabels(sc => {
  const marks = bounceMarks(sc.v).filter(m => sc.L.lots[m.i]), levels = tagLevels(marks.map(m => sc.L.lots[m.i]));
  return marks.map((m, k) => {
    const lot = sc.L.lots[m.i];
    return { x: lot.x + 8, y: lot.y - 1 - levels[k] * 8, anchor: 'bottom', text: m.tag, cls: 'bounce', fixed: true };
  });
}, 30);

/* The Key's row: the crate as this theme dresses it for rework. */
registerKey({ group: 'other', rows: theme => [
  [c => { theme.crate(c, 3, 4, 'feature', null); reworked(c, 3, 4, 0); },
    'Crate dressed for rework at a house, a red ↺ tag over it: the merge gate handed it back (↺2: the second time; broken boards from then on)', 13]] });

