/* ---------- Worlds ----------
 * A world pairs a dev village (what builds) with the platform it builds
 * (what runs). The platform is the village of the environment it runs,
 * production unless the world names another (#407): it stands in that
 * slot of the environment row, its live workloads as the village's houses,
 * over the slot's version sign, warehouse and docks. Releases reach it as
 * the row's voyages; issues it raises leave the building they came from,
 * float down beside it to the row's river and along it, right to left, to
 * the dev village's docks. The platform is drawn in the other style, so the
 * two read apart at a glance. A dev village with no environments stacks the
 * platform over it, with a road up for releases on the right. */
import { state } from './state.js';
import { F, P, R, blink } from './palette.js';
import { puffs } from './sprites/houses.js';
import { MEDIEVAL } from './themes/medieval.js';
import { M, MODERN } from './themes/modern.js';
import { FUTURE } from './themes/future.js';
import { drawVillage } from './registry.js';
import { ctx, otherTheme, porterPos, sizeCanvas, worldGeom, worldScenes } from './layout.js';
import { DETAIL, prepare } from './detail.js';
import { SAIL } from './ship.js';
import { fit } from './panels.js';

function offscreen(sc) {
  if (!sc.canvas) { sc.canvas = document.createElement('canvas'); sc.cx = sc.canvas.getContext('2d'); }
  if (sc.canvas.width !== sc.L.W * DETAIL || sc.canvas.height !== sc.L.H * DETAIL) { sc.canvas.width = sc.L.W * DETAIL; sc.canvas.height = sc.L.H * DETAIL; }
  prepare(sc.cx);
  return sc.cx;
}
/* A channel that flows down: water in the village, a conveyor road in the district. */
function downstream(c, x, y, w, h, t) {
  const modern = state.T !== MEDIEVAL, fut = state.T === FUTURE;
  R(c, x, y, w, h, fut ? F.space : modern ? M.asphaltD : P.water);
  const edge = fut ? F.magentaD : modern ? M.concreteD : P.waterD;
  R(c, x, y, 1, h, edge); R(c, x + w - 1, y, 1, h, edge);
  const off = Math.floor(t / 70) % 8;
  for (let yy = y - 8 + off; yy < y + h; yy += 8) if (yy >= y) {
    if (modern) R(c, x + (w >> 1), yy, 1, 4, fut ? F.plasmaL : P.white);
    else { R(c, x + 3, yy, 1, 2, P.waterL); R(c, x + w - 5, yy + 4, 1, 2, P.waterL); }
  }
}
function cart(c, x, y, loads) {
  const modern = state.T !== MEDIEVAL;
  x = Math.round(x); y = Math.round(y);
  R(c, x, y, 14, 8, P.o); R(c, x + 1, y + 1, 12, 5, modern ? P.white : P.wood);
  if (modern) { R(c, x + 10, y - 3, 4, 4, P.o); R(c, x + 11, y - 2, 2, 2, M.glassL); }
  R(c, x + 2, y + 7, 3, 3, P.o); R(c, x + 9, y + 7, 3, 3, P.o);
  for (let k = 0; k < Math.min(loads, modern ? 1 : 2); k++) state.T.crate(c, x + 2 + k * 5, y - 4, 'workshop', null, true);
}
/* Fold a new world into its state: new incidents set off downriver, and the
 * platform lights up when the release lands. */
