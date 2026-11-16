/* ---------- A worker control's result, played (#507, for #488) ----------
 * controls.js calls controlDone() when the server says a control worked.
 * The house shows it at once, from the table in controlfx.js: the worker
 * gets up (lifecycle.js), the barrel empties (barrels.js), the crate is
 * carried back to the docks (a trip, trips.js) or the house goes dark (the
 * layer here). With reduced motion nothing moves: a toast says it. The
 * override is local and short-lived (controlfx.js), never the model. */
import { state } from './state.js';
import { registerLayer } from './registry.js';
import { toast } from './panels.js';
import { carryBack } from './trips.js';
import { darkHouse, fxOn, fxToast, startFx } from './controlfx.js';

/* `c` is what the house dialog passed (lifecycle.js: project, house, name,
 * issue); `action` the control that just succeeded. */
export function controlDone(c, action) {
  if (state.reduce) {
    const words = fxToast(action, c.name, c.issue);
    if (words) toast(`${c.project}: ${words}`);
    return;
  }
  const sc = state.scenes[c.project], v = sc && sc.v;
  const i = v ? v.houses.findIndex(h => h.id === c.house) : -1;
  if (i < 0) return;
  const h = v.houses[i], t = performance.now(), fx = startFx(c.project, h, action, t);
  if (!fx || !fx.carry || !h.issue || !sc.L || !sc.L.lots[i]) return;
  // The crate goes back to the end of the docks, carried as trips are
  // (state.porters is the scene being drawn, so swap this scene's in, as data.js does).
  const drawing = state.porters;
  state.porters = sc.porters;
  carryBack(sc, sc.L, sc.L.lots[i], h, v.backlog.length, v.backlog.length + 1, t);
  sc.porters = state.porters;
  state.porters = drawing;
}

/* A stopped house, dark until the next poll says so itself. */
registerLayer({ z: 72.6, draw(c, { v, lay }, t) {
  v.houses.forEach((h, i) => {
    const lot = lay.lots[i], fx = lot && fxOn(v.project, h, t);
    if (fx && fx.dark) darkHouse(c, lot.x, lot.y);
  });
} });
