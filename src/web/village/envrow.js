/* ---------- The environment row ----------
 * East of the dev village, one village per deploy environment, left to
 * right in pipeline order, production right-most (docs/environment-row.md).
 * Each has a sign with the version it runs, a warehouse holding what it runs
 * (its cargo, grouped under the banners of the features it belongs to), a
 * dock each side on the river, and a health mark: smoke and a red flag when
 * its newest deployment failed, a fire for an open incident, a "?" when
 * nothing says what it runs. Its look follows its rank: a town, and the
 * capital (production) with a keep. A world's platform stands in the slot
 * of the environment it runs (worlds.js): that slot has no houses or keep of
 * its own, and its sign stands by the warehouse, under the platform. The
 * ships between them are voyages.js; the strip of villages over the map is
 * the mini-map, at the end. */
import { state } from './state.js';
import { F, P, R, blink, featureColours, shadow } from './palette.js';
import { puffs } from './sprites/houses.js';
import { badge, bubble } from './scenery.js';
import { MODERN, M } from './themes/modern.js';
import { FUTURE } from './themes/future.js';
import { registerHit, registerLabels, registerLayer, registerOverlay, tap } from './registry.js';
import { cv, shownScenes } from './layout.js';
import { issueItems, isSha, link, repoUrl } from './dialogs.js';
import { sourceWords } from './ship.js';
import { esc } from './panels.js';

/* Where things stand in an environment village, from its left edge and the canal. */
export const SHED = { dx: 30, w: 60, h: 32, up: 44 }, ENV_DOCK = 20, BAY_COLS = 9, BAY_ROWS = 3;

/* What an environment runs, in a word: its release tag, else its commit. */
export const versionOf = s => s.tag || (s.sha ? String(s.sha).slice(0, 7) : null);
/* How long ago an RFC 3339 time was, in a few words; null when it isn't one. */
export function agoWords(iso, now = Date.now()) {
  const at = Date.parse(iso || '');
  if (Number.isNaN(at)) return null;
  const s = Math.max(0, (now - at) / 1000);
  if (s < 90) return 'just now';
  if (s < 5400) return `${Math.round(s / 60)} min ago`;
  if (s < 129600) return `${Math.round(s / 3600)} h ago`;
  return `${Math.round(s / 86400)} days ago`;
}
/* An environment's cargo, grouped by the feature each issue belongs to (in
 * the order the village lists its features), then the fixes that belong to
 * none. Each group keeps its banner colour from the feature strip. */
export function cargoGroups(v, ns) {
  const fs = v.features || [], cols = featureColours(fs), groups = fs.map(f => ({ f, col: cols.get(f.number), ns: [] })), fixes = { f: null, col: P.stone, ns: [] };
  ns.forEach(n => {
    const k = fs.findIndex(f => (f.subtasks || []).some(s => s.number === n));
    (k >= 0 ? groups[k] : fixes).ns.push(n);
  });
  return [...groups, fixes].filter(g => g.ns.length);
}
/* The cargo's slots in the warehouse bay: a banner opening each group, then
 * its crates, until the bay is full; how many more there are besides. */
export function baySlots(groups, more = 0) {
  const cap = BAY_COLS * BAY_ROWS, slots = [];
  let left = more;
  groups.forEach(g => {
    if (slots.length < cap - 1) slots.push({ banner: g });
    g.ns.forEach(n => { if (slots.length < cap) slots.push({ n }); else left++; });
  });
  return { slots, more: left };
}

/* ---------- Drawing ---------- */
const envLook = () => {
  const mod = state.T === MODERN, fut = state.T === FUTURE;
  return { roof: fut ? F.purple : mod ? M.steel : P.barn, roofD: fut ? F.purpleD : mod ? M.steelD : P.barnD,
    wall: fut ? F.hullD : mod ? M.concrete : P.wood, bay: fut ? F.navy : mod ? M.asphaltD : P.woodD,
    keep: fut ? F.hull : mod ? M.glass : P.stone, keepD: fut ? F.hullD : mod ? M.glassD : P.stoneD };
};
function envShed(c, x, y, w, h) {
  const k = envLook();
  shadow(c, x + 2, y + h, w, 2);
  R(c, x, y, w, h, P.o);
  R(c, x + 1, y + 1, w - 2, 7, k.roof); R(c, x + 1, y + 7, w - 2, 1, k.roofD);
  for (let xx = x + 4; xx < x + w - 2; xx += 6) R(c, xx, y + 2, 1, 4, k.roofD);
  R(c, x + 1, y + 8, w - 2, h - 9, k.wall);
  R(c, x + 2, y + 9, w - 4, h - 10, k.bay);
}
/* The capital's keep (a tower downtown, a spire in the future), its flag in the health colour. */
function envKeep(c, x, y, h, flag, t) {
  const k = envLook();
  shadow(c, x + 2, y + h, 12, 2);
  R(c, x, y, 12, h, P.o); R(c, x + 1, y + 1, 10, h - 2, k.keep); R(c, x + 1, y + 1, 2, h - 2, k.keepD);
  for (let xx = x; xx < x + 12; xx += 4) R(c, xx, y - 2, 2, 2, P.o);
  for (let yy = y + 5; yy < y + h - 6; yy += 7) R(c, x + 5, yy, 2, 3, P.dark);
  R(c, x + 5, y - 10, 1, 8, P.woodD); R(c, x + 6, y - 10 + (blink(t, 700) ? 1 : 0), 5, 3, flag);
}
function envSign(c, x, y, face) {
  const [frame, board, ink] = state.T.sign || [P.woodD, P.parch, P.woodD];
  R(c, x + 6, y + 8, 2, 12, frame);
  R(c, x, y, 15, 9, P.o); R(c, x + 1, y + 1, 13, 7, face || board);
  R(c, x + 3, y + 3, 9, 1, ink); R(c, x + 3, y + 5, 6, 1, ink);
}
const healthOf = s => (s.health && s.health.state) || (s.source === 'unknown' ? 'unknown' : 'ok');
const HEALTH_COL = { ok: '#38c848', failed: '#e02828', incident: '#f07020', unknown: '#8088a8' };

