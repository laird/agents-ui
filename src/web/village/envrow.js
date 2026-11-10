/* ---------- The environment row ----------
 * Under the dev village, along a river of its own across the bottom of the
 * map (#445), one village per deploy environment, left to right in pipeline
 * order, production right-most (docs/environment-row.md).
 * Each has a sign with the version it runs, a warehouse holding what it runs
 * (its cargo, grouped under the banners of the features it belongs to), a
 * house per feature it runs flying that feature's colour, a dock each side
 * on the river, and a health mark: smoke and a red flag when its newest
 * deployment failed, a fire for an open incident, a "?" when nothing says
 * what it runs. Its look follows its rank: a town, and the capital
 * (production) with a keep. One label names it, at its warehouse, with the
 * version it runs (#440). A world's platform stands in the slot
 * of the environment it runs (worlds.js): that slot has no houses or keep of
 * its own, and its sign stands by the warehouse, under the platform. The
 * ships between them are voyages.js; the strip of villages over the map is
 * the mini-map, at the end. */
import { state } from './state.js';
import { F, P, R, blink, featureColours, shadow } from './palette.js';
import { puffs } from './sprites/houses.js';
import { badge, badgeSize, bubble } from './scenery.js';
import { MODERN, M } from './themes/modern.js';
import { FUTURE } from './themes/future.js';
import { registerHit, registerLabels, registerLayer, registerOverlay, tap } from './registry.js';
import { cv, shownScale, shownScenes } from './layout.js';
import { issueItems, isSha, link, repoUrl } from './dialogs.js';
import { sourceWords } from './ship.js';
import { esc } from './panels.js';
import { agoWords, envHouses, envLabel, versionOf } from './envlabels.js';
import { showFeature } from './features.js';

