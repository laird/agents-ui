/* ---------- Key ----------
 * What every symbol means, in the current theme. Each swatch is drawn by the
 * renderer's own functions (T.crate, pennant, bubble, cobwebs, person) from
 * its own tables (KINDCOL, ICONS, STATE, SPECIALTIES, NOTECOL), and the rows
 * are listed from those tables, so the key cannot drift from the map. A
 * feature adds its own rows where it draws them (registerKey in
 * registry.js), so a new one needs no edit here. */
import { state } from './state.js';
import { DETAIL, prepare } from './detail.js';
import { KEYS } from './registry.js';
import { FEATURE_COLS, KINDCOL, NOTECOL, P, PRIOCOL, R, SPECIALTIES } from './palette.js';
import { cobwebs, pennant } from './sprites/houses.js';
import { ICONS, bubble, glint, ribbon, roped, tether, zzz } from './scenery.js';
import { alarm, boardFrame, failed, lamp, ring } from './alarms.js';
import { FRESH_DONE_MS } from './rest.js';
import { person } from './people.js';

const KIND_WORDS = { bug: 'Bug', feature: 'Feature', tests: 'Tests', security: 'Security', research: 'Research', perf: 'Performance',
  refactor: 'Refactor', docs: 'Docs', chores: 'Chore', workshop: 'Finished work, sealed (quay and stages)', cottage: 'Other work' };
const ICON_WORDS = { blocked: 'Blocked: waiting on you', done: `Done: just finished (for ${FRESH_DONE_MS / 60000} minutes)`, unknown: 'Unknown: no word from it', stalled: 'Stalled: no progress' };
const PRIO_WORDS = { P0: 'P0: urgent', P1: 'P1: high', P2: 'P2: normal', P3: 'P3: low' };
const PENNANT_WORDS = { P0: 'P0: urgent (waves)', P1: 'P1: high', P2: 'P2: normal' };
function swatch(paint, size = 14) {
  const c = document.createElement('canvas');
  // The map's detail grid, so a 2× sprite keeps its detail in the key too.
  c.width = size * DETAIL; c.height = size * DETAIL; c.setAttribute('aria-hidden', 'true');
  const g = c.getContext('2d'); prepare(g); paint(g);
  return c;
}
export function renderLegend() {
  const L = state.T.labels, body = document.getElementById('legend-body');
  document.getElementById('legend-title').textContent = `Key · ${state.T.name}`;
  const groups = [
    ['kinds', `Crates at the ${L.docks}: the kind of work`, Object.keys(KINDCOL).map(k => [c => state.T.crate(c, 1, 1, k, null, k === 'workshop'), KIND_WORDS[k] || k, 7])],
    ['priority', 'Priority', [...Object.keys(PRIOCOL).map(p => [c => state.T.crate(c, 1, 1, 'feature', p), `Mark on a crate: ${PRIO_WORDS[p]}`, 7]),
      [c => state.T.crate(c, 1, 1, 'feature', null), 'No mark: not prioritised', 7],
      ...Object.keys(PENNANT_WORDS).map(p => [c => pennant(c, 5, 7, p, 0), `Pennant over a house: ${PENNANT_WORDS[p]}`]),
      [() => {}, 'No pennant: P3']]],
    ['docks', `Ready or waiting, at the ${L.docks}`, [
      [c => { state.T.crate(c, 2, 2, 'feature', null); glint(c, 2, 2, 0); }, 'Glinting crate: ready to pick up', 8],
      [c => { state.T.crate(c, 1, 1, 'feature', null); ribbon(c, 1, 1, FEATURE_COLS[0]); }, "Crate edged in a flag's colour: part of that feature (its epic)", 7],
      [c => { tether(c, { x: 1, y: 8 }, { x: 8, y: 1 }); state.T.crate(c, 8, 1, 'feature', null); state.T.crate(c, 1, 8, 'feature', null); roped(c, 1, 8); }, 'Rope between crates: the roped one waits on the other', 14],
      [() => {}, 'Order: highest priority first; a crate stands after what it waits on, so a chain reads as one run'],
      [() => {}, `#N under a crate: its issue. Blocked issues are notes on the ${L.board}`]]],
    ['trouble', 'Trouble, loudest first', [
      [c => { ring(c, 11, 20, 0); alarm(c, 6, 0, 0); }, 'Big red "!" and a ring at the door: waiting on you (a question or a permission prompt)', 23],
      [c => failed(c, 2, 1), 'Red ✗: failed or stalled'],
      [c => { state.T.board(c, 3, 7, [{ label: 'needs-design' }, { label: 'failed' }]); boardFrame(c, 3, 7, 2, 0); }, `Red frame and count on the ${L.board}: notes waiting on you`, 24],
      [c => lamp(c, 6, 4, 0), 'Red lamp on the gatehouse: a merge failed or conflicted']]],
    ['bubbles', 'Bubbles over people and stops', Object.keys(ICONS).filter(k => k !== 'blocked' && k !== 'stalled').map(k => [c => bubble(c, 2, 1, k), ICON_WORDS[k] || k])],
    ['other', 'Other marks', [[c => { person(c, 1, 6, 'key:asleep', 'sit', { status: 'asleep' }); zzz(c, 9, 9, 0); }, 'Sitting on the doorstep under "zzz": idle, nothing claimed, waiting for work', 18],
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
  groups.filter(([, , rows]) => rows.length).forEach(([, title, rows]) => {
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
