/* ---------- What the key says ----------
 * The Key's sections and their words (#442), in the theme's own names for
 * places. No DOM, so its tests run under node
 * (tools/village-tests/keyrows.test.mjs); key.js draws a swatch for each row
 * with the renderer's own sprites.
 *
 * Places come first (what each part of the map is), then States (what a
 * worker is doing, loudest last), then the marks. */

/* Places: one sprite and one line each, in the theme's names (`L` is
 * `theme.labels`). */
export function placeRows(L) {
  return [
    ['house', `${(L.houses || 'Houses').replace(/s$/, '')}: a worker agent; under it its name and the issue it is on (wt-1 · #400)`],
    ['hall', `${L.hall}: the manager, who plans and hands out the work`],
    ['docks', `${L.docks}: the backlog, a crate per open issue`],
    ['board', `${L.board}: blocked issues and proposals, a note each`],
    ['gatehouse', `${L.gate}: the merge gate, a table lit per merge being tested`],
    ['quay', `${L.quay}: work landed on the integration branch, not yet deployed`],
    ['lock', 'Lock on the canal: a stage the work passes on its way out'],
    ['harbour', `${L.castle}: the last stage, where the ${L.ship.toLowerCase()} loads each release`],
    ['env', 'Village down the river: a deploy environment; its sign gives the version, its warehouse what it runs'],
    ['ship', `${L.ship}: a deployment, sailing to the next environment with what it carries`],
    ['flag', "Flag: an epic in progress, its colour on its crates; under it how far it has got"]
  ].map(([id, words]) => ({ id, words }));
}

/* States: what a worker is doing, the quiet ones first, trouble last and loudest. */
export function stateRows(freshDoneMinutes) {
  return [
    ['busy', 'Busy: lit windows, smoke from the chimney, the worker hammering at the door'],
    ['done', `Done: a ✓ for ${freshDoneMinutes} minutes after finishing`],
    ['idle', 'Idle: sitting on the doorstep under "zzz", nothing claimed'],
    ['failed', 'Blocked, failed or stalled: a red ✗'],
    ['waiting', 'Waiting on you: a big red "!" and a ring at the door (a question or a permission prompt)']
  ].map(([id, words]) => ({ id, words }));
}

/* The priority marks, with the P-number first so each reads at a glance;
 * each level has its own pattern too, not just its colour (#443). */
export const PRIO_ROWS = [
  ['P0', 'P0 · urgent: a solid red mark on the crate'],
  ['P1', 'P1 · high: an orange mark, its two ends'],
  ['P2', 'P2 · normal: a yellow mark, dashed'],
  ['P3', 'P3 · low: a grey dot in the middle']
].map(([p, words]) => ({ p, words }));

/* The Key's sections, in order. */
export const SECTIONS = ['Places', 'States', 'Marks'];
