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
import { NOTECOL, P, blink } from './palette.js';
import { badge, numberTag } from './scenery.js';
import { drawAction, person } from './people.js';
import { registerHit, registerLabels, registerLayer, tap } from './registry.js';
import { lamp } from './alarms.js';
import { detail } from './detail.js';
import { disc, px } from './art2x.js';
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
/* Ink a small bitmap ('#' for ink) in detail pixels. */
const stampInk = (g, x, y, rows, col) => rows.forEach((row, j) => { for (let i = 0; i < row.length; i++) if (row[i] === '#') px(g, x + i, y + j, 1, 1, col); });
const STAMP_CHECK = ['........##', '.......##.', '##....##..', '.##..##...', '..####....', '...##.....'];
const STAMP_CROSS = ['##....##', '.##..##.', '..####..', '..####..', '.##..##.', '##....##'];
const STAMP_KNOT = ['....####......', '...##..##.....', '..##....##....', '..##...######.', '...##.##...##.', '....###....##.', '...##.##..##..', '..##...####...', '.##...........', '##............'];
/* A stamp the inspector brings down on a crate: green for passed, red for
 * rejected, a knot for a conflict. A card 9 by 7 from (x - 1, y - 1), drawn
 * at 2× (#459): a rounded card, a stamped border and its mark. */
function stampMark(c, x, y, kind) {
  detail(c, x - 1, y - 1, g => {
    px(g, 1, 0, 16, 1, P.o); px(g, 1, 13, 16, 1, P.o); px(g, 0, 1, 1, 12, P.o); px(g, 17, 1, 1, 12, P.o);
    px(g, 1, 1, 16, 12, kind === 'knot' ? NOTECOL.conflict : P.white);
    if (kind === 'knot') { stampInk(g, 2, 2, STAMP_KNOT, P.woodD); return; }
    const col = kind === 'ok' ? P.leaf : P.p0;
    px(g, 2, 2, 14, 1, col); px(g, 2, 11, 14, 1, col); px(g, 2, 2, 1, 10, col); px(g, 15, 2, 1, 10, col);
    if (kind === 'ok') stampInk(g, 4, 4, STAMP_CHECK, col); else stampInk(g, 5, 4, STAMP_CROSS, col);
  });
}
/* A rope from (x0, y0) to (x1, y1), sagging between them, its twist picked
 * out every few pixels; tangled, it knots up halfway. Drawn at 2× (#459). */
