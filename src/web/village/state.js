/* What the drawing reads and the page sets, in one object: ES imports are
 * read-only bindings, so a module that sets one of these sets `state.x`.
 *
 * `T` is the theme being drawn. `current`, `L`, `porters`, `floats` and
 * `fireworks` hold the scene being drawn while paint() runs: its model, its
 * layout and the animations in flight. `S` is that scene itself (its voyage
 * and lock gates). */
export const state = {
  T: null,
  reduce: window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches,
  current: null, L: null, porters: [], floats: [], fireworks: -1e9, S: null,
};

/* The animation clock, held still when motion is reduced. */
export const now = () => (state.reduce ? 0 : performance.now());
