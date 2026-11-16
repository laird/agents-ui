/* ---------- A valley of related repos: the decisions (#510) ----------
 * `/village?valley=<group>` draws a group of repos (declared in
 * ~/.agents-ui/group.json, served by `/api/valley`) as one valley: the
 * repos the others build on upstream (west), their consumers downstream
 * (east), a river along each `package` edge, a road along each `api` edge.
 * This decides, with no DOM, where each repo sits and where the rivers and
 * roads run, so node tests it (tools/village-tests/valleymodel.test.mjs).
 * valley.js draws it. */

/* Each caravan's row in its band below the lane, and the gap above the band (#530). */
export const CARAVAN_ROW = 16, CARAVAN_GAP = 10;

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
  // Caravans (#530) travel a band of their own below the lane, a row each.
  const nCar = ((group && group.caravans) || []).length, band = nCar ? CARAVAN_GAP + nCar * CARAVAN_ROW : 0, caravanTop = lane + 6 + CARAVAN_GAP;
  const grid = gridLayout(group && group.grid, lane + 24 + band);
  return { W: Math.max(W, grid ? grid.W : 0), H: grid ? grid.bottom + m : H + band, cells, rivers, riverEdges, roads, grid, caravanTop };
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

/* ---------- Ropes, caravans and barges (#530) ----------
 * From `/api/valley`'s `ropes`, `caravans` (crossrepo.rs, #528) and
 * `barges` (breakage.rs, #529): where each is drawn and what a click on it
 * says. valleylinks.js draws them. */

/* A caravan's lead (the horse) and each crate, in scene pixels. */
export const CARAVAN_LEAD = 10, CARAVAN_CRATE = 7, BARGE_TRIP_MS = 16000;

const repoIndex = (group, name) => ((group && group.repos) || []).findIndex(r => r.name === name);

/* Each rope as a segment between the facing sides of its two repos:
 * { x0, y0, x1, y1, rope }. Ropes between the same two repos stand apart. */
export function valleyRopes(lay, group) {
  const pairs = {};
  return ((group && group.ropes) || []).map(rope => {
    const a = lay.cells[repoIndex(group, rope.from.repo)], b = lay.cells[repoIndex(group, rope.to.repo)];
    if (!a || !b || a === b) return null;
    const key = [rope.from.repo, rope.to.repo].sort().join('|'), k = (pairs[key] = (pairs[key] ?? -1) + 1) % 6;
    if (a.x + a.w <= b.x || b.x + b.w <= a.x) {
      const east = a.x < b.x;
      return { x0: east ? a.x + a.w : a.x, y0: a.y + 14 + 6 * k, x1: east ? b.x : b.x + b.w, y1: b.y + 14 + 6 * k, rope };
    }
    const south = a.y < b.y;
    return { x0: a.x + 24 + 10 * k, y0: south ? a.y + a.h : a.y, x1: b.x + 24 + 10 * k, y1: south ? b.y : b.y + b.h, rope };
  }).filter(Boolean);
}

/* Distance from (x, y) to the segment (x0, y0)–(x1, y1). */
function toSegment(x, y, { x0, y0, x1, y1 }) {
  const dx = x1 - x0, dy = y1 - y0, len = dx * dx + dy * dy;
  const u = len ? Math.max(0, Math.min(1, ((x - x0) * dx + (y - y0) * dy) / len)) : 0;
  return Math.hypot(x - (x0 + u * dx), y - (y0 + u * dy));
}

/* The rope within `r` scene pixels of (x, y), or null. */
export function valleyRopeAt(ropes, x, y, r = 3) {
  return ropes.find(rp => toSegment(x, y, rp) <= r) || null;
}

const issueLink = i => (i && i.url ? { label: `${i.repo}#${i.number}`, url: i.url } : null);

/* What a click on a rope says: which issue waits on which, with links. */
export function ropeWords(rope) {
  const { from: a, to: b } = rope;
  return {
    title: `${a.repo}#${a.number} waits on ${b.repo}#${b.number}`,
    line: `${a.repo}#${a.number} can't go on until ${b.repo}#${b.number} is done.`,
    meta: [`Waiting: ${a.repo}#${a.number} ${a.title}`, `Blocking: ${b.repo}#${b.number} ${b.title}`],
    links: [issueLink(a), issueLink(b)].filter(Boolean),
  };
}

