/* ---------- Ghosts at the gatehouse ----------
 * A flaky test (`flaky` in /api/village, #501: it failed, then passed on
 * the same tree) haunts the merge gate as a small ghost hovering by the
 * gatehouse (#502), labelled with the test and how often it flaked this
 * week. It fades as the test stays clean, and is gone after a clean week or
 * once its `Flaky:` issue closes. Each theme has its own:
 *   medieval  a sheet ghost, eyes dark, its hem ragged;
 *   modern    a glitching hologram, cyan, its scanlines slipping;
 *   future    a static-noise sprite, its pixels crawling.
 * A click (or Enter, #478) lists the failures: when, which issue's merge was
 * being gated, and the log. Which ghosts, where and how faint is decided in
 * ghostmarks.js; the art is drawn at 2× here. */
import { state } from './state.js';
import { P, hash } from './palette.js';
import { detail } from './detail.js';
import { pixmap, px } from './art2x.js';
import { registerHit, registerKey, registerLabels, registerLayer, tap } from './registry.js';
import { issueLabel, issueOf, link } from './dialogs.js';
import { when } from './bouncemarks.js';
import { failureLines, ghostRoom, ghostSpot, ghostTags, ghostsOf, noteFlakyIssues } from './ghostmarks.js';
import { MODERN } from './themes/modern.js';
import { FUTURE } from './themes/future.js';

/* Each `Flaky:` issue seen per village, so one that closes is noticed. */
const SEEN = new Map();
const seenIn = v => {
  if (!SEEN.has(v.project)) SEEN.set(v.project, new Map());
  return SEEN.get(v.project);
};
/* The ghosts shown for a village now. The issues are noted after, so a
 * ghost leaves on the poll that sees its issue gone. */
function shown(v) {
  const seen = seenIn(v), all = ghostsOf(v, Date.now(), seen);
  noteFlakyIssues(v, seen);
  return all;
}

/* The ghost's shape, 12×14 detail pixels: B body, E eye, H hem shade. */
const SHEET = [
  '....BBBB....', '..BBBBBBBB..', '.BBBBBBBBBB.', '.BBEEBBEEBB.', 'BBBEEBBEEBBB', 'BBBBBBBBBBBB',
  'BBBBBEEBBBBB', 'BBBBBEEBBBBB', 'BBBBBBBBBBBB', 'BBBBBBBBBBBB', 'HBBBBBBBBBBH', 'HBBHBBBBHBBH', '.HH.HBBH.HH.', '.....HH.....'];

/* One ghost at scene (x, y), `alpha` solid, in this theme's way. */
export function ghost(c, x, y, alpha, t, theme = state.T) {
  c.save();
  c.globalAlpha *= alpha;
  detail(c, x, y, d => {
    if (theme === MODERN) {
      // A hologram: cyan rows, every other one dim, one row slipping sideways now and then.
      const slip = state.reduce ? -1 : Math.floor(t / 90) % 23;
      SHEET.forEach((row, j) => {
        const dx = j === slip % 14 ? 1 : 0, cols = { B: j % 2 ? 'rgba(96,224,232,.55)' : '#60e0e8', E: '#185070', H: 'rgba(96,224,232,.35)' };
        pixmap(d, dx, j, [row], cols);
      });
      px(d, 2, 14, 8, 1, 'rgba(96,224,232,.4)');
    } else if (theme === FUTURE) {
      // Static: each pixel of the shape grey, white or dark, reshuffled every frame.
      const frame = state.reduce ? 0 : Math.floor(t / 120);
      SHEET.forEach((row, j) => [...row].forEach((ch, i) => {
        if (ch === '.') return;
        const n = Math.floor(hash(i + frame * 13, j + frame * 7) * 3);
        px(d, i, j, 1, 1, ch === 'E' ? P.o : [P.stoneL, P.white, P.stoneD][n]);
      }));
    } else {
      pixmap(d, 0, 0, SHEET, { B: P.white, E: P.o, H: P.stoneL });
    }
  });
  c.restore();
}

registerLayer({ z: 78, draw(c, { v, lay }, t) {
  const g = lay.gate; if (!g) return;
  shown(v).slice(0, ghostRoom(g)).forEach(({ f, alpha }, k) => {
    const { x, y } = ghostSpot(g, k, t, state.reduce);
    ghost(c, x, y, alpha, t);
    tap(x, y, 6, 8, { kind: 'ghost', test: f.test });
  });
} });

registerHit({ kind: 'ghost', dialog(o, sc) {
  const v = sc.v, f = ((v.flaky) || []).find(x => x.test === o.test);
  if (!f) return null;
  const it = f.issue ? issueOf(v, f.issue) : null;
  return {
    title: `Flaky: ${f.test}`,
    line: `Failed and then passed on the same tree ${f.count === 1 ? 'once' : f.count + ' times'} this week${f.issue ? `, filed as #${f.issue}` : ''}`,
    meta: failureLines(f, when),
    links: [it ? link(issueLabel(f.issue), it.url) : null],
  };
} });

/* One label over the ghosts, fixed there so it can't drift from them (other
 * labels keep clear): each test and its count, wrapping between them, and
 * how many more there are than the roof has room for. */
registerLabels(sc => {
  const g = sc.L.gate; if (!g) return [];
  const all = ghostsOf(sc.v, Date.now(), seenIn(sc.v)); if (!all.length) return [];
  const n = Math.min(all.length, ghostRoom(g)), first = ghostSpot(g, 0), last = ghostSpot(g, n - 1);
  return [{ x: (first.x + last.x) / 2 + 3, y: first.y - 1, anchor: 'bottom', fixed: true, cls: 'ghost', width: 64,
    text: ghostTags(all, n).join(' · '), parts: ghostTags(all, n) }];
}, 32);

/* The Key's row: this theme's ghost. */
registerKey({ group: 'other', rows: theme => [
  [c => ghost(c, 3, 2, 1, 0, theme), 'Ghost by the merge gate: a flaky test (failed, then passed on the same tree), with how often this week; it fades as the test stays clean', 10]] });
