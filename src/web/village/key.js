/* ---------- Key ----------
 * What every symbol means, in the current theme: Places first (what each
 * part of the map is), then States, then the marks (#442; words in keyrows.js). Each swatch is drawn by the
 * renderer's own functions (T.crate, pennant, bubble, cobwebs, person) from
 * its own tables (KINDCOL, ICONS, STATE, SPECIALTIES, NOTECOL), and the rows
 * are listed from those tables, so the key cannot drift from the map. A
 * feature adds its own rows where it draws them (registerKey in
 * registry.js), so a new one needs no edit here. */
import { state } from './state.js';
import { DETAIL, prepare } from './detail.js';
import { KEYS } from './registry.js';
import { FEATURE_COLS, KINDCOL, NOTECOL, P, R, SPECIALTIES, STOPCOL } from './palette.js';
import { cobwebs, pennant } from './sprites/houses.js';
import { ICONS, bubble, tether, zzz } from './scenery.js';
import { glint, ribbon, roped } from './crates.js';
import { alarm, boardFrame, failed, lamp, ring } from './alarms.js';
import { FRESH_DONE_MS } from './rest.js';
import { person } from './people.js';
import { drawBanner } from './features.js';
import { envShed, envSign } from './envrow.js';
import { PRIO_ROWS, placeRows, stateRows } from './keyrows.js';

const KIND_WORDS = { bug: 'Bug', feature: 'Feature', tests: 'Tests', security: 'Security', research: 'Research', perf: 'Performance',
  refactor: 'Refactor', docs: 'Docs', chores: 'Chore', workshop: 'Finished work, sealed (quay and stages)', cottage: 'Other work' };
const ICON_WORDS = { blocked: 'Blocked: waiting on you', done: `Done: just finished (for ${FRESH_DONE_MS / 60000} minutes)`, unknown: 'Unknown: no word from it', stalled: 'Stalled: no progress' };
/* Bubbles the States section already shows. */
const IN_STATES = new Set(['blocked', 'done', 'stalled']);
const PENNANT_WORDS = { P0: 'P0: urgent (waves)', P1: 'P1: high', P2: 'P2: normal' };
function swatch(paint, size = 14) {
  const c = document.createElement('canvas');
  // The map's detail grid, so a 2× sprite keeps its detail in the key too.
  c.width = size * DETAIL; c.height = size * DETAIL; c.setAttribute('aria-hidden', 'true');
  // A big sprite (a place) shows larger, so it reads at key size.
  if (size >= 30) c.classList.add('big');
  const g = c.getContext('2d'); prepare(g); paint(g);
  return c;
}
/* A crate drawn twice its size, so the priority bands tell apart at key size. */
const bigCrate = (c, pr) => { c.save(); c.scale(2, 2); state.T.crate(c, 1, 1, 'feature', pr); c.restore(); };
/* A gatehouse with two tables, one lit. */
const KEY_GATE = { x0: 0, y: 0, w: 30, rx: 11, rw: 8, tables: [[2, 12], [19, 12]] };
/* A sprite framed to fit its swatch, whatever the theme draws: drawn once
 * off-screen at the origin to find its bounds, then centred in a square
 * swatch one pixel larger all round. Returns [paint, size]. */
