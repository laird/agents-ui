/* ---------- The Future theme's buildings at 2× (#465) ----------
 * No DOM, so node tests it (tools/village-tests/futurebuildings.test.mjs).
 * themes/future.js draws its houses, hall, board, dock and scaffold with these. */
import { now } from '../state.js';
import { F, P, SHADE, blink, hash } from '../palette.js';
import { note } from '../cvdmarks.js';
import { puffs } from '../sprites/houses.js';
import { detail } from '../detail.js';
import { disc, px } from '../art2x.js';

/* Each draws in detail pixels (detail.js) on the scene pixels it always
 * covered: pods with a smooth dome, round portholes and panel seams, the
 * command dome with its ribs and meridians, the holo-board, the cargo bay,
 * the scaffold. (The scanner arch is in future.js, redrawn the same way.) */
/* Portholes: lit cyan-white while working, dark when nobody can say. */
const fWin = st => st === 'working' ? F.cyanL : (st === 'unknown' || st === 'stopped') ? P.dark : F.glassD;
/* A porthole 4 scene pixels across at detail (X, Y): a round frame, its
 * glass shaded, a glint on it while working. */
function porthole(g, X, Y, st) {
  const lit = st === 'working';
  disc(g, X, Y, 8, { outline: P.o, fill: fWin(st), light: lit ? P.white : fWin(st), dark: lit ? F.cyan : P.o });
  if (lit) px(g, X + 2, Y + 2, 1, 1, P.white);
}
/* A habitat pod, in detail pixels from its origin: a rounded module under
 * a smooth domed roof with a highlight, two round portholes, panel seams
 * and rivets, and an arched hatch with a window. The hatch stands open
 * (dark) while its robot waits on you. */
