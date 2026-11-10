/* ---------- Layout ----------
 * Where everything stands on a village's map, the ground it stands on, and
 * where each village shown sits on the canvas (alone, or both halves of a
 * world). */
import { state } from './state.js';
import { featureColours, hash, shadow } from './palette.js';
import { THEMES } from './themes/registry.js';
import { registerLayer } from './registry.js';

export const cv = document.getElementById('map'), ctx = cv.getContext('2d');

export const LW = 20, LH = 26, RD = 6, DOCK_W = 64;
/* The merge gate's band between the last row of houses and the quay, and
 * the most inspection tables its gatehouse has room for. */
export const GATE_H = 30, GATE_TABLES = 6;
/* The gatehouse over the road at y, with one table per gate slot (at least
 * one, and one per job gating if more report it; a train is one job), or
 * null: a runtime village, or one whose report says nothing about its gate. */
function gateBand(v, roadX, y) {
  const merge = v.pipeline.merge, gates = v.pipeline.gates || [];
  const tablesN = v.plane === 'runtime' || (!merge && !gates.length) ? 0
    : Math.min(GATE_TABLES, Math.max(1, (merge && merge.slots) || 0, new Set(gates.filter(e => e.state === 'gating').map(e => e.train || e.issue)).size));
  if (!tablesN) return null;
  const x0 = roadX + RD + 4, w = 8 + tablesN * 12;
  return { y, x0, w, rx: roadX, rw: RD, tables: Array.from({ length: tablesN }, (_, k) => [x0 + 4 + k * 12, y + 10]),
    queueMax: Math.max(1, Math.floor((roadX - 22) / 16) + 1) };
}
export function layout(v, slot = null) {
  const n = v.houses.length;
  const per = Math.max(1, Math.ceil(Math.sqrt(Math.max(n, 1) / 2)));
  const rows = Math.max(1, Math.ceil(n / (2 * per)));
  const cx0 = DOCK_W + 8;
  const vw = Math.max(per * LW * 2 + RD, 72);
  const roadX = cx0 + Math.floor((vw - RD) / 2);
  const hallX = roadX + RD / 2 - 16;
  const top = 60;
  const lots = [];
  for (let i = 0; i < rows * 2 * per; i++) {
    const r = Math.floor(i / (2 * per)), j = i % (2 * per), side = j % 2, pos = Math.floor(j / 2);
    const x = side === 0 ? roadX - (pos + 1) * LW + 2 : roadX + RD + pos * LW + 2;
    lots.push({ x, y: top + r * LH, lane: top + r * LH + 18 });
  }
  const gate = gateBand(v, roadX, top + rows * LH + 2);
  const quayY = top + rows * LH + 4 + (gate ? GATE_H : 0);
  const canalY = quayY + 8;
  // Every stop but the last is a lock; the last stop is the castle.
  const locks = Math.max(0, v.pipeline.stops.length - 1);
  // The locks start east of the gatehouse, so its tables never stand in a lock's tower.
  const lockStart = Math.max(roadX + 40, gate ? gate.x0 + gate.w + 10 : 0);
  const castleX = lockStart + locks * 34 + 6;
  const forestX = Math.max(cx0 + vw + 6, hallX + 64);
  // Feature banners stand in a strip cleared out of the forest, east of the
  // houses; the map widens if they need the room. No features, no strip.
  const fcols = featureColours(v.features);
  const banners = (v.features || []).map((f, k) => ({ f, x: forestX + 12 + k * 30, y: 14, col: fcols.get(f.number) }));
  const devW = Math.max(cx0 + vw + 40, roadX + 70, castleX + 46, gate ? gate.x0 + gate.w + 4 : 0, banners.length ? forestX + 12 + banners.length * 30 : 0);
  // The environment row: a village per environment east of the dev village,
  // a stretch of river before each, production right-most (docs/environment-row.md).
  // A world's platform widens the slot of the environment it runs (`slot`).
  let ex = devW + RIVER;
  const envs = envStops(v).map(({ s, i }, k, all) => {
    const platform = !!slot && s.name === slot.name, w = platform ? Math.max(ENV_W, slot.w) : ENV_W;
    const e = { s, i, k, x: ex, w, rank: k === all.length - 1 ? 'capital' : 'town', platform };
    ex += w + RIVER;
    return e;
  });
  const W = envs.length ? envs[envs.length - 1].x + envs[envs.length - 1].w + 6 : devW;
  const H = canalY + 34;
  // What's built on: river, docks, plaza, roads, house rows, canal and its
  // stations. Every free 9×8 cell may get a decoration.
  const taken = [
    [0, 0, W, 14], [4, 8, 62, 46], [4, 46, cx0 + vw, 14], [hallX - 10, 4, 74, 46],
    [roadX - 2, 46, RD + 4, quayY - 40], [roadX - 18, quayY - 12, W, H],
    ...(gate ? [[4, gate.y - 2, gate.x0 + gate.w, GATE_H + 2]] : []),
    [forestX - 2, 12, W, canalY - 46], [devW, 0, W, H]
  ];
  for (let r = 0; r < rows; r++) {
    const left = roadX - per * LW, right = roadX + RD + per * LW;
    taken.push([left - 4, top + r * LH - 10, right - left + 8, LH + 4]);
  }
  const free = (x, y, w, h) => !taken.some(([tx, ty, tw, th]) => x < tx + tw && x + w > tx && y < ty + th && y + h > ty);
  // Big free ground grows small groves; what's left gets a few props. The
  // prop kind drifts slowly across the map, so neighbours form a garden
  // (flowerbeds together, rocks together) instead of random clutter.
  const decor = [];
  for (let y = 16; y < H - 18; y += 15) for (let x = 6; x < W - 16; x += 14) {
    if (!free(x, y, 14, 15) || hash(x * 7, y * 3) > 0.45) continue;
    decor.push({ x: x + Math.floor(hash(y, x) * 3), y: y + Math.floor(hash(x, y) * 2), tree: true });
    taken.push([x, y, 14, 15]);
  }
  for (let y = 16; y < H - 10; y += 9) for (let x = 6; x < W - 14; x += 11) {
    if (!free(x, y, 11, 9) || hash(x * 3, y * 5) > 0.2) continue;
    const zone = Math.floor(hash(x >> 5, y >> 5) * 6), k = hash(x + 11, y + 7) < 0.7 ? zone : Math.floor(hash(y, x + 3) * 6);
    decor.push({ x: x + Math.floor(hash(y, x) * 4), y: y + Math.floor(hash(x, y + 1) * 3), k });
  }
  return { per, rows, cx0, vw, roadX, hallX, top, lots, quayY, canalY, lockStart, castleX, W, H, decor, banners, devW, envs, gate, slot };
}
/* The environment row's sizes: each environment village, and the river before it. */
export const ENV_W = 120, RIVER = 56;
/* The stops drawn as villages of their own: every environment of a
 * development village. The sentinel's green ref stays a lock (a gate), and a
 * runtime's stages stay its pipeline. */