export function updateWorld(w, t) {
  const { worldState, scenes } = state;
  const st = worldState[w.name] || (worldState[w.name] = { seen: null, floaters: [], landed: undefined, upgradedAt: -1e9 });
  const now = new Set(w.incidents.map(i => i.number));
  if (st.seen) w.incidents.forEach(i => { if (!st.seen.has(i.number)) st.floaters.push({ n: i.number, origin: i.origin, t0: t }); });
  st.seen = now;
  if (st.landed === false && w.landed === true) {
    st.upgradedAt = t;
    if (scenes[w.platform]) scenes[w.platform].fireworks = t;
  }
  st.landed = w.landed;
}
/* Where an incident sets off: its building on the platform, else the platform's gate. */
function originPoint(Ps, id, g) {
  const i = Ps.v.houses.findIndex(h => h.id === id), lot = i >= 0 ? Ps.L.lots[i] : null;
  return lot ? [g.px + lot.x + 8, g.py + lot.y + 8] : [g.px + 12, g.py + Ps.L.top - 6];
}
/* The channel down from the platform's gate at x, and its sluice. */
function sluice(x, y0, y1, t) {
  if (y1 - y0 > 4) downstream(ctx, x, y0, 14, y1 - y0, t);
  R(ctx, x - 3, y0 - 8, 20, 8, P.o); R(ctx, x - 2, y0 - 7, 18, 6, state.T === MODERN ? M.concrete : state.T === FUTURE ? F.hullD : P.stone); R(ctx, x + 4, y0 - 6, 6, 5, P.dark);
}
function drawWorld(t, w) {
  const both = worldScenes(w);
  if (!both) return;
  const [Ps, Ds] = both, g = worldGeom(w, Ps, Ds), st = state.worldState[w.name] || { floaters: [], upgradedAt: -1e9 };
  if (sizeCanvas(g.W, g.H)) fit();
  prepare(ctx);
  const devTheme = state.T;
  state.T.ground(ctx, 0, 0, g.W, g.H);
  drawVillage(offscreen(Ds), Ds, t);
  ctx.drawImage(Ds.canvas, g.dx, g.devTop, Ds.L.W, Ds.L.H);

  state.T = otherTheme();
  if (!g.row) state.T.ground(ctx, 0, 0, g.W, Ps.L.H);
  drawVillage(offscreen(Ps), Ps, t);
  ctx.drawImage(Ps.canvas, g.px, g.py, Ps.L.W, Ps.L.H);
  // Platform buildings with an open incident against them are on fire.
  w.damaged.forEach(id => {
    const i = Ps.v.houses.findIndex(h => h.id === id), lot = Ps.L.lots[i];
    if (!lot) return;
    const x = g.px + lot.x, y = g.py + lot.y;
    R(ctx, x, y - 1, 16, 17, 'rgba(40,24,24,.35)');
    for (let k = 0; k < 3; k++) {
      puffs(ctx, x + 4 + k * 4, y - 6, t + k * 450, '#484858', '#282830');
      puffs(ctx, x + 5 + k * 4, y - 12, t + k * 450 + 900, '#585870', '#383848');
    }
    const f = blink(t, 140) ? 1 : 0;
    R(ctx, x + 4, y + 2 - f, 3, 3 + f, P.p0); R(ctx, x + 5, y + 1 - f, 1, 2, P.gold);
    R(ctx, x + 10, y + 3 + f, 3, 3 - f, P.p0); R(ctx, x + 11, y + 2 + f, 1, 2, P.gold);
  });
  if (t - st.upgradedAt < 2400) Ps.v.houses.forEach((h, i) => {
    const lot = Ps.L.lots[i]; if (lot && blink(t + i * 90, 160)) R(ctx, g.px + lot.x + 2 + (i * 5) % 12, g.py + lot.y - 2, 1, 1, P.gold);
  });
  state.T = devTheme;

  // Incidents float down beside the platform to the row's river, along it to
  // the map's left edge and up a stream there to the river by the dev
  // village's docks (in a stack: down its left edge), then to the docks.
  const ry = g.devTop + 6, rx = g.row ? g.e.x - 16 : 5, rowY = g.row ? g.e.cy + 4 : ry;
  if (g.row) {
    sluice(rx, g.srcY, g.e.cy, t);
    downstream(ctx, 0, 11, 6, g.e.cy - 11, t);
  } else sluice(rx, g.srcY, g.devTop + 11, t);
  if (!g.row) roadUp(t, w, g, Ps, Ds);
  st.floaters = st.floaters.filter(f => {
    const [ox, oy] = originPoint(Ps, f.origin, g);
    const pts = g.row ? [[ox, oy], [rx + 7, oy], [rx + 7, rowY], [3, rowY], [3, ry], [g.dx + 30, ry]]
      : [[ox, oy], [rx + 7, oy], [rx + 7, ry], [g.dx + 30, ry]];
    const at = porterPos({ t0: f.t0, speed: 0.04, pts }, t);
    if (!at) return false;
    state.T.crate(ctx, at[0] - 2, at[1] - 2, 'bug', 'P0');
    return true;
  });
}
/* A stack's road up: from the dev village's harbour to the platform's main
 * road, with released work on a cart until the platform's workers run it.
 * The cart takes over from the ship: it sets off from the harbour as the
 * ship sails off the dev village's edge, and none while the ship is still in sight. */
function roadUp(t, w, g, Ps, Ds) {
  const ry = g.devTop + Ds.L.canalY + 4, out = g.dx + Ds.L.castleX + 34;
  state.T.road(ctx, g.W - 20, g.roadY - 3, 16, ry - g.roadY + 9);
  state.T.road(ctx, out + 2, ry, g.W - 20 - (out + 2), 6);
  const roadEnd = g.px + Ps.L.cx0 + Ps.L.vw - 4;
  state.T.road(ctx, roadEnd, g.roadY - 3, g.W - 20 - roadEnd, 6);
  const sailed = Ds.voyage ? Ds.voyage.t0 + SAIL : 0;
  if (w.shipping.length && t >= sailed) {
    const pts = [[out, ry], [g.W - 12, ry], [g.W - 12, g.roadY], [roadEnd, g.roadY]];
    const len = pts.slice(1).reduce((n, p, k) => n + Math.hypot(p[0] - pts[k][0], p[1] - pts[k][1]), 0);
    const d = state.reduce ? len / 2 : ((t - sailed) / 30) % len;
    const at = porterPos({ t0: 0, speed: 1, pts }, d) || pts[0];
    cart(ctx, at[0] - 7, at[1] - 6, w.shipping.length);
  }
}
/* One frame of what is shown: a village, or a world. */
export function draw(t) {
  const view = state.view;
  if (!view) return;
  prepare(ctx);
  if (view.world) { const w = state.worlds.find(x => x.name === view.world); if (w) drawWorld(t, w); return; }
  const sc = state.scenes[view.village];
  if (sc && sc.v) drawVillage(ctx, sc, t);
}

/* The bar's line for a world. The map draws the incidents, the fixes, the
 * houses and what is shipping; what it can't draw is which release the
 * platform runs and the build IDs its workers report, so the line says so. */
const short = s => (s ? s.slice(0, 7) : '—');
export function worldWords(w) {
  const release = w.landed === null ? 'platform reports no build IDs' : w.landed ? 'running on the platform' : 'rolling out';
  const builds = (w.platform_builds || []).map(short).join(' ');
  return `· ${w.dev} builds ${w.platform} · release ${short(w.release)} ${release}${builds ? ` · platform builds ${builds}` : ''}`;
}
