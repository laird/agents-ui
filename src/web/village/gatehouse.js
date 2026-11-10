/* ---------- The merge gate ----------
 * The report says where each task is in the merge gate (`pipeline.gates`,
 * and the host's slots and trains in `pipeline.merge`), and a gatehouse on
 * the road from the houses down to the quay draws it (layout().gate):
 *   queued           in line in front of it, in order, numbered;
 *   merging          under the arch, roped to the master crate (the rope
 *                    tangles when a re-sync has it merging again);
 *   gating           on an inspection table, one per gate slot, checked
 *                    with a glass while a meter fills as tests run;
 *   passed           a green stamp, then out to the quay;
 *   landed-unproven  at the quay with a yellow tag;
 *   failed           a red REJECTED stamp, carried back up to the board,
 *                    dented, its house marked with the attempt;
 *   conflict         a knot, carried back the same way;
 *   red-base         a storm cloud over the quay: the integration branch
 *                    is red, which is not this crate's fault.
 * A merge train rides one cart, its crates chained, at its first member's
 * place. Busy tables are lit and idle ones dark. The walks between these
 * places are trips (trips.js); the stamps they leave are drawn here. */
import { state } from './state.js';
import { NOTECOL, P, R, blink } from './palette.js';
import { badge, numberTag } from './scenery.js';
import { drawAction, person } from './people.js';
import { registerHit, registerLabels, registerLayer, tap } from './registry.js';
import { quaySpot } from './ship.js';
import { issueLabel, issueOf, link } from './dialogs.js';

export const IN_GATE = new Set(['queued', 'merging', 'gating']);
export const gateEntries = v => v.pipeline.gates || [];
export const gateOf = (v, n) => gateEntries(v).find(e => e.issue === n);
/* Where each entry still in the gate is drawn. A unit is one crate, or one
 * train's cart: { lead, members, state, x, y, table }. spots maps each
 * issue to { x, y, state, unit, lead }. Past the line's room the rest wait
 * off the map (`waiting`, a "+N"). */
export function gateSpots(v, lay) {
  const g = lay.gate, spots = new Map(), units = [];
  if (!g) return { spots, units, queued: 0, gating: 0, waiting: 0 };
  const trains = new Map();
  gateEntries(v).filter(e => IN_GATE.has(e.state)).forEach(e => {
    if (e.train && trains.has(e.train)) { trains.get(e.train).members.push(e); return; }
    const u = { lead: e, members: [e], state: e.state };
    units.push(u);
    if (e.train) trains.set(e.train, u);
  });
  let q = 0, m = 0, waiting = 0;
  const used = new Set();
  units.forEach(u => {
    const e = u.lead;
    if (e.state === 'gating') {
      let k = e.slot ? e.slot - 1 : -1;
      if (k < 0 || k >= g.tables.length || used.has(k)) k = g.tables.findIndex((_, i) => !used.has(i));
      if (k >= 0) { used.add(k); u.table = k; u.x = g.tables[k][0] + 2; u.y = g.tables[k][1]; }
      else { u.x = g.rx + 1; u.y = g.y + 14; }
    } else if (e.state === 'merging') { u.x = g.rx + 1; u.y = g.y + 6 + (m++ % 3) * 6; }
    else {
      const k = q++;
      if (k >= g.queueMax) { waiting++; return; }
      u.x = g.rx - 16 - k * 16; u.y = g.y + 20;
    }
    u.members.forEach((mm, i) => spots.set(mm.issue, { x: u.x + Math.min(i, 2) * 6, y: u.y, state: u.state, unit: u, lead: i === 0 }));
  });
  return { spots, units: units.filter(u => u.x !== undefined), queued: q, gating: units.filter(u => u.state === 'gating').length, waiting };
}
/* "3m 20s", "1h 4m". */
function duration(secs) {
  if (secs == null) return '';
  const m = Math.floor(secs / 60), h = Math.floor(m / 60);
  return h ? `${h}h ${m % 60}m` : m ? `${m}m ${secs % 60}s` : `${secs}s`;
}
const GATE_WORDS = { queued: 'queued for the merge gate', passed: 'passed the gate',
  'landed-unproven': 'landed, waiting for the sentinel to prove the tip green', failed: 'failed the gate: handed back',
  conflict: 'conflict: rebase needed', 'red-base': 'the integration branch is red without it' };