export function envStops(v) {
  if (v.plane === 'runtime') return [];
  return v.pipeline.stops.map((s, i) => ({ s, i })).filter(({ s }) => s.kind !== 'gate');
}
/* Where backlog crate i of n rests at the docks: 8 to a row, 6 pixels
 * apart, room for 40; up to 24 stand 8 apart, 6 to a row, leaving room
 * between them for their numbers. */
export const slotPos = (i, n = 40) => (n <= 24
  ? { x: 11 + (i % 6) * 8, y: 22 + Math.floor(i / 6) * 8 }
  : { x: 10 + (i % 8) * 6, y: 22 + Math.floor(i / 8) * 6 });
/* The strip cleared out of the forest for the feature banners. */
export function inBannerStrip(lay, x, y) {
  const b = lay.banners;
  return b.length > 0 && y < 48 && x >= b[0].x - 14 && x <= b[b.length - 1].x + 22;
}

/* Where a walk along `pts` is `t - p.t0` after it set off at `p.speed`, or null once over. */
export function porterPos(p, t) {
  let d = (t - p.t0) * p.speed;
  for (let k = 0; k < p.pts.length - 1; k++) {
    const [ax, ay] = p.pts[k], [bx, by] = p.pts[k + 1], len = Math.hypot(bx - ax, by - ay);
    if (d <= len) return [ax + (bx - ax) * d / len, ay + (by - ay) * d / len];
    d -= len;
  }
  return null;
}

/* A world: its platform is the village of the environment it runs (#407),
 * by default production, the right-most. It stands in that environment's
 * slot of the row, widened to fit it, over the slot's sign, warehouse and
 * docks; the dev row moves down only as far as that needs (OVER: the
 * warehouse's band above the river). A dev village with no environments
 * stacks the platform over it instead, with a margin each side for the
 * river down and the road up. */