/* Where things stand in an environment village, from its left edge and the canal. */
export const SHED = { dx: 30, w: 60, h: 32, up: 44 }, ENV_DOCK = 20, BAY_COLS = 9, BAY_ROWS = 3;

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
export function envShed(c, x, y, w, h) {
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
export function envSign(c, x, y, face) {
  const [frame, board, ink] = state.T.sign || [P.woodD, P.parch, P.woodD];
  R(c, x + 6, y + 8, 2, 12, frame);
  R(c, x, y, 15, 9, P.o); R(c, x + 1, y + 1, 13, 7, face || board);
  R(c, x + 3, y + 3, 9, 1, ink); R(c, x + 3, y + 5, 6, 1, ink);
}
const healthOf = s => (s.health && s.health.state) || (s.source === 'unknown' ? 'unknown' : 'ok');
const HEALTH_COL = { ok: '#38c848', failed: '#e02828', incident: '#f07020', unknown: '#8088a8' };

/* The row's river, the width of the map. */
registerLayer({ z: 6, draw(c, { lay }, t) {
  if (!lay.envs || !lay.envs.length) return;
  state.T.channel(c, 0, lay.envCy, lay.W, 14, t, 0.6);
  tap(0, lay.envCy, lay.W, 14, { kind: 'canal' });
} });
/* The villages: ground, road, docks, houses by rank, the sign and the warehouse. */
registerLayer({ z: 60, draw(c, { v, lay }, t) {
  lay.envs.forEach(e => {
    const { x, w, s, cy, top } = e, health = healthOf(s), signY = cy - 30;
    if (e.platform) tap(x, cy - SHED.up - 4, w, SHED.up + 4, { kind: 'env', i: e.i });
    else tap(x, top, w, cy - top, { kind: 'env', i: e.i });
    state.T.road(c, x + ENV_DOCK - 2, cy - 10, w - 2 * ENV_DOCK + 4, 5);
    state.T.dock(c, x, cy - 7, ENV_DOCK); state.T.dock(c, x + w - ENV_DOCK, cy - 7, ENV_DOCK);
    tap(x, cy - 7, ENV_DOCK, 7, { kind: 'env', i: e.i });
    // A house per feature it runs, flying its colour (#440), beside the
    // warehouse so the row stays low (#445); the capital has a keep too.
    // A platform's houses are its workloads.
    if (!e.platform) {
      const { houses, more } = envHouses(cargoGroups(v, s.cargo || []), e.rank);
      houses.forEach((h, k) => {
        const hx = x + SHED.dx + SHED.w + 4 + k * 18;
        state.T.B.cottage(c, hx, cy - 30, 'idle', t);
        R(c, hx + 2, cy - 38, 1, 9, P.woodD); R(c, hx + 3, cy - 38 + (blink(t + k * 250, 900) ? 1 : 0), 4, 3, h.col);
        tap(hx, cy - 38, 16, 24, { kind: 'envhouse', f: h.f.number });
      });
      if (more > 0) badge(c, x + SHED.dx + SHED.w + 4 + houses.length * 18, cy - 34, more);
    }
    if (e.rank === 'capital' && !e.platform) envKeep(c, x + w - 18, cy - 42, 32, HEALTH_COL[health], t);
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
  lay.envs.forEach(e => {
    const sx = e.x + SHED.dx, sy = e.cy - SHED.up, { slots, more } = baySlots(cargoGroups(v, e.s.cargo || []), e.s.cargo_more || 0);
    slots.forEach((sl, k) => {
      const x = sx + 3 + (k % BAY_COLS) * 6, y = sy + 10 + Math.floor(k / BAY_COLS) * 7;
      if (sl.banner) { R(c, x + 1, y, 1, 6, P.woodL); R(c, x + 2, y, 3, 3, sl.banner.col); return; }
      state.T.crate(c, x, y + 1, 'workshop', null, true, sl.n); tap(x, y + 1, 5, 5, { kind: 'crate', n: sl.n, at: 'env', i: e.i });
    });
    // Right-aligned to the shed, so even three digits stay on it (#436).
    if (more > 0) {
      const { w, h } = badgeSize(more), bx = sx + SHED.w - w + 1;
      badge(c, bx, sy - 3, more); tap(bx - 1, sy - 5, w, h, { kind: 'env', i: e.i });
    }
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

/* A feature's house: the feature's dialog. */
registerHit({ kind: 'envhouse', act(o, sc) { const v = sc.v, f = (v.features || []).find(f => f.number === o.f); if (f) showFeature(f, v); return !!f; } });

/* ---------- Labels: one per village, over its warehouse, with its version (#440) ---------- */
registerLabels(sc => (sc.L.envs || []).map(e => {
  const { text, sub } = envLabel(e.s);
  return { x: e.x + SHED.dx + SHED.w / 2, y: e.cy - SHED.up - 3, anchor: 'bottom', text, sub, width: e.w - 8 };
}), 25);

/* ---------- The mini-map ----------
 * A strip over the map with a box per village, coloured by health, and a dot
 * per ship under sail, shown only when the map is bigger than the panel
 * (zoomed in). The part in view is framed; a box scrolls to its village,
 * the dev village up top or one of the row's under it. */
const miniBox = document.getElementById('minimap'), miniVp = document.getElementById('viewport');
export function scrollToVillage(x, y = 0) {
  const s = shownScale();
  miniVp.scrollTo({ left: Math.max(0, x * s - 8), top: Math.max(0, y * s - 8), behavior: state.reduce ? 'auto' : 'smooth' });
}
function placeMini() {
  // The row's own village: the one shown, or a world's dev village when its platform is in the row.
  const row = shownScenes().find(([x, ox]) => ox === 0 && x.L.envs.length), sc = row && row[0], lay = sc && sc.L;
  const shown = cv.getBoundingClientRect();
  const wide = lay && lay.envs.length && (shown.width > miniVp.clientWidth + 2 || shown.height > miniVp.clientHeight + 2);
  miniBox.hidden = !wide;
  if (!wide) { miniBox.innerHTML = ''; return; }
  const s = shownScale(), from = miniVp.scrollLeft / s, to = from + miniVp.clientWidth / s;
  const fromY = miniVp.scrollTop / s, toY = fromY + miniVp.clientHeight / s;
  const sailing = (sc.v.pipeline.voyages || []).filter(vo => vo.state === 'sailing' || vo.state === 'loading');
  const boxes = [{ name: sc.v.project, x: 0, y: 0, w: lay.devW, h: lay.devH, health: 'ok' },
    ...lay.envs.map(e => ({ name: e.s.name, x: e.x, y: e.top, w: e.w, h: lay.H - e.top, health: healthOf(e.s) }))];
  const html = boxes.map(b => {
    const inView = b.x < to && b.x + b.w > from && b.y < toY && b.y + b.h > fromY, ships = sailing.filter(vo => vo.to === b.name).length;
    return `<button class="mm${inView ? ' in' : ''}" data-x="${b.x}" data-y="${b.y}" style="--hc:${HEALTH_COL[b.health]}" aria-label="Scroll to ${esc(b.name)}">${esc(b.name)}${'<i></i>'.repeat(ships)}</button>`;
  }).join('');
  if (miniBox.drawn === html) return;
  miniBox.innerHTML = html; miniBox.drawn = html;
  miniBox.querySelectorAll('button').forEach(b => b.addEventListener('click', () => scrollToVillage(+b.dataset.x, +b.dataset.y)));
}
miniVp.addEventListener('scroll', placeMini, { passive: true });
window.addEventListener('resize', placeMini);
registerOverlay(placeMini, 40);
