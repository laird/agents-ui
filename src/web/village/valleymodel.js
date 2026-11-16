/* ---------- A valley of related repos: the decisions (#510) ----------
 * `/village?valley=<group>` draws a group of repos (declared in
 * ~/.agents-ui/group.json, served by `/api/valley`) as one valley: the
 * repos the others build on upstream (west), their consumers downstream
 * (east), a river along each `package` edge, a road along each `api` edge.
 * This decides, with no DOM, where each repo sits and where the rivers and
 * roads run, so node tests it (tools/village-tests/valleymodel.test.mjs).
 * valley.js draws it. */

/* A repo's patch of ground, the gaps between, and the margin round it all. */
export const VALLEY_CELL = { w: 104, h: 72 }, VALLEY_GAP = { x: 64, y: 40 }, VALLEY_MARGIN = 24;

/* The group the URL asks for (`?valley=shop`), or null. */
export function valleyName(search) {
  const v = new URLSearchParams(String(search || '')).get('valley');
  return v ? v : null;
}

/* Where everything sits, in scene pixels: one column per depth (upstream
 * first), each repo below the one before it in its column. `cells[k]` is
 * `repos[k]`'s; `rivers` and `roads` are polylines, [[x, y], ...]. */
export function valleyLayout(group) {
  const repos = (group && group.repos) || [], { w, h } = VALLEY_CELL, gx = VALLEY_GAP.x, gy = VALLEY_GAP.y, m = VALLEY_MARGIN;
  const rowOf = {}, cells = [], at = {};
  repos.forEach(r => {
    const d = r.depth || 0, row = rowOf[d] = (rowOf[d] ?? -1) + 1;
    const cell = { x: m + d * (w + gx), y: m + row * (h + gy), w, h };
    cells.push(cell); at[r.name] = cell;
  });
  const cols = repos.length ? Math.max(...repos.map(r => r.depth || 0)) + 1 : 1;
  const rows = Math.max(1, ...Object.values(rowOf).map(n => n + 1));
  const W = 2 * m + cols * w + (cols - 1) * gx;
  // The roads' lane runs below the lowest row.
  const lane = m + rows * h + (rows - 1) * gy + 14, H = lane + 12;
  // Rivers and roads keep to the gaps, never under a repo: a river that
  // skips a column climbs to the lane above every row; a road leaves its
  // repo's west side and runs along the lane below every row.
  const top = m >> 1, rivers = [], riverEdges = [], roads = [];
  ((group && group.edges) || []).forEach(e => {
    const a = at[e.from], b = at[e.to];
    if (!a || !b) return;
    if (e.via === 'package') {
      const x0 = a.x + w, y0 = a.y + (h >> 1), x1 = b.x, y1 = b.y + (h >> 1), out = x0 + (gx >> 1), into = x1 - (gx >> 1);
      if (out === into) rivers.push(y0 === y1 ? [[x0, y0], [x1, y1]] : [[x0, y0], [out, y0], [out, y1], [x1, y1]]);
      else rivers.push([[x0, y0], [out, y0], [out, top], [into, top], [into, y1], [x1, y1]]);
      riverEdges.push(e);
    } else {
      const xa = a.x - 14, xb = b.x - 14, ya = a.y + h - 12, yb = b.y + h - 12;
      roads.push([[a.x, ya], [xa, ya], [xa, lane], [xb, lane], [xb, yb], [b.x, yb]]);
    }
  });
  const grid = gridLayout(group && group.grid, lane + 24);
  return { W: Math.max(W, grid ? grid.W : 0), H: grid ? grid.bottom + m : H, cells, rivers, riverEdges, roads, grid };
}

/* ---------- The environment grid (#512) ----------
 * Below the valley, the repos joined by `api` edges by environment: a row
 * per repo, a column per environment, the prod column drawn as a city.
 * Each cell is what that repo runs there; a cell outside what a neighbour
 * in the same environment expects is marked. */
export const GRID_CELL = { w: 56, h: 30 }, GRID_HEAD = 14, GRID_LABEL = 64, GRID_GAP = 6;

/* Whether an environment's column is the city: prod (or production). */
export const isCity = env => /^prod(uction)?$/i.test(String(env || ''));

