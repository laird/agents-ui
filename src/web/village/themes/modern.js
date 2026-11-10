/* The Modern business district theme. */
import { KINDCOL, P, PRIOCOL, R, blink, hash, shadow } from '../palette.js';
import { note, prioRun } from '../cvdmarks.js';
import { puffs } from '../sprites/houses.js';
import { detail } from '../detail.js';
import { disc, dither, px } from '../art2x.js';
import { CRATE_W, CRATE_H, seal, stencil } from '../crates.js';
import { registerTheme } from './registry.js';

/* Downtown props. MODERN has no `decor` slot, so these are not drawn. */
export const modernDecor = (c, x, y, k, t) => {
  switch (k) {
    case 0: // planter
      shadow(c, x + 1, y + 6, 7, 1);
      R(c, x, y + 3, 7, 3, P.o); R(c, x + 1, y + 3, 5, 2, M.concreteL);
      R(c, x + 1, y, 5, 3, P.o); R(c, x + 2, y, 3, 3, P.leaf); R(c, x + 2, y, 1, 1, P.leafL); break;
    case 1: // bench
      shadow(c, x, y + 5, 8, 1);
      R(c, x, y + 1, 8, 2, P.o); R(c, x + 1, y + 1, 6, 1, M.box); R(c, x, y + 3, 8, 1, M.boxD);
      R(c, x + 1, y + 4, 1, 1, P.o); R(c, x + 6, y + 4, 1, 1, P.o); break;
    case 2: // lamp post, lit at night-blue intervals
      shadow(c, x + 3, y + 7, 3, 1);
      R(c, x + 2, y + 1, 1, 7, M.steelD); R(c, x + 1, y, 3, 2, P.o); R(c, x + 2, y, 1, 1, blink(t + x * 37, 1800) ? P.lamp : M.glassL); break;
    case 3: // bike rack
      for (let i = 0; i < 3; i++) { R(c, x + i * 3, y + 2, 1, 3, M.steel); R(c, x + i * 3, y + 2, 2, 1, M.steel); }
      R(c, x, y + 5, 8, 1, M.concreteD); break;
    case 4: // fountain
      shadow(c, x + 1, y + 6, 8, 1);
      R(c, x, y + 3, 8, 3, P.o); R(c, x + 1, y + 3, 6, 2, M.concreteL); R(c, x + 2, y + 3, 4, 1, M.glassL);
      R(c, x + 3, y + 1, 2, 2, M.glassL); if (blink(t + y * 13, 300)) R(c, x + 3, y, 1, 1, P.white); break;
    default: // hedge strip
      R(c, x, y + 2, 8, 3, P.o); R(c, x + 1, y + 2, 6, 2, P.leaf); R(c, x + 1, y + 2, 3, 1, P.leafL);
  }
};
/* A low fence downtown. MODERN has no `border` slot, so it is not drawn. */
export const modernBorder = (c, W, H) => {
  R(c, 0, H - 2, W, 2, M.concreteD); R(c, 0, 11, 2, H - 11, M.concreteD); R(c, W - 2, 11, 2, H - 11, M.concreteD);
  for (let x = 2; x < W - 2; x += 6) R(c, x, H - 3, 1, 1, M.steel);
};

