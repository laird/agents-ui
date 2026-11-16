/* ---------- The almanac ----------
 * Whether the swarm is getting more or less done (#485, #499): a field at
 * the village's east edge (layout.js gives it its clearing), one furrow per
 * day for the last 14 days, today's at the bottom. Each furrow grows a crop
 * per issue landed, red pips for the times the merge gate bounced work back,
 * and a purple cross when something was reverted. Downtown it is an office
 * chart, in the future a holo-graph. Today's row grows with the gate's tally
 * as crates leave the quay. A click (or Enter, #478) opens the numbers.
 * Drawn from the village's `github` summary (#498); nothing without one. The
 * numbers and the marks per day are decided in almanactally.js. */
import { state } from './state.js';
import { P, blink } from './palette.js';
import { detail } from './detail.js';
import { px } from './art2x.js';
import { registerHit, registerKey, registerLabels, registerLayer, tap } from './registry.js';
import { MODERN } from './themes/modern.js';
import { FUTURE } from './themes/future.js';
import {
  ALMANAC_BAR_X, ALMANAC_DAYS, ALMANAC_DW, ALMANAC_FOOT, ALMANAC_GUTTER_X, ALMANAC_HEAD, ALMANAC_PIP_X, ALMANAC_ROW, ALMANAC_UNIT,
  almanacDetailH, almanacFacts, almanacRows, almanacToday,
} from './almanactally.js';

/* The marks every theme shares, so the Key reads the same in each. */
export const ALMANAC_BOUNCE = '#e02828', ALMANAC_REVERT = '#b050e0';
/* Each theme's look: frame, rows (alternate and today's), and the landed bar. */
const ALMANAC_LOOKS = {
  medieval: { name: 'Almanac field', rim: P.wood, rimL: P.woodL, head: P.woodD, row: '#704c28', alt: '#62421f', now: '#8c6234', grid: null, bar: P.gold, barL: P.leafL },
  modern: { name: 'Throughput chart', rim: '#c8ccd8', rimL: '#e8eaf0', head: '#384050', row: '#f4f4f8', alt: '#eaecf2', now: '#d8e6fa', grid: '#d4d8e4', bar: '#3878d0', barL: '#68a0e8' },
  future: { name: 'Holo-graph', rim: '#1a2a50', rimL: '#40e0f0', head: '#101830', row: '#0c1428', alt: '#101a34', now: '#14284a', grid: 'rgba(64,224,240,.18)', bar: '#40e0f0', barL: 'rgba(64,224,240,.35)' },
};
export const almanacLook = theme => theme === MODERN ? ALMANAC_LOOKS.modern : theme === FUTURE ? ALMANAC_LOOKS.future : ALMANAC_LOOKS.medieval;

/* The cross for a day with a revert, 4×4 from (x, y). */
function almanacCross(d, x, y) {
  [[0, 0], [3, 0], [1, 1], [2, 1], [1, 2], [2, 2], [0, 3], [3, 3]].forEach(([dx, dy]) => px(d, x + dx, y + dy, 1, 1, ALMANAC_REVERT));
}
/* One day's landed work along its row, at row top `ry`. */
function almanacBarArt(d, theme, look, ry, bar) {
  const x0 = ALMANAC_BAR_X;
  if (theme === MODERN) {
    // A bar on the chart, lit along its top, darker at its end.
    px(d, x0, ry + 1, bar, 4, look.bar); px(d, x0, ry + 1, bar, 1, look.barL); px(d, x0 + bar - 1, ry + 1, 1, 4, '#2858a0');
    return;
  }
  if (theme === FUTURE) {
    // A holo bar in segments, with a glow over it.
    px(d, x0, ry, bar, 1, look.barL);
    for (let x = 0; x < bar; x += ALMANAC_UNIT) px(d, x0 + x, ry + 1, Math.min(3, bar - x), 4, look.bar);
    return;
  }
  // A crop per landed issue: a golden head on a green stalk.
  for (let x = 0; x < bar; x += ALMANAC_UNIT) {
    const w = Math.min(2, bar - x);
    px(d, x0 + x + 1, ry + 3, 1, 2, P.leaf); px(d, x0 + x + 2, ry + 3, 1, 1, look.barL);
    px(d, x0 + x, ry + 1, w, 2, look.bar);
  }
}

