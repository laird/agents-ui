/* ---------- A valley of related repos (#510) ----------
 * `/village?valley=<group>`: a group of repos that depend on each other
 * (~/.agents-ui/group.json, `/api/valley`) as one valley. The repos the
 * others build on sit upstream (west), their consumers downstream (east),
 * a river runs along each `package` edge and a road along each `api` edge.
 * Each repo is a small village: its shore in its health colour, three houses
 * with its working, idle and blocked counts, a label with its backlog and
 * release. A repo with no running swarm is drawn dormant, faded. Clicking a
 * repo (or Enter on its keyboard target) opens its own village.
 *
 * Like the overview (#481), the valley is not a village scene: draw()
 * (worlds.js) hands it its frame and data.js's select() shows it. Where
 * everything sits is decided in valleymodel.js. */
import { state } from './state.js';
import { P, R, STATE } from './palette.js';
import { prepare, toScene } from './detail.js';
import { cv, ctx, shownScale, sizeCanvas } from './layout.js';
import { numberTag, numberTagSize } from './scenery.js';
import { registerOverlay } from './registry.js';
import { esc, fit } from './panels.js';
import { SHORE, ovHouse } from './overview.js';
import { gridAt, gridWords, repoWords, valleyShipAt, shipWords, valleyAt, valleyLayout, valleyMissing, valleyName, valleyShips, valleyWords } from './valleymodel.js';
import { drawGrid, gridKeys, gridLabels } from './valleygrid.js';
import { drawShip, shipKeys, shipLabels } from './valleyships.js';
import { openInfo } from './dialogs.js';

const VALLEY_SHORE = { ...SHORE, dormant: '#808890' };
const VALLEY_HEALTH = { red: 'needs you', amber: 'blocked or stalled', green: 'all well', dormant: 'dormant' };
let valleyResp = null; // the last /api/valley answer

/* The group the URL names, when the page is a valley. */
export const valleyWanted = () => valleyName(location.search);
const shownGroup = () => {
  const name = state.view && state.view.valley;
  return name && valleyResp ? (valleyResp.groups || []).find(g => g.name === name) || null : null;
};

/* Fetch the valley now and then; the villages' own poll (data.js) goes on as ever. */
async function pollValley() {
  try {
    const res = await fetch('/api/valley');
    if (res.ok) {
      valleyResp = await res.json();
      if (state.view && state.view.valley) selectValley(state.view.valley);
    }
  } catch { /* data.js says when the server can't be reached */ }
  setTimeout(pollValley, 5000);
}
if (valleyWanted()) pollValley();

/* Show the valley `name`: the map sized to it, or why there is none. */
export function selectValley(name) {
  state.view = { valley: name };
  const g = shownGroup(), empty = document.getElementById('empty');
  const missing = valleyMissing(valleyResp, name);
  empty.hidden = !missing;
  if (missing) empty.textContent = missing;
  cv.hidden = !g;
  document.getElementById('workflow').textContent = g ? '· ' + valleyWords(g, valleyResp.errors) : '';
  if (g) {
    const { W, H } = valleyLayout(g);
    sizeCanvas(W, H);
  }
  fit();
  if (state.reduce) drawValley(0);
  placeValley();
}

/* A leg of a river or road, `thick` scene pixels across, from (x0, y0) to (x1, y1). */
function leg(x0, y0, x1, y1, thick, fill) {
  const x = Math.min(x0, x1) - (thick >> 1), y = Math.min(y0, y1) - (thick >> 1);
  fill(x, y, Math.abs(x1 - x0) + thick, Math.abs(y1 - y0) + thick);
}

/* One repo: its shore, its ground, three houses with their counts. */
function drawRepo(c, r, cell) {
  const { x, y, w, h } = cell, s = r.summary || {};
  R(c, x - 3, y - 3, w + 6, h + 6, P.o);
  R(c, x - 2, y - 2, w + 4, h + 4, VALLEY_SHORE[s.health] || VALLEY_SHORE.dormant);
  state.T.ground(c, x, y, w, h);
  if (r.dormant) c.globalAlpha = 0.45;
  const figs = [['working', s.working || 0], ['idle', s.idle || 0], ['blocked', s.blocked || 0]];
  const fy = y + h - 30, step = Math.floor(w / 3);
  figs.forEach(([kind, n], k) => {
    const fx = x + k * step + ((step - 16) >> 1);
    ovHouse(c, fx, fy, kind);
    numberTag(c, fx + 8 - (numberTagSize(n).w >> 1) + 1, fy + 18, n, kind === 'blocked' && n ? STATE.blocked : P.parch);
  });
  c.globalAlpha = 1;
}