/* Where the grid's cells sit, from `top` down: { x, y, W, bottom, cols, rows, cells }.
 * Null with no grid. */
export function gridLayout(grid, top) {
  if (!grid || !(grid.envs || []).length || !(grid.repos || []).length) return null;
  const { w, h } = GRID_CELL, x0 = VALLEY_MARGIN + GRID_LABEL, y0 = top + GRID_HEAD;
  const cols = grid.envs.map((env, k) => ({ env, x: x0 + k * (w + GRID_GAP), city: isCity(env) }));
  const rows = grid.repos.map((repo, k) => ({ repo, y: y0 + k * (h + GRID_GAP) }));
  const cells = (grid.cells || []).map(cell => {
    const c = cols.find(o => o.env === cell.env), r = rows.find(o => o.repo === cell.repo);
    return c && r ? { x: c.x, y: r.y, w, h, city: c.city, cell } : null;
  }).filter(Boolean);
  const W = x0 + cols.length * (w + GRID_GAP) - GRID_GAP + VALLEY_MARGIN;
  return { x: x0, y: top, W, bottom: y0 + rows.length * (h + GRID_GAP) - GRID_GAP, cols, rows, cells };
}

/* The grid cell under (x, y), or null. */
export function gridAt(lay, x, y) {
  const g = lay && lay.grid;
  return (g && g.cells.find(c => x >= c.x && x < c.x + c.w && y >= c.y && y < c.y + c.h)) || null;
}

/* What a click on a cell says: what runs there, and whom it doesn't suit. */
export function gridWords(cell) {
  const marks = cell.mismatches || [];
  return {
    title: `${cell.repo} in ${cell.env}`,
    line: marks.length
      ? `Runs ${cell.version}, outside what ${marks.map(m => m.neighbour).join(' and ')} in ${cell.env} expect${marks.length === 1 ? 's' : ''}.`
      : (cell.version ? `Runs ${cell.version}.` : 'What it runs is not known.'),
    meta: marks.map(m => `${m.neighbour} in ${m.env} expects ${cell.repo} ${m.expects}; ${m.contract ? `its contract (${m.contract}) says` : 'it runs'} ${m.version}`),
  };
}

/* ---------- Ships: releases flowing downstream (#511) ----------
 * Each `package` edge says what its consumer pins and the library's latest
 * release (`/api/valley`, pins.rs). A consumer behind the latest release has
 * a ship waiting at its dock, labelled with the version it is waiting to
 * take. A release made within SHIP_SAIL_MS sails down the river to each
 * consumer that has taken it. */
export const SHIP_SAIL_MS = 24 * 3600 * 1000, SHIP_TRIP_MS = 12000, SHIP_DOCK = 10, SHIP_BERTH = 12;

/* The point `f` (0–1) of the way along a polyline. */
export function along(poly, f) {
  const legs = poly.slice(1).map(([x, y], k) => Math.abs(x - poly[k][0]) + Math.abs(y - poly[k][1]));
  let d = Math.max(0, Math.min(1, f)) * legs.reduce((a, b) => a + b, 0);
  for (let k = 0; k < legs.length; k++) {
    if (d <= legs[k] || k === legs.length - 1) {
      const [x0, y0] = poly[k], [x1, y1] = poly[k + 1], u = legs[k] ? Math.min(1, d / legs[k]) : 0;
      return [Math.round(x0 + (x1 - x0) * u), Math.round(y0 + (y1 - y0) * u)];
    }
    d -= legs[k];
  }
  return poly[0];
}

/* The ships on the valley's rivers at time `t` (ms of animation), `now` the
 * wall clock: { x, y, edge, waiting }. A waiting ship sits SHIP_DOCK pixels
 * short of its consumer; a consumer behind on several libraries has a berth
 * for each, SHIP_BERTH apart, alternately above and below the river's mouth
 * (every river into it ends at the same point). A sailing one loops down the
 * river every SHIP_TRIP_MS. */
