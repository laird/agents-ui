/* ---------- Key ----------
 * What every symbol means, in the current theme. Each swatch is drawn by the
 * renderer's own functions (T.crate, pennant, bubble, cobwebs, person) from
 * its own tables (KINDCOL, ICONS, STATE, SPECIALTIES, NOTECOL), and the rows
 * are listed from those tables, so the key cannot drift from the map. */
import { state } from './state.js';
import { KINDCOL, NOTECOL, P, R, SPECIALTIES } from './palette.js';
import { cobwebs, pennant } from './sprites/houses.js';
import { ICONS, bubble, roped } from './scenery.js';
import { person } from './people.js';
import { fogCol } from './features.js';

const KIND_WORDS = { bug: 'Bug', feature: 'Feature', tests: 'Tests', security: 'Security', research: 'Research', perf: 'Performance',
  refactor: 'Refactor', docs: 'Docs', chores: 'Chore', workshop: 'Finished work, sealed (quay and stages)', cottage: 'Other work' };
const ICON_WORDS = { blocked: 'Blocked: waiting on you', done: 'Done', unknown: 'Unknown: no word from it', stalled: 'Stalled: no progress' };
const PENNANT_WORDS = { P0: 'P0: urgent (waves)', P1: 'P1: high', P2: 'P2: normal' };
function swatch(paint, size = 14) {
  const c = document.createElement('canvas');
  c.width = size; c.height = size; c.setAttribute('aria-hidden', 'true');
  const g = c.getContext('2d'); g.imageSmoothingEnabled = false; paint(g);
  return c;
}
export function renderLegend() {
  const L = state.T.labels, body = document.getElementById('legend-body');
  document.getElementById('legend-title').textContent = `Key · ${state.T.name}`;
  const groups = [
    [`Crates at the ${L.docks}: the kind of work`, Object.keys(KINDCOL).map(k => [c => state.T.crate(c, 1, 1, k, null, k === 'workshop'), KIND_WORDS[k] || k, 7])],
    ['Priority', [[c => state.T.crate(c, 1, 1, 'feature', 'P0'), 'Red mark on a crate: P0', 7],
      ...Object.keys(PENNANT_WORDS).map(p => [c => pennant(c, 5, 7, p, 0), `Pennant over a house: ${PENNANT_WORDS[p]}`]),
      [() => {}, 'No pennant: P3']]],
    ['Bubbles over people and stops', Object.keys(ICONS).map(k => [c => bubble(c, 2, 1, k), ICON_WORDS[k] || k])],
    ['Other marks', [[c => { state.T.crate(c, 1, 1, 'feature', null); roped(c, 1, 1); }, 'Roped crate: waits on another issue ("Depends on")', 7], [c => cobwebs(c, -1, -6), 'Cobwebs on a house: stalled'],
      [c => { R(c, 0, 0, 14, 14, fogCol(state.T)); bubble(c, 2, 1, 'unknown'); }, 'Grey fog over a stage: nothing says what runs there (no Deployment, no branch)'],
      [c => { R(c, 4, 2, 1, 10, P.woodD); R(c, 5, 2, 6, 4, P.p0); }, 'Red flag on a warehouse or a ship: the newest deployment failed, and the ship turned back']]],
    ['Sub-agent accents: their specialty', [...SPECIALTIES.map(([name, kind]) => [c => person(c, 3, 1, 'key:' + name, 'stand', { accent: KINDCOL[kind], sub: true }), name]),
      [c => person(c, 3, 1, 'key:other', 'stand', { accent: P.white, sub: true }), 'any other']]],
    [`Notes on the ${L.board}`, Object.keys(NOTECOL).map(k => [c => { R(c, 1, 1, 5, 5, P.o); R(c, 2, 2, 3, 3, NOTECOL[k]); }, k.replace(/-/g, ' '), 7])]
  ];
  body.textContent = '';
  groups.forEach(([title, rows]) => {
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