function framed(fn) {
  const off = document.createElement('canvas'), N = 240;
  off.width = N; off.height = N;
  const g = off.getContext('2d');
  g.translate(N / 2, N / 2); fn(g);
  const d = g.getImageData(0, 0, N, N).data;
  let x0 = N, y0 = N, x1 = -1, y1 = -1;
  for (let y = 0; y < N; y++) for (let x = 0; x < N; x++) if (d[(y * N + x) * 4 + 3]) { if (x < x0) x0 = x; if (x > x1) x1 = x; if (y < y0) y0 = y; if (y > y1) y1 = y; }
  if (x1 < 0) return [fn, 14];
  const w = x1 - x0 + 1, h = y1 - y0 + 1, size = Math.max(w, h) + 2;
  const dx = 1 + ((size - 2 - w) >> 1) - (x0 - N / 2), dy = 1 + ((size - 2 - h) >> 1) - (y0 - N / 2);
  return [c => { c.save(); c.translate(dx, dy); fn(c); c.restore(); }, size];
}
/* Each place's sprite, drawn by the theme at the origin and framed to fit. */
const PLACE_ART = {
  house: c => state.T.B.feature(c, 0, 0, 'idle', 0),
  hall: c => state.T.hall(c, 0, 0, 'working'),
  docks: c => { state.T.dock(c, 0, 6, 20); ['bug', 'feature', 'tests'].forEach((k, j) => state.T.crate(c, 2 + j * 6, 0, k, null)); },
  board: c => state.T.board(c, 0, 0, [{ label: 'needs-design' }, { label: 'proposal' }]),
  gatehouse: c => state.T.gatehouse(c, KEY_GATE, 0, new Set([1])),
  quay: c => { state.T.dock(c, 0, 6, 20); [0, 1, 2].forEach(j => state.T.crate(c, 2 + j * 6, 0, 'workshop', null, true)); },
  lock: c => state.T.gate(c, 0, 0, false, STOPCOL[0]),
  harbour: c => state.T.destination(c, 0, 0, 0, false),
  env: c => { envSign(c, 0, 0); envShed(c, 18, 4, 26, 18); },
  ship: c => state.T.vessel(c, 0, 0, 0, 0, 0),
  flag: c => drawBanner(c, { x: 0, y: 0, col: FEATURE_COLS[0], f: { number: 417, stuck: [] } }, false, 0)
};
/* Each state's look. */
const STATE_ART = {
  busy: [c => { state.T.B.feature(c, 1, 5, 'working', 0); person(c, 13, 9, 'key:busy', 'work', { status: 'working' }); }, 24],
  done: [c => bubble(c, 2, 1, 'done'), 14],
  idle: [c => { person(c, 1, 6, 'key:asleep', 'sit', { status: 'asleep' }); zzz(c, 9, 9, 0); }, 18],
  failed: [c => failed(c, 2, 1), 14],
  waiting: [c => { ring(c, 11, 20, 0); alarm(c, 6, 0, 0); }, 23]
};
export function renderLegend() {
  const L = state.T.labels, body = document.getElementById('legend-body');
  document.getElementById('legend-title').textContent = `Key · ${state.T.name}`;
  const groups = [
    ['places', 'What each part of the map is', placeRows(L).map(r => { const [paint, size] = framed(PLACE_ART[r.id]); return [paint, r.words, size]; })],
    ['states', 'What a worker is doing, trouble loudest', stateRows(FRESH_DONE_MS / 60000).map(r => [STATE_ART[r.id][0], r.words, STATE_ART[r.id][1]])],
    ['kinds', `Crates at the ${L.docks}: the kind of work`, Object.keys(KINDCOL).map(k => [c => state.T.crate(c, 1, 1, k, null, k === 'workshop'), KIND_WORDS[k] || k, 7])],
    ['priority', 'Priority', [...PRIO_ROWS.map(({ p, words }) => [c => bigCrate(c, p), words, 12]),
      [c => bigCrate(c, null), 'No band: not prioritised', 12],
      ...Object.keys(PENNANT_WORDS).map(p => [c => pennant(c, 5, 7, p, 0), `Pennant over a house: ${PENNANT_WORDS[p]}`]),
      [c => state.T.B.feature(c, 3, 5, 'idle', 0), 'No pennant over a house: P3', 22]]],
    ['docks', `Ready or waiting, at the ${L.docks}`, [
      [c => { state.T.crate(c, 2, 2, 'feature', null); glint(c, 2, 2, 0); }, 'Glinting crate: ready to pick up', 8],
      [c => { state.T.crate(c, 1, 1, 'feature', null); ribbon(c, 1, 1, FEATURE_COLS[0]); }, "Crate edged in a flag's colour: part of that feature (its epic)", 7],
      [c => { tether(c, { x: 1, y: 8 }, { x: 8, y: 1 }); state.T.crate(c, 8, 1, 'feature', null); state.T.crate(c, 1, 8, 'feature', null); roped(c, 1, 8); }, 'Rope between crates: the roped one waits on the other', 14],
      [c => ['P0', 'P1', 'P2'].forEach((p, j) => state.T.crate(c, 1 + j * 6, 7, 'feature', p)), 'Order: highest priority first; a crate stands after what it waits on, so a chain reads as one run', 19],
      [c => state.T.crate(c, 1, 1, 'feature', null, false, 420), `Number on a crate: its issue. Blocked issues are notes on the ${L.board}`, 7]]],
    ['trouble', 'Trouble elsewhere', [
      [c => { state.T.board(c, 3, 7, [{ label: 'needs-design' }, { label: 'failed' }]); boardFrame(c, 3, 7, 2, 0); }, `Red frame and count on the ${L.board}: notes waiting on you`, 24],
      [c => lamp(c, 6, 4, 0), `Red lamp on the ${L.gate}: a merge failed or conflicted`]]],
    ['bubbles', 'Bubbles over people and stops', Object.keys(ICONS).filter(k => !IN_STATES.has(k)).map(k => [c => bubble(c, 2, 1, k), ICON_WORDS[k] || k])],
    ['other', 'Other marks', [
      [c => { state.T.crate(c, 1, 1, 'feature', null); roped(c, 1, 1); }, 'Roped crate: waits on another issue ("Depends on")', 7], [c => cobwebs(c, -1, -6), 'Cobwebs on a house: stalled'],
      [c => { R(c, 4, 2, 1, 10, P.woodD); R(c, 5, 2, 6, 4, P.p0); }, 'Red flag on a warehouse or a ship: the newest deployment failed, and the ship turned back'],
      [c => { state.T.B.cottage(c, 3, 5, 'idle', 0); R(c, 5, 0, 1, 7, P.woodD); R(c, 6, 0, 4, 3, FEATURE_COLS[0]); }, "House in an environment village: a feature it runs, flying that feature's colour (+N: more features)", 22]]],
    ['subagents', 'Sub-agent accents: their specialty', [...SPECIALTIES.map(([name, kind]) => [c => person(c, 3, 1, 'key:' + name, 'stand', { accent: KINDCOL[kind], sub: true }), name]),
      [c => person(c, 3, 1, 'key:other', 'stand', { accent: P.white, sub: true }), 'any other']]],
    ['notes', `Notes on the ${L.board}`, Object.keys(NOTECOL).map(k => [c => { R(c, 1, 1, 5, 5, P.o); R(c, 2, 2, 3, 3, NOTECOL[k]); }, k.replace(/-/g, ' '), 7])]
  ];
  // Each feature's own rows (registerKey): into its group, or a new one after them.
  KEYS.forEach(k => {
    const rows = k.rows(state.T), g = groups.find(([id]) => id === k.group);
    if (g) g[2].push(...rows);
    else groups.push([k.group, k.group, rows]);
  });
  body.textContent = '';
  // A heading over each section: Places, States, then the marks (every other group).
  const SECTION_OF = { places: 'Places', states: 'States' };
  let section = null;
  groups.filter(([, , rows]) => rows.length).forEach(([id, title, rows]) => {
    const now = SECTION_OF[id] || 'Marks';
    if (now !== section) {
      section = now;
      const h3 = document.createElement('h3');
      h3.className = 'ksec'; h3.textContent = section;
      body.append(h3);
    }
    const h = document.createElement('h4'), ul = document.createElement('ul');
    h.textContent = title;
    rows.forEach(([paint, words, size]) => {
      const li = document.createElement('li'), span = document.createElement('span');
      span.textContent = words;
      li.append(swatch(paint, size), span); ul.appendChild(li);
    });
    body.append(h, ul);
  });
}
const legendBtn = document.getElementById('legend-btn');
export const legend = document.getElementById('legend');
legendBtn.addEventListener('click', () => {
  legend.hidden = !legend.hidden;
  legendBtn.setAttribute('aria-expanded', String(!legend.hidden));
  if (!legend.hidden) renderLegend();
});

// The sheet's own close button: on a phone the key covers the bar and its "?".
document.getElementById('legend-close').addEventListener('click', () => {
  legend.hidden = true;
  legendBtn.setAttribute('aria-expanded', 'false');
  legendBtn.focus();
});