const ROPE_TANGLE = ['.##..#..', '#..##.#.', '.#.##..#', '#..#.##.', '.##..#..'];
function rope(c, x0, y0, x1, y1, tangled) {
  detail(c, 0, 0, g => {
    const ax = 2 * x0, ay = 2 * y0, bx = 2 * x1, by = 2 * y1, n = Math.max(1, Math.abs(bx - ax), Math.abs(by - ay));
    for (let k = 0; k <= n; k++) {
      const x = Math.round(ax + (bx - ax) * k / n), y = Math.round(ay + (by - ay) * k / n + 3 * Math.sin(Math.PI * k / n));
      px(g, x, y, 1, 2, P.woodD);
      if (k % 3 === 1) px(g, x, y, 1, 1, P.woodL);
    }
    if (tangled) stampInk(g, ((ax + bx) >> 1) - 4, ((ay + by) >> 1) - 1, ROPE_TANGLE, P.p0);
  });
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
    // Tally strokes, each with a darker foot (2×, #459).
    detail(c, lot.x, lot.y, g => { for (let k = 0; k < n; k++) { px(g, 2 + k * 4, 2, 2, 6, P.p0); px(g, 2 + k * 4, 7, 2, 1, '#901818'); } });
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
  // A red lamp on the gatehouse while any merge failed or conflicted (#439).
  if (gateEntries(v).some(e => e.state === 'failed' || e.state === 'conflict')) lamp(c, g.x0 + g.w - 2, g.y - 4, t);
  // The master crate: the integration branch.
  const mx = g.rx - 12, my = g.y + 3;
  detail(c, mx - 1, my - 1, d => {
    px(d, 1, 0, 16, 1, P.o); px(d, 1, 17, 16, 1, P.o); px(d, 0, 1, 1, 16, P.o); px(d, 17, 1, 1, 16, P.o);
    px(d, 1, 1, 16, 16, P.gold); px(d, 1, 3, 16, 1, '#f8e070'); px(d, 1, 16, 16, 1, '#b08820');
    px(d, 1, 1, 16, 2, P.woodD); px(d, 8, 1, 2, 16, P.woodD);
    px(d, 3, 6, 1, 1, P.o); px(d, 14, 6, 1, 1, P.o); px(d, 3, 13, 1, 1, P.o); px(d, 14, 13, 1, 1, P.o);
  });
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
      detail(c, x, y, d => {
        px(d, -2, 10, 2 * cw + 2, 4, P.o); px(d, 0, 10, 2 * cw - 2, 2, P.woodL); px(d, 0, 12, 2 * cw - 2, 1, P.wood);
        for (const wx of [0, 2 * cw - 6]) { px(d, wx + 1, 14, 2, 1, P.o); px(d, wx, 15, 4, 2, P.o); px(d, wx + 1, 17, 2, 1, P.o); px(d, wx + 1, 15, 2, 2, P.stoneL); }
      });
      shown.forEach((mm, i) => {
        T.crate(c, x + i * 6, y, 'workshop', null, false, mm.issue);
        // A chain link to the crate in front.
        if (i) detail(c, x + i * 6 - 1, y + 2, d => { px(d, 0, 0, 2, 1, P.stoneD); px(d, 0, 1, 2, 1, P.stoneL); px(d, 0, 2, 2, 1, P.stoneD); });
      });
      if (u.members.length > 3) badge(c, x + cw + 1, y - 1, u.members.length - 3);
      tap(x - 1, y - 1, cw + 2, 10, { kind: 'train', id: e.train });
    } else {
      T.crate(c, x, y, 'workshop', null, false, n);
      tap(x, y, 5, 5, { kind: 'gatecrate', n, at: 'gate' });
    }
    if (u.state === 'queued') numberTag(c, x + 2 - String(n).length * 2, y - 8, n);
    if (u.state === 'gating' && u.table !== undefined) {
      // The meter under the table: tests run against the last full run's count.
      const total = S.testTotal || 0, ran = e.tests_run || 0, mw = 9;
      // At 2× (#459): a slate track, filled a detail pixel at a time.
      detail(c, x - 3, y + 10, d => {
        px(d, 1, 0, 2 * mw + 2, 1, P.o); px(d, 1, 5, 2 * mw + 2, 1, P.o); px(d, 0, 1, 1, 4, P.o); px(d, 2 * mw + 3, 1, 1, 4, P.o);
        px(d, 1, 1, 2 * mw + 2, 4, '#283040');
        if (total > 0) { const w = Math.max(1, Math.round(2 * mw * Math.min(1, ran / total))); px(d, 2, 2, w, 2, P.leaf); px(d, 2, 2, w, 1, '#78d878'); }
        else if (!state.reduce) px(d, 2 + Math.floor(t / 75) % (2 * mw - 4), 2, 6, 2, P.leaf);
      });
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
    // A pennant on a pole (2×, #459).
    const ok = e.state === 'passed';
    detail(c, x + 4, y - 2, d => { px(d, 0, 0, 1, 6, P.o); px(d, 1, 0, 3, 4, P.o); px(d, 1, 0, 2, 3, ok ? P.leaf : P.gold); px(d, 1, 0, 2, 1, ok ? '#78d878' : '#f8e070'); });
    tap(x, y - 2, 6, 7, { kind: 'gatecrate', n, at: 'quay' });
  });
  if (!gateEntries(v).some(e => e.state === 'red-base')) return;
  // Over the quay's landing, just below the gatehouse.
  const sx = lay.roadX - 4, sy = lay.quayY - 8;
  // At 2× (#459): billowing lumps over a flat base, rain and a forked bolt.
  detail(c, sx, sy, d => {
    for (let k = 0; k < 4; k++) disc(d, k * 10 + 1, 2 * (k % 2), 12, { outline: P.o, fill: P.stoneD, light: P.stone, dark: '#383848' });
    px(d, 2, 7, 40, 3, P.stoneD); px(d, 2, 10, 40, 1, P.o); px(d, 1, 7, 1, 3, P.o); px(d, 42, 7, 1, 3, P.o);
    if (!state.reduce) for (let k = 0; k < 5; k++) if (blink(t + k * 80, 160)) px(d, 4 + k * 8, 13 + (Math.floor(t / 45) + k) % 9, 1, 3, P.glass);
    if (!state.reduce && blink(t, 1300) && Math.floor(t / 1300) % 3 === 0) stampInk(d, 15, 11, ['..##', '.##.', '####', '..#.', '.#..', '#...'], P.gold);
  });
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