export const SIDE = 24, GAP = 28, OVER = 50;
export const otherTheme = () => THEMES[state.T.pair] || THEMES.medieval;
/* A world's [platform, dev] scenes, once both have been read. */
export function worldScenes(w) {
  const Ps = w && state.scenes[w.platform], Ds = w && state.scenes[w.dev];
  return Ps && Ds && Ps.v && Ds.v ? [Ps, Ds] : null;
}
/* The environment a world's platform runs: the one it names, else the last. */
export function platformEnv(w, v) {
  const envs = envStops(v);
  if (!envs.length) return null;
  return envs.some(({ s }) => s.name === w.environment) ? w.environment : envs[envs.length - 1].s.name;
}
export function worldGeom(w, Ps, Ds) {
  const name = platformEnv(w, Ds.v);
  if (name) {
    const need = Ps.L.W + 8;
    if (!Ds.L.slot || Ds.L.slot.name !== name || Ds.L.slot.w !== need) Ds.L = layout(Ds.v, { name, w: need });
    const e = Ds.L.envs.find(x => x.platform);
    if (e) {
      const devTop = Math.max(0, Ps.L.H + OVER - Ds.L.canalY);
      return { row: true, e, W: Ds.L.W, H: devTop + Ds.L.H, devTop, dx: 0, px: e.x + ((e.w - Ps.L.W) >> 1), py: 0, srcY: Ps.L.top };
    }
  }
  const W = Math.max(Ps.L.W, Ds.L.W) + 2 * SIDE, devTop = Ps.L.H + GAP;
  return { row: false, W, H: devTop + Ds.L.H, devTop, dx: SIDE, px: SIDE, py: 0, srcY: Ps.L.top, roadY: 53 };
}
/* The scenes on screen and where each sits on the canvas, the one drawn on top last. */
export function shownScenes() {
  const { view, scenes, worlds } = state;
  if (!view) return [];
  if (!view.world) { const sc = scenes[view.village]; return sc && sc.v ? [[sc, 0, 0]] : []; }
  const w = worlds.find(x => x.name === view.world), both = worldScenes(w);
  if (!both) return [];
  const [Ps, Ds] = both, g = worldGeom(w, Ps, Ds);
  return [[Ds, g.dx, g.devTop], [Ps, g.px, g.py]];
}
/* The theme a shown scene is drawn in: a world's platform takes the other one. */
export const themeAt = sc => {
  const w = state.view && state.view.world && state.worlds.find(x => x.name === state.view.world);
  return w && sc.v && sc.v.project === w.platform ? otherTheme() : state.T;
};

/* The ground: grass, the river and the docks' yard. */
registerLayer({ z: 0, draw(c, { lay }, t) {
  const { W, H } = lay;
  state.T.ground(c, 0, 0, W, H);
  if (state.T.border) state.T.border(c, W, H);
  state.T.channel(c, 0, 0, W, 11, t, 1);
} });
/* Streets: the main road, the road down to the quay, the plaza, a lane per row of houses. */
registerLayer({ z: 20, draw(c, { lay }) {
  const { roadX, hallX, quayY, cx0, vw } = lay;
  state.T.road(c, 4, 50, cx0 + vw - 4, 6);
  state.T.road(c, roadX, 50, RD, quayY - 50 + 2);
  state.T.yard(c, hallX - 8, 42, 48, 9);
  for (let r = 0; r < lay.rows; r++) {
    const left = roadX - lay.per * LW + 2, right = roadX + RD + lay.per * LW - 2;
    state.T.road(c, left - 1, lay.top + r * LH + 17, right - left + 2, 4);
  }
} });
/* The forest east of the houses (clear of the banner strip), groves and props on free ground. */
registerLayer({ z: 40, draw(c, { lay }, t) {
  const { devW, hallX, canalY, cx0, vw } = lay;
  for (let x = Math.max(cx0 + vw + 6, hallX + 64); x < devW - 14; x += 15) for (let y = 16; y < canalY - 38; y += 16) if (hash(x, y) < 0.55 && !inBannerStrip(lay, x, y)) state.T.tree(c, x + Math.floor(hash(y, x) * 4), y);
  lay.decor.forEach(d => d.tree ? state.T.tree(c, d.x, d.y) : state.T.decor && state.T.decor(c, d.x, d.y, d.k, state.reduce ? 0 : t));
  shadow(c, hallX - 2, 40, 42, 2);
} });