export function valleyShips(lay, t, now = Date.now()) {
  const ships = [], berths = {};
  (lay.riverEdges || []).forEach((e, k) => {
    if (!e.latest) return;
    const poly = lay.rivers[k], [ex, ey] = poly[poly.length - 1];
    if (e.behind) {
      const n = berths[e.to] = (berths[e.to] ?? -1) + 1, off = n === 0 ? 0 : (n % 2 ? -1 : 1) * Math.ceil(n / 2) * SHIP_BERTH;
      ships.push({ x: ex - SHIP_DOCK, y: ey + off, edge: e, waiting: true });
      return;
    }
    const made = Date.parse(e.latest_at || '');
    if (e.behind === false && made && now - made >= 0 && now - made < SHIP_SAIL_MS) {
      const [x, y] = along(poly, (t % SHIP_TRIP_MS) / SHIP_TRIP_MS);
      ships.push({ x, y, edge: e, waiting: false });
    }
  });
  return ships;
}

/* The ship within `r` scene pixels of (x, y), or null. */
export function valleyShipAt(ships, x, y, r = 6) {
  return ships.find(s => Math.abs(s.x - x) <= r && Math.abs(s.y - y) <= r) || null;
}

/* "2026-11-16T09:00:00Z" as "16 Nov 09:00", in local time; '' when unknown. */
export function shipWhen(at) {
  const d = new Date(Date.parse(at || ''));
  if (isNaN(d)) return '';
  const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'], p = n => String(n).padStart(2, '0');
  return `${d.getDate()} ${months[d.getMonth()]} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

/* What a click on a ship says: which versions, and since when. */
export function shipWords(ship) {
  const e = ship.edge, when = shipWhen(e.latest_at), since = when ? ` since ${when}` : '';
  return ship.waiting
    ? { title: `${e.from} ${e.latest} waits at ${e.to}'s dock`,
      line: `${e.to} pins ${e.from} ${e.pinned}, behind ${e.latest}${since}.`,
      meta: [`Pinned by ${e.to}: ${e.pinned}`, `Latest release of ${e.from}: ${e.latest}${when ? ` (${when})` : ''}`] }
    : { title: `${e.from} ${e.latest} sails to ${e.to}`,
      line: `A new release${since}; ${e.to} pins ${e.pinned}, so it has it.`,
      meta: [`Pinned by ${e.to}: ${e.pinned}`, `Latest release of ${e.from}: ${e.latest}${when ? ` (${when})` : ''}`] };
}

/* The repo under (x, y), or null. */
export function valleyAt(cells, x, y) {
  const k = cells.findIndex(c => x >= c.x && x < c.x + c.w && y >= c.y && y < c.y + c.h);
  return k < 0 ? null : k;
}

/* What a repo's label says: its name, and its houses, backlog and release. */
export function repoWords(r) {
  const s = r.summary || {};
  if (r.dormant) return { text: r.name, sub: ['dormant', s.release].filter(Boolean).join(' · ') };
  const parts = [`${s.working || 0} working`, `${s.idle || 0} idle`];
  if (s.blocked) parts.push(`${s.blocked} blocked`);
  parts.push(`${s.backlog || 0} in backlog`);
  if (s.release) parts.push(s.release);
  return { text: r.name, sub: parts.join(' · ') };
}

/* The bar's line for a group: how many repos and links, and what is wrong. */
export function valleyWords(group, errors = []) {
  const repos = (group && group.repos) || [], edges = (group && group.edges) || [];
  const n = (k, one, many) => `${k} ${k === 1 ? one : many}`;
  const pkg = edges.filter(e => e.via === 'package').length, api = edges.length - pkg;
  const wrong = [...errors, ...((group && group.errors) || [])];
  return [n(repos.length, 'repo', 'repos'), n(pkg, 'package link', 'package links'), n(api, 'API link', 'API links'),
    ...(wrong.length ? [n(wrong.length, 'problem', 'problems') + ': ' + wrong.join('; ')] : [])].join(' · ');
}

/* Why nothing is drawn: no manifest, or no such group. Null when there is one. */
export function valleyMissing(resp, name) {
  if (!resp) return null;
  if (!resp.found) return `No valleys yet: declare a group of repos in ${resp.manifest} (see docs/village.md).`;
  if (!(resp.groups || []).some(g => g.name === name)) {
    const names = (resp.groups || []).map(g => g.name);
    const why = (resp.errors || []).length ? ` ${resp.errors.join('; ')}.` : '';
    return `No group named "${name}" in ${resp.manifest}.${why}` + (names.length ? ` Groups: ${names.join(', ')}.` : '');
  }
  return null;
}
