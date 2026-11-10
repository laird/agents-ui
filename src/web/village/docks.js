/* ---------- Where each crate stands at the docks ----------
 * No DOM and no drawing: the order the backlog's crates stand in, and what
 * state each is in, so node can test them (tools/village-tests/docks.test.mjs).
 *
 * The docks fill slot by slot from the front. A crate that waits on another
 * stands after it, crates waiting on the same issue stand together, and a
 * chain (#395 → #396 → #397/#398/#399) reads as one short run. Each run is
 * a group of crates tied by their dependencies; groups go in priority order,
 * the best crate in a group deciding where the whole group stands. The same
 * backlog always gives the same order, and a dependency cycle falls back to
 * backlog order for the crates in it. */

const RANK = { P0: 0, P1: 1, P2: 2, P3: 3 };
const rank = c => RANK[c.priority] ?? 4;

/* The backlog in dock order: a new array of the same crates. */
export function arrangeDocks(backlog) {
  const list = backlog || [];
  const at = new Map(list.map((c, i) => [c.number, i]));
  // Only crates at the docks tie crates together: a dependency elsewhere
  // (in a house, on the board) leaves its waiting crate free to stand anywhere.
  const needs = list.map(c => [...new Set((c.waits_on || []).filter(d => at.has(d) && d !== c.number))].map(d => at.get(d)));
  const parent = list.map((_, i) => i);
  const root = i => { while (parent[i] !== i) i = parent[i] = parent[parent[i]]; return i; };
  needs.forEach((ds, i) => ds.forEach(j => { const a = root(i), b = root(j); if (a !== b) parent[Math.max(a, b)] = Math.min(a, b); }));
  const groups = new Map();
  list.forEach((_, i) => { const r = root(i); (groups.get(r) || groups.set(r, []).get(r)).push(i); });
  // The best crate in a group (priority, then backlog order) places it.
  const key = g => g.reduce((k, i) => Math.min(k, rank(list[i]) * 1e6 + i), Infinity);
  const ordered = [...groups.values()].sort((a, b) => key(a) - key(b));
  return ordered.flatMap(g => chain(g, needs, list)).map(i => list[i]);
}

/* One group's crates, prerequisites first: by depth (how many crates stand
 * before it in its longest chain), then beside the crate they wait on, then
 * by priority and backlog order. A cycle leaves the group in backlog order. */
function chain(group, needs, list) {
  if (group.length === 1) return group;
  const depth = new Map(), seen = new Set();
  const visit = (i, path) => {
    if (depth.has(i)) return depth.get(i);
    if (path.has(i)) return null;
    path.add(i);
    let d = 0;
    for (const j of needs[i]) { const dj = visit(j, path); if (dj === null) return null; d = Math.max(d, dj + 1); }
    path.delete(i); seen.add(i); depth.set(i, d);
    return d;
  };
  for (const i of group) if (visit(i, new Set()) === null) return [...group].sort((a, b) => a - b);
  const out = [], pos = new Map();
  const levels = [...new Set(group.map(i => depth.get(i)))].sort((a, b) => a - b);
  for (const lv of levels) {
    const here = group.filter(i => depth.get(i) === lv);
    // Beside what it waits on: the nearest of its prerequisites already placed.
    const near = i => Math.min(...needs[i].map(j => pos.get(j) ?? Infinity), Infinity);
    here.sort((a, b) => (near(a) - near(b)) || (rank(list[a]) - rank(list[b])) || (a - b));
    here.forEach(i => { pos.set(i, out.length); out.push(i); });
  }
  return out;
}

/* A backlog crate's state: 'ready' to be picked up, or 'waiting' on an open
 * issue. Issues blocked by a label are notes on the board, not crates. */
export const crateState = c => ((c.waits_on || []).length ? 'waiting' : 'ready');

/* Where an issue a crate waits on is, in words: "at the docks", "being worked
 * on at wt-2", "on the board: needs-design", or nothing when it isn't drawn. */
export function whereWaitedOn(v, n, houseName) {
  if (v.backlog.some(c => c.number === n)) return 'at the docks';
  const h = v.houses.find(h => h.issue === n);
  if (h) return `being worked on at ${houseName(v, h)}`;
  const note = v.board.find(b => b.number === n);
  if (note) return `on the board: ${note.label}`;
  return '';
}
