/* ---------- All swarms at a glance (#481) ----------
 * `/village#*`: every swarm on one map, each an island in the sea, its shore
 * in one health colour (red: something waits on you, the build is broken or
 * a deploy failed; amber: a house blocked or stalled; green: all well), its
 * working, idle and blocked-or-stalled houses as three small houses with a
 * count under each, and a storm over it while its build is red. Its name and
 * why it is red or amber are written over it. Clicking an island, or Enter
 * on its keyboard target, opens that village (or world).
 *
 * The overview is not a village: it is not in shownScenes(), so the village
 * layers, labels and hit tests leave it alone. worlds.js's draw() hands its
 * frame here; data.js's select() shows it. What each island says and where
 * it sits is decided in overviewmodel.js. The screensaver can show it between
 * swarms or alone (`overview=1`, screensaver.js). */
import { state } from './state.js';
import { P, R, STATE, hash } from './palette.js';
import { px } from './art2x.js';
import { detail, prepare, toScene } from './detail.js';
import { cv, ctx, shownScale, sizeCanvas } from './layout.js';
import { numberTag, numberTagSize } from './scenery.js';
import { clouds, rain } from './weatherart.js';
import { registerOverlay } from './registry.js';
import { esc, fit } from './panels.js';
import { islandAt, overviewGrid, overviewIslands } from './overviewmodel.js';

export const SHORE = { red: '#e02828', amber: '#f0a020', green: '#38c848' };
const HEALTH_WORDS = { red: 'needs you', amber: 'blocked or stalled', green: 'all well' };
const OV_STORM = { style: 'storm', cloud: '#50506a', cloudD: '#30303f', rain: P.glass };

/* What the overview shows now: its islands and where each sits. */
export function overviewNow() {
  const islands = overviewIslands(state.villages, state.worlds);
  return { islands, grid: overviewGrid(islands.length) };
}
/* Size the map for the overview; true when its size changed. */
export function sizeOverview() {
  const { grid } = overviewNow();
  return sizeCanvas(grid.W, grid.H);
}

/* A house, 16 × 16 scene pixels at (x, y), big enough to read across a
 * room: a lit window when working, a dark one when idle, an orange wall when
 * blocked or stalled. Drawn one scene pixel per pixel (detail's d = 1). */
function ovHouse(c, x, y, kind) {
  const wall = kind === 'blocked' ? STATE.blocked : P.plank, glass = kind === 'working' ? P.lamp : P.dark;
  detail(c, x, y, g => {
    // The roof: three steps, outlined.
    px(g, 6, 0, 4, 2, P.o); px(g, 4, 2, 8, 2, P.o); px(g, 2, 4, 12, 2, P.o); px(g, 0, 6, 16, 2, P.o);
    px(g, 7, 1, 2, 1, P.red); px(g, 5, 3, 6, 1, P.red); px(g, 3, 5, 10, 1, P.red); px(g, 1, 7, 14, 1, P.redD);
    // The walls, a window and a door.
    px(g, 2, 8, 12, 8, P.o); px(g, 3, 8, 10, 7, wall);
    px(g, 4, 10, 3, 3, glass); px(g, 9, 11, 3, 4, P.woodD);
    if (kind === 'blocked') { px(g, 5, 10, 1, 2, P.white); }
  }, 1);
}

/* One island: its shore, its ground, three houses with their counts, and
 * the storm over it while its build is red. */
function drawIsland(c, isl, cell, t) {
  const { x, y, w, h } = cell, wx = (state.T && state.T.weather) || OV_STORM;
  R(c, x - 3, y - 3, w + 6, h + 6, P.o);
  R(c, x - 2, y - 2, w + 4, h + 4, SHORE[isl.health]);
  state.T.ground(c, x, y, w, h);
  const figs = [['working', isl.working], ['idle', isl.idle], ['blocked', isl.blocked]];
  // A tree or two on the shore, the same on every frame.
  [[6, 8], [w - 16, 10]].forEach(([tx, ty], k) => { if (hash(x + k, y + tx) < 0.7) state.T.tree(c, x + tx, y + ty); });
  const fy = y + h - 34, step = Math.floor(w / 3);
  figs.forEach(([kind, n], k) => {
    const fx = x + k * step + ((step - 16) >> 1);
    ovHouse(c, fx, fy, kind);
    const tw = numberTagSize(n).w;
    numberTag(c, fx + 8 - (tw >> 1) + 1, fy + 20, n, kind === 'blocked' && n ? STATE.blocked : P.parch);
  });
  if (isl.storm) {
    clouds(c, x, y + 2, w, wx, t);
    if (wx.rain) rain(c, x, y + 8, w, h - 10, Math.floor(w / 4), wx.rain, t);
  }
}

