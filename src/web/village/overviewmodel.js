/* ---------- All swarms at a glance: the decisions (#481) ----------
 * The overview map (overview.js) shows every swarm at once, each as an
 * island. This decides, with no DOM, what each island says and where it
 * sits, so node tests it (tools/village-tests/overviewmodel.test.mjs).
 *
 * One island per thing the picker and the screensaver cycle through: each
 * world (its dev village and its platform together), then each village that
 * is not half of a world. Its health is one colour:
 *   red    something waits on you (the board, a blocked manager, work the
 *          gate handed back), the build is broken, or a deploy failed;
 *   amber  a house is blocked or stalled;
 *   green  otherwise.
 * The overview's name in the URL is `*` (`/village#*`), which no project
 * name can be: a village's name is its directory's. */
import { nextName, saverSummary } from './saver.js';

export const OVERVIEW_NAME = '*';

/* Why one village is red or amber, in a few words each. */
function villageTrouble(v) {
  const s = saverSummary(v), red = [], amber = [];
  if (v.build && v.build.state === 'red') red.push('build broken');
  const failed = ((v.pipeline && v.pipeline.stops) || []).filter(st => st.health && st.health.state === 'failed').map(st => st.name);
  if (failed.length) red.push(`deploy to ${failed.join(', ')} failed`);
  const board = (v.board || []).length + (v.manager && v.manager.status === 'blocked' ? 1 : 0) + s.gate.failed;
  if (board) red.push(`${board} waiting on you`);
  if (s.blocked) amber.push(`${s.blocked} blocked`);
  const stalled = (v.houses || []).filter(h => h.status === 'stalled').length;
  if (stalled) amber.push(`${stalled} stalled`);
  return { s, red, amber, stalled };
}

/* One island: a swarm of one village or two (a world's dev village first). */
export function swarmIsland(name, title, villages) {
  const each = villages.filter(Boolean).map(villageTrouble);
  const sum = k => each.reduce((n, e) => n + e.s[k], 0);
  const red = each.flatMap(e => e.red), amber = each.flatMap(e => e.amber);
  return {
    name, title,
    health: red.length ? 'red' : amber.length ? 'amber' : 'green',
    why: [...red, ...amber],
    working: sum('working'),
    // A house held by a usage limit is idle here: it works on nothing.
    idle: sum('idle') + sum('limited') - each.reduce((n, e) => n + e.stalled, 0),
    blocked: sum('blocked') + each.reduce((n, e) => n + e.stalled, 0),
    storm: villages.some(v => v && v.build && v.build.state === 'red'),
  };
}

/* Every swarm's island, in the picker's order. */
export function overviewIslands(villages, worlds) {
  const vs = villages || [], ws = worlds || [];
  const byName = n => vs.find(v => v.project === n);
  const inWorld = new Set(ws.flatMap(w => [w.dev, w.platform]));
  return [
    ...ws.map(w => swarmIsland(w.name, `${w.dev} ⇄ ${w.platform}`, [byName(w.dev), byName(w.platform)])),
    ...vs.filter(v => !inWorld.has(v.project)).map(v => swarmIsland(v.project, v.project, [v])),
  ];
}

/* Where n islands sit, in scene pixels: a grid with the columns that bring
 * the map nearest a 16:9 screen, each cell an island of OV_TILE with a strait
 * of OV_GAP round it. */
export const OV_TILE = { w: 112, h: 76 }, OV_GAP = 16;
export function overviewGrid(n, aspect = 16 / 9) {
  if (!n) return { cols: 0, rows: 0, W: 2 * OV_GAP + OV_TILE.w, H: 2 * OV_GAP + OV_TILE.h, cells: [] };
  const size = (cols, rows) => [cols * OV_TILE.w + (cols + 1) * OV_GAP, rows * OV_TILE.h + (rows + 1) * OV_GAP];
  let best = null;
  for (let cols = 1; cols <= n; cols++) {
    const rows = Math.ceil(n / cols);
    if (best && rows === best.rows) continue; // as many rows in fewer columns is narrower, never better
    const [W, H] = size(cols, rows), off = Math.abs(Math.log(W / H / aspect));
    if (!best || off < best.off - 1e-9) best = { cols, rows, W, H, off };
  }
  const { cols, rows, W, H } = best;
  const cells = Array.from({ length: n }, (_, k) => {
    const r = Math.floor(k / cols), inRow = r === rows - 1 ? n - r * cols : cols;
    // A short last row is centred under the others.
    const lead = ((cols - inRow) * (OV_TILE.w + OV_GAP)) >> 1;
    return { x: OV_GAP + lead + (k % cols) * (OV_TILE.w + OV_GAP), y: OV_GAP + r * (OV_TILE.h + OV_GAP), w: OV_TILE.w, h: OV_TILE.h };
  });
  return { cols, rows, W, H, cells };
}

/* The island under a scene point, or null. */
export function islandAt(cells, x, y) {
  const k = cells.findIndex(c => x >= c.x && x < c.x + c.w && y >= c.y && y < c.y + c.h);
  return k < 0 ? null : k;
}

/* The overview in a few words, for the screensaver's strip:
 * "2 need you · 1 blocked or stalled · 3 fine". */
export function overviewWords(islands) {
  const n = h => islands.filter(i => i.health === h).length;
  const parts = [];
  if (n('red')) parts.push(`${n('red')} ${n('red') === 1 ? 'needs' : 'need'} you`);
  if (n('amber')) parts.push(`${n('amber')} blocked or stalled`);
  if (n('green')) parts.push(`${n('green')} fine`);
  return parts.join(' · ') || 'No swarms';
}

/* What the screensaver shows next. With `overview=1` it comes back to the
 * overview between swarms: a swarm, the overview, the next swarm, ...
 * `last` is the swarm shown before the overview, so the round goes on. */
export function nextSaverScene(names, current, last, overview) {
  if (!overview) return nextName(names, current);
  if (current !== OVERVIEW_NAME || !names.length) return OVERVIEW_NAME;
  return nextName(names, last);
}