function pod(g, st, o = {}) {
  const shell = o.shell || F.hull, shellD = o.shellD || F.hullD, roof = o.roof || F.hullL;
  for (let j = 2; j < 12; j++) {
    const hw = Math.max(7, Math.round(16 * Math.sqrt(1 - ((12 - j) / 11) ** 2)));
    px(g, 16 - hw, j, 2 * hw, 1, P.o);
    if (j > 2) px(g, 17 - hw, j, 2 * hw - 2, 1, j < 5 ? roof : shell);
    if (j > 3 && j < 9) px(g, 19 - hw, j, 2, 1, 'rgba(255,255,255,.3)');
  }
  px(g, 0, 10, 32, 22, P.o); px(g, 2, 10, 28, 20, shell);
  px(g, 2, 10, 1, 18, 'rgba(255,255,255,.25)');
  px(g, 2, 28, 28, 2, shellD); px(g, 24, 10, 4, 18, SHADE);
  px(g, 2, 14, 28, 2, o.band || F.cyanD); px(g, 2, 15, 28, 1, 'rgba(0,0,0,.18)');
  [8, 22].forEach(sx => px(g, sx, 17, 1, 11, 'rgba(0,0,0,.18)'));
  [[4, 12], [27, 12], [4, 26], [27, 26]].forEach(([rx, ry]) => px(g, rx, ry, 1, 1, F.rivet));
  if (o.portholes !== false) { porthole(g, 4, 18, st); porthole(g, 20, 18, st); }
  px(g, 13, 20, 6, 12, P.o); px(g, 12, 21, 8, 11, P.o);
  px(g, 14, 22, 4, 10, st === 'blocked' ? P.o : shellD);
  if (st !== 'blocked') { px(g, 15, 23, 2, 2, F.glassD); px(g, 17, 28, 1, 1, F.rivet); }
  px(g, 12, 18, 8, 2, st === 'working' ? F.cyan : F.cyanD); px(g, 13, 18, 6, 1, st === 'working' ? F.cyanL : F.cyanD);
}
/* Draw a pod-based house: the pod, then `extra` over it, both in detail pixels. */
const podHouse = (o, extra) => (c, x, y, st, t) => detail(c, x, y, g => { pod(g, st, typeof o === 'function' ? o(st) : o); if (extra) extra(g, st, t); });
export const FB = {
  cottage: ((c, x, y, st, t) => {
    detail(c, x, y, g => {
      pod(g, st);
      // A comms mast with a lit tip.
      px(g, 24, -6, 2, 10, F.hullD); px(g, 24, -6, 1, 10, F.hull);
      const on = st === 'working' && blink(t, 500);
      px(g, 22, -8, 6, 2, on ? F.cyan : F.cyanD); px(g, 24, -8, 2, 1, on ? P.white : F.cyanD);
    });
    if (st === 'working') puffs(c, x + 4, y - 1, t, F.cyanL, F.hullD);
  }),
  workshop: podHouse({ shell: '#b8b0d0', shellD: '#8078a0' }, (g, st, t) => {
    // A lattice crane arm swinging its hook; sparks fly while it works.
    const a = st === 'working' ? Math.floor(t / 300) % 2 : 0;
    px(g, 8, -6, 4, 10, F.hullD); px(g, 10, -8, 10, 4, F.hullD); px(g, 8, -6, 1, 10, F.hull);
    for (let k = 0; k < 4; k++) px(g, 11 + 2 * k, -7 + (k % 2), 1, 1, 'rgba(0,0,0,.35)');
    px(g, 18, -6 + 2 * a, 4, 6, P.o); px(g, 19, -5 + 2 * a, 2, 4, F.hull);
    if (st === 'working' && blink(t, 120)) {
      px(g, 20, 2 + 2 * a, 1, 1, P.white); px(g, 21, 3 + 2 * a, 1, 1, P.white);
      px(g, 22, 2 * a, 2, 1, P.gold); px(g, 24, 4 + 2 * a, 1, 2, F.cyan);
    }
  }),
  feature: podHouse({ band: F.magentaD }, (g, st, t) => {
    // A lattice tower crane in magenta, its hook on a fine cable.
    px(g, 26, -8, 2, 40, P.o); px(g, 28, -8, 2, 40, F.magenta);
    for (let yy = -6; yy < 32; yy += 4) px(g, 28, yy, 2, 1, F.magentaD);
    px(g, 6, -8, 26, 2, F.magenta); px(g, 6, -6, 26, 2, P.o);
    for (let xx = 6; xx < 32; xx += 4) px(g, xx, -8, 1, 2, F.magentaD);
    const hook = st === 'working' ? 4 + Math.floor((Math.sin(t / 400) + 1) * 2) : 5;
    const hx = 2 * (5 + (st === 'working' ? Math.floor(t / 700) % 3 : 0));
    px(g, hx, -4, 1, 2 * hook, F.cyan);
    px(g, hx - 2, -4 + 2 * hook, 5, 2, F.cyanL); px(g, hx - 2, -2 + 2 * hook, 1, 1, F.cyanL);
  }),
  bug: podHouse({}, (g, st, t) => {
    // A specimen dome on the roof, a bug inside it under a scanning line.
    px(g, 10, -8, 12, 1, 'rgba(64,224,240,.35)'); px(g, 8, -7, 16, 9, 'rgba(64,224,240,.35)');
    px(g, 10, -7, 2, 1, 'rgba(255,255,255,.45)');
    px(g, 13, -6, 6, 6, P.bug); px(g, 12, -5, 8, 4, P.bug); px(g, 16, -6, 1, 6, P.o);
    px(g, 14, -6, 2, 2, P.o); px(g, 16, -4, 2, 2, P.o);
    [[10, -4], [10, -2], [20, -4], [20, -2]].forEach(([lx, ly]) => px(g, lx, ly, 2, 1, P.bug));
    px(g, 14, 2, 4, 2, F.cyan); px(g, 15, 2, 2, 1, F.cyanL);
    if (st === 'working') px(g, 8, -8 + 2 * (Math.floor(t / 160) % 5), 16, 1, F.cyanL);
  }),
  security: podHouse({ shell: F.panel, shellD: P.shield, roof: F.panelL }, (g, st, t) => {
    // A gold shield badge, a sweeping radar, an alarm light.
    px(g, 12, 4, 8, 4, P.gold); px(g, 13, 8, 6, 1, P.gold); px(g, 14, 9, 4, 1, P.gold);
    px(g, 14, 5, 4, 2, P.shield); px(g, 12, 4, 8, 1, '#fff0a0');
    px(g, 6, -4, 2, 10, F.hullD);
    const a = st === 'working' ? t / 300 : 0.6;
    for (let d = 1; d < 9; d++) px(g, 6 + Math.round(Math.cos(a) * d), -4 + Math.round(Math.sin(a) * d * 0.5), 1, 1, d > 6 ? F.hullL : F.hull);
    px(g, 24, -2, 6, 4, P.o);
    const on = st === 'working' && blink(t, 400);
    px(g, 26, -2, 2, 2, on ? P.p0 : F.hullD); if (on) px(g, 26, -2, 1, 1, P.white);
  }),
  tests: ((c, x, y, st, t) => {
    detail(c, x, y, g => {
      pod(g, st, { shell: F.glass, shellD: F.glassD, roof: F.cyanL });
      // Two test tubes, glass lit along one side, a bubble rising in each.
      [6, 22].forEach((tx, k) => {
        px(g, tx, -6, 6, 10, P.o); px(g, tx + 2, -4, 2, 8, k ? P.fizzB : P.fizzA);
        px(g, tx + 1, -5, 1, 8, 'rgba(255,255,255,.3)'); px(g, tx + 2, -3 + ((t / 200 + k * 3) | 0) % 6, 1, 1, P.white);
      });
    });
    if (st === 'working') [x + 3, x + 11].forEach((tx, k) => puffs(c, tx + 1, y - 4, t + k * 500, P.fizzA, P.fizzB));
  }),
  research: podHouse({ roof: F.cyanL }, (g, st, t) => {
    // A telescope tracking across the sky, a lens at its tip.
    const a = -2.2 + (st === 'working' ? Math.sin(t / 900) * 0.5 : 0);
    for (let d = 0; d < 12; d++) px(g, 16 + Math.round(Math.cos(a) * d), 4 + Math.round(Math.sin(a) * d), 2, 2, d < 10 ? F.hullD : P.o);
    const lx = 16 + Math.round(Math.cos(a) * 12), ly = 4 + Math.round(Math.sin(a) * 12);
    px(g, lx, ly, 2, 2, st === 'working' ? P.gold : F.cyanD); px(g, lx, ly, 1, 1, P.white);
  }),
  docs: podHouse({}, (g, st, t) => {
    // A row of data cartridges, each with a light label on its spine.
    [F.magenta, F.cyan, P.gold, F.plasmaL].forEach((col, k) => {
      const dy = 2 * (k % 2);
      px(g, 8 + 4 * k, -6 + dy, 4, 8, P.o); px(g, 9 + 4 * k, -5 + dy, 2, 6, col);
      px(g, 9 + 4 * k, -3 + dy, 2, 1, 'rgba(255,255,255,.5)');
    });
    if (st === 'working') px(g, 8 + 4 * (Math.floor(t / 250) % 4), -8, 2, 1, P.white);
  }),
  refactor: podHouse({}, (g, st, t) => {
    // Half the pod under a magenta rebuild field, fine scan lines across it.
    px(g, 16, 2, 16, 30, 'rgba(240,72,192,.35)');
    for (let yy = 4; yy < 32; yy += 3) px(g, 16, yy, 16, 1, F.magentaD);
    if (st === 'working') px(g, 16, 4 + 2 * (Math.floor(t / 140) % 13), 16, 1, F.magenta);
  }),
  perf: podHouse({ shell: F.hullD, shellD: F.deckL }, (g, st, t) => {
    // Two cooling fans on the roof, round housings, blades and a hub.
    [8, 22].forEach((fx, k) => {
      px(g, fx - 5, -4, 12, 12, P.o); px(g, fx - 6, -3, 14, 10, P.o);
      px(g, fx - 4, -2, 10, 8, F.deck); px(g, fx - 5, -1, 12, 6, F.deck);
      const a = st === 'working' ? t / 90 + k : 0.3;
      for (let d = -4; d <= 4; d++) px(g, fx + Math.round(Math.cos(a) * d), 2 + Math.round(Math.sin(a) * d * 0.7), 1, 1, F.cyan);
      px(g, fx - 1, 1, 2, 2, F.cyanL);
    });
  }),
  chores: ((c, x, y, st, t) => detail(c, x, y, g => {
    // A depot: a flat-roofed block with hazard striping, a roll-up door, a crate.
    px(g, 0, 6, 32, 26, P.o); px(g, 2, 8, 28, 22, F.hullD);
    px(g, 2, 8, 28, 4, F.deckL); px(g, 2, 8, 28, 1, F.hull);
    px(g, 2, 12, 28, 2, P.gold); for (let k = 2; k < 30; k += 4) px(g, k, 12, 2, 2, P.o);
    [[4, 15], [27, 15]].forEach(([rx, ry]) => px(g, rx, ry, 1, 1, F.rivet));
    px(g, 8, 16, 16, 16, P.o); px(g, 10, 18, 12, 14, st === 'blocked' ? P.o : F.deck);
    if (st !== 'blocked') for (let yy = 20; yy < 32; yy += 2) px(g, 10, yy, 12, 1, F.deckD);
    if (st === 'working') { px(g, 10, 18, 12, 2, F.cyan); px(g, 11, 18, 10, 1, F.cyanL); }
    px(g, 24, 24, 8, 8, P.o); px(g, 26, 26, 4, 4, F.magenta); px(g, 26, 26, 2, 1, 'rgba(255,255,255,.4)');
  }))
};
/* The cargo bay's deck edge at 2×: a lit lip, plate seams and rivets,
 * landing lights with a white-hot core. */