/* The almanac for `rows` (almanacRows()) at scene (x, y), in `theme`. */
export function almanacArt(c, x, y, rows, theme = state.T, t = 0) {
  const look = almanacLook(theme), W = ALMANAC_DW, H = almanacDetailH(rows.length);
  detail(c, x, y, d => {
    px(d, 0, 0, W, H, P.o); px(d, 1, 1, W - 2, H - 2, look.rim); px(d, 1, 1, W - 2, 1, look.rimL);
    px(d, 2, 2, W - 4, ALMANAC_HEAD - 3, look.head);
    px(d, 2, H - ALMANAC_FOOT + 1, W - 4, ALMANAC_FOOT - 3, look.head);
    if (theme === MODERN) [6, 14, 22].forEach(tx => px(d, tx, 3, 6, 1, P.white));
    if (theme === FUTURE) px(d, 2, ALMANAC_HEAD - 2, W - 4, 1, look.rimL);
    rows.forEach((r, k) => {
      const ry = ALMANAC_HEAD + k * ALMANAC_ROW;
      px(d, 2, ry, W - 4, ALMANAC_ROW, r.today ? look.now : k % 2 ? look.alt : look.row);
      if (look.grid) for (let gx = ALMANAC_BAR_X + 8; gx < ALMANAC_PIP_X - 2; gx += 8) px(d, gx, ry, 1, ALMANAC_ROW, look.grid);
      if (r.revert) almanacCross(d, ALMANAC_GUTTER_X, ry + 1);
      if (r.bar) almanacBarArt(d, theme, look, ry, r.bar);
      for (let p = 0; p < r.pips; p++) px(d, ALMANAC_PIP_X + 3 * p, ry + 2, 2, 2, ALMANAC_BOUNCE);
      if (r.more) px(d, ALMANAC_PIP_X + 3 * (r.pips - 1), ry + 1, 2, 1, P.white);
      // Today's row grows: its tip glints, unless motion is reduced.
      if (r.today && !state.reduce && blink(t, 500)) px(d, ALMANAC_BAR_X + r.bar, ry + 1, 1, 4, P.white);
    });
  });
}

registerLayer({ z: 45, draw(c, { v, lay }, t) {
  const a = lay.almanac, rows = a ? almanacRows(v.github, v.pipeline && v.pipeline.merge_stats) : [];
  if (!rows.length) return;
  almanacArt(c, a.x, a.y, rows, state.T, t);
  tap(a.x, a.y, a.w, a.h, { kind: 'almanac' });
} });

registerHit({ kind: 'almanac', dialog(o, sc, theme) {
  const facts = almanacFacts(sc.v.github, sc.v.pipeline && sc.v.pipeline.merge_stats);
  if (!facts) return null;
  return { title: `${almanacLook(theme).name}: the last ${ALMANAC_DAYS} days`, line: facts.line, meta: facts.meta };
} });

/* Its name under it, with today's count; labels keep off it. */
registerLabels((sc, theme) => {
  const a = sc.L.almanac, ms = sc.v.pipeline && sc.v.pipeline.merge_stats, sub = a && almanacToday(sc.v.github, ms);
  if (!sub) return [];
  return [{ avoid: { x: a.x, y: a.y, w: a.w, h: a.h } },
    { x: a.x + (a.w >> 1), y: a.y + a.h + 1, anchor: 'top', text: almanacLook(theme).name, sub }];
}, 34);

/* In the Key: three days of it, a quiet one, a bounced one and a reverted one. */
const ALMANAC_SAMPLE = [
  { landed: 3, bar: 3 * ALMANAC_UNIT, pips: 0 },
  { landed: 6, bar: 6 * ALMANAC_UNIT, pips: 2 },
  { landed: 4, bar: 4 * ALMANAC_UNIT, pips: 1, revert: true, today: true },
];
registerKey({ group: 'other', order: 60, rows: theme => [
  [c => almanacArt(c, 1, 1, ALMANAC_SAMPLE, theme, 0),
    `${almanacLook(theme).name} at the east edge: a row per day for the last two weeks, today's last; landed work grows along it, red pips are bounces, a purple cross a revert (click it for the numbers)`, 46],
] });