/* The villages: ground, road, docks, houses by rank, the sign and the warehouse. */
registerLayer({ z: 60, draw(c, { v, lay }, t) {
  const cy = lay.canalY;
  lay.envs.forEach(e => {
    const { x, w, s } = e, health = healthOf(s), signY = e.platform ? cy - 30 : 16;
    if (e.platform) tap(x, cy - SHED.up - 4, w, SHED.up + 4, { kind: 'env', i: e.i });
    else tap(x, 12, w, cy - 12, { kind: 'env', i: e.i });
    state.T.road(c, x + ENV_DOCK - 2, cy - 10, w - 2 * ENV_DOCK + 4, 5);
    state.T.dock(c, x, cy - 7, ENV_DOCK); state.T.dock(c, x + w - ENV_DOCK, cy - 7, ENV_DOCK);
    tap(x, cy - 7, ENV_DOCK, 7, { kind: 'env', i: e.i });
    // A town has two houses; the capital three and a keep. A platform's are its workloads.
    const houses = e.platform ? 0 : e.rank === 'capital' ? 3 : 2;
    for (let k = 0; k < houses; k++) state.T.B.cottage(c, x + 24 + k * 20, 18, 'idle', t);
    if (e.rank === 'capital' && !e.platform) envKeep(c, x + w - 18, 16, 34, HEALTH_COL[health], t);
    envSign(c, x + 4, signY, health === 'unknown' ? P.stoneL : null);
    tap(x + 2, signY - 2, 19, 22, { kind: 'env', i: e.i });
    if (health === 'unknown') bubble(c, x + 20, signY - 3, 'unknown');
    const sx = x + SHED.dx, sy = cy - SHED.up;
    envShed(c, sx, sy, SHED.w, SHED.h);
    tap(sx, sy, SHED.w, 9, { kind: 'env', i: e.i });
    if (health === 'failed') {
      puffs(c, sx + 10, sy - 2, t, '#484858', '#282830'); puffs(c, sx + 16, sy - 4, t + 500, '#585870', '#383848');
      R(c, sx + SHED.w - 8, sy - 9, 1, 9, P.woodD); R(c, sx + SHED.w - 7, sy - 9, 5, 3, P.p0);
    } else if (health === 'incident') {
      const f = blink(t, 140) ? 1 : 0;
      R(c, sx + 8, sy - 3 - f, 3, 3 + f, P.p0); R(c, sx + 9, sy - 4 - f, 1, 2, P.gold);
      R(c, sx + 40, sy - 2 + f, 3, 3 - f, P.p0); R(c, sx + 41, sy - 3 + f, 1, 2, P.gold);
      puffs(c, sx + 9, sy - 8, t, '#484858', '#282830');
    }
  });
} });
/* What each environment runs: crates in the warehouse bay, under their banners. */
registerLayer({ z: 85, draw(c, { v, lay }) {
  const cy = lay.canalY;
  lay.envs.forEach(e => {
    const sx = e.x + SHED.dx, sy = cy - SHED.up, { slots, more } = baySlots(cargoGroups(v, e.s.cargo || []), e.s.cargo_more || 0);
    slots.forEach((sl, k) => {
      const x = sx + 3 + (k % BAY_COLS) * 6, y = sy + 10 + Math.floor(k / BAY_COLS) * 7;
      if (sl.banner) { R(c, x + 1, y, 1, 6, P.woodL); R(c, x + 2, y, 3, 3, sl.banner.col); return; }
      state.T.crate(c, x, y + 1, 'workshop', null, true); tap(x, y + 1, 5, 5, { kind: 'crate', n: sl.n, at: 'env', i: e.i });
    });
    if (more > 0) { badge(c, sx + SHED.w - 14, sy - 3, more); tap(sx + SHED.w - 15, sy - 4, 16, 7, { kind: 'env', i: e.i }); }
  });
} });

