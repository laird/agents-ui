/* ---------- A worker control's result, at once (#507, for #488) ----------
 * When a control from the house dialog succeeds (controls.js, #506), the
 * house shows it before the next poll does: a short-lived local override,
 * kept here by project and house, never written into the model the server
 * sent. It ends after CONTROL_FX_MS, or as soon as a poll shows the house
 * changed (its status, issue or context), since then the real state says it.
 *
 * What each action shows is one table, CONTROL_FX, keyed by action:
 *   up      the worker gets up: stands at its door instead of sleeping
 *           (lifecycle.js asks fxRest);
 *   empty   its context barrel is drawn empty (barrels.js asks fxContext);
 *   carry   its crate is carried back to the docks (controlplay.js, a trip);
 *   dark    the house goes dark (controlplay.js draws darkHouse over it).
 * All four exist in controls.rs (#504, #505, #567). With reduced motion
 * nothing moves: fxToast says it.
 * DOM-free: tools/village-tests/controlfx.test.mjs. */
import { detail } from './detail.js';
import { px } from './art2x.js';

export const CONTROL_FX_MS = 6000;

export const CONTROL_FX = {
  nudge: { up: true, say: (name) => `Nudged ${name}: it gets up and starts its loop.` },
  release: { carry: true, say: (name, issue) => `Released ${issue ? '#' + issue : 'its issue'} from ${name}: the crate goes back to the docks.` },
  restart: { up: true, empty: true, say: (name) => `Restarted ${name}: a fresh session, its context emptied.` },
  stop: { dark: true, say: (name) => `Stopped ${name}: the house goes dark.` },
};

/* What a house looked like when the control was sent: a poll that differs ends the override. */
export const fxSnapshot = h => (h ? `${h.status}|${h.issue || ''}|${h.context_pct ?? ''}` : '');
const fxKeyOf = (project, house) => `${project}\u0000${house}`;

/* The overrides in force, by project and house. */
export const CONTROL_FX_LIVE = new Map();

/* Start the override for `action` on house `h` of `project` at `now` (ms).
 * Returns it, or null for an action with no visible result. */
export function startFx(project, h, action, now, live = CONTROL_FX_LIVE) {
  const fx = CONTROL_FX[action];
  if (!fx || !h) return null;
  const entry = { ...fx, action, t0: now, until: now + CONTROL_FX_MS, snap: fxSnapshot(h) };
  live.set(fxKeyOf(project, h.id), entry);
  return entry;
}

/* The override on house `h` at `now`, or null: an expired one, or one the
 * house has moved on from, is dropped. */
export function fxOn(project, h, now, live = CONTROL_FX_LIVE) {
  if (!h || !live.size) return null;
  const k = fxKeyOf(project, h.id), fx = live.get(k);
  if (!fx) return null;
  if (now >= fx.until || fxSnapshot(h) !== fx.snap) { live.delete(k); return null; }
  return fx;
}

/* A worker the override got up stands at its door (rest 'fresh'). */
export const fxRest = (fx, rest) => (fx && fx.up ? 'fresh' : rest);
/* An emptied barrel reads 0%. */
export const fxContext = (fx, pct) => (fx && fx.empty && pct != null ? 0 : pct);

/* What a toast says instead, with motion reduced. */
export function fxToast(action, name, issue) {
  const fx = CONTROL_FX[action];
  return fx ? fx.say(name || 'the worker', issue) : null;
}

/* A house gone dark: a shadow over its lot, 16×25 scene pixels at (x, y - 6). */
export const DARK = 'rgba(6,6,18,.72)';
export function darkHouse(c, x, y) {
  detail(c, x - 1, y - 6, g => { px(g, 2, 2, 32, 42, DARK); px(g, 0, 4, 2, 38, DARK); px(g, 34, 4, 2, 38, DARK); });
}
