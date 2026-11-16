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
  const rivers = [], roads = [];
  ((group && group.edges) || []).forEach(e => {
    const a = at[e.from], b = at[e.to];
    if (!a || !b) return;
    if (e.via === 'package') {
      // Out of the upstream repo's east side, down (or up) in the gap after it, into the downstream one's west side.
      const x0 = a.x + w, y0 = a.y + (h >> 1), x1 = b.x, y1 = b.y + (h >> 1), mid = x0 + (gx >> 1);
      rivers.push(y0 === y1 ? [[x0, y0], [x1, y1]] : [[x0, y0], [mid, y0], [mid, y1], [x1, y1]]);
    } else {
      // From the bottom of one to the bottom of the other, along the lane.
      const xa = a.x + (w >> 1) - 6, xb = b.x + (w >> 1) + 6;
      roads.push([[xa, a.y + h], [xa, lane], [xb, lane], [xb, b.y + h]]);
    }
  });
  return { W, H, cells, rivers, roads };
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