/* Where an entry is, in words: "gating · table 1 · 412 tests run · 3m 20s · attempt 2". */
export function gateEntryWords(e, v) {
  const head = e.state === 'merging' ? `merging onto ${v.pipeline.integration || 'the integration branch'}`
    : e.state === 'gating' ? 'gating' : GATE_WORDS[e.state] || e.state;
  return [head, e.state === 'gating' && e.slot ? `table ${e.slot}` : null, e.progress, e.elapsed_secs != null ? duration(e.elapsed_secs) : null,
    e.attempt > 1 ? `attempt ${e.attempt}` : null, e.train ? `train ${e.train}` : null].filter(Boolean).join(' · ');
}

/* ---------- Drawing ---------- */
/* A stamp the inspector brings down on a crate: green for passed, red for rejected, a knot for a conflict. */
function stampMark(c, x, y, kind) {
  if (kind === 'knot') {
    R(c, x - 1, y - 1, 9, 7, P.o); R(c, x, y, 7, 5, NOTECOL.conflict);
    [[1, 1], [3, 2], [5, 1], [2, 3], [4, 3], [3, 0], [3, 4]].forEach(([dx, dy]) => R(c, x + dx, y + dy, 1, 1, P.woodD));
    return;
  }
  const col = kind === 'ok' ? P.leaf : P.p0;
  R(c, x - 1, y - 1, 9, 7, P.o); R(c, x, y, 7, 5, P.white); R(c, x, y, 7, 1, col); R(c, x, y + 4, 7, 1, col); R(c, x, y, 1, 5, col); R(c, x + 6, y, 1, 5, col);
  if (kind === 'ok') { R(c, x + 2, y + 2, 1, 1, col); R(c, x + 3, y + 3, 1, 1, col); R(c, x + 4, y + 1, 1, 2, col); }
  else for (let k = 1; k < 4; k++) { R(c, x + 1 + k, y + k, 1, 1, col); R(c, x + 5 - k, y + k, 1, 1, col); }
}
/* A 1px rope from (x0, y0) to (x1, y1); tangled, it knots up halfway. */
function rope(c, x0, y0, x1, y1, tangled) {
  const n = Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0));
  for (let k = 0; k <= n; k += 1) R(c, x0 + (x1 - x0) * k / n, y0 + (y1 - y0) * k / n + (k % 4 === 2 ? 1 : 0), 1, 1, P.woodD);
  if (tangled) { const mx = (x0 + x1) >> 1, my = (y0 + y1) >> 1; [[-1, -1], [1, -1], [0, 0], [-1, 1], [1, 1], [2, 0], [-2, 0]].forEach(([dx, dy]) => R(c, mx + dx, my + dy, 1, 1, P.p0)); }
}
const inspectFrame = t => (!state.reduce && blink(t, 180) ? 'work2' : 'work');
/* Before anything is drawn: where each entry in the gate is. Its crate rests
 * there, so no house, board or quay draws it too (scene.bound); the gate
 * itself skips only what a trip is carrying (gate.onTrip). */
registerLayer({ z: -5, draw(c, scene) {
  const gs = scene.gate = gateSpots(scene.v, scene.lay);
  gs.onTrip = scene.bound;
  if (gs.spots.size) scene.bound = new Set([...scene.bound, ...gs.spots.keys()]);
} });
/* A house whose work the gate handed back: one strike per attempt over its door. */
registerLayer({ z: 72, draw(c, { v, lay }) {
  v.houses.forEach((h, i) => {
    const lot = lay.lots[i], e = lot && h.issue && gateOf(v, h.issue);
    if (!e || (e.state !== 'failed' && e.state !== 'conflict')) return;
    const n = Math.min(4, Math.max(1, e.attempt || 1));
    for (let k = 0; k < n; k++) R(c, lot.x + 1 + k * 2, lot.y + 1, 1, 3, P.p0);
    tap(lot.x, lot.y, n * 2 + 2, 5, { kind: 'gatecrate', n: h.issue, at: 'house' });
  });
} });
/* The gatehouse, the master crate the merging work is roped to, the line,
 * the tables with their inspectors and meters, the trains' carts, and the
 * stamps falling on crates as they leave. */
