/* ---------- The Future theme's airlock, spaceport, shuttle and cargo shuttle at 2× (#472) ----------
 * Each draws in detail pixels (detail.js) on the scene pixels its 1× art
 * covered (tools/village-tests/future2x.test.mjs holds the footprints).
 * themes/future.js puts them in its gate, destination, vessel and supply. */
import { state, now } from '../state.js';
import { F, P, blink } from '../palette.js';
import { detail } from '../detail.js';
import { pixmap, planks, px } from '../art2x.js';
import { rounded } from '../glyphs2x.js';
import { wfFireworks } from '../fireworks.js';

const POD = { outline: P.o, fill: F.hull, light: F.hullL, dark: F.hullD };
const FIELD = 'rgba(240,72,192,.35)', FIELD_L = 'rgba(240,72,192,.6)';
/* An airlock between stops: a control pod with the stop's light on top, two
 * emitter pylons, and a shimmering force field between them while shut. */
export function futureGate(c, x, cy, open, flag) {
  const t = now();
  detail(c, x, cy, g => {
    rounded(g, -8, -30, 24, 20, POD);
    px(g, -4, -24, 16, 6, P.o); px(g, -3, -23, 14, 4, F.glass); px(g, -3, -23, 14, 1, F.cyanL);
    px(g, -2, -22, 2, 1, P.white); px(g, -7, -14, 22, 1, F.cyan);
    px(g, -10, -32, 28, 4, P.o); px(g, -9, -31, 26, 2, F.hullD); px(g, -9, -31, 26, 1, F.hull);
    px(g, 3, -40, 6, 1, P.o); px(g, 2, -39, 8, 7, P.o);
    px(g, 4, -38, 4, 4, flag); px(g, 4, -38, 1, 1, P.white);
    // The pylons: glowing emitters top and bottom, status lights between.
    [-2, 10].forEach(X => {
      px(g, X, -8, 4, 44, P.o); px(g, X + 1, -6, 2, 40, F.hullD); px(g, X + 1, -6, 1, 40, F.hull);
      [-6, 32].forEach(Y => { px(g, X, Y, 4, 2, F.cyan); px(g, X + 1, Y, 2, 1, F.cyanL); });
      for (let k = 2; k < 30; k += 8) px(g, X + 1, k, 2, 1, F.cyanD);
    });
    if (!open) {
      px(g, 2, 0, 8, 28, FIELD);
      const s = Math.floor(t / 120);
      for (let k = 0; k < 28; k += 7) px(g, 2, (k + s) % 28, 8, 1, FIELD_L);
      for (let k = 0; k < 28; k += 5) px(g, 3 + ((k + s) % 6), k, 1, 1, F.magenta);
    }
  });
}

/* The spaceport: a deck with running lights, a tower with a glazed control
 * cab and a beacon, and the launch pit with its landing ring. */
export function futurePort(c, x, cy, t, fw) {
  const lit = blink(t, 700) || t - fw < 1600, chase = state.reduce ? -1 : Math.floor(t / 300) % 6;
  detail(c, x, cy, g => {
    px(g, 0, -16, 76, 16, P.o); px(g, 1, -15, 74, 14, F.hullD);
    px(g, 1, -15, 74, 1, F.hull); px(g, 1, -3, 74, 2, F.deckD);
    for (let k = 6; k < 72; k += 12) px(g, k, -14, 1, 12, F.hull);
    for (let k = 4, i = 0; k < 72; k += 12, i++) { px(g, k, -8, 6, 2, i === chase ? F.cyanL : F.cyan); px(g, k + 1, -8, 4, 1, F.cyanL); }
    // The tower, lit on the left, its panels seamed, a column of lights.
    px(g, 6, -92, 12, 76, P.o); px(g, 7, -91, 10, 74, F.hull);
    px(g, 7, -91, 2, 74, F.hullL); px(g, 14, -91, 3, 74, F.hullD);
    planks(g, 7, -91, 10, 74, 8, F.hullD);
    for (let y = -84, i = 0; y < -22; y += 10, i++) { px(g, 10, y, 4, 4, P.o); px(g, 11, y + 1, 2, 2, (i + 6 - chase) % 6 === 0 ? P.white : F.cyanL); }
    // The control cab: rounded glazing with bars and a glint; the aerial and its beacon.
    rounded(g, 4, -104, 16, 12, { outline: P.o, fill: F.glass, light: F.cyanL, dark: F.glassD });
    px(g, 9, -102, 1, 8, P.o); px(g, 14, -102, 1, 8, P.o); px(g, 6, -102, 2, 1, P.white);
    px(g, 10, -112, 2, 8, F.hull); px(g, 11, -112, 1, 8, F.hullD);
    px(g, 8, -114, 6, 2, lit ? P.p0 : F.magentaD); if (lit) px(g, 10, -114, 2, 1, '#ffb0b0');
  });
  wfFireworks(c, x, cy, t, fw, [[6, P.gold], [21, F.magenta], [34, F.cyan]], 58);
  detail(c, x, cy, g => {
    // The launch pit: deep space under a landing ring and its lights.
    px(g, 16, -4, 44, 36, P.o); px(g, 18, 0, 40, 30, F.space);
    px(g, 22, 6, 34, 1, F.cyanD); px(g, 22, 24, 34, 1, F.cyanD); px(g, 22, 6, 1, 19, F.cyanD); px(g, 55, 6, 1, 19, F.cyanD);
    for (let k = 0; k < 4; k++) { px(g, 22 + k * 10, 14, 4, 2, F.cyanD); px(g, 23 + k * 10, 14, 2, 1, k === chase % 4 ? F.cyanL : F.cyan); }
    for (let k = 0; k < 6; k++) px(g, 19 + ((k * 13) % 38), 2 + ((k * 7) % 26), 1, 1, F.hullD);
  });
}