export const futureDock = ((c, x, y, w) => detail(c, x, y, g => {
  px(g, 0, 0, 2 * w, 14, P.o); px(g, 2, 0, 2 * w - 4, 12, F.hullD); px(g, 2, 0, 2 * w - 4, 1, F.hull);
  for (let k = 8; k < 2 * w - 2; k += 8) { px(g, k, 1, 1, 8, 'rgba(0,0,0,.25)'); px(g, k + 2, 2, 1, 1, F.rivet); }
  for (let k = 0; k < w - 2; k += 4) { px(g, 2 + 2 * k, 10, 4, 2, F.cyan); px(g, 3 + 2 * k, 10, 2, 1, F.cyanL); }
}));
/* Command dome at 2× (#465): a glass dome with curved ribs and meridians
 * on a hull ring, an antenna with a blinking beacon on top. The hatch goes
 * dark when the manager waits on you. */
export const futureHall = ((c, x, y, st) => {
  const t = now(), lit = st === 'working';
  detail(c, x - 4, y, g => {
    px(g, 40, -32, 2, 24, F.hull); px(g, 40, -32, 1, 24, F.hullL);
    px(g, 34, -24, 14, 2, F.hullD); px(g, 36, -18, 10, 1, F.hullD);
    px(g, 39, -38, 4, 6, P.o); px(g, 38, -37, 6, 4, P.o);
    const beacon = blink(t, 700) ? P.p0 : F.magentaD;
    px(g, 40, -36, 2, 2, beacon); if (beacon === P.p0) px(g, 40, -36, 1, 1, P.white);
    // The dome, row by row on the detail grid: ribs across it, meridians down it.
    for (let j = -26; j < 2; j++) {
      const w = Math.floor(Math.sqrt(28.5 * 28.5 - j * j));
      px(g, 41 - w, 20 + j, 2 * w, 1, P.o);
      if (j > -26) {
        px(g, 42 - w, 20 + j, 2 * w - 2, 1, (j + 26) % 8 === 0 ? F.cyanD : F.glass);
        const m = Math.round(w / 2);
        px(g, 41 - m, 20 + j, 1, 1, F.cyanD); px(g, 41 + m, 20 + j, 1, 1, F.cyanD);
      }
    }
    for (let wy = 0; wy < 20; wy += 8) for (let wx = 24; wx <= 56; wx += 8) {
      px(g, wx, wy, 4, 4, fWin(st)); if (lit) px(g, wx + 1, wy + 1, 1, 1, P.white);
    }
    [[22, -1, 4], [24, -2, 5], [27, -3, 4]].forEach(([hx, hy, hw]) => px(g, hx, hy, hw, 1, 'rgba(255,255,255,.55)'));
    // The hull ring: a lit top edge, a cyan band, landing lights, seams and rivets.
    px(g, 0, 20, 80, 24, P.o); px(g, 2, 22, 76, 20, F.hull);
    px(g, 2, 22, 76, 2, F.hullL); px(g, 2, 28, 76, 2, F.cyanD); px(g, 2, 29, 76, 1, 'rgba(0,0,0,.2)');
    for (let k = 3; k < 38; k += 6) { px(g, 2 * k, 32, 4, 4, F.cyan); px(g, 2 * k + 1, 33, 2, 2, F.cyanL); }
    for (let k = 12; k < 78; k += 12) px(g, k, 37, 1, 5, F.hullD);
    for (let k = 6; k < 78; k += 12) px(g, k, 40, 1, 1, F.rivet);
    px(g, 32, 26, 16, 18, P.o);
    px(g, 34, 28, 12, 16, st === 'blocked' ? P.o : F.hullD);
    if (st !== 'blocked') { px(g, 39, 28, 2, 16, P.o); px(g, 35, 30, 1, 10, F.cyanD); px(g, 44, 30, 1, 10, F.cyanD); }
  });
});
/* Notes float on a hologram between two emitters, at 2×: rounded
 * emitters with a hot core, a scanlined field, bright corner brackets. */
