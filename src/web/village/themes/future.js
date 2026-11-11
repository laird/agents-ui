/* The Future theme. */
import { state, now } from '../state.js';
import { F, KINDCOL, P, PRIOCOL, R, SHADE, blink, hash, shadow } from '../palette.js';
import { prioRun } from '../cvdmarks.js';
import { detail } from '../detail.js';
import { planks, px } from '../art2x.js';
import { CRATE_W, CRATE_H, seal, stencil } from '../crates.js';
import { registerTheme } from './registry.js';
import { FB, at2x, futureBoard, futureDock, futureHall, futureScaffold } from './futurebuildings.js';
/* ---------- Future ----------
 * A space station worked by little robots: deck plates for ground, light-
 * strip walkways for roads, a plasma canal, habitat pods with portholes. It
 * keeps the shared status language: lit portholes while working, an open
 * hatch for blocked, dark glass for unknown. */
export const FUTURE = {
  name: 'Future',
  pair: 'modern',
  // A broken build in orbit: a red alert, pulsing, with light bars and roof alarms.
  weather: { style: 'alert', cloud: '#501828', cloudD: '#300818', rain: '#ff7090', tint: 'rgba(224,24,48,0.20)', alarm: '#ff2040' },
  labels: { docks: 'Cargo bay', board: 'Holo-board', houses: 'Pods', quay: 'Launch rail', castle: 'Spaceport', hall: 'Command dome', ship: 'Shuttle', sails: 'launches', gate: 'Scanner arch' },
  towers: [[2, -58, 9, 50], [13, -55, 18, 45]],
  /* Deck plates at 2× (#453): each seam lit along one side, so the plates
   * read as bevelled; rivets with a cap; now and then a vent slot. */
  ground(c, x0, y0, w, h) {
    R(c, x0, y0, w, h, F.deckD);
    for (let x = x0 + 7; x < x0 + w; x += 8) R(c, x, y0, 1, h, F.seam);
    for (let y = y0 + 7; y < y0 + h; y += 8) R(c, x0, y, w, 1, F.seam);
    detail(c, x0, y0, g => {
      for (let x = 7; x < w; x += 8) if (x + 1 < w) px(g, 2 * x + 2, 0, 1, 2 * h, F.deck);
      for (let y = 7; y < h; y += 8) if (y + 1 < h) px(g, 0, 2 * y + 2, 2 * w, 1, F.deck);
      for (let y = 1; y < h; y += 16) for (let x = 1; x < w; x += 16) { px(g, 2 * x, 2 * y, 2, 2, F.rivet); px(g, 2 * x, 2 * y, 1, 1, F.deckL); }
      for (let y = 3; y + 2 < h; y += 8) for (let x = 2; x + 4 < w; x += 8) if (hash(x0 + x, y0 + y) < 0.12) { px(g, 2 * x, 2 * y, 7, 1, F.seam); px(g, 2 * x, 2 * y + 2, 7, 1, F.seam); }
    });
  },
  /* Walkways with a glowing light strip down the middle. */
  /* At 2× the edges are a detail pixel of trim and a detail pixel of glow,
   * and each light in the strip has a white-hot core. */
  road(c, x0, y0, w, h) {
    R(c, x0, y0, w, h, F.deckL);
    detail(c, x0, y0, g => {
      const W = 2 * w, H = 2 * h;
      if (w > h && h >= 4) {
        px(g, 0, 0, W, 1, F.cyanD); px(g, 0, 1, W, 1, F.cyan); px(g, 0, H - 1, W, 1, F.cyanD); px(g, 0, H - 2, W, 1, F.deck);
        for (let x = 1; x < w - 2; x += 4) { px(g, 2 * x, 2 * (h >> 1), 4, 2, F.cyan); px(g, 2 * x + 1, 2 * (h >> 1), 2, 1, F.cyanL); }
      }
      if (h > w && w >= 4) {
        px(g, 0, 0, 1, H, F.cyanD); px(g, 1, 0, 1, H, F.cyan); px(g, W - 1, 0, 1, H, F.cyanD); px(g, W - 2, 0, 1, H, F.deck);
        for (let y = 1; y < h - 2; y += 4) { px(g, 2 * (w >> 1), 2 * y, 2, 4, F.cyan); px(g, 2 * (w >> 1), 2 * y + 1, 1, 2, F.cyanL); }
      }
    });
  },
  yard(c, x0, y0, w, h) {
    R(c, x0, y0, w, h, F.deck);
    for (let x = x0 + 3; x < x0 + w; x += 4) R(c, x, y0, 1, h, F.deckD);
    for (let y = y0 + 3; y < y0 + h; y += 4) R(c, x0, y, w, 1, F.deckD);
    // A lit rivet where every other pair of seams cross.
    detail(c, x0, y0, g => { for (let y = 3; y < h; y += 8) for (let x = 3; x < w; x += 8) px(g, 2 * x, 2 * y, 1, 1, F.deckL); });
  },
  /* Rivers and canals are plasma conduits; their pulses flow. */
  channel(c, x0, y0, w, h, t, flow) {
    R(c, x0, y0, w, h, F.space);
    R(c, x0, y0, w, 1, F.magentaD); R(c, x0, y0 + h - 1, w, 1, F.magentaD);
    const off = Math.floor(t / 60 * flow) % 10, mid = h >> 1;
    // At 2×: a glow just inside each wall, and pulses with a hot core and a trail.
    detail(c, x0, y0, g => {
      const W = 2 * w, H = 2 * h;
      px(g, 0, 2, W, 1, F.magentaD); px(g, 0, H - 3, W, 1, F.magentaD);
      for (let x = -10 + off; x < w; x += 10) if (x >= 0) {
        const X = 2 * x, Y = 2 * mid;
        px(g, X, Y, Math.min(8, W - X), 2, F.plasmaL); px(g, X + 5, Y, Math.min(3, W - X - 5), 1, P.white);
        if (X + 10 < W) px(g, X + 10, Y - 4, Math.min(4, W - X - 10), 1, F.plasma);
        if (X + 4 < W) px(g, X + 4, Y + 4, Math.min(4, W - X - 4), 1, F.cyanD);
      }
    });
  },
  /* Groves become station kit: an antenna mast, a solar array or a small dome. */
  tree(c, x, y) {
    const k = Math.floor(hash(x, y) * 3);
    // At 2× (#453): rungs and lit tips on the mast, a finer cell grid and a
    // glint on the array, a dome with a highlight curve and lit base lights.
    if (k === 0) {
      R(c, x + 4, y + 9, 5, 3, P.o); R(c, x + 5, y + 9, 3, 2, F.hullD);
      R(c, x + 6, y + 1, 1, 8, F.hull); R(c, x + 3, y + 3, 7, 1, F.hullD); R(c, x + 4, y + 6, 5, 1, F.hullD);
      R(c, x + 5, y, 3, 2, P.o); R(c, x + 6, y, 1, 1, blink(now() + x * 31, 900) ? P.p0 : F.magentaD);
      detail(c, x, y, g => {
        for (let yy = 4; yy < 18; yy += 3) px(g, 13, yy, 1, 1, F.hullD);
        px(g, 6, 6, 1, 1, F.cyanL); px(g, 19, 6, 1, 1, F.cyanL); px(g, 8, 12, 1, 1, F.cyan); px(g, 17, 12, 1, 1, F.cyan);
        px(g, 11, 19, 1, 1, F.rivet); px(g, 14, 19, 1, 1, F.rivet);
      });
    } else if (k === 1) {
      R(c, x + 5, y + 7, 2, 5, F.hullD);
      R(c, x, y + 2, 12, 6, P.o); R(c, x + 1, y + 3, 10, 4, F.panel);
      detail(c, x, y, g => {
        planks(g, 2, 6, 20, 8, 2, F.panelL);
        for (let xx = 5; xx < 22; xx += 3) px(g, xx, 6, 1, 8, F.panelL);
        px(g, 2, 6, 20, 1, F.panelL);
        for (let k2 = 0; k2 < 4; k2++) px(g, 13 + k2, 7 + k2, 1, 1, P.white);
        px(g, 11, 14, 1, 10, F.hull);
      });
    } else {
      [3, 4, 5, 5].forEach((w, dy) => { R(c, x + 6 - w, y + 4 + dy, 2 * w, 1, P.o); if (dy) R(c, x + 7 - w, y + 4 + dy, 2 * w - 2, 1, dy === 1 ? F.cyanL : F.glass); });
      R(c, x + 1, y + 8, 10, 3, P.o); R(c, x + 2, y + 8, 8, 2, F.hullD);
      detail(c, x, y, g => {
        px(g, 8, 11, 3, 1, P.white); px(g, 7, 12, 1, 2, P.white); px(g, 16, 14, 1, 2, F.cyanD);
        [6, 10, 14, 18].forEach((lx, i) => px(g, lx, 18, 1, 1, blink(now() + x * 17 + i * 200, 800) ? F.cyanL : F.cyan));
      });
    }
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
  /* An airlock: two pylons with a force field between them and a status light on the control tower. */
  gate(c, x, cy, open, flag) {
    R(c, x - 4, cy - 15, 12, 10, P.o); R(c, x - 3, cy - 14, 10, 8, F.hull); R(c, x - 2, cy - 12, 8, 3, F.glass);
    R(c, x - 5, cy - 16, 14, 2, F.hullD);
    R(c, x + 1, cy - 20, 4, 4, P.o); R(c, x + 2, cy - 19, 2, 2, flag);
    [x - 1, x + 5].forEach(px => { R(c, px, cy - 4, 2, 22, P.o); R(c, px, cy - 3, 1, 20, F.hullD); R(c, px, cy - 3, 2, 1, F.cyan); R(c, px, cy + 16, 2, 1, F.cyan); });
    if (!open) {
      R(c, x + 1, cy, 4, 14, 'rgba(240,72,192,.35)');
      const t = now();
      for (let k = 0; k < 14; k += 3) R(c, x + 1 + ((k + Math.floor(t / 150)) % 4), cy + k, 1, 1, F.magenta);
    }
  },
  /* The spaceport: released work leaves on a shuttle. The sky lights up
   * when it does. The shuttle is drawn by vessel() below, so it can load and launch. */
  destination(c, x, cy, t, fw) {
    R(c, x, cy - 8, 38, 8, P.o); R(c, x + 1, cy - 7, 36, 6, F.hullD);
    for (let k = 2; k < 36; k += 6) R(c, x + k, cy - 4, 3, 1, F.cyan);
    R(c, x + 3, cy - 46, 6, 38, P.o); R(c, x + 4, cy - 45, 4, 36, F.hull);
    for (let yy = cy - 42; yy < cy - 10; yy += 5) R(c, x + 5, yy, 2, 2, F.cyanL);
    R(c, x + 2, cy - 52, 8, 6, P.o); R(c, x + 3, cy - 51, 6, 4, F.glass);
    R(c, x + 5, cy - 56, 1, 4, F.hull); R(c, x + 4, cy - 57, 3, 1, blink(t, 700) || t - fw < 1600 ? P.p0 : F.magentaD);
    // At 2×: the tower's panel seams and a lit edge, glazing bars in the
    // control cab, seams along the pad's skirt, and a landing ring on the pad.
    detail(c, x, cy, g => {
      planks(g, 8, -90, 8, 72, 6, F.hullD); px(g, 8, -90, 1, 72, F.hullL);
      px(g, 9, -102, 1, 8, P.o); px(g, 14, -102, 1, 8, P.o); px(g, 7, -102, 2, 1, P.white);
      for (let k = 6; k < 72; k += 12) px(g, k, -14, 1, 12, F.hull);
      px(g, 22, 6, 34, 1, F.cyanD); px(g, 22, 24, 34, 1, F.cyanD); px(g, 22, 6, 1, 19, F.cyanD); px(g, 55, 6, 1, 19, F.cyanD);
    });
    const ft = t - fw;
    if (ft >= 0 && ft < 1600) {
      const p = ft / 1600;
      [[x + 6, P.gold], [x + 21, F.magenta], [x + 34, F.cyan]].forEach(([fx, col], i) => {
        const fy = cy - 58 - p * 18 - i * 3;
        for (let k = 0; k < 8; k++) { const a = k / 8 * Math.PI * 2; R(c, fx + Math.cos(a) * p * 9, fy + Math.sin(a) * p * 9, 1, 1, col); }
      });
    }
    R(c, x + 8, cy - 2, 22, 18, P.o); R(c, x + 9, cy, 20, 15, F.space);
    for (let k = 0; k < 4; k++) R(c, x + 11 + k * 5, cy + 7, 2, 1, F.cyanD);
  },
  /* The shuttle for the next release, with its cargo rack, moved by (ox, oy).
   * Its engines fire while it climbs. */
  vessel(c, x, cy, t, ox, oy) {
    const rx = Math.round(x + 17 + ox), sy = Math.round(cy - 40 + oy);
    R(c, rx + 3, sy - 6, 6, 4, P.o); R(c, rx + 4, sy - 5, 4, 3, F.hullL);
    R(c, rx + 1, sy - 2, 10, 32, P.o); R(c, rx + 2, sy - 1, 8, 30, F.hullL); R(c, rx + 2, sy - 1, 2, 30, F.hull);
    R(c, rx + 2, sy + 8, 8, 2, F.magenta); R(c, rx + 4, sy + 2, 4, 3, P.o); R(c, rx + 5, sy + 3, 2, 1, F.cyan);
    R(c, rx - 2, sy + 20, 3, 10, P.o); R(c, rx - 1, sy + 21, 2, 8, F.magentaD);
    R(c, rx + 11, sy + 20, 3, 10, P.o); R(c, rx + 11, sy + 21, 2, 8, F.magentaD);
    R(c, rx + 15, sy + 14, 1, 18, F.hullD); R(c, rx + 15, sy + 31, 12, 1, F.cyanD);
    if (oy < 0) for (let k = 0; k < 4; k++) R(c, rx + 2 + k * 2, sy + 30 + (blink(t + k * 60, 80) ? 1 : 0), 2, 3 + (k % 2) * 2, k % 2 ? F.cyan : F.cyanL);
    // At 2×: a lit edge down the hull and its panel seams, a highlight on the
    // nose, the cockpit's glazing, and a white-hot core in each engine flame.
    detail(c, rx, sy, g => {
      px(g, 8, 0, 1, 58, F.hull); planks(g, 4, -2, 16, 18, 6, F.hull); planks(g, 4, 20, 16, 38, 6, F.hull);
      px(g, 9, -10, 3, 1, P.white); px(g, 11, 6, 2, 1, P.white);
      px(g, -2, 42, 1, 16, F.magenta); px(g, 23, 42, 1, 16, F.magenta);
      if (oy < 0) for (let k = 0; k < 4; k++) px(g, 5 + k * 4, 61 + (blink(t + k * 60, 80) ? 2 : 0), 1, 2 + (k % 2) * 2, P.white);
    });
  },
  hold: [[33, -14], [38, -14], [33, -19], [38, -19], [33, -24], [38, -24]], holdBadge: [31, -32], hull: [14, -58, 30, 50], flies: true,
  /* The cargo shuttle that brings new issues down the conduit, hovering. */
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
  supply(c, x, t, load) {
    x = Math.round(x);
    const y = !state.reduce && blink(t, 500) ? 1 : 0;
    R(c, x, y + 4, 27, 5, P.o); R(c, x + 1, y + 5, 25, 3, F.hull); R(c, x + 1, y + 5, 25, 1, F.hullL); R(c, x + 26, y + 5, 3, 3, P.o); R(c, x + 26, y + 6, 2, 1, F.cyan);
    R(c, x + 18, y + 2, 7, 3, P.o); R(c, x + 19, y + 3, 5, 1, F.glass);
    R(c, x - 2, y + 5, 2, 3, blink(t, 120) ? F.cyan : F.cyanL);
    // At 2×: hull seams and rivets, a glint on the canopy, a hot core in the thruster.
    detail(c, x, y, g => {
      for (let k = 8; k < 52; k += 10) { px(g, k, 10, 1, 6, F.hullD); px(g, k + 3, 14, 1, 1, F.rivet); }
      px(g, 40, 6, 2, 1, P.white); px(g, -3, 12, 2, 2, P.white);
    });
    for (let k = 0; k < Math.min(load, 3); k++) this.crate(c, x + 2 + k * 5, y, 'workshop', null);
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

at2x(FUTURE.gatehouse);
registerTheme('future', FUTURE);
