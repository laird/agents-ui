/* ---------- Returned goods ----------
 * Landed work that came back (#497): reopened, reverted on the integration
 * branch, or named in a regression report. Each sits on the quay, left of
 * the road where landed work arrives, as its crate with a red tag tied on,
 * until it is claimed again. A click says why it came back and links the
 * issue and the report; the share of the window's landings that came back
 * is in the dialog and the almanac. What it says is in returnwords.js. */
import { state } from './state.js';
import { P, R } from './palette.js';
import { detail } from './detail.js';
import { badge, badgeSize } from './scenery.js';
import { registerHit, registerKey, registerLabels, registerLayer, tap } from './registry.js';
import { issueLabel, issueOf, link, repoUrl } from './dialogs.js';
import { RETURNED_CAP, returnedGoods, returnedLine, returnedShare } from './returnwords.js';

/* Where the k-th returned crate rests: side by side on the quay, left of
 * the road, below the last row of houses (their lots end 4 px above it). */
const returnSpot = (lay, k) => [lay.roadX - 15 + k * 6, lay.quayY - 3];

/* A red tag tied to the crate's top corner, on a short string. */
export function redTag(c, x, y) {
  detail(c, x, y, d => {
    R(d, 9, -3, 1, 3, P.o);
    R(d, 8, -6, 4, 3, P.red); R(d, 8, -6, 4, 1, '#f06050'); R(d, 11, -4, 1, 1, P.redD);
  });
}

registerLayer({ z: 82, draw(c, { v, lay }) {
  const goods = returnedGoods(v);
  goods.slice(0, RETURNED_CAP).forEach((r, k) => {
    const [x, y] = returnSpot(lay, k);
    state.T.crate(c, x, y, 'workshop', null, true, r.number);
    redTag(c, x, y);
    tap(x - 1, y - 3, 7, 8, { kind: 'returned', n: r.number });
  });
  // More than fit: a "+N" to their left (over them it would hide the
  // tags), and the dialog lists them all.
  if (goods.length > RETURNED_CAP) {
    const [x, y] = returnSpot(lay, 0), more = goods.length - RETURNED_CAP, { w, h } = badgeSize(more), bx = x - w;
    badge(c, bx, y, more);
    tap(bx - 1, y - 2, w, h, { kind: 'returned', n: null });
  }
} });

registerHit({ kind: 'returned', dialog(o, sc) {
  const v = sc.v, goods = returnedGoods(v), share = returnedShare(v.github), repo = repoUrl(v);
  const r = goods.find(g => g.number === o.n);
  if (!r) {
    return { title: 'Returned goods', line: `${goods.length} landed ${goods.length === 1 ? 'issue' : 'issues'} came back`,
      meta: [...goods.map(g => `#${g.number} ${g.title || ''}: ${returnedLine(g).toLowerCase()}`), share ? `Over 14 days: ${share.words}` : null].filter(Boolean) };
  }
  const it = issueOf(v, r.number);
  return {
    title: `#${r.number} ${r.title || it.title || ''}`.trim(),
    line: `${returnedLine(r)}. It waits at the quay with a red tag until it is claimed again.`,
    meta: [r.at ? `Came back ${new Date(r.at).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' })}` : null,
      share ? `Over 14 days: ${share.words}` : null].filter(Boolean),
    links: [link(issueLabel(r.number), it.url),
      r.report ? link(`The report, #${r.report}`, repo ? `${repo}/issues/${r.report}` : null) : null,
      r.how === 'reverted' && repo ? link('The integration branch’s history', `${repo}/commits/${encodeURIComponent((v.pipeline && v.pipeline.integration) || 'master')}`) : null],
  };
} });

/* Labels keep clear of the returned crates. */
registerLabels(sc => {
  const n = returnedGoods(sc.v).length;
  if (!n) return [];
  const [x, y] = returnSpot(sc.L, 0), left = n > RETURNED_CAP ? 13 : 1;
  return [{ avoid: { x: x - left, y: y - 3, w: 12 + left, h: 9 } }];
}, 31);

registerKey({ group: 'other', rows: theme => [
  [c => { theme.crate(c, 3, 6, 'workshop', null, true, 12); redTag(c, 3, 6); },
    'Crate on the quay with a red tag: landed work that came back (reopened, reverted, or named in a regression report)', 13]] });