/* Each caravan as a train in its row: { x, y, w, caravan, loaded, total }.
 * It travels the row as its crates load (the share loaded), and stands at
 * the east end once it has arrived. */
export function valleyCaravans(lay, group) {
  const m = VALLEY_MARGIN;
  return ((group && group.caravans) || []).map((caravan, k) => {
    const total = caravan.crates.length, loaded = caravan.crates.filter(c => c.loaded).length;
    const w = CARAVAN_LEAD + total * CARAVAN_CRATE, track = Math.max(0, lay.W - 2 * m - w - 60);
    const share = caravan.arrived ? 1 : (total ? loaded / total : 0);
    return { x: m + Math.round(share * track), y: lay.caravanTop + k * CARAVAN_ROW, w, caravan, loaded, total };
  });
}

/* The caravan under (x, y), or null. */
export function valleyCaravanAt(trains, x, y) {
  return trains.find(t => x >= t.x - 2 && x < t.x + t.w + 2 && y >= t.y - 2 && y < t.y + CARAVAN_ROW - 4) || null;
}

/* What a click on a caravan says: each repo's part, landed or not, with links. */
export function caravanWords(train) {
  const { caravan: c, loaded, total } = train, p = c.parent;
  const crateIssue = (cr, n) => (cr.slug ? { label: `${cr.repo}#${n}`, url: `https://github.com/${cr.slug}/issues/${n}` } : null);
  return {
    title: `Caravan: ${p.repo}#${p.number} ${p.title}`,
    line: c.arrived ? 'Arrived: every repo\'s part has landed.' : `${loaded} of ${total} repos' parts landed.`,
    meta: c.crates.map(cr => `${cr.repo}: ${cr.issues.map(n => '#' + n).join(', ')} · ${cr.loaded ? 'landed' : 'not yet'}`),
    links: [issueLink(p), ...c.crates.flatMap(cr => cr.issues.map(n => crateIssue(cr, n)))].filter(Boolean),
  };
}

/* Each barge on its way back up the river from the consumer to the library
 * its failure names: { x, y, barge }. With `t` (ms of animation) it sails
 * upstream every BARGE_TRIP_MS; at 0 (reduced motion) it waits mid-river. A
 * consumer and library with no river between them: it waits at the
 * consumer's west side. Several on one river keep apart. */
export function valleyBarges(lay, group, t) {
  const seen = {};
  return ((group && group.barges) || []).map(barge => {
    const n = seen[barge.from + '>' + barge.to] = (seen[barge.from + '>' + barge.to] ?? -1) + 1;
    const k = (lay.riverEdges || []).findIndex(e => e.from === barge.to && e.to === barge.from);
    if (k >= 0) {
      const f = t ? 1 - (t % BARGE_TRIP_MS) / BARGE_TRIP_MS : 0.5;
      const [x, y] = along(lay.rivers[k], (f + n * 0.2) % 1);
      return { x, y, barge };
    }
    const cell = lay.cells[repoIndex(group, barge.from)];
    return cell ? { x: cell.x - 10, y: cell.y + (cell.h >> 1) + n * 8, barge } : null;
  }).filter(Boolean);
}

/* The barge within `r` scene pixels of (x, y), or null. */
export function valleyBargeAt(barges, x, y, r = 6) {
  return barges.find(b => Math.abs(b.x - x) <= r && Math.abs(b.y - y) <= r) || null;
}

/* What a click on a barge says: whose build fails, why it names the library, and the run. */
export function bargeWords(b) {
  const { from, to, why, run } = b.barge;
  return {
    title: `${from}'s build fails, back to ${to}`,
    line: `${from} is red, and the failure names ${to}: ${why}.`,
    meta: [`From: ${from} (the consumer)`, `To: ${to} (the library)`],
    links: run ? [{ label: 'The failing run', url: run }] : [],
  };
}
