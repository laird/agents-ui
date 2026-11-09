/* ---------- Worlds ----------
 * A world stacks the platform village (what runs) over the dev village
 * (what builds it). Issues the platform raises leave the building they
 * came from and float down the river on the left to the dev village's
 * docks; releases leave the dev village's harbour and climb the road on
 * the right to the platform, until its workers run them. The platform is
 * drawn in the other style, so the two read apart at a glance. */
import { state } from './state.js';
import { F, P, R, blink } from './palette.js';
import { puffs } from './sprites/houses.js';
import { MEDIEVAL } from './themes/medieval.js';
import { M, MODERN } from './themes/modern.js';
import { FUTURE } from './themes/future.js';
import { drawVillage, registerCounter } from './registry.js';
import { SIDE, ctx, cv, otherTheme, porterPos, worldGeom } from './layout.js';
import { SAIL } from './ship.js';
import { fit } from './panels.js';

function offscreen(sc) {
  if (!sc.canvas) { sc.canvas = document.createElement('canvas'); sc.cx = sc.canvas.getContext('2d'); }
  if (sc.canvas.width !== sc.L.W || sc.canvas.height !== sc.L.H) { sc.canvas.width = sc.L.W; sc.canvas.height = sc.L.H; }
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
function originPoint(Ps, id) {
  const i = Ps.v.houses.findIndex(h => h.id === id), lot = i >= 0 ? Ps.L.lots[i] : null;
  return lot ? [SIDE + lot.x + 8, lot.y + 8] : [12, Ps.L.top - 6];
}
function drawWorld(t, w) {
  const Ps = state.scenes[w.platform], Ds = state.scenes[w.dev];
  if (!Ps || !Ps.v || !Ds || !Ds.v) return;
  const g = worldGeom(Ps, Ds), st = state.worldState[w.name] || { floaters: [], upgradedAt: -1e9 };
  if (cv.width !== g.W || cv.height !== g.H) { cv.width = g.W; cv.height = g.H; fit(); }
  const devTheme = state.T;
  state.T.ground(ctx, 0, 0, g.W, g.H);

  state.T = otherTheme();
  state.T.ground(ctx, 0, 0, g.W, Ps.L.H);
  drawVillage(offscreen(Ps), Ps, t);
  ctx.drawImage(Ps.canvas, SIDE, 0);
  // Platform buildings with an open incident against them are on fire.
  w.damaged.forEach(id => {
    const i = Ps.v.houses.findIndex(h => h.id === id), lot = Ps.L.lots[i];
    if (!lot) return;
    const x = SIDE + lot.x, y = lot.y;
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
    const lot = Ps.L.lots[i]; if (lot && blink(t + i * 90, 160)) R(ctx, SIDE + lot.x + 2 + (i * 5) % 12, lot.y - 2, 1, 1, P.gold);
  });
  state.T = devTheme;

  drawVillage(offscreen(Ds), Ds, t);
  ctx.drawImage(Ds.canvas, SIDE, g.devTop);

  // The river down: from the platform's gate to the dev village's river.
  downstream(ctx, 5, g.srcY, 14, g.devTop + 11 - g.srcY, t);
  R(ctx, 2, g.srcY - 8, 20, 8, P.o); R(ctx, 3, g.srcY - 7, 18, 6, state.T === MODERN ? M.concrete : state.T === FUTURE ? F.hullD : P.stone); R(ctx, 9, g.srcY - 6, 6, 5, P.dark);
  // The road up: from the dev village's harbour to the platform's main road.
  // With an environment row it leaves from the end of the row, not across it.
  const ry = g.devTop + Ds.L.canalY + 4, out = SIDE + ((Ds.L.envs || []).length ? Ds.L.W - 4 : Ds.L.castleX + 34);
  state.T.road(ctx, g.W - 20, g.roadY - 3, 16, ry - g.roadY + 9);
  state.T.road(ctx, out + 2, ry, g.W - 20 - (out + 2), 6);
  const roadEnd = SIDE + Ps.L.cx0 + Ps.L.vw - 4;
  state.T.road(ctx, roadEnd, g.roadY - 3, g.W - 20 - roadEnd, 6);

  // Released work on its way up, until the platform's workers run it.
  // The cart takes over from the ship: it sets off from the harbour as the
  // ship sails off the dev village's edge, and none while the ship is still in sight.
  const sailed = Ds.voyage ? Ds.voyage.t0 + SAIL : 0;
  if (w.shipping.length && t >= sailed) {
    const pts = [[out, ry], [g.W - 12, ry], [g.W - 12, g.roadY], [roadEnd, g.roadY]];
    const len = pts.slice(1).reduce((n, p, k) => n + Math.hypot(p[0] - pts[k][0], p[1] - pts[k][1]), 0);
    const d = state.reduce ? len / 2 : ((t - sailed) / 30) % len;
    const at = porterPos({ t0: 0, speed: 1, pts }, d) || pts[0];
    cart(ctx, at[0] - 7, at[1] - 6, w.shipping.length);
  }
  // New incidents leave their building and float down to the docks.
  st.floaters = st.floaters.filter(f => {
    const [ox, oy] = originPoint(Ps, f.origin);
    const pts = [[ox, oy], [12, oy], [12, g.devTop + 6], [SIDE + 30, g.devTop + 6]];
    const at = porterPos({ t0: f.t0, speed: 0.04, pts }, t);
    if (!at) return false;
    state.T.crate(ctx, at[0] - 2, at[1] - 2, 'bug', 'P0');
    return true;
  });
}
/* One frame of what is shown: a village, or a world. */
export function draw(t) {
  const view = state.view;
  if (!view) return;
  ctx.imageSmoothingEnabled = false;
  if (view.world) { const w = state.worlds.find(x => x.name === view.world); if (w) drawWorld(t, w); return; }
  const sc = state.scenes[view.village];
  if (sc && sc.v) drawVillage(ctx, sc, t);
}

/* World counters follow the loop: raised, being fixed, released, running. */
const short = s => (s ? s.slice(0, 7) : '—');
const busy = sc => (sc && sc.v ? sc.v.houses.filter(h => h.status === 'working').length : 0);
registerCounter({ plane: 'world', order: 10, render: w => ['Incidents', 'raised by the platform', w.incidents.filter(i => !i.in_progress).length] });
registerCounter({ plane: 'world', order: 20, render: w => ['Being fixed', `claimed in ${w.dev}`, w.incidents.filter(i => i.in_progress).length] });
registerCounter({ plane: 'world', order: 30, render: w => ['Dev village', 'houses working', busy(state.scenes[w.dev])] });
registerCounter({ plane: 'world', order: 40, render: w => ['Shipping', 'released, not yet running', w.shipping.length] });
registerCounter({ plane: 'world', order: 50, render: w => ['Release', w.landed === null ? 'platform reports no build IDs' : w.landed ? 'running on the platform' : 'rolling out', short(w.release)] });
registerCounter({ plane: 'world', order: 60, render: w => ['Platform', 'workstreams busy', busy(state.scenes[w.platform])] });
registerCounter({ plane: 'world', order: 70, render: w => ['Platform builds', 'reported by workers', w.platform_builds.map(short).join(' ') || '—'] });