registerLayer({ z: 75, draw(c, { v, lay, gate: gs }, t) {
  const g = lay.gate; if (!g) return;
  const T = state.T, S = state.S, merge = v.pipeline.merge || {};
  const busy = new Set([...(merge.busy || []), ...gs.units.filter(u => u.table !== undefined).map(u => u.table + 1)]);
  T.gatehouse(c, g, state.reduce ? 0 : t, busy);
  tap(g.x0 - 1, g.y, g.w + 2, 24, { kind: 'gatehouse' }); tap(g.rx - 4, g.y, g.rw + 8, 22, { kind: 'gatehouse' });
  // The master crate: the integration branch.
  const mx = g.rx - 12, my = g.y + 3;
  R(c, mx - 1, my - 1, 9, 9, P.o); R(c, mx, my, 7, 7, P.gold); R(c, mx, my, 7, 1, P.woodD); R(c, mx + 3, my, 1, 7, P.woodD);
  tap(mx - 1, my - 1, 9, 9, { kind: 'gatehouse' });
  gs.units.forEach(u => {
    const e = u.lead, n = e.issue;
    if (u.members.every(mm => gs.onTrip.has(mm.issue))) return;
    const { x, y } = u, train = u.members.length > 1;
    if (u.state === 'gating' && u.table !== undefined) {
      person(c, x - 5, y - 8, 'inspector:' + v.project + u.table, inspectFrame(t + u.table * 211), {});
      drawAction(c, x - 5, y - 8, 'searching', t + u.table * 97);
    }
    if (u.state === 'merging') rope(c, x + 2, y + 1, mx + 6, my + 6, e.attempt > 1);
    if (train) {
      // A cart under the train's crates, chained together.
      const shown = u.members.slice(0, 3), cw = shown.length * 6;
      R(c, x - 1, y + 5, cw + 1, 2, P.o); R(c, x, y + 5, cw - 1, 1, P.woodL);
      R(c, x, y + 7, 2, 2, P.o); R(c, x + cw - 3, y + 7, 2, 2, P.o);
      shown.forEach((mm, i) => { T.crate(c, x + i * 6, y, 'workshop', null, false); if (i) R(c, x + i * 6 - 1, y + 2, 1, 1, P.stoneD); });
      if (u.members.length > 3) badge(c, x + cw + 1, y - 1, u.members.length - 3);
      tap(x - 1, y - 1, cw + 2, 10, { kind: 'train', id: e.train });
    } else {
      T.crate(c, x, y, 'workshop', null, false);
      tap(x, y, 5, 5, { kind: 'gatecrate', n, at: 'gate' });
    }
    if (u.state === 'queued') numberTag(c, x + 2 - String(n).length * 2, y - 8, n);
    if (u.state === 'gating' && u.table !== undefined) {
      // The meter under the table: tests run against the last full run's count.
      const total = S.testTotal || 0, ran = e.tests_run || 0, mw = 9;
      R(c, x - 3, y + 10, mw + 2, 3, P.o);
      if (total > 0) R(c, x - 2, y + 11, Math.max(1, Math.round(mw * Math.min(1, ran / total))), 1, P.leaf);
      else if (!state.reduce) R(c, x - 2 + Math.floor(t / 150) % (mw - 2), y + 11, 3, 1, P.leaf);
    }
  });
  if (gs.waiting) badge(c, Math.max(4, g.rx - 16 - g.queueMax * 16 + 8), g.y + 14, gs.waiting);
  S.stamps = (S.stamps || []).filter(st => {
    if (t - st.t0 > 1600) return false;
    if (t >= st.t0) stampMark(c, st.x, st.y - (t - st.t0 < 150 && !state.reduce ? 3 : 0), st.kind);
    return true;
  });
} });
/* At the quay: a yellow tag on work landed but not yet proven green, green
 * once the gate says it passed. A storm over the quay while the integration
 * branch is red without the work at the gate. */
registerLayer({ z: 85, draw(c, { v, lay, atQuay }, t) {
  (atQuay || []).forEach((n, k) => {
    const e = gateOf(v, n);
    if (!e || (e.state !== 'landed-unproven' && e.state !== 'passed')) return;
    const [x, y] = quaySpot(lay, k);
    R(c, x + 4, y - 2, 2, 3, P.o); R(c, x + 4, y - 2, 1, 2, e.state === 'passed' ? P.leaf : P.gold);
    tap(x, y - 2, 6, 7, { kind: 'gatecrate', n, at: 'quay' });
  });
  if (!gateEntries(v).some(e => e.state === 'red-base')) return;
  // Over the quay's landing, just below the gatehouse.
  const sx = lay.roadX - 4, sy = lay.quayY - 8;
  for (let k = 0; k < 4; k++) { R(c, sx + k * 5, sy + (k % 2), 7, 5, P.o); R(c, sx + 1 + k * 5, sy + 1 + (k % 2), 5, 3, P.stoneD); }
  if (!state.reduce) for (let k = 0; k < 5; k++) if (blink(t + k * 80, 160)) R(c, sx + 2 + k * 4, sy + 7 + (Math.floor(t / 90) + k) % 6, 1, 2, P.glass);
  if (!state.reduce && blink(t, 1300) && Math.floor(t / 1300) % 3 === 0) { R(c, sx + 9, sy + 6, 1, 3, P.gold); R(c, sx + 8, sy + 9, 1, 2, P.gold); }
  tap(sx, sy, 22, 12, { kind: 'redbase' });
} });

