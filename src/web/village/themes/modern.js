/* The Modern business district theme. */
import { KINDCOL, NOTECOL, P, PRIOCOL, R, blink, hash } from '../palette.js';
import { state } from '../state.js';
import { note, prioRun } from '../cvdmarks.js';
import { detail } from '../detail.js';
import { disc, dither, px } from '../art2x.js';
import { CRATE_W, CRATE_H, seal, stencil } from '../crates.js';
import { registerTheme } from './registry.js';
import { BLIND, CRANE_D, MB, MC, MLIT, MSHADE, mWin } from './modernhouses.js';

/* A manhole cover, 5 detail pixels square: a dark rim, a lit edge on top
 * and a cross-hatch in the iron. */
function manhole(g, x, y) {
  px(g, x + 1, y, 3, 1, MC.asphaltD); px(g, x, y + 1, 5, 3, MC.asphaltD); px(g, x + 1, y + 4, 3, 1, MC.asphaltD);
  px(g, x + 1, y + 1, 3, 3, MC.concreteD); px(g, x + 2, y + 1, 1, 3, MC.asphalt); px(g, x + 1, y + 2, 3, 1, MC.asphalt);
  px(g, x + 1, y + 1, 1, 1, MC.concreteL);
}
/* The theme's colours, kept with its offices (modernhouses.js). */
export const M = MC;
export const MODERN = {
  name: 'Modern business district',
  pair: 'medieval',
  // A broken build downtown: brown smog, and an alarm light on every roof.
  weather: { style: 'smog', cloud: '#8a7a5c', cloudD: '#5c5240', rain: '#b8b0a0', tint: 'rgba(96,74,40,0.30)', alarm: '#ff3030' },
  labels: { docks: 'Mailroom', board: 'Whiteboard', houses: 'Offices', quay: 'Loading bay', castle: 'Launch pad', hall: 'Headquarters', ship: 'Rocket', sails: 'lifts off', gate: 'QA checkpoint' },
  towers: [[3, -51, 6, 43], [12, -51, 18, 43]],
  /* The lawn, roads, paving and the traffic road, wholly at 2× (#452, #471). */
  /* A mown lawn: stripes, each lit along its top; the odd clover, and daisies
   * with a yellow heart. */
  ground(c, x0, y0, w, h) {
    detail(c, x0, y0, g => {
      const W = 2 * w;
      for (let y = 0; y < h; y += 4) {
        const n = Math.min(4, h - y), dark = ((y0 + y) >> 2) % 2;
        px(g, 0, 2 * y, W, 2 * n, dark ? M.lawnD : M.lawn);
        px(g, 0, 2 * y, W, 1, dark ? M.lawn : '#78c060');
      }
      dither(g, 0, 0, W, 2 * h, 3, (x, y) => {
        const v = hash((x0 * 2 + x) >> 1, (y0 * 2 + y) >> 1);
        return v < 0.03 ? P.leafD : null;
      });
      for (let y = 1; y + 3 < h; y += 5) for (let x = 1; x + 2 < w; x += 5) {
        const v = hash(x0 + x + 3, y0 + y + 17), X = 2 * x, Y = 2 * y;
        if (v < 0.04) { px(g, X + 1, Y, 1, 1, P.white); px(g, X, Y + 1, 3, 1, P.white); px(g, X + 1, Y + 2, 1, 1, P.white); px(g, X + 1, Y + 1, 1, 1, P.gold); }
        else if (v < 0.08) { px(g, X, Y, 2, 1, '#489838'); px(g, X + 1, Y + 1, 2, 1, '#489838'); px(g, X, Y, 1, 1, '#88d070'); }
      }
    });
  },
  /* Asphalt: a kerb lit along its top and shaded at its foot, aggregate,
   * worn dashes down the middle, and now and then a manhole cover. */
  road(c, x0, y0, w, h) {
    detail(c, x0, y0, g => {
      const W = 2 * w, H = 2 * h;
      px(g, 0, 0, W, H, M.asphalt);
      dither(g, 0, 0, W, H, 3, (x, y) => {
        const v = hash((x0 * 2 + x), (y0 * 2 + y) + 11);
        return v < 0.12 ? M.asphaltD : v > 0.94 ? M.concreteD : null;
      });
      if (w > h) {
        px(g, 0, 0, W, 1, M.concreteL); px(g, 0, H - 1, W, 1, M.asphaltD);
        if (h >= 6) for (let x = 1; x < w - 2; x += 6) { px(g, 2 * x, 2 * (h >> 1), 6, 2, M.dash); px(g, 2 * x + 1, 2 * (h >> 1), 4, 1, '#f8e888'); }
        if (h >= 5) for (let x = 6; x + 6 < w; x += 23) if (hash(x0 + x, y0 + 31) < 0.3) manhole(g, 2 * x, h >= 6 ? 1 : (H - 5) >> 1);
      } else {
        px(g, 0, 0, 1, H, M.concreteL); px(g, W - 1, 0, 1, H, M.asphaltD);
        if (w >= 6) for (let y = 1; y < h - 2; y += 6) { px(g, 2 * (w >> 1), 2 * y, 2, 6, M.dash); px(g, 2 * (w >> 1), 2 * y + 1, 1, 4, '#f8e888'); }
      }
    });
  },
  /* Paving slabs: each joint a fine groove with a lit lip; some slabs a
   * shade lighter, some stained, and the odd drain grate. */
  yard(c, x0, y0, w, h) {
    detail(c, x0, y0, g => {
      const W = 2 * w, H = 2 * h;
      px(g, 0, 0, W, H, M.concrete);
      for (let y = 0; y < h; y += 4) for (let x = 0; x < w; x += 4) {
        const v = hash(x0 + x, y0 + y), X = 2 * x, Y = 2 * y, sw = Math.min(6, W - X), sh = Math.min(6, H - Y);
        if (v > 0.8) px(g, X, Y, sw, sh, '#cacad2');
        if (v < 0.15 && sw >= 5 && sh >= 5) { px(g, X + 2, Y + 2, 3, 2, M.concreteD); px(g, X + 3, Y + 4, 1, 1, M.concreteD); }
        else if (v > 0.97 && sw >= 6 && sh >= 6) { px(g, X + 1, Y + 1, 5, 5, P.dark); for (let k = 0; k < 5; k += 2) px(g, X + 1, Y + 1 + k, 5, 1, M.concreteD); }
      }
      for (let x = 3; x < w; x += 4) { px(g, 2 * x, 0, 1, H, M.concreteD); px(g, 2 * x + 1, 0, 1, H, M.concreteL); }
      for (let y = 3; y < h; y += 4) { px(g, 0, 2 * y, W, 1, M.concreteD); px(g, 0, 2 * y + 1, W, 1, M.concreteL); }
    });
  },
  /* Rivers and canals become roads with moving traffic markings: a kerb with
   * a lit edge each side, solid edge lines with cat's eyes that glint in
   * turn, aggregate, and the centre dashes rolling with the flow. */
  channel(c, x0, y0, w, h, t, flow) {
    t = state.reduce ? 0 : t;
    const off = Math.floor(t / 60 * flow) % 8;
    detail(c, x0, y0, g => {
      const W = 2 * w, H = 2 * h, mid = 2 * (h >> 1);
      px(g, 0, 0, W, H, M.asphaltD);
      px(g, 0, 0, W, 2, M.concreteD); px(g, 0, H - 2, W, 2, M.concreteD);
      px(g, 0, 1, W, 1, M.concreteL); px(g, 0, H - 2, W, 1, M.concreteL);
      dither(g, 0, 6, W, Math.max(0, H - 12), 3, (x, y) => (hash(x0 * 2 + x, y0 * 2 + y + 5) < 0.1 ? M.asphalt : null));
      if (h >= 6) {
        px(g, 0, 4, W, 1, M.dash); px(g, 0, H - 5, W, 1, M.dash);
        for (let x = 4; x < W; x += 24) {
          const lit = Math.floor(t / 400 + x / 24) % 6 === 0;
          px(g, x, 4, 1, 1, lit ? P.white : '#c8b030'); if (x + 12 < W) px(g, x + 12, H - 5, 1, 1, lit ? P.white : '#c8b030');
        }
      }
      for (let x = -8 + off; x < w; x += 8) if (x >= 0) { px(g, 2 * x, mid, Math.min(8, W - 2 * x), 2, P.white); px(g, 2 * x, mid + 1, Math.min(8, W - 2 * x), 1, M.concreteL); }
    });
  },
  /* The loading dock at 2× (#464): a concrete lip lit along its top, the
   * slab's expansion joints, rubber bumpers, and a hazard edge. */
  dock(c, x, y, w) {
    detail(c, x, y, g => {
      px(g, 0, 0, 2 * w, 14, P.o); px(g, 2, 0, 2 * w - 4, 12, M.concrete);
      px(g, 2, 0, 2 * w - 4, 1, M.concreteL); px(g, 2, 9, 2 * w - 4, 1, M.concreteD);
      for (let k = 14; k < 2 * w - 4; k += 16) { px(g, k, 1, 1, 8, M.concreteD); px(g, k - 8, 3, 3, 5, P.dark); }
      px(g, 2, 10, 2 * w - 4, 2, P.o);
      for (let k = 0; k < w - 2; k += 4) px(g, 2 + 2 * k, 10, 4, 2, M.dash);
    });
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
  /* Headquarters at 2× (#464): a glass tower with a gold crown over a
   * colonnaded podium, flags out front. The lobby doors go dark when the
   * manager waits on you. */
  hall(c, x, y, st) {
    const x0 = x - 4;
    detail(c, x0, y, g => {
      // The tower: its curtain wall, each office lit or clear, mullions and a sheen.
      px(g, 18, -16, 44, 44, P.o); px(g, 20, -14, 40, 40, M.glassD);
      const lit = mWin(st);
      for (let wy = -12; wy < 24; wy += 6) for (let wx = 22; wx < 58; wx += 6) {
        px(g, wx, wy, 4, 4, lit); px(g, wx, wy + 3, 4, 1, MSHADE);
        if (st === 'working') px(g, wx, wy, 4, 1, BLIND); else if (lit === M.glassL) px(g, wx, wy, 1, 1, P.white);
      }
      [26, 38, 50].forEach(mx => { px(g, mx, -14, 1, 40, M.glassL); px(g, mx + 1, -14, 1, 40, M.glass); });
      for (let k = 0; k < 12; k++) px(g, 30 + k, -14 + 3 * k, 1, 2, MLIT);
      // The crown, in two gold tiers, and the spire with its beacon.
      px(g, 18, -22, 44, 8, P.o); px(g, 20, -20, 40, 4, P.gold); px(g, 20, -20, 40, 1, '#fff0a0'); px(g, 20, -17, 40, 1, '#c8a020');
      px(g, 28, -26, 24, 4, P.o); px(g, 30, -26, 20, 2, P.gold); px(g, 30, -26, 20, 1, '#fff0a0');
      px(g, 38, -34, 4, 8, P.gold); px(g, 38, -32, 1, 6, '#fff0a0'); px(g, 41, -32, 1, 6, '#c8a020'); px(g, 39, -34, 2, 2, P.p0);
      // The podium: a gold band, columns with capitals and bases, the lobby doors.
      px(g, 0, 26, 80, 18, P.o); px(g, 2, 28, 76, 14, M.concreteL); px(g, 2, 28, 76, 2, P.gold); px(g, 2, 29, 76, 1, '#c8a020');
      for (let k = 3; k < 38; k += 5) {
        px(g, 2 * k, 30, 4, 12, M.concrete); px(g, 2 * k + 3, 30, 1, 12, M.concreteD);
        px(g, 2 * k - 1, 30, 6, 1, M.concreteD); px(g, 2 * k - 1, 41, 6, 1, M.concreteD);
      }
      px(g, 30, 30, 20, 14, P.o);
      if (st !== 'blocked') {
        px(g, 32, 32, 16, 12, M.glassL); px(g, 40, 32, 2, 12, P.o);
        px(g, 33, 33, 1, 8, P.white); px(g, 43, 33, 1, 8, P.white);
        px(g, 38, 37, 1, 3, M.steelD); px(g, 43, 37, 1, 3, M.steelD);
      } else px(g, 46, 32, 1, 12, M.glassD);
      // The flags: a pole with a lit side, and the cloth with a fold in it.
      [[-4, P.gold], [80, '#3878e0']].forEach(([fx, col]) => {
        px(g, fx, 0, 1, 44, M.steel); px(g, fx + 1, 0, 1, 44, M.steelD);
        px(g, fx + 2, 0, 8, 6, P.o); px(g, fx + 2, 1, 7, 4, col); px(g, fx + 2, 1, 7, 1, MLIT); px(g, fx + 5, 1, 1, 4, MSHADE);
      });
    });
  },
  /* The whiteboard at 2× (#464): an aluminium frame on two legs, the last
   * meeting's marker scribbles under the notes, and a tray of markers. */
  board(c, x, y, notes) {
    detail(c, x, y, g => {
      px(g, 4, 18, 2, 12, M.steelD); px(g, 30, 18, 2, 12, M.steelD);
      px(g, 2, 29, 6, 1, P.o); px(g, 28, 29, 6, 1, P.o);
      px(g, 0, 0, 36, 22, P.o); px(g, 1, 1, 34, 20, M.steel); px(g, 1, 1, 34, 1, M.concreteL);
      px(g, 2, 2, 32, 18, P.white);
      [[5, 6, 6], [14, 7, 8], [6, 13, 10], [20, 14, 7], [24, 4, 6]].forEach(([sx, sy, len]) => px(g, sx, sy, len, 1, '#c8d0e0'));
      px(g, 6, 20, 24, 2, M.steelD);
      [[10, P.red], [15, '#3870c0'], [20, '#58a048']].forEach(([mx, col]) => px(g, mx, 20, 3, 1, col));
      // Each note's square, on this grid so the board does not cover it;
      // note() below adds its glyph over it.
      notes.slice(0, 8).forEach((n, i) => px(g, 4 + (i % 4) * 8, 4 + Math.floor(i / 4) * 8, 6, 6, NOTECOL[n.label] || P.gold));
    });
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
  /* A house going up, at 2× (#464): scaffold rails lit along the top, with
   * boards laid on them and braced bays, under a lattice crane; a welder's
   * spark flickers along the second rail. */
  scaffold(c, x, y, t) {
    detail(c, x, y, g => {
      for (let yy = 4; yy < 32; yy += 8) {
        px(g, 4, yy - 1, 24, 1, P.wood);
        px(g, 2, yy, 28, 2, M.steel); px(g, 2, yy, 28, 1, M.concreteL);
        if (yy + 8 < 32) for (let k = 0; k < 6; k++) px(g, 5 + Math.round(k * 4), yy + 2 + k, 1, 1, M.steelD);
      }
      px(g, 2, 4, 2, 28, M.steelD); px(g, 28, 4, 2, 28, M.steelD);
      px(g, 14, -8, 4, 40, M.crane); px(g, 14, -8, 1, 40, P.o);
      for (let yy = -4; yy < 32; yy += 4) px(g, 15, yy, 3, 1, CRANE_D);
      px(g, 4, -8, 26, 4, M.crane); px(g, 4, -8, 26, 1, '#f8e070');
      for (let xx = 6; xx < 28; xx += 4) px(g, xx, -6, 2, 1, CRANE_D);
      if (blink(t, 90)) {
        const sx = 2 * (4 + Math.floor(hash(Math.floor(t / 90), 5) * 8));
        px(g, sx, 12, 2, 2, P.white); px(g, sx - 1, 12, 1, 1, P.gold); px(g, sx + 2, 13, 1, 1, P.gold);
      }
    });
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
  /* The QA checkpoint at 2× (#464): a security gate over the road, its LED
   * sign green while it checks work, and a lab with one conveyor scanner per
   * gate slot; a busy one runs its belt, scans and shows green. */
  gatehouse(c, g, t, busy) {
    const { x0, y, w, rx, rw } = g;
    detail(c, 0, 0, d => {
      [rx - 3, rx + rw + 1].forEach(p => {
        px(d, 2 * p, 2 * y + 4, 4, 40, P.o); px(d, 2 * p, 2 * y + 6, 2, 36, M.steel); px(d, 2 * p, 2 * y + 6, 1, 36, M.concreteL);
        for (let k = 0; k < 2; k++) px(d, 2 * p, 2 * y + 34 + 4 * k, 2, 2, M.dash);
      });
      px(d, 2 * rx - 8, 2 * y, 2 * rw + 16, 6, P.o); px(d, 2 * rx - 6, 2 * y + 2, 2 * rw + 12, 2, busy.size ? M.led : P.p0);
      for (let k = 2; k < 2 * rw + 12; k += 3) px(d, 2 * rx - 6 + k, 2 * y + 2, 1, 2, MSHADE);
      const X = 2 * x0, Y = 2 * y + 6;
      px(d, X, Y, 2 * w, 40, P.o); px(d, X + 2, Y + 2, 2 * w - 4, 36, M.concreteL);
      px(d, X + 2, Y + 2, 2 * w - 4, 4, M.concrete); px(d, X + 2, Y + 2, 2 * w - 4, 1, M.concreteL); px(d, X + 2, Y + 6, 2 * w - 4, 1, MSHADE);
      for (let k = X + 10; k < X + 2 * w - 2; k += 12) px(d, k, Y + 30, 1, 8, M.concrete);
      g.tables.forEach(([tx, ty], k) => {
        const on = busy.has(k + 1), off = on ? Math.floor(t / 60) % 6 : 0, TX = 2 * tx, TY = 2 * ty;
        px(d, TX + 8, TY - 18, 12, 12, P.o); px(d, TX + 10, TY - 16, 8, 8, on ? M.led : M.glassD);
        if (on) px(d, TX + 10, TY - 16 + Math.floor(t / 100) % 8, 8, 1, '#b8f8c8'); else px(d, TX + 10, TY - 16, 2, 1, M.glass);
        px(d, TX + 13, TY - 6, 2, 2, M.steelD);
        px(d, TX, TY + 10, 20, 6, P.o); px(d, TX + 2, TY + 10, 16, 2, M.steelD);
        for (let k2 = off; k2 < 16; k2 += 6) { px(d, TX + 2 + k2, TY + 12, 2, 2, M.steel); px(d, TX + 2 + k2, TY + 12, 1, 1, M.concreteL); px(d, TX + 2 + k2, TY + 10, 1, 1, M.steel); }
        if (on) px(d, TX + 15, TY + 1, 4, 4, 'rgba(88,232,120,0.35)');
        px(d, TX + 16, TY + 2, 2, 2, on ? M.led : P.dark);
      });
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