/* The shuttle for the next release, with its cargo rack, moved by (ox, oy).
 * Its engines fire while it climbs. */
const SHUTTLE_NOSE = [
  '...oooooo...', '..oLLLLLho..', '.oLLLLLLhho.', '.oLLLLLLhho.',
  'oLLLLLLLLhho', 'oLLLLLLLLhho', 'oLLLLLLLLhho', 'oLLLLLLLLhho',
];
export function futureShuttle(c, x, cy, t, ox, oy) {
  const rx = Math.round(x + 17 + ox), sy = Math.round(cy - 40 + oy);
  detail(c, rx, sy, g => {
    pixmap(g, 6, -12, SHUTTLE_NOSE, { o: P.o, L: F.hullL, h: F.hull });
    px(g, 8, -11, 2, 1, P.white);
    // The hull: lit on the left, panel seams, a magenta stripe, the cockpit.
    px(g, 2, -4, 20, 64, P.o); px(g, 3, -4, 18, 63, F.hullL);
    px(g, 5, -4, 1, 63, P.white); px(g, 16, -4, 5, 63, F.hull); px(g, 19, -4, 2, 63, F.hullD);
    planks(g, 3, -2, 18, 18, 6, F.hull); planks(g, 3, 20, 18, 38, 6, F.hull);
    px(g, 3, 16, 18, 4, F.magenta); px(g, 3, 16, 18, 1, '#ff90dc'); px(g, 16, 16, 5, 4, F.magentaD);
    px(g, 9, 4, 6, 1, P.o); px(g, 8, 5, 8, 4, P.o); px(g, 9, 9, 6, 1, P.o);
    px(g, 9, 5, 6, 4, F.cyan); px(g, 10, 5, 2, 1, P.white); px(g, 13, 7, 2, 1, F.cyanD);
    px(g, 5, 55, 14, 4, F.hullD); px(g, 5, 55, 14, 1, F.hull);
    // Fins, swept back, lit on their leading edges.
    for (let j = 0; j < 20; j++) {
      const w = Math.min(6, 1 + Math.floor(j / 3)), y = 40 + j;
      px(g, 2 - w, y, w, 1, P.o); px(g, 22, y, w, 1, P.o);
      if (w > 1 && j < 19) { px(g, 3 - w, y, w - 1, 1, F.magentaD); px(g, 22, y, w - 1, 1, F.magentaD); px(g, 3 - w, y, 1, 1, F.magenta); }
    }
    // The cargo rack.
    px(g, 30, 28, 2, 36, F.hullD); px(g, 30, 28, 1, 36, F.hull);
    px(g, 30, 62, 24, 2, F.cyanD); px(g, 30, 62, 24, 1, F.cyan);
    // Engine flames with white-hot cores, flickering as it climbs.
    if (oy < 0) for (let k = 0; k < 4; k++) {
      const f = blink(t + k * 60, 80) ? 2 : 0, h = 6 + (k % 2) * 4, X = 4 + 4 * k;
      px(g, X, 60 + f, 4, h, k % 2 ? F.cyan : F.cyanL);
      px(g, X + 1, 61 + f, 2, h - 3, k % 2 ? F.cyanL : P.white);
    }
  });
}

/* The cargo shuttle that brings new issues down the conduit, hovering. Its
 * crates ride on top (themes/future.js draws them); `y` is its bob. */
export function futureCargo(c, x, t, y) {
  const Y = 2 * y;
  detail(c, Math.round(x), 0, g => {
    rounded(g, 0, Y + 8, 54, 10, POD);
    for (let k = 8; k < 52; k += 10) { px(g, k, Y + 10, 1, 6, F.hullD); px(g, k + 3, Y + 14, 1, 1, F.rivet); }
    px(g, 2, Y + 15, 50, 1, F.cyanD);
    // The nose, with a running light; the canopy, with a glint.
    px(g, 52, Y + 10, 4, 6, P.o); px(g, 56, Y + 11, 2, 4, P.o);
    px(g, 52, Y + 11, 3, 4, F.hull); px(g, 54, Y + 12, 2, 2, F.cyan);
    rounded(g, 36, Y + 4, 14, 7, { outline: P.o, fill: F.glass, light: F.cyanL });
    px(g, 39, Y + 5, 2, 1, P.white);
    // The thruster's glow, flickering unless motion is reduced.
    px(g, -4, Y + 10, 4, 6, !state.reduce && blink(t, 120) ? F.cyan : F.cyanL); px(g, -3, Y + 12, 2, 2, P.white);
  });
}