/* One frame of the overview. */
export function drawOverview(t) {
  const { islands, grid } = overviewNow();
  if (sizeCanvas(grid.W, grid.H)) { fit(); placeOverview(); }
  prepare(ctx);
  state.T.channel(ctx, 0, 0, grid.W, grid.H, t, 1);
  islands.forEach((isl, k) => drawIsland(ctx, isl, grid.cells[k], t));
}

/* Open an island's village or world, as the picker would. */
export function openSwarm(name) {
  location.hash = encodeURIComponent(name) + (state.screensaver ? '&screensaver' : '');
}
cv.addEventListener('click', e => {
  if (!state.view || !state.view.overview) return;
  const r = cv.getBoundingClientRect();
  const x = toScene((e.clientX - r.left) / r.width * cv.width), y = toScene((e.clientY - r.top) / r.height * cv.height);
  const { islands, grid } = overviewNow(), k = islandAt(grid.cells, x, y);
  if (k !== null) openSwarm(islands[k].name);
});

/* Over the map: each island's name and what is wrong, and a keyboard target
 * per island. Kept in their own boxes, so the village's labels and targets
 * (labels.js, dialogs.js) never touch them. */
const ovViewport = document.getElementById('viewport');
const obox = document.createElement('div'), okeys = document.createElement('div');
obox.className = 'alabels ovlabels'; obox.hidden = true;
okeys.className = 'hitkeys ovkeys'; okeys.hidden = true;
ovViewport.append(obox, okeys);

export function placeOverview() {
  const on = !!(state.view && state.view.overview) && !cv.hidden;
  obox.hidden = okeys.hidden = !on;
  if (!on) { obox.innerHTML = ''; okeys.innerHTML = ''; return; }
  const { islands, grid } = overviewNow();
  const r = cv.getBoundingClientRect(), vr = ovViewport.getBoundingClientRect(), s = shownScale();
  const ox0 = r.left - vr.left + ovViewport.scrollLeft, oy0 = r.top - vr.top + ovViewport.scrollTop;
  obox.style.setProperty('--lf', Math.round(Math.max(10, Math.min(18, 3 + 2.5 * s))) + 'px');
  obox.innerHTML = islands.map((isl, k) => {
    const c = grid.cells[k];
    return `<div class="al ov ov-${isl.health}" style="left:${ox0 + (c.x + c.w / 2) * s}px;top:${oy0 + (c.y + 4) * s}px;max-width:${(c.w + 8) * s}px;border-color:${SHORE[isl.health]}">` +
      `${esc(isl.title)}<small>${esc(isl.why.join(' · ') || HEALTH_WORDS.green)}</small></div>`;
  }).join('');
  const focused = document.activeElement && okeys.contains(document.activeElement) ? document.activeElement.dataset.name : null;
  okeys.innerHTML = islands.map((isl, k) => {
    const c = grid.cells[k];
    const words = [isl.title, HEALTH_WORDS[isl.health], ...isl.why, `${isl.working} working, ${isl.idle} idle, ${isl.blocked} blocked or stalled`].join(': ');
    return `<button class="hk" data-name="${esc(isl.name)}" aria-label="${esc(words)}" style="left:${ox0 + c.x * s}px;top:${oy0 + c.y * s}px;width:${c.w * s}px;height:${c.h * s}px"></button>`;
  }).join('');
  [...okeys.children].forEach(b => {
    b.addEventListener('click', () => openSwarm(b.dataset.name));
    if (b.dataset.name === focused) b.focus({ preventScroll: true });
  });
}
registerOverlay(placeOverview, 25);