/* One frame of the valley: the ground, the roads, the rivers, the repos. */
export function drawValley(t) {
  const g = shownGroup();
  if (!g) return;
  const lay = valleyLayout(g);
  if (sizeCanvas(lay.W, lay.H)) { fit(); placeValley(); }
  prepare(ctx);
  state.T.ground(ctx, 0, 0, lay.W, lay.H);
  lay.roads.forEach(p => p.slice(1).forEach(([x, y], k) => leg(p[k][0], p[k][1], x, y, 6, (lx, ly, lw, lh) => state.T.road(ctx, lx, ly, lw, lh))));
  lay.rivers.forEach(p => p.slice(1).forEach(([x, y], k) => leg(p[k][0], p[k][1], x, y, 8, (lx, ly, lw, lh) => state.T.channel(ctx, lx, ly, lw, lh, t, 1))));
  valleyShips(lay, state.reduce ? 0 : t).forEach(sh => drawShip(ctx, sh)); // releases flowing downstream (#511)
  g.repos.forEach((r, k) => drawRepo(ctx, r, lay.cells[k]));
  drawGrid(ctx, lay, state); // the api-joined repos by environment (#512)
}

/* Open a repo's own village; a repo with no village has nothing to open. */
function openRepo(r) {
  if (!r || !r.project) return;
  // The rest of the query (the theme, the screensaver) goes along; the valley doesn't.
  const q = new URLSearchParams(location.search);
  q.delete('valley');
  location.href = location.pathname + (String(q) ? '?' + q : '') + '#' + encodeURIComponent(r.project);
}
cv.addEventListener('click', e => {
  const g = state.view && state.view.valley && shownGroup();
  if (!g) return;
  const rect = cv.getBoundingClientRect();
  const x = toScene((e.clientX - rect.left) / rect.width * cv.width), y = toScene((e.clientY - rect.top) / rect.height * cv.height);
  const lay = valleyLayout(g), ship = valleyShipAt(valleyShips(lay, state.reduce ? 0 : performance.now()), x, y);
  if (ship) { openInfo(shipWords(ship)); return; }
  const cell = gridAt(lay, x, y);
  if (cell) { openInfo(gridWords(cell.cell)); return; }
  const k = valleyAt(lay.cells, x, y);
  if (k !== null) openRepo(g.repos[k]);
});

/* Over the map: each repo's name and summary, and a keyboard target per
 * repo, in boxes of their own (as the overview's). */
const vViewport = document.getElementById('viewport');
const vbox = document.createElement('div'), vkeys = document.createElement('div');
vbox.className = 'alabels ovlabels'; vbox.hidden = true;
vkeys.className = 'hitkeys ovkeys'; vkeys.hidden = true;
vViewport.append(vbox, vkeys);

export function placeValley() {
  const g = shownGroup(), on = !!g && !cv.hidden;
  vbox.hidden = vkeys.hidden = !on;
  if (!on) { vbox.innerHTML = ''; vkeys.innerHTML = ''; return; }
  const lay = valleyLayout(g), { cells } = lay, ships = valleyShips(lay, 0);
  const r = cv.getBoundingClientRect(), vr = vViewport.getBoundingClientRect(), s = shownScale();
  const ox0 = r.left - vr.left + vViewport.scrollLeft, oy0 = r.top - vr.top + vViewport.scrollTop;
  vbox.style.setProperty('--lf', Math.round(Math.max(10, Math.min(16, 3 + 2.5 * s))) + 'px');
  vbox.innerHTML = g.repos.map((repo, k) => {
    const c = cells[k], words = repoWords(repo), health = (repo.summary || {}).health;
    return `<div class="al ov ov-${esc(health || 'dormant')}" aria-hidden="true" style="left:${ox0 + (c.x + c.w / 2) * s}px;top:${oy0 + (c.y + 2) * s}px;max-width:${(c.w + 8) * s}px;border-color:${VALLEY_SHORE[health] || VALLEY_SHORE.dormant}">` +
      `${esc(words.text)}<small>${esc(words.sub)}</small></div>`;
  }).join('') + shipLabels(ships, ox0, oy0, s) + gridLabels(lay, ox0, oy0, s);
  const keep = new Set();
  g.repos.forEach((repo, k) => {
    const c = cells[k], words = repoWords(repo);
    let b = [...vkeys.children].find(x => x.dataset.name === repo.name);
    if (!b) {
      b = document.createElement('button'); b.className = 'hk'; b.dataset.name = repo.name;
      b.addEventListener('click', () => openRepo((shownGroup() || { repos: [] }).repos.find(x => x.name === b.dataset.name)));
    }
    if (vkeys.children[k] !== b) vkeys.insertBefore(b, vkeys.children[k] || null);
    keep.add(b);
    b.disabled = !repo.project;
    const health = VALLEY_HEALTH[(repo.summary || {}).health];
    b.setAttribute('aria-label', `${words.text}: ${health && !repo.dormant ? health + ', ' : ''}${words.sub}${repo.project ? '. Open its village' : ''}`);
    Object.assign(b.style, { left: ox0 + c.x * s + 'px', top: oy0 + c.y * s + 'px', width: c.w * s + 'px', height: c.h * s + 'px' });
  });
  shipKeys(vkeys, ships, ox0, oy0, s, sh => openInfo(shipWords(sh))).forEach(b => keep.add(b));
  gridKeys(vkeys, lay, ox0, oy0, s, c => openInfo(gridWords(c))).forEach(b => keep.add(b));
  [...vkeys.children].forEach(b => { if (!keep.has(b)) b.remove(); });
}
registerOverlay(placeValley, 26);
