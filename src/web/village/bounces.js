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
 * bouncemarks.js; the art, at 2× (#457), is in bounceart.js. */
import { registerHit, registerKey, registerLabels, registerLayer, tap } from './registry.js';
import { issueLabel, issueOf, link } from './dialogs.js';
import { bounceLines, bounceMarks, bouncesOf, tagLevels } from './bouncemarks.js';
import { debris, reworked } from './bounceart.js';

/* Where the house's crate sits (lifecycle.js draws it there). */
const crateAt = lot => [lot.x + 11, lot.y + 16];

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