export const futureBoard = ((c, x, y, notes) => {
  detail(c, x, y, g => {
    [2, 28].forEach(ex => { px(g, ex + 1, 22, 4, 8, P.o); px(g, ex, 23, 6, 7, P.o); px(g, ex + 2, 22, 2, 6, F.cyan); px(g, ex + 2, 22, 1, 2, F.cyanL); });
    px(g, 0, 0, 36, 22, 'rgba(64,224,240,.28)');
    for (let k = 3; k < 21; k += 3) px(g, 2, k, 32, 1, 'rgba(64,224,240,.12)');
    px(g, 0, 0, 36, 2, F.cyan); px(g, 0, 20, 36, 2, F.cyanD); px(g, 0, 0, 2, 22, F.cyanD); px(g, 34, 0, 2, 22, F.cyanD);
    [[0, 0], [32, 0], [0, 21], [32, 21]].forEach(([bx, by]) => px(g, bx, by, 4, 1, F.cyanL));
  });
  notes.slice(0, 8).forEach((n, i) => note(c, x + 2 + (i % 4) * 4, y + 2 + Math.floor(i / 4) * 4, n.label, F.cyanL));
});
/* A pod frame going up at 2×, cross-braced, a drone welding at it. */
export const futureScaffold = ((c, x, y, t) => detail(c, x, y, g => {
  for (let r = 0; r < 4; r++) {
    const by = 4 + 8 * r;
    px(g, 2, by, 28, 2, F.hullD); px(g, 2, by, 28, 1, F.hull);
    if (r < 3) for (let k = 0; k < 8; k++) px(g, 4 + Math.round(k * 23 / 7), by + 1 + k, 1, 1, F.hullD);
  }
  [2, 28].forEach(sx => { px(g, sx, 4, 2, 28, F.hull); px(g, sx + 1, 4, 1, 28, F.hullD); });
  const dx = 2 * (3 + Math.floor((Math.sin(t / 600) + 1) * 4)), dy = 2 * (-2 + (blink(t, 300) ? 1 : 0));
  px(g, dx + 1, dy, 8, 6, P.o); px(g, dx, dy + 1, 10, 4, P.o); px(g, dx + 2, dy + 2, 6, 2, F.hull);
  px(g, dx - 2, dy, 3, 1, F.hullD); px(g, dx + 9, dy, 3, 1, F.hullD);
  px(g, dx + 4, dy + 2, 2, 2, F.cyan); px(g, dx + 4, dy + 2, 1, 1, P.white);
  if (blink(t, 90)) {
    px(g, dx + 4, dy + 6, 1, 2, F.cyanL); px(g, dx + 4, dy + 8, 1, 2, P.white);
    px(g, dx + 2 + 2 * Math.floor(hash(Math.floor(t / 90), 5) * 3), dy + 10, 1, 1, P.gold); px(g, dx + 7, dy + 9, 1, 1, P.gold);
  }
}));