/* ---------- The sign's dialog ---------- */
export function envLinks(v, s) {
  const repo = repoUrl(v), deploys = s.deployment_url || (repo && s.source === 'deployment' ? `${repo}/deployments/${encodeURIComponent(s.name)}` : null);
  const rel = s.url || (repo && s.tag ? `${repo}/releases/tag/${encodeURIComponent(s.tag)}` : null);
  return [link(s.tag ? `Release ${s.tag}` : 'What it runs', rel), link(`${s.name} deployments`, deploys),
    repo && isSha(s.sha) ? link(`Commit ${String(s.sha).slice(0, 7)}`, `${repo}/commit/${s.sha}`) : null,
    s.health && s.health.url ? link('What went wrong', s.health.url) : null];
}
registerHit({ kind: 'env', dialog(o, sc) {
  const v = sc.v, s = v.pipeline.stops[o.i];
  if (!s) return null;
  const ver = versionOf(s), when = agoWords(s.deployed_at), health = healthOf(s), cargo = s.cargo || [];
  const label = new Map(cargoGroups(v, cargo).flatMap(g => g.ns.map(n => [n, g.f ? `${g.f.title || 'feature #' + g.f.number}` : 'fix'])));
  return { title: `${s.name} · environment village`, line: ver ? `Runs ${ver}${when ? ', deployed ' + when : ''}` : 'Nothing says what runs here',
    meta: [`From: ${sourceWords(s)}`, s.deployed_by ? `Deployed by ${s.deployed_by}` : null,
      health === 'failed' ? `Newest deployment failed${s.health.reason ? ': ' + s.health.reason : ''}` : null,
      health === 'incident' ? `Incident${s.health.reason ? ': ' + s.health.reason : ''}` : null,
      `Runs ${cargo.length + (s.cargo_more || 0)} ${cargo.length + (s.cargo_more || 0) === 1 ? 'issue' : 'issues'}${s.cargo_more ? ` (the ${s.cargo_more} oldest not listed)` : ''}`],
    items: issueItems(v, cargo).map(it => ({ ...it, label: label.get(it.number) })), links: envLinks(v, s) };
} });

/* ---------- Labels: each village's name over its sign, its version under it ---------- */
registerLabels(sc => (sc.L.envs || []).map(e => {
  const ver = versionOf(e.s), when = agoWords(e.s.deployed_at);
  return { x: e.x + e.w / 2, y: sc.L.canalY + 15, anchor: 'top', text: e.s.name, sub: ver ? `${ver}${when ? ' · ' + when : ''}` : 'unknown', width: e.w - 8 };
}), 25);

/* ---------- The mini-map ----------
 * A strip over the map with a box per village, coloured by health, and a dot
 * per ship under sail, shown only when the row is wider than the panel. The
 * part in view is framed; a box scrolls to its village. */
const miniBox = document.getElementById('minimap'), miniVp = document.getElementById('viewport');
export function scrollToVillage(x) {
  const s = cv.getBoundingClientRect().width / cv.width;
  miniVp.scrollTo({ left: Math.max(0, x * s - 8), behavior: state.reduce ? 'auto' : 'smooth' });
}
function placeMini() {
  // The row's own village: the one shown, or a world's dev village when its platform is in the row.
  const row = shownScenes().find(([x, ox]) => ox === 0 && x.L.envs.length), sc = row && row[0], lay = sc && sc.L;
  const wide = lay && lay.envs.length && cv.getBoundingClientRect().width > miniVp.clientWidth + 2;
  miniBox.hidden = !wide;
  if (!wide) { miniBox.innerHTML = ''; return; }
  const s = cv.getBoundingClientRect().width / cv.width, from = miniVp.scrollLeft / s, to = from + miniVp.clientWidth / s;
  const sailing = (sc.v.pipeline.voyages || []).filter(vo => vo.state === 'sailing' || vo.state === 'loading');
  const boxes = [{ name: sc.v.project, x: 0, w: lay.devW, health: 'ok' }, ...lay.envs.map(e => ({ name: e.s.name, x: e.x, w: e.w, health: healthOf(e.s) }))];
  const html = boxes.map(b => {
    const inView = b.x < to && b.x + b.w > from, ships = sailing.filter(vo => vo.to === b.name).length;
    return `<button class="mm${inView ? ' in' : ''}" data-x="${b.x}" style="--hc:${HEALTH_COL[b.health]}" aria-label="Scroll to ${esc(b.name)}">${esc(b.name)}${'<i></i>'.repeat(ships)}</button>`;
  }).join('');
  if (miniBox.drawn === html) return;
  miniBox.innerHTML = html; miniBox.drawn = html;
  miniBox.querySelectorAll('button').forEach(b => b.addEventListener('click', () => scrollToVillage(+b.dataset.x)));
}
miniVp.addEventListener('scroll', placeMini, { passive: true });
window.addEventListener('resize', placeMini);
registerOverlay(placeMini, 40);
