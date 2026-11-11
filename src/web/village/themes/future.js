/* The Future theme. */
import { state, now } from '../state.js';
import { F, KINDCOL, P, PRIOCOL, R, SHADE, blink, hash, shadow } from '../palette.js';
import { prioRun } from '../cvdmarks.js';
import { detail } from '../detail.js';
import { dither, lobes, planks, px } from '../art2x.js';
import { CRATE_W, CRATE_H, seal, stencil } from '../crates.js';
import { registerTheme } from './registry.js';
import { FB, futureBoard, futureDock, futureHall, futureScaffold } from './futurebuildings.js';
import { futureCargo, futureGate, futurePort, futureShuttle } from './futureport.js';
/* ---------- Future ----------
 * A space station worked by little robots: deck plates for ground, light-
 * strip walkways for roads, a plasma canal, habitat pods with portholes. It
 * keeps the shared status language: lit portholes while working, an open
 * hatch for blocked, dark glass for unknown. */
/* A crystal in detail pixels: `w` wide and `h` tall from its tip at (cx, top),
 * pointed, outlined, lit down its left face and shaded down its right. */
const CYAN = { light: F.cyanL, fill: F.cyan, dark: F.cyanD };
function crystal(g, cx, top, w, h, cols) {
  const half = w >> 1;
  for (let j = 0; j < h; j++) {
    const hw = Math.min(half, j + 1), x0 = cx - hw;
    px(g, x0, top + j, 2 * hw, 1, P.o);
    if (j === 0 || j === h - 1 || hw < 2) continue;
    px(g, x0 + 1, top + j, hw - 1, 1, cols.light);
    px(g, cx, top + j, hw - 1, 1, cols.dark);
    px(g, cx - 1, top + j, 1, 1, cols.fill);
  }
}
/* A glowing mushroom: a dome cap `w` wide at (x, y) on a pale stem `stem`
 * tall, lit on top, its gills shaded underneath, spots on the cap. */