export const M = {
  lawn: '#68b050', lawnD: '#58a040', asphalt: '#505868', asphaltD: '#383c48', dash: '#f0d850',
  concrete: '#c0c0c8', concreteD: '#9898a8', concreteL: '#e0e0e8',
  glass: '#5890c8', glassD: '#386898', glassL: '#a8d0f0', brick: '#a85840', brickD: '#783828',
  steel: '#8890a0', steelD: '#586070', crane: '#f0c020', box: '#c89858', boxD: '#987040',
  tarp: '#48a068', tarpD: '#307848', awnA: '#f8f8f8', awnB: '#2890a0', led: '#58e878'
};
const mWin = st => st === 'working' ? P.lamp : (st === 'unknown' || st === 'stopped') ? P.dark : M.glassL;
/* Flat-roofed block: parapet roof, facade, a row of windows, glass door. */
function block(c, x, y, st, o) {
  const top = o.top ?? 1;
  R(c, x, y + top, 16, 16 - top, P.o);
  R(c, x + 1, y + top + 1, 14, 3, o.roof || M.concreteD); R(c, x + 1, y + top + 1, 14, 1, o.roofL || M.concrete);
  R(c, x + 1, y + top + 4, 14, 11 - top, o.wall);
  if (o.rows !== 0) for (let wy = y + top + 6; wy < y + 10; wy += 3) [x + 2, x + 6, x + 10].forEach(wx => R(c, wx, wy, 3, 2, mWin(st)));
  if (o.door !== false) {
    R(c, x + 6, y + 11, 4, 5, st === 'blocked' ? P.o : M.glassD);
    if (st !== 'blocked') R(c, x + 8, y + 11, 1, 5, P.o);
  }
}
const MB = {
  cottage(c, x, y, st, t) {
    block(c, x, y, st, { wall: M.concreteL, rows: 0, top: 3 });
    R(c, x + 1, y + 9, 14, 3, P.o);
    for (let k = 0; k < 7; k++) R(c, x + 2 + k * 2, y + 10, 2, 1, k % 2 ? M.awnA : M.awnB);
    R(c, x + 2, y + 13, 3, 2, mWin(st)); R(c, x + 11, y + 13, 3, 2, mWin(st));
    if (st === 'working') puffs(c, x + 12, y - 1, t, P.white, M.concreteL);
  },
  workshop(c, x, y, st, t) {
    block(c, x, y, st, { wall: M.brick });
    for (let yy = y + 6; yy < y + 16; yy += 3) R(c, x + 1, yy, 14, 1, M.brickD);
    [x + 2, x + 10].forEach(wx => { R(c, wx, y + 6, 4, 2, mWin(st)); R(c, wx, y + 9, 4, 1, mWin(st)); });
  },
  feature(c, x, y, st, t) {
    block(c, x, y, st, { wall: M.concrete, top: 4, rows: 0 });
    R(c, x + 2, y + 10, 3, 2, mWin(st));
    for (let yy = y + 5; yy < y + 16; yy += 4) R(c, x + 9, yy, 7, 1, M.steel);
    R(c, x + 9, y + 5, 1, 11, M.steelD); R(c, x + 15, y + 5, 1, 11, M.steelD);
    R(c, x + 13, y - 4, 2, 20, M.crane); R(c, x + 13, y - 4, 1, 20, P.o);
    R(c, x + 3, y - 4, 13, 2, M.crane); R(c, x + 3, y - 3, 13, 1, P.o);
    const hook = st === 'working' ? 5 + Math.floor((Math.sin(t / 400) + 1) * 2) : 6;
    R(c, x + 5 + (st === 'working' ? Math.floor(t / 700) % 3 : 0), y - 2, 1, hook, P.o);
  },
  bug(c, x, y, st, t) {
    block(c, x, y, st, { wall: M.concreteL, rows: 0 });
    R(c, x + 2, y + 6, 12, 4, P.o); R(c, x + 3, y + 7, 10, 2, mWin(st));
    R(c, x + 4, y - 3, 8, 5, P.o); R(c, x + 5, y - 2, 6, 3, P.white);
    R(c, x + 6, y - 2, 4, 3, P.bug); R(c, x + 7, y - 2, 1, 1, P.o); R(c, x + 8, y - 1, 1, 1, P.o); R(c, x + 8, y - 2, 1, 3, P.o);
    R(c, x + 6, y + 2, 1, 2, P.o); R(c, x + 9, y + 2, 1, 2, P.o);
    if (st === 'working') R(c, x + 3 + Math.floor(t / 160) % 9, y + 7, 1, 1, P.o);
  },
  security(c, x, y, st, t) {
    block(c, x, y, st, { wall: M.concreteD, rows: 0, top: 2, roof: M.steelD, roofL: M.steel });
    [x + 3, x + 11].forEach(wx => R(c, wx, y + 8, 2, 1, mWin(st)));
    R(c, x + 6, y + 6, 4, 4, P.gold); R(c, x + 7, y + 7, 2, 2, P.shield);
    R(c, x + 3, y - 2, 1, 5, P.o);
    const a = st === 'working' ? t / 300 : 0.6;
    for (let d = 1; d < 5; d++) R(c, x + 3 + Math.round(Math.cos(a) * d), y - 2 + Math.round(Math.sin(a) * d * 0.5), 1, 1, M.steelD);
    R(c, x + 12, y - 1, 3, 2, P.o); R(c, x + 13, y - 1, 1, 1, st === 'working' && blink(t, 400) ? P.p0 : M.steel);
  },
  tests(c, x, y, st, t) {
    block(c, x, y, st, { wall: M.glass, rows: 0 });
    for (let wy = y + 5; wy < y + 11; wy += 2) R(c, x + 1, wy, 14, 1, st === 'working' ? P.fizzB : M.glassL);
    R(c, x + 2, y + 6, 1, 4, M.glassD); R(c, x + 13, y + 6, 1, 4, M.glassD);
    R(c, x + 3, y - 2, 3, 3, P.o); R(c, x + 4, y - 1, 1, 2, P.copper);
    R(c, x + 10, y - 2, 3, 3, P.o); R(c, x + 11, y - 1, 1, 2, P.copper);
    if (st === 'working') { puffs(c, x + 4, y - 3, t, P.fizzA, P.fizzB); puffs(c, x + 11, y - 3, t + 500, P.fizzB, P.fizzA); }
  },
  research(c, x, y, st, t) {
    block(c, x, y, st, { wall: P.white, top: 2 });
    const a = -2.2 + (st === 'working' ? Math.sin(t / 900) * 0.5 : 0);
    for (let dy = -4; dy <= 0; dy++) for (let dx = -4; dx <= 4; dx++) {
      if (dx * dx + dy * dy > 16) continue;
      R(c, x + 8 + dx, y + 2 + dy, 1, 1, dx * dx + dy * dy > 9 ? P.o : M.concreteL);
    }
    R(c, x + 8 + Math.round(Math.cos(a) * 4), y + 1 + Math.round(Math.sin(a) * 4), 1, 1, P.gold);
    R(c, x + 8, y + 2, 1, 2, P.o);
  },
  docs(c, x, y, st, t) {
    block(c, x, y, st, { wall: P.parch, rows: 0 });
    R(c, x + 2, y + 6, 4, 4, mWin(st)); R(c, x + 10, y + 6, 4, 4, mWin(st));
    ['#c04838', '#3870c0', '#58a048', '#f8d040'].forEach((col, k) => R(c, x + 4 + k * 2, y - 2, 2, 4, col));
    R(c, x + 3, y + 2, 10, 1, P.o);
  },
  refactor(c, x, y, st, t) {
    block(c, x, y, st, { wall: M.brick });
    R(c, x + 8, y + 1, 8, 15, P.o); R(c, x + 9, y + 2, 6, 13, M.tarp);
    for (let yy = y + 4; yy < y + 15; yy += 4) R(c, x + 9, yy, 6, 1, M.tarpD);
    for (let yy = y + 2; yy < y + 16; yy += 5) R(c, x + 7, yy, 9, 1, M.steel);
    if (st === 'working' && blink(t, 140)) R(c, x + 11, y + 6 + Math.floor(t / 280) % 6, 1, 1, P.white);
  },
  perf(c, x, y, st, t) {
    block(c, x, y, st, { wall: M.steelD, rows: 0, top: 2, roof: M.steel, roofL: M.concrete });
    [x + 4, x + 11].forEach((fx, k) => {
      R(c, fx - 3, y - 1, 7, 6, P.o); R(c, fx - 2, y, 5, 4, M.steelD);
      const a = st === 'working' ? t / 90 + k : 0.3;
      for (let d = -2; d <= 2; d++) R(c, fx + Math.round(Math.cos(a) * d), y + 2 + Math.round(Math.sin(a) * d * 0.7), 1, 1, M.concreteL);
    });
    for (let k = 0; k < 6; k++) R(c, x + 2 + k * 2, y + 9, 1, 1, st === 'working' && hash(k, Math.floor(t / 200)) < 0.6 ? M.led : P.dark);
  },
  chores(c, x, y, st, t) {
    R(c, x, y + 3, 16, 13, P.o); R(c, x + 1, y + 4, 14, 11, M.steel);
    for (let xx = x + 2; xx < x + 15; xx += 2) R(c, xx, y + 4, 1, 11, M.steelD);
    R(c, x + 1, y + 4, 14, 2, M.concreteD);
    R(c, x + 4, y + 8, 8, 8, P.o); R(c, x + 5, y + 9, 6, 7, st === 'blocked' ? P.o : M.concreteL);
    if (st !== 'blocked') for (let yy = y + 10; yy < y + 16; yy += 2) R(c, x + 5, yy, 6, 1, M.concreteD);
    if (st === 'working') R(c, x + 5, y + 9, 6, 1, P.lamp);
    R(c, x + 12, y + 12, 4, 4, P.o); R(c, x + 13, y + 13, 2, 2, M.box);
  }
};
export const MODERN = {
  name: 'Modern business district',
  pair: 'medieval',
  // A broken build downtown: brown smog, and an alarm light on every roof.
  weather: { style: 'smog', cloud: '#8a7a5c', cloudD: '#5c5240', rain: '#b8b0a0', tint: 'rgba(96,74,40,0.30)', alarm: '#ff3030' },
  labels: { docks: 'Mailroom', board: 'Whiteboard', houses: 'Offices', quay: 'Loading bay', castle: 'Launch pad', hall: 'Headquarters', ship: 'Rocket', sails: 'lifts off', gate: 'QA checkpoint' },
  towers: [[3, -51, 6, 43], [12, -51, 18, 43]],
  ground(c, x0, y0, w, h) {
    for (let y = y0; y < y0 + h; y += 4) R(c, x0, y, w, Math.min(4, y0 + h - y), (y >> 2) % 2 ? M.lawn : M.lawnD);
    // 2× (#452): the mower's stripes catch the light on one edge, and the
    // lawn has the odd clover and daisy.
    detail(c, x0, y0, g => {
      for (let y = 0; y < h; y += 4) px(g, 0, 2 * y, 2 * w, 1, (((y0 + y) >> 2) % 2 ? M.lawnD : M.lawn));
      dither(g, 0, 0, 2 * w, 2 * h, 3, (x, y) => {
        const v = hash((x0 * 2 + x) >> 1, (y0 * 2 + y) >> 1);
        return v < 0.03 ? P.leafD : v > 0.992 ? P.white : null;
      });
    });
  },
  road(c, x0, y0, w, h) {
    R(c, x0, y0, w, h, M.asphalt);
    if (w > h && h >= 6) for (let x = x0 + 1; x < x0 + w - 2; x += 6) R(c, x, y0 + (h >> 1), 3, 1, M.dash);
    if (h > w && w >= 6) for (let y = y0 + 1; y < y0 + h - 2; y += 6) R(c, x0 + (w >> 1), y, 1, 3, M.dash);
    // 2× (#452): aggregate in the asphalt, and worn dashes.
    detail(c, x0, y0, g => {
      dither(g, 0, 0, 2 * w, 2 * h, 3, (x, y) => {
        const v = hash((x0 * 2 + x), (y0 * 2 + y) + 11);
        return v < 0.12 ? M.asphaltD : v > 0.94 ? M.concreteD : null;
      });
      if (w > h && h >= 6) for (let x = 1; x < w - 2; x += 6) px(g, 2 * x + 1, 2 * (h >> 1) + 1, 4, 1, M.concreteL);
      if (h > w && w >= 6) for (let y = 1; y < h - 2; y += 6) px(g, 2 * (w >> 1) + 1, 2 * y + 1, 1, 4, M.concreteL);
    });
  },
  yard(c, x0, y0, w, h) {
    R(c, x0, y0, w, h, M.concrete);
    for (let x = x0 + 3; x < x0 + w; x += 4) R(c, x, y0, 1, h, M.concreteD);
    for (let y = y0 + 3; y < y0 + h; y += 4) R(c, x0, y, w, 1, M.concreteD);
    // 2× (#452): each paving slab's joint is a fine groove with a lit lip, and
    // some slabs are stained.
    detail(c, x0, y0, g => {
      for (let x = 3; x < w; x += 4) { px(g, 2 * x, 0, 1, 2 * h, M.concreteD); px(g, 2 * x + 1, 0, 1, 2 * h, M.concreteL); }
      for (let y = 3; y < h; y += 4) { px(g, 0, 2 * y, 2 * w, 1, M.concreteD); px(g, 0, 2 * y + 1, 2 * w, 1, M.concreteL); }
      for (let y = 0; y < h - 3; y += 4) for (let x = 0; x < w - 3; x += 4) {
        if (hash(x0 + x, y0 + y) < 0.15) { px(g, 2 * x + 2, 2 * y + 2, 3, 2, M.concreteD); px(g, 2 * x + 3, 2 * y + 4, 1, 1, M.concreteD); }
      }
    });
  },
  /* Rivers and canals become roads with moving traffic markings. */
  channel(c, x0, y0, w, h, t, flow) {
    R(c, x0, y0, w, h, M.asphaltD);
    R(c, x0, y0, w, 1, M.concreteD); R(c, x0, y0 + h - 1, w, 1, M.concreteD);
    const off = Math.floor(t / 60 * flow) % 8;
    for (let x = x0 - 8 + off; x < x0 + w; x += 8) if (x >= x0) R(c, x, y0 + (h >> 1), 4, 1, P.white);
    // 2× (#452): a kerb with a lit edge each side, solid edge lines inside
    // them, and aggregate in the asphalt.
    detail(c, x0, y0, g => {
      px(g, 0, 1, 2 * w, 1, M.concreteL); px(g, 0, 2 * h - 2, 2 * w, 1, M.concreteL);
      if (h >= 6) { px(g, 0, 4, 2 * w, 1, M.dash); px(g, 0, 2 * h - 5, 2 * w, 1, M.dash); }
      dither(g, 0, 6, 2 * w, Math.max(0, 2 * h - 12), 3, (x, y) => (hash(x0 * 2 + x, y0 * 2 + y + 5) < 0.1 ? M.asphalt : null));
    });
  },
  dock(c, x, y, w) {
    R(c, x, y, w, 7, P.o); R(c, x + 1, y, w - 2, 6, M.concrete);
    for (let k = 0; k < w - 2; k += 4) R(c, x + 1 + k, y + 5, 2, 1, M.dash);
  },
  /* A street tree in a concrete planter, at 2× (#452): a round canopy lit
   * from the top left with leaf clusters, over a planter with a lit rim. */
  tree(c, x, y) {
    detail(c, x, y, g => {
      px(g, 6, 18, 12, 6, P.o); px(g, 8, 18, 8, 4, M.concrete); px(g, 8, 18, 8, 1, M.concreteL); px(g, 9, 22, 6, 1, M.concreteD);
      px(g, 11, 16, 2, 3, P.woodD);
      disc(g, 4, 2, 18, { outline: P.o, fill: P.leaf, light: P.leafL, dark: P.leafD });
      [[8, 6], [13, 9], [10, 12]].forEach(([dx, dy]) => { px(g, dx, dy, 2, 1, P.leafL); px(g, dx + 1, dy + 1, 2, 1, P.leafD); });
    });
  },
  /* Headquarters: a glass tower with a gold crown over a colonnaded
   * podium, flags out front. The lobby doors go dark when the manager
   * waits on you. */
  hall(c, x, y, st) {
    const x0 = x - 4, gy = y + 22;
    R(c, x0 + 9, y - 8, 22, 22, P.o); R(c, x0 + 10, y - 7, 20, 20, M.glassD);
    for (let wy = y - 6; wy < y + 12; wy += 3) for (let wx = x0 + 11; wx < x0 + 29; wx += 3) R(c, wx, wy, 2, 2, mWin(st));
    for (let wx = x0 + 13; wx < x0 + 29; wx += 6) R(c, wx, y - 7, 1, 20, M.glassL);
    R(c, x0 + 9, y - 11, 22, 4, P.o); R(c, x0 + 10, y - 10, 20, 2, P.gold);
    R(c, x0 + 14, y - 13, 12, 2, P.o); R(c, x0 + 15, y - 13, 10, 1, P.gold);
    R(c, x0 + 19, y - 17, 2, 4, P.gold);
    R(c, x0, gy - 9, 40, 9, P.o); R(c, x0 + 1, gy - 8, 38, 7, M.concreteL);
    R(c, x0 + 1, gy - 8, 38, 1, P.gold);
    for (let k = 3; k < 38; k += 5) R(c, x0 + k, gy - 7, 2, 6, M.concrete);
    R(c, x0 + 15, gy - 7, 10, 7, P.o); R(c, x0 + 16, gy - 6, 8, 6, st === 'blocked' ? P.o : M.glassL);
    if (st !== 'blocked') R(c, x0 + 20, gy - 6, 1, 6, P.o);
    [[x0 - 2, P.gold], [x0 + 40, '#3878e0']].forEach(([fx, col]) => { R(c, fx, gy - 22, 1, 22, M.steelD); R(c, fx + 1, gy - 22, 4, 3, P.o); R(c, fx + 1, gy - 21, 3, 1, col); });
  },
  board(c, x, y, notes) {
    R(c, x + 2, y + 9, 1, 6, M.steelD); R(c, x + 15, y + 9, 1, 6, M.steelD);
    R(c, x, y, 18, 11, P.o); R(c, x + 1, y + 1, 16, 9, P.white);
    notes.slice(0, 8).forEach((n, i) => note(c, x + 2 + (i % 4) * 4, y + 2 + Math.floor(i / 4) * 4, n.label, P.gold));
  },
  /* A parcel at 2× (crates.js): taped shut along the top in the kind's
   * colour, the issue number stencilled on, a shipping tag for its priority. */
  crate(c, x, y, kind, pr, sealed, n) {
    detail(c, x, y, d => {
      R(d, 0, 0, CRATE_W, CRATE_H, P.o); R(d, 1, 1, 9, 8, M.box); R(d, 1, 8, 9, 1, M.boxD);
      R(d, 1, 1, 9, 1, KINDCOL[kind] || M.boxD);
      // Its priority: a shipping tag on the lower left (none when unprioritised).
      if (PRIOCOL[pr]) prioRun(d, 1, 8, 4, pr, PRIOCOL[pr]);
      stencil(d, n, P.o, P.o, P.white);
      if (sealed) seal(d);
    });
  },
  B: MB,
  scaffold(c, x, y, t) {
    for (let yy = y + 2; yy < y + 16; yy += 4) R(c, x + 1, yy, 14, 1, M.steel);
    R(c, x + 1, y + 2, 1, 14, M.steelD); R(c, x + 14, y + 2, 1, 14, M.steelD);
    R(c, x + 7, y - 4, 2, 20, M.crane); R(c, x + 2, y - 4, 13, 2, M.crane);
    if (blink(t, 90)) R(c, x + 4 + Math.floor(hash(Math.floor(t / 90), 5) * 8), y + 6, 1, 1, P.white);
  },
  /* Toll booth with a boom barrier across the road. */
  gate(c, x, cy, open, flag) {
    R(c, x - 4, cy - 15, 12, 10, P.o); R(c, x - 3, cy - 14, 10, 8, M.concreteL);
    R(c, x - 2, cy - 12, 8, 3, M.glassL); R(c, x - 5, cy - 16, 14, 2, M.steelD);
    R(c, x + 1, cy - 20, 4, 4, P.o); R(c, x + 2, cy - 19, 2, 2, flag);
    R(c, x + 2, cy - 4, 2, 4, P.o);
    if (!open) for (let k = 0; k < 14; k += 2) R(c, x + 2, cy + k, 2, 2, (k / 2) % 2 ? P.white : P.p0);
  },
  /* Launch pad: released work lifts off. The sky lights up when it does.
   * The rocket on it is drawn by vessel() below, so it can load and fly. */
  destination(c, x, cy, t, fw) {
    R(c, x, cy - 8, 38, 8, P.o); R(c, x + 1, cy - 7, 36, 6, M.concrete);
    for (let k = 2; k < 36; k += 6) R(c, x + k, cy - 4, 3, 1, M.dash);
    R(c, x + 4, cy - 50, 4, 42, P.o); R(c, x + 5, cy - 49, 2, 40, P.red);
    for (let yy = cy - 46; yy < cy - 8; yy += 6) R(c, x + 8, yy, 6, 1, M.steelD);
    const ft = t - fw;
    if (ft >= 0 && ft < 1600) {
      const p = ft / 1600;
      [[x + 6, P.gold], [x + 21, P.fizzA], [x + 34, P.fizzB]].forEach(([fx, col], i) => {
        const fy = cy - 58 - p * 18 - i * 3;
        for (let k = 0; k < 8; k++) { const a = k / 8 * Math.PI * 2; R(c, fx + Math.cos(a) * p * 9, fy + Math.sin(a) * p * 9, 1, 1, col); }
      });
    }
    R(c, x + 8, cy - 2, 20, 18, P.o); R(c, x + 9, cy, 18, 15, P.dark);
    // 2× (#452): the tower's cross-bracing, hazard stripes along the pad, and
    // rivets down the gantry.
    detail(c, x, cy, g => {
      for (let yy = -92; yy < -18; yy += 12) for (let k = 0; k < 6; k++) { px(g, 10 + k, yy + k * 2, 1, 1, P.redD); px(g, 15 - k, yy + k * 2, 1, 1, P.redD); }
      for (let k = 4; k < 72; k += 6) { px(g, k, -14, 3, 1, P.gold); px(g, k + 1, -13, 3, 1, P.o); }
      for (let yy = -92; yy < -16; yy += 6) px(g, 11, yy, 1, 1, '#f8a0a0');
    });
  },
  /* The rocket for the next release, with its cargo rack, moved by (ox, oy).
   * Its engines fire while it climbs. */
  vessel(c, x, cy, t, ox, oy) {
    const rx = Math.round(x + 16 + ox); cy = Math.round(cy + oy);
    R(c, rx, cy - 44, 10, 36, P.o); R(c, rx + 1, cy - 42, 8, 33, P.white); R(c, rx + 1, cy - 42, 2, 33, M.concreteL);
    R(c, rx + 2, cy - 48, 6, 4, P.o); R(c, rx + 3, cy - 47, 4, 4, P.red); R(c, rx + 4, cy - 50, 2, 2, P.red);
    R(c, rx + 3, cy - 34, 4, 3, P.o); R(c, rx + 4, cy - 33, 2, 1, M.glassL);
    R(c, rx - 3, cy - 16, 3, 8, P.o); R(c, rx - 2, cy - 15, 2, 6, P.red);
    R(c, rx + 10, cy - 16, 3, 8, P.o); R(c, rx + 10, cy - 15, 2, 6, P.red);
    R(c, rx + 2, cy - 18, 6, 1, M.steelD);
    R(c, rx + 13, cy - 26, 1, 18, M.steelD); R(c, rx + 13, cy - 9, 11, 1, M.steelD);
    if (oy < 0) for (let k = 0; k < 4; k++) R(c, rx + 2 + k * 2, cy - 8 + (blink(t + k * 60, 80) ? 1 : 0), 2, 3 + (k % 2) * 2, k % 2 ? P.gold : '#f8f8f8');
    // 2× (#452): panel seams and rivets on the body, a glint on the porthole,
    // a lit edge on each fin and on the nose.
    detail(c, rx, cy, g => {
      [-76, -56, -40].forEach(yy => { px(g, 2, yy, 16, 1, M.concreteL); for (let k = 4; k < 18; k += 4) px(g, k, yy + 1, 1, 1, M.steelD); });
      px(g, 8, -66, 1, 1, P.white);
      px(g, -4, -30, 1, 12, '#f8a0a0'); px(g, 20, -30, 1, 12, '#f8a0a0');
      px(g, 6, -94, 1, 6, '#f8a0a0');
    });
  },
  hold: [[30, -14], [35, -14], [30, -19], [35, -19], [30, -24], [35, -24]], holdBadge: [29, -32], hull: [12, -52, 28, 46], flies: true,
  /* The delivery truck that brings new issues along the top road. */
  /* The QA checkpoint: a security gate over the road, and a lab with one
   * conveyor scanner per gate slot; a busy one runs and shows green. */
  gatehouse(c, g, t, busy) {
    const { x0, y, w, rx, rw } = g;
    [rx - 3, rx + rw + 1].forEach(px => { R(c, px, y + 2, 2, 20, P.o); R(c, px, y + 3, 1, 18, M.steel); });
    R(c, rx - 4, y, rw + 8, 3, P.o); R(c, rx - 3, y + 1, rw + 6, 1, busy.size ? M.led : P.p0);
    R(c, x0, y + 3, w, 20, P.o); R(c, x0 + 1, y + 4, w - 2, 18, M.concreteL); R(c, x0 + 1, y + 4, w - 2, 2, M.concrete);
    g.tables.forEach(([tx, ty], k) => {
      const on = busy.has(k + 1), off = on ? Math.floor(t / 120) % 3 : 0;
      R(c, tx + 4, ty - 9, 6, 6, P.o); R(c, tx + 5, ty - 8, 4, 4, on ? M.led : M.glassD);
      R(c, tx, ty + 5, 10, 3, P.o); R(c, tx + 1, ty + 5, 8, 1, M.steelD);
      for (let k2 = off; k2 < 8; k2 += 3) R(c, tx + 1 + k2, ty + 6, 1, 1, M.steel);
      R(c, tx + 8, ty + 1, 1, 1, on ? M.led : P.dark);
    });
  },
  supply(c, x, t, load) {
    x = Math.round(x);
    R(c, x, 5, 20, 4, P.o); R(c, x + 1, 5, 18, 2, M.steel); R(c, x + 1, 5, 18, 1, M.concreteL);
    R(c, x + 19, 1, 7, 8, P.o); R(c, x + 20, 2, 5, 5, P.white); R(c, x + 22, 3, 3, 2, M.glassL);
    [x + 3, x + 14, x + 21].forEach(wx => { R(c, wx, 8, 3, 3, P.o); R(c, wx + 1, 9, 1, 1, M.steel); });
    // 2× (#452): a glint on the windscreen, a headlight, the cab door's seam,
    // hubs on the wheels and a stripe along the box.
    detail(c, x, 0, g => {
      px(g, 46, 6, 1, 2, P.white); px(g, 50, 13, 2, 1, P.gold); px(g, 42, 6, 1, 8, P.o);
      [6, 28, 42].forEach(wx => px(g, wx + 2, 18, 2, 2, M.concreteL));
      px(g, 2, 13, 36, 1, P.red);
    });
    for (let k = 0; k < Math.min(load, 3); k++) this.crate(c, x + 2 + k * 5, 0, 'workshop', null);
  },
  people: 'modern',
  steam: [P.white, M.concreteL], sign: [P.o, P.dark, M.led]
};

registerTheme('modern', MODERN);