/* ---------- Dialogs ---------- */
const ORDER = { gating: 0, merging: 1, queued: 2, 'red-base': 3, failed: 4, conflict: 5, 'landed-unproven': 6, passed: 7 };
const entryItem = (v, e) => ({ ...issueOf(v, e.issue), number: e.issue, label: gateEntryWords(e, v), url: e.failure_url || issueOf(v, e.issue).url });
/* The queue, what is on each table, and recent outcomes. */
registerHit({ kind: 'gatehouse', dialog(o, sc, theme) {
  const v = sc.v, gs = gateSpots(v, sc.L), merge = v.pipeline.merge;
  return { title: `${theme.labels.gate} · merge gate`, line: `${gs.queued} queued · ${gs.gating} gating`,
    meta: [merge ? `Gate slots: ${(merge.busy || []).length} of ${merge.slots} busy` : null,
      ...((merge && merge.trains) || []).map(tr => `Train ${tr.id}: ${tr.members.map(m => '#' + m).join(' ')} · ${tr.state}`)],
    items: [...gateEntries(v)].sort((a, b) => (ORDER[a.state] ?? 9) - (ORDER[b.state] ?? 9)).map(e => entryItem(v, e)), links: [] };
} });
registerHit({ kind: 'train', dialog(o, sc) {
  const v = sc.v, tr = ((v.pipeline.merge || {}).trains || []).find(x => x.id === o.id), members = gateEntries(v).filter(e => e.train === o.id);
  return { title: `Merge train ${o.id || ''}`.trim(), line: `${members.length} branches gated together as one tree${tr ? ' · ' + tr.state : ''}`,
    items: members.map(e => entryItem(v, e)), links: [] };
} });
registerHit({ kind: 'redbase', dialog(o, sc) {
  const v = sc.v, red = gateEntries(v).filter(e => e.state === 'red-base');
  return { title: 'Storm over the quay', line: `${v.pipeline.integration || 'The integration branch'} fails without these changes: not their fault`,
    items: red.map(e => entryItem(v, e)), links: [] };
} });
/* A crate in line, on a table, tagged at the quay or handed back to its house. */
registerHit({ kind: 'gatecrate', dialog(o, sc, theme) {
  const v = sc.v, it = issueOf(v, o.n), e = gateOf(v, o.n);
  return { title: `#${o.n} ${it.title}`.trim(), line: e ? gateEntryWords(e, v) : `at the ${theme.labels.gate.toLowerCase()}`,
    meta: [it.priority ? `Priority: ${it.priority}` : null, e && e.attempt ? `Gate attempt ${e.attempt}` : null],
    links: [link(issueLabel(o.n), it.url), e && e.failure_url && e.failure_url !== it.url ? link('What went wrong', e.failure_url) : null] };
} });

/* ---------- Label ---------- */
/* Merge jobs that failed or hit a conflict today. */
function failedToday(v) {
  const today = new Date().toISOString().slice(0, 10);
  return gateEntries(v).filter(e => (e.state === 'failed' || e.state === 'conflict') && (!e.started_at || e.started_at.slice(0, 10) === today)).length;
}
registerLabels((sc, theme) => {
  const g = sc.L.gate; if (!g) return [];
  const gs = gateSpots(sc.v, sc.L), failed = failedToday(sc.v);
  // West of the master crate, over the line: the ground under the gatehouse is the quay's, and east of it stand the locks.
  return [{ x: g.rx - 15, y: g.y + 6, anchor: 'right', text: `${theme.labels.gate} · merge gate`, sub: `${gs.queued} queued · ${gs.gating} gating${failed ? ` · ${failed} failed today` : ''}` }];
}, 15);