const GLOW_M = { light: '#ff9ce0', fill: F.magenta, dark: F.magentaD }, GLOW_C = CYAN;
function mushroom(g, x, y, w, stem, cols) {
  const sx = x + (w >> 1) - 1, ch = Math.max(3, w >> 1);
  px(g, sx - 1, y + ch - 1, 4, stem + 1, P.o); px(g, sx, y + ch - 1, 2, stem, F.hullL); px(g, sx + 1, y + ch, 1, stem - 1, F.hull);
  for (let j = 0; j < ch; j++) {
    const inset = Math.max(0, 2 - j), cw = w - 2 * inset;
    px(g, x + inset, y + j, cw, 1, P.o);
    if (j && cw > 2) px(g, x + inset + 1, y + j, cw - 2, 1, j === ch - 1 ? cols.dark : j === 1 ? cols.light : cols.fill);
  }
  px(g, x + 2, y + 1, 1, 1, P.white);
  if (w >= 8) { px(g, x + w - 3, y + 2, 1, 1, cols.light); px(g, x + (w >> 1), y + 1, 1, 1, cols.light); }
}
export const FUTURE = {
  name: 'Future',
  pair: 'modern',
  // A broken build in orbit: a red alert, pulsing, with light bars and roof alarms.
  weather: { style: 'alert', cloud: '#501828', cloudD: '#300818', rain: '#ff7090', tint: 'rgba(224,24,48,0.20)', alarm: '#ff2040' },
  labels: { docks: 'Cargo bay', board: 'Holo-board', houses: 'Pods', quay: 'Launch rail', castle: 'Spaceport', hall: 'Command dome', ship: 'Shuttle', sails: 'launches', gate: 'Scanner arch' },
  towers: [[2, -58, 9, 50], [13, -55, 18, 45]],
  /* The deck, walkways, tiles and the plasma canal, wholly at 2× (#453, #471). */
  /* Deck plates: each plate bevelled, lit along its top and left, shaded
   * along its foot; rivets with a cap at the corners; now and then a vent
   * slot or a little status lamp. */
  ground(c, x0, y0, w, h) {
    detail(c, x0, y0, g => {
      const W = 2 * w, H = 2 * h;
      px(g, 0, 0, W, H, F.deckD);
      for (let x = 7; x < w; x += 8) { px(g, 2 * x, 0, 2, H, F.seam); if (x + 1 < w) px(g, 2 * x + 2, 0, 1, H, F.deck); }
      for (let y = 7; y < h; y += 8) { px(g, 0, 2 * y, W, 2, F.seam); if (y + 1 < h) px(g, 0, 2 * y + 2, W, 1, F.deck); }
      for (let y = 7; y < h; y += 8) px(g, 0, 2 * y - 1, W, 1, '#20243f');
      for (let y = 1; y < h; y += 16) for (let x = 1; x < w; x += 16) { px(g, 2 * x, 2 * y, 2, 2, F.rivet); px(g, 2 * x, 2 * y, 1, 1, F.deckL); }
      for (let y = 3; y + 2 < h; y += 8) for (let x = 2; x + 4 < w; x += 8) {
        const v = hash(x0 + x, y0 + y);
        if (v < 0.12) { px(g, 2 * x, 2 * y, 7, 1, F.seam); px(g, 2 * x, 2 * y + 2, 7, 1, F.seam); px(g, 2 * x, 2 * y + 1, 7, 1, F.deck); }
        else if (v > 0.95) { px(g, 2 * x + 2, 2 * y, 3, 3, F.seam); px(g, 2 * x + 3, 2 * y + 1, 1, 1, v > 0.975 ? F.cyan : F.magenta); }
      }
    });
  },
  /* Walkways with a glowing light strip down the middle: the edges a detail
   * pixel of trim and a detail pixel of glow, grip dots in the deck, and
   * each light in the strip with a white-hot core in a soft halo. */
  road(c, x0, y0, w, h) {
    detail(c, x0, y0, g => {
      const W = 2 * w, H = 2 * h;
      px(g, 0, 0, W, H, F.deckL);
      dither(g, 2, 2, W - 4, H - 4, 4, (x, y) => (hash(x0 * 2 + x, y0 * 2 + y + 3) < 0.4 ? F.deck : null));
      if (w > h && h >= 4) {
        px(g, 0, 0, W, 1, F.cyanD); px(g, 0, 1, W, 1, F.cyan); px(g, 0, H - 1, W, 1, F.cyanD); px(g, 0, H - 2, W, 1, F.deck);
        for (let x = 1; x < w - 2; x += 4) {
          const X = 2 * x, Y = 2 * (h >> 1);
          if (h >= 6) { px(g, X - 1, Y - 1, 6, 1, F.panel); px(g, X - 1, Y + 2, 6, 1, F.panel); }
          px(g, X, Y, 4, 2, F.cyan); px(g, X + 1, Y, 2, 1, F.cyanL);
        }
      }
      if (h > w && w >= 4) {
        px(g, 0, 0, 1, H, F.cyanD); px(g, 1, 0, 1, H, F.cyan); px(g, W - 1, 0, 1, H, F.cyanD); px(g, W - 2, 0, 1, H, F.deck);
        for (let y = 1; y < h - 2; y += 4) {
          const X = 2 * (w >> 1), Y = 2 * y;
          if (w >= 6) { px(g, X - 1, Y - 1, 1, 6, F.panel); px(g, X + 2, Y - 1, 1, 6, F.panel); }
          px(g, X, Y, 2, 4, F.cyan); px(g, X, Y + 1, 1, 2, F.cyanL);
        }
      }
    });
  },
  /* Deck tiles: bevelled squares, lit along the top and left of each, a lit
   * rivet where every other pair of seams cross, and a cyan trim at the edge. */
  yard(c, x0, y0, w, h) {
    detail(c, x0, y0, g => {
      const W = 2 * w, H = 2 * h;
      px(g, 0, 0, W, H, F.deck);
      for (let y = 0; y < h; y += 4) px(g, 0, 2 * y, W, 1, F.deckL);
      for (let x = 0; x < w; x += 4) px(g, 2 * x, 0, 1, H, F.deckL);
      for (let x = 3; x < w; x += 4) px(g, 2 * x, 0, 2, H, F.deckD);
      for (let y = 3; y < h; y += 4) px(g, 0, 2 * y, W, 2, F.deckD);
      for (let y = 3; y < h; y += 8) for (let x = 3; x < w; x += 8) { px(g, 2 * x, 2 * y, 2, 2, F.rivet); px(g, 2 * x, 2 * y, 1, 1, F.cyanL); }
      px(g, 0, H - 1, W, 1, F.cyanD);
    });
  },
  /* Rivers and canals are plasma conduits: walls with a glow just inside,
   * the plasma brightening towards the middle, pulses with a hot core and a
   * trail flowing along, and sparks that twinkle (still, with reduced motion). */
  channel(c, x0, y0, w, h, t, flow) {
    t = state.reduce ? 0 : t;
    const off = Math.floor(t / 60 * flow) % 10, mid = h >> 1;
    detail(c, x0, y0, g => {
      const W = 2 * w, H = 2 * h, Y = 2 * mid;
      px(g, 0, 0, W, H, F.space);
      if (H >= 12) px(g, 0, Y - 3, W, 8, '#1c1440');
      if (H >= 16) px(g, 0, Y - 1, W, 4, '#281c58');
      px(g, 0, 0, W, 2, F.magentaD); px(g, 0, H - 2, W, 2, F.magentaD);
      px(g, 0, 0, W, 1, F.magenta); px(g, 0, 2, W, 1, F.magentaD); px(g, 0, H - 3, W, 1, F.magentaD);
      for (let x = 3; x < w - 1; x += 7) {
        const v = hash(x0 + x, y0 + 13);
        if (v < 0.4 && Math.floor(t / 300 + v * 20) % 4 === 0) { const sy = 4 + Math.floor(v * 10) % Math.max(1, H - 8); px(g, 2 * x, sy, 1, 1, F.cyanL); }
      }
      for (let x = -10 + off; x < w; x += 10) if (x >= 0) {
        const X = 2 * x;
        px(g, X, Y, Math.min(8, W - X), 2, F.plasmaL); px(g, X + 5, Y, Math.min(3, W - X - 5), 1, P.white);
        if (X > 3) px(g, X - 4, Y, 4, 1, F.plasma);
        if (X + 10 < W) px(g, X + 10, Y - 4, Math.min(4, W - X - 10), 1, F.plasma);
        if (X + 4 < W) px(g, X + 4, Y + 4, Math.min(4, W - X - 4), 1, F.cyanD);
      }
    });
  },
  /* The station's groves (#473): a crystal spire on a hull plinth, a glow
   * tree whose light pods pulse in turn, and a clump of glowing mushrooms.
   * Drawn at 2× in the box each kind of station kit filled before. With
   * reduced motion the glow holds still. */
  tree(c, x, y) {
    const k = Math.floor(hash(x, y) * 3), t = now();
    detail(c, x, y, g => {
      if (k === 0) {
        px(g, 9, 22, 11, 1, SHADE); px(g, 8, 23, 12, 1, SHADE);
        px(g, 6, 19, 14, 4, P.o); px(g, 7, 19, 12, 2, F.hullD); px(g, 7, 19, 12, 1, F.hullL); px(g, 15, 20, 2, 1, F.hull);
        crystal(g, 9, 8, 4, 12, CYAN); crystal(g, 17, 6, 4, 14, CYAN); crystal(g, 13, 0, 6, 20, CYAN);
        const on = blink(t + x * 31, 1100);
        px(g, 13, 2, 1, on ? 3 : 1, P.white); if (on) px(g, 12, 3, 3, 1, F.cyanL);
        px(g, 10, 20, 2, 1, F.cyanL); px(g, 14, 20, 1, 1, F.cyan);
      } else if (k === 1) {
        px(g, 4, 22, 20, 2, SHADE);
        px(g, 10, 16, 4, 8, P.o); px(g, 11, 16, 2, 7, F.hullD); px(g, 11, 16, 1, 7, F.hull); px(g, 8, 23, 8, 1, F.hullD);
        lobes(g, [[0, 6, 14], [8, 4, 14]], { outline: P.o, fill: F.plasma, light: F.plasmaL, dark: F.purple });
        [[5, 10], [12, 8], [16, 13], [7, 15]].forEach(([dx, dy], i) => {
          const on = blink(t + x * 17 + i * 350, 1400);
          px(g, dx, dy, 2, 2, on ? F.cyanL : F.cyan); if (on) px(g, dx, dy, 1, 1, P.white);
        });
        px(g, 11, 20, 1, 2, F.cyanL);
      } else {
        px(g, 4, 20, 18, 2, SHADE);
        mushroom(g, 2, 15, 6, 4, GLOW_M); mushroom(g, 14, 13, 8, 5, GLOW_C); mushroom(g, 4, 8, 12, 7, GLOW_M);
        if (blink(t + x * 23, 1300)) { px(g, 7, 9, 1, 1, P.white); px(g, 17, 14, 1, 1, P.white); }
      }
    });
  },
  /* A cargo crate riding a little hover-pad. */
  crate(c, x, y, kind, pr, sealed, n) {
    detail(c, x, y, d => {
      R(d, 0, 0, CRATE_W, CRATE_H, P.o); R(d, 1, 1, 9, 8, F.hull); R(d, 1, 7, 9, 1, F.hullL);
      R(d, 1, 1, 9, 1, KINDCOL[kind] || F.hullD);
      // Its priority: a status light along the foot (none when unprioritised).
      R(d, 1, 8, 9, 1, F.hullD); if (PRIOCOL[pr]) prioRun(d, 1, 8, 9, pr, PRIOCOL[pr]);
      // The hover-pad's glow under it.
      R(d, 2, CRATE_H, 7, 1, F.cyan);
      stencil(d, n, F.navy, F.navy, F.cyan);
      if (sealed) seal(d);
    });
  },
  B: FB,
  hall: futureHall, board: futureBoard, dock: futureDock, scaffold: futureScaffold,
  /* The airlock, spaceport and shuttle at 2× (futureport.js, #472). */
  gate: futureGate, destination: futurePort,
  vessel(c, x, cy, t, ox, oy) { futureShuttle(c, x, cy, t, ox, oy); },
  hold: [[33, -14], [38, -14], [33, -19], [38, -19], [33, -24], [38, -24]], holdBadge: [31, -32], hull: [14, -58, 30, 50], flies: true,
  /* The scanner arch at 2×: laser grids across the walkway while anything
   * is gating, and one scanner pad per gate slot, its screen glowing while
   * busy. Drawn in the order it always was, so what overlaps still does. */
  gatehouse(c, g, t, busy) {
    const { x0, y, w, rx, rw } = g, gh = g;
    detail(c, 0, 0, g => {
      const X = v => 2 * v, Y = v => 2 * v;
      [rx - 3, rx + rw + 1].forEach(p => {
        px(g, X(p), Y(y + 2), 4, 40, P.o); px(g, X(p), Y(y + 3), 2, 36, F.hullD); px(g, X(p) + 1, Y(y + 3), 1, 36, F.hull);
        px(g, X(p), Y(y + 3), 4, 2, F.cyan); px(g, X(p) + 1, Y(y + 3), 2, 1, F.cyanL);
        for (let k = 14; k < 40; k += 8) px(g, X(p) + 1, Y(y + 2) + k, 2, 1, F.cyanD);
      });
      px(g, X(rx - 4), Y(y), X(rw + 8), 6, P.o); px(g, X(rx - 3), Y(y + 1), X(rw + 6), 2, F.magenta); px(g, X(rx - 3), Y(y + 1), X(rw + 6), 1, 'rgba(255,255,255,.35)');
      if (busy.size && !state.reduce) for (let k = 0; k < 4; k++) if (blink(t + k * 90, 180)) {
        px(g, X(rx - 1), Y(y + 5 + k * 4), X(rw + 2), 1, F.magenta); px(g, X(rx - 1), Y(y + 5 + k * 4) + 1, X(rw + 2), 1, 'rgba(240,72,192,.35)');
      }
      px(g, X(x0), Y(y + 3), X(w), 40, P.o); px(g, X(x0 + 1), Y(y + 4), X(w - 2), 36, F.hullD); px(g, X(x0 + 1), Y(y + 4), X(w - 2), 2, F.hull);
      for (let k = X(x0) + 10; k < X(x0 + w) - 2; k += 10) px(g, k, Y(y + 5), 1, 32, 'rgba(0,0,0,.2)');
      gh.tables.forEach(([tx, ty], k) => {
        const on = busy.has(k + 1);
        px(g, X(tx + 4), Y(ty - 9), 12, 12, P.o); px(g, X(tx + 5), Y(ty - 8), 8, 8, on ? F.cyanL : F.navy);
        if (on) { px(g, X(tx + 5) + 1, Y(ty - 8) + 2, 6, 1, F.cyan); px(g, X(tx + 5) + 1, Y(ty - 8) + 5, 4, 1, F.cyan); }
        else px(g, X(tx + 5), Y(ty - 8), 2, 1, 'rgba(255,255,255,.2)');
        px(g, X(tx), Y(ty + 5), 20, 6, P.o); px(g, X(tx + 1), Y(ty + 5), 16, 4, on ? F.cyan : F.deckD);
        px(g, X(tx + 1), Y(ty + 5), 16, 1, on ? F.cyanL : F.deck);
        if (on && !state.reduce && blink(t + k * 70, 140)) px(g, X(tx + 2 + (Math.floor(t / 140) % 6)), Y(ty + 4), 2, 2, F.cyanL);
      });
    });
  },
  /* The cargo shuttle that brings new issues down the conduit (futureport.js). */
  supply(c, x, t, load) {
    const y = !state.reduce && blink(t, 500) ? 1 : 0;
    futureCargo(c, x, t, y);
    for (let k = 0; k < Math.min(load, 3); k++) this.crate(c, Math.round(x) + 2 + k * 5, y, 'workshop', null);
  },
  /* Station kit at 2× (#465): an antenna with rungs, a solar panel with
   * cells, a glass dome, a maintenance drone, light bollards, a vent grille. */
  decor(c, x, y, k, t) {
    switch (k) {
      case 0: // antenna
        shadow(c, x + 3, y + 7, 3, 1);
        detail(c, x, y, g => {
          px(g, 6, 2, 2, 14, F.hull); px(g, 6, 2, 1, 14, F.hullL); px(g, 2, 6, 10, 2, F.hullD); px(g, 4, 11, 6, 1, F.hullD);
          px(g, 4, 0, 6, 4, P.o); const on = blink(t + x * 37, 900); px(g, 6, 0, 2, 2, on ? P.p0 : F.magentaD); if (on) px(g, 6, 0, 1, 1, P.white);
        }); break;
      case 1: // solar panel
        detail(c, x, y, g => {
          px(g, 6, 8, 2, 6, F.hullD); px(g, 0, 0, 16, 8, P.o); px(g, 2, 2, 12, 4, F.panel);
          for (let cx = 5; cx < 14; cx += 3) px(g, cx, 2, 1, 4, F.panelL); px(g, 2, 2, 12, 1, F.panelL); px(g, 2, 4, 12, 1, 'rgba(0,0,0,.2)');
          px(g, 10, 3, 2, 1, P.white);
        }); break;
      case 2: // small dome
        shadow(c, x + 1, y + 6, 7, 1);
        detail(c, x, y, g => {
          [[5, 2, 6], [3, 3, 10], [2, 4, 12]].forEach(([dx, dy, w]) => px(g, dx, dy, w, 1, P.o));
          px(g, 2, 5, 12, 7, P.o); px(g, 4, 4, 8, 1, F.glass); px(g, 3, 5, 10, 5, F.glass);
          px(g, 4, 5, 3, 1, F.cyanL); px(g, 4, 6, 1, 2, P.white); px(g, 2, 10, 12, 2, F.hullD); px(g, 4, 10, 1, 1, F.cyanL); px(g, 11, 10, 1, 1, F.cyanL);
        }); break;
      case 3: { // maintenance drone, hovering: rotors blurred, a lens with a glint, a skid
        const b = blink(t + y * 29, 400) ? 1 : 0;
        shadow(c, x + 2, y + 7, 4, 1);
        detail(c, x, y + b, g => {
          px(g, 3, 2, 10, 6, P.o); px(g, 2, 3, 12, 4, P.o); px(g, 4, 4, 8, 2, F.hull); px(g, 6, 4, 2, 2, F.cyan);
          px(g, 0, 0, 4, 1, blink(t + x * 7, 60) ? F.hull : F.hullD); px(g, 12, 0, 4, 1, blink(t + x * 7 + 30, 60) ? F.hull : F.hullD);
          px(g, 3, 1, 1, 2, P.o); px(g, 12, 1, 1, 2, P.o);
          px(g, 6, 4, 1, 1, P.white); px(g, 4, 8, 8, 1, F.hullL); px(g, 4, 9, 1, 1, F.rivet); px(g, 11, 9, 1, 1, F.rivet);
        });
        break;
      }
      case 4: // light bollards
        detail(c, x, y, g => {
          [2, 10].forEach(bx => { px(g, bx, 4, 4, 8, P.o); px(g, bx + 1, 6, 2, 5, F.hullD); const on = blink(t + (x + bx / 2) * 13, 1400); px(g, bx, 4, 4, 2, on ? F.cyanL : F.cyan); if (on) px(g, bx + 1, 4, 2, 1, P.white); });
          px(g, 0, 12, 16, 2, F.deckL); px(g, 0, 12, 16, 1, F.deck);
        }); break;
      default: // vent grille
        detail(c, x, y, g => {
          px(g, 0, 2, 16, 10, P.o);
          for (let k = 0; k < 8; k++) px(g, 2, 3 + k, 12, 1, k % 2 ? F.deck : F.deckL);
          px(g, 1, 3, 1, 1, F.rivet); px(g, 14, 3, 1, 1, F.rivet); px(g, 1, 10, 1, 1, F.rivet); px(g, 14, 10, 1, 1, F.rivet);
        });
    }
  },
  /* Hull trim with a starfield outside it, at 2×: stars of two sizes and
   * colours, the trim lit along its inner edge. */
  border(c, W, H) {
    detail(c, 0, 0, g => {
      px(g, 0, 2 * H - 6, 2 * W, 6, F.space); px(g, 0, 22, 6, 2 * H - 22, F.space); px(g, 2 * W - 6, 22, 6, 2 * H - 22, F.space);
      for (let x = 1; x < 2 * W; x += 7) { const star = hash(x, H); px(g, x, 2 * H - 3 - (x % 3), star < 0.15 ? 2 : 1, star < 0.15 ? 2 : 1, star < 0.5 ? P.white : F.cyanL); }
      for (let y = 26; y < 2 * H - 6; y += 9) { px(g, 1 + (y % 3), y, 1, 1, hash(1, y) < 0.3 ? F.cyanL : P.white); px(g, 2 * W - 3 + (y % 2), y + 4, 1, 1, hash(2, y) < 0.3 ? P.white : F.cyanL); }
      px(g, 6, 2 * H - 6, 2 * W - 12, 2, F.magentaD); px(g, 6, 22, 2, 2 * H - 28, F.magentaD); px(g, 2 * W - 8, 22, 2, 2 * H - 28, F.magentaD);
      px(g, 6, 2 * H - 7, 2 * W - 12, 1, F.magenta); px(g, 8, 22, 1, 2 * H - 29, F.magenta); px(g, 2 * W - 9, 22, 1, 2 * H - 29, F.magenta);
    });
  },
  people: 'future', robots: true,
  steam: [F.cyanL, F.hullD], sign: [P.o, F.navy, F.cyan]
};

registerTheme('future', FUTURE);
