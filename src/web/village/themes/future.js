/* The Future theme. */
import { state, now } from '../state.js';
import { F, KINDCOL, NOTECOL, P, PRIOCOL, R, SHADE, blink, hash, shadow } from '../palette.js';
import { puffs } from '../sprites/houses.js';
import { registerTheme } from './registry.js';
/* ---------- Future ----------
 * A space station worked by little robots: deck plates for ground, light-
 * strip walkways for roads, a plasma canal, habitat pods with portholes. It
 * keeps the shared status language: lit portholes while working, an open
 * hatch for blocked, dark glass for unknown. */
/* Portholes: lit cyan-white while working, dark when nobody can say. */
const fWin = st => st === 'working' ? F.cyanL : (st === 'unknown' || st === 'stopped') ? P.dark : F.glassD;
function porthole(c, x, y, st) {
  R(c, x + 1, y, 2, 4, P.o); R(c, x, y + 1, 4, 2, P.o); R(c, x + 1, y + 1, 2, 2, fWin(st));
  if (st === 'working') R(c, x + 1, y + 1, 1, 1, P.white);
}
/* A habitat pod: a rounded module with a domed roof, two portholes and a
 * hatch. The hatch stands open (dark) while its robot waits on you. */
function pod(c, x, y, st, o = {}) {
  const shell = o.shell || F.hull, shellD = o.shellD || F.hullD, roof = o.roof || F.hullL;
  [4, 5, 6, 7, 7].forEach((w, dy) => {
    R(c, x + 8 - w, y + 1 + dy, 2 * w, 1, P.o);
    if (dy) R(c, x + 9 - w, y + 1 + dy, 2 * w - 2, 1, dy === 1 ? roof : shell);
  });
  R(c, x, y + 5, 16, 11, P.o); R(c, x + 1, y + 5, 14, 10, shell);
  R(c, x + 1, y + 14, 14, 1, shellD); R(c, x + 12, y + 5, 2, 9, SHADE);
  R(c, x + 1, y + 7, 14, 1, o.band || F.cyanD);
  if (o.portholes !== false) { porthole(c, x + 2, y + 9, st); porthole(c, x + 10, y + 9, st); }
  R(c, x + 6, y + 10, 4, 6, P.o);
  R(c, x + 7, y + 11, 2, 5, st === 'blocked' ? P.o : shellD);
  R(c, x + 6, y + 9, 4, 1, st === 'working' ? F.cyan : F.cyanD);
}
const FB = {
  cottage(c, x, y, st, t) {
    pod(c, x, y, st);
    R(c, x + 12, y - 3, 1, 5, F.hullD); R(c, x + 11, y - 4, 3, 1, st === 'working' && blink(t, 500) ? F.cyan : F.cyanD);
    if (st === 'working') puffs(c, x + 4, y - 1, t, F.cyanL, F.hullD);
  },
  workshop(c, x, y, st, t) {
    pod(c, x, y, st, { shell: '#b8b0d0', shellD: '#8078a0' });
    const a = st === 'working' ? Math.floor(t / 300) % 2 : 0;
    R(c, x + 4, y - 3, 2, 5, F.hullD); R(c, x + 5, y - 4, 5, 2, F.hullD); R(c, x + 9, y - 3 + a, 2, 3, P.o);
    if (st === 'working' && blink(t, 120)) { R(c, x + 10, y + 1 + a, 1, 1, P.white); R(c, x + 11, y + a, 1, 1, P.gold); R(c, x + 12, y + 2 + a, 1, 1, F.cyan); }
  },
  feature(c, x, y, st, t) {
    pod(c, x, y, st, { band: F.magentaD });
    R(c, x + 13, y - 4, 2, 20, F.magenta); R(c, x + 13, y - 4, 1, 20, P.o);
    R(c, x + 3, y - 4, 13, 2, F.magenta); R(c, x + 3, y - 3, 13, 1, P.o);
    const hook = st === 'working' ? 4 + Math.floor((Math.sin(t / 400) + 1) * 2) : 5;
    const hx = x + 5 + (st === 'working' ? Math.floor(t / 700) % 3 : 0);
    R(c, hx, y - 2, 1, hook, F.cyan); R(c, hx - 1, y - 2 + hook, 3, 1, F.cyanL);
  },
  bug(c, x, y, st, t) {
    pod(c, x, y, st);
    R(c, x + 4, y - 4, 8, 5, 'rgba(64,224,240,.35)');
    R(c, x + 6, y - 3, 4, 3, P.bug); R(c, x + 7, y - 3, 1, 1, P.o); R(c, x + 8, y - 2, 1, 1, P.o);
    R(c, x + 5, y - 2, 1, 1, P.bug); R(c, x + 10, y - 2, 1, 1, P.bug);
    R(c, x + 7, y + 1, 2, 1, F.cyan);
    if (st === 'working') R(c, x + 4, y - 4 + Math.floor(t / 160) % 5, 8, 1, F.cyanL);
  },
  security(c, x, y, st, t) {
    pod(c, x, y, st, { shell: F.panel, shellD: P.shield, roof: F.panelL });
    R(c, x + 6, y + 2, 4, 3, P.gold); R(c, x + 7, y + 3, 2, 1, P.shield);
    R(c, x + 3, y - 2, 1, 5, F.hullD);
    const a = st === 'working' ? t / 300 : 0.6;
    for (let d = 1; d < 5; d++) R(c, x + 3 + Math.round(Math.cos(a) * d), y - 2 + Math.round(Math.sin(a) * d * 0.5), 1, 1, F.hull);
    R(c, x + 12, y - 1, 3, 2, P.o); R(c, x + 13, y - 1, 1, 1, st === 'working' && blink(t, 400) ? P.p0 : F.hullD);
  },
  tests(c, x, y, st, t) {
    pod(c, x, y, st, { shell: F.glass, shellD: F.glassD, roof: F.cyanL });
    [x + 3, x + 11].forEach((tx, k) => {
      R(c, tx, y - 3, 3, 5, P.o); R(c, tx + 1, y - 2, 1, 4, k ? P.fizzB : P.fizzA);
      if (st === 'working') puffs(c, tx + 1, y - 4, t + k * 500, P.fizzA, P.fizzB);
    });
  },
  research(c, x, y, st, t) {
    pod(c, x, y, st, { roof: F.cyanL });
    const a = -2.2 + (st === 'working' ? Math.sin(t / 900) * 0.5 : 0);
    for (let d = 0; d < 6; d++) R(c, x + 8 + Math.round(Math.cos(a) * d), y + 2 + Math.round(Math.sin(a) * d), 1, 1, d < 5 ? F.hullD : P.o);
    R(c, x + 8 + Math.round(Math.cos(a) * 6), y + 2 + Math.round(Math.sin(a) * 6), 1, 1, st === 'working' ? P.gold : F.cyanD);
  },
  docs(c, x, y, st, t) {
    pod(c, x, y, st);
    [F.magenta, F.cyan, P.gold, F.plasmaL].forEach((col, k) => { R(c, x + 4 + k * 2, y - 3 + (k % 2), 2, 4, P.o); R(c, x + 4 + k * 2, y - 2 + (k % 2), 1, 2, col); });
    if (st === 'working') R(c, x + 4 + Math.floor(t / 250) % 4 * 2, y - 4, 1, 1, P.white);
  },
  refactor(c, x, y, st, t) {
    pod(c, x, y, st);
    R(c, x + 8, y + 1, 8, 15, 'rgba(240,72,192,.35)');
    for (let yy = y + 2; yy < y + 16; yy += 3) R(c, x + 8, yy, 8, 1, F.magentaD);
    if (st === 'working') R(c, x + 8, y + 2 + Math.floor(t / 140) % 13, 8, 1, F.magenta);
  },
  perf(c, x, y, st, t) {
    pod(c, x, y, st, { shell: F.hullD, shellD: F.deckL });
    [x + 4, x + 11].forEach((fx, k) => {
      R(c, fx - 3, y - 2, 7, 6, P.o); R(c, fx - 2, y - 1, 5, 4, F.deck);
      const a = st === 'working' ? t / 90 + k : 0.3;
      for (let d = -2; d <= 2; d++) R(c, fx + Math.round(Math.cos(a) * d), y + 1 + Math.round(Math.sin(a) * d * 0.7), 1, 1, F.cyan);
    });
  },
  chores(c, x, y, st, t) {
    R(c, x, y + 3, 16, 13, P.o); R(c, x + 1, y + 4, 14, 11, F.hullD);
    R(c, x + 1, y + 4, 14, 2, F.deckL); R(c, x + 1, y + 6, 14, 1, P.gold);
    R(c, x + 4, y + 8, 8, 8, P.o); R(c, x + 5, y + 9, 6, 7, st === 'blocked' ? P.o : F.deck);
    if (st !== 'blocked') for (let yy = y + 10; yy < y + 16; yy += 2) R(c, x + 5, yy, 6, 1, F.deckD);
    if (st === 'working') R(c, x + 5, y + 9, 6, 1, F.cyan);
    R(c, x + 12, y + 12, 4, 4, P.o); R(c, x + 13, y + 13, 2, 2, F.magenta);
  }
};
export const FUTURE = {
  name: 'Future',
  pair: 'modern',
  // A broken build in orbit: a red alert, pulsing, with light bars and roof alarms.
  weather: { style: 'alert', cloud: '#501828', cloudD: '#300818', rain: '#ff7090', tint: 'rgba(224,24,48,0.20)', alarm: '#ff2040' },
  labels: { docks: 'Cargo bay', board: 'Holo-board', houses: 'Pods', quay: 'Launch rail', castle: 'Spaceport', hall: 'Command dome', ship: 'Shuttle', sails: 'launches', gate: 'Scanner arch' },
  towers: [[2, -58, 9, 50], [13, -55, 18, 45]],
  ground(c, x0, y0, w, h) {
    R(c, x0, y0, w, h, F.deckD);
    for (let x = x0 + 7; x < x0 + w; x += 8) R(c, x, y0, 1, h, F.seam);
    for (let y = y0 + 7; y < y0 + h; y += 8) R(c, x0, y, w, 1, F.seam);
    for (let y = y0 + 1; y < y0 + h; y += 16) for (let x = x0 + 1; x < x0 + w; x += 16) R(c, x, y, 1, 1, F.rivet);
  },
  /* Walkways with a glowing light strip down the middle. */
  road(c, x0, y0, w, h) {
    R(c, x0, y0, w, h, F.deckL);
    if (w > h && h >= 4) { R(c, x0, y0, w, 1, F.cyanD); R(c, x0, y0 + h - 1, w, 1, F.cyanD); for (let x = x0 + 1; x < x0 + w - 2; x += 4) R(c, x, y0 + (h >> 1), 2, 1, F.cyan); }
    if (h > w && w >= 4) { R(c, x0, y0, 1, h, F.cyanD); R(c, x0 + w - 1, y0, 1, h, F.cyanD); for (let y = y0 + 1; y < y0 + h - 2; y += 4) R(c, x0 + (w >> 1), y, 1, 2, F.cyan); }
  },
  yard(c, x0, y0, w, h) {
    R(c, x0, y0, w, h, F.deck);
    for (let x = x0 + 3; x < x0 + w; x += 4) R(c, x, y0, 1, h, F.deckD);
    for (let y = y0 + 3; y < y0 + h; y += 4) R(c, x0, y, w, 1, F.deckD);
  },
  /* Rivers and canals are plasma conduits; their pulses flow. */
  channel(c, x0, y0, w, h, t, flow) {
    R(c, x0, y0, w, h, F.space);
    R(c, x0, y0, w, 1, F.magentaD); R(c, x0, y0 + h - 1, w, 1, F.magentaD);
    const off = Math.floor(t / 60 * flow) % 10;
    for (let x = x0 - 10 + off; x < x0 + w; x += 10) if (x >= x0) {
      R(c, x, y0 + (h >> 1), 4, 1, F.plasmaL); R(c, x + 5, y0 + (h >> 1) - 2, 2, 1, F.plasma); R(c, x + 2, y0 + (h >> 1) + 2, 2, 1, F.cyanD);
    }
  },
  dock(c, x, y, w) {
    R(c, x, y, w, 7, P.o); R(c, x + 1, y, w - 2, 6, F.hullD);
    for (let k = 0; k < w - 2; k += 4) R(c, x + 1 + k, y + 5, 2, 1, F.cyan);
  },
  /* Groves become station kit: an antenna mast, a solar array or a small dome. */
  tree(c, x, y) {
    const k = Math.floor(hash(x, y) * 3);
    if (k === 0) {
      R(c, x + 4, y + 9, 5, 3, P.o); R(c, x + 5, y + 9, 3, 2, F.hullD);
      R(c, x + 6, y + 1, 1, 8, F.hull); R(c, x + 3, y + 3, 7, 1, F.hullD); R(c, x + 4, y + 6, 5, 1, F.hullD);
      R(c, x + 5, y, 3, 2, P.o); R(c, x + 6, y, 1, 1, blink(now() + x * 31, 900) ? P.p0 : F.magentaD);
    } else if (k === 1) {
      R(c, x + 5, y + 7, 2, 5, F.hullD);
      R(c, x, y + 2, 12, 6, P.o); R(c, x + 1, y + 3, 10, 4, F.panel);
      for (let xx = x + 3; xx < x + 11; xx += 3) R(c, xx, y + 3, 1, 4, F.panelL);
      R(c, x + 1, y + 3, 10, 1, F.panelL);
    } else {
      [3, 4, 5, 5].forEach((w, dy) => { R(c, x + 6 - w, y + 4 + dy, 2 * w, 1, P.o); if (dy) R(c, x + 7 - w, y + 4 + dy, 2 * w - 2, 1, dy === 1 ? F.cyanL : F.glass); });
      R(c, x + 1, y + 8, 10, 3, P.o); R(c, x + 2, y + 8, 8, 2, F.hullD);
    }
  },
  /* Command dome: a glass dome on a hull ring, an antenna with a blinking
   * beacon on top. The hatch goes dark when the manager waits on you. */
  hall(c, x, y, st) {
    const x0 = x - 4, gy = y + 22, cx = x0 + 20, base = y + 10, t = now();
    R(c, cx, y - 16, 1, 12, F.hull); R(c, cx - 3, y - 12, 7, 1, F.hullD);
    R(c, cx - 1, y - 19, 3, 3, P.o); R(c, cx, y - 18, 1, 1, blink(t, 700) ? P.p0 : F.magentaD);
    for (let dy = -13; dy <= 0; dy++) {
      const w = Math.floor(Math.sqrt(14 * 14 - dy * dy));
      R(c, cx - w, base + dy, 2 * w + 1, 1, P.o);
      if (dy > -13) R(c, cx - w + 1, base + dy, 2 * w - 1, 1, (dy + 13) % 4 === 0 ? F.cyanD : F.glass);
    }
    for (let wy = y; wy < base; wy += 4) for (let wx = cx - 8; wx <= cx + 8; wx += 4) R(c, wx, wy, 2, 2, fWin(st));
    R(c, cx - 9, base - 11, 3, 2, 'rgba(255,255,255,.55)');
    R(c, x0, gy - 12, 40, 12, P.o); R(c, x0 + 1, gy - 11, 38, 10, F.hull);
    R(c, x0 + 1, gy - 11, 38, 1, F.hullL); R(c, x0 + 1, gy - 8, 38, 1, F.cyanD);
    for (let k = 3; k < 38; k += 6) R(c, x0 + k, gy - 6, 2, 2, F.cyan);
    R(c, x0 + 16, gy - 9, 8, 9, P.o);
    R(c, x0 + 17, gy - 8, 6, 8, st === 'blocked' ? P.o : F.hullD);
    if (st !== 'blocked') R(c, x0 + 20, gy - 8, 1, 8, P.o);
  },
  /* Notes float on a hologram between two emitters. */
  board(c, x, y, notes) {
    R(c, x + 1, y + 11, 3, 4, P.o); R(c, x + 2, y + 11, 1, 3, F.cyan); R(c, x + 14, y + 11, 3, 4, P.o); R(c, x + 15, y + 11, 1, 3, F.cyan);
    R(c, x, y, 18, 11, 'rgba(64,224,240,.28)');
    R(c, x, y, 18, 1, F.cyan); R(c, x, y + 10, 18, 1, F.cyanD); R(c, x, y, 1, 11, F.cyanD); R(c, x + 17, y, 1, 11, F.cyanD);
    notes.slice(0, 8).forEach((n, i) => R(c, x + 2 + (i % 4) * 4, y + 2 + Math.floor(i / 4) * 4, 3, 3, NOTECOL[n.label] || F.cyanL));
  },
  /* A cargo crate riding a little hover-pad. */
  crate(c, x, y, kind, pr, sealed) {
    x = Math.round(x); y = Math.round(y);
    R(c, x, y, 5, 5, P.o); R(c, x + 1, y + 1, 3, 3, F.hull);
    R(c, x + 1, y + 2, 3, 1, KINDCOL[kind] || F.hullD);
    R(c, x + 1, y + 4, 3, 1, F.cyan);
    // Its priority: a status light along the top (none when unprioritised).
    if (PRIOCOL[pr]) R(c, x + 1, y + 1, 2, 1, PRIOCOL[pr]);
    if (sealed) R(c, x + 3, y + 1, 1, 1, P.seal);
  },
  B: FB,
  /* A pod frame going up, a drone welding at it. */
  scaffold(c, x, y, t) {
    for (let yy = y + 2; yy < y + 16; yy += 4) R(c, x + 1, yy, 14, 1, F.hullD);
    R(c, x + 1, y + 2, 1, 14, F.hull); R(c, x + 14, y + 2, 1, 14, F.hull);
    const dx = x + 3 + Math.floor((Math.sin(t / 600) + 1) * 4), dy = y - 2 + (blink(t, 300) ? 1 : 0);
    R(c, dx, dy, 5, 3, P.o); R(c, dx + 1, dy + 1, 3, 1, F.hull); R(c, dx - 1, dy, 1, 1, F.hullD); R(c, dx + 5, dy, 1, 1, F.hullD);
    R(c, dx + 2, dy + 1, 1, 1, F.cyan);
    if (blink(t, 90)) { R(c, dx + 2, dy + 4, 1, 1, P.white); R(c, dx + 1 + Math.floor(hash(Math.floor(t / 90), 5) * 3), dy + 5, 1, 1, P.gold); }
  },
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
  },
  hold: [[33, -14], [38, -14], [33, -19], [38, -19], [33, -24], [38, -24]], holdBadge: [31, -32], hull: [14, -58, 30, 50], flies: true,
  /* The cargo shuttle that brings new issues down the conduit, hovering. */
  /* The scanner arch: laser grids across the walkway while anything is
   * gating, and one scanner pad per gate slot, glowing while busy. */
  gatehouse(c, g, t, busy) {
    const { x0, y, w, rx, rw } = g;
    [rx - 3, rx + rw + 1].forEach(px => { R(c, px, y + 2, 2, 20, P.o); R(c, px, y + 3, 1, 18, F.hullD); R(c, px, y + 3, 2, 1, F.cyan); });
    R(c, rx - 4, y, rw + 8, 3, P.o); R(c, rx - 3, y + 1, rw + 6, 1, F.magenta);
    if (busy.size && !state.reduce) for (let k = 0; k < 4; k++) if (blink(t + k * 90, 180)) R(c, rx - 1, y + 5 + k * 4, rw + 2, 1, F.magenta);
    R(c, x0, y + 3, w, 20, P.o); R(c, x0 + 1, y + 4, w - 2, 18, F.hullD); R(c, x0 + 1, y + 4, w - 2, 1, F.hull);
    g.tables.forEach(([tx, ty], k) => {
      const on = busy.has(k + 1);
      R(c, tx + 4, ty - 9, 6, 6, P.o); R(c, tx + 5, ty - 8, 4, 4, on ? F.cyanL : F.navy);
      R(c, tx, ty + 5, 10, 3, P.o); R(c, tx + 1, ty + 5, 8, 2, on ? F.cyan : F.deckD);
      if (on && !state.reduce && blink(t + k * 70, 140)) R(c, tx + 2 + (Math.floor(t / 140) % 6), ty + 4, 1, 1, F.cyanL);
    });
  },
  supply(c, x, t, load) {
    x = Math.round(x);
    const y = !state.reduce && blink(t, 500) ? 1 : 0;
    R(c, x, y + 4, 27, 5, P.o); R(c, x + 1, y + 5, 25, 3, F.hull); R(c, x + 1, y + 5, 25, 1, F.hullL); R(c, x + 26, y + 5, 3, 3, P.o); R(c, x + 26, y + 6, 2, 1, F.cyan);
    R(c, x + 18, y + 2, 7, 3, P.o); R(c, x + 19, y + 3, 5, 1, F.glass);
    R(c, x - 2, y + 5, 2, 3, blink(t, 120) ? F.cyan : F.cyanL);
    for (let k = 0; k < Math.min(load, 3); k++) this.crate(c, x + 2 + k * 5, y, 'workshop', null);
  },
  decor(c, x, y, k, t) {
    switch (k) {
      case 0: // antenna
        shadow(c, x + 3, y + 7, 3, 1);
        R(c, x + 3, y + 1, 1, 7, F.hull); R(c, x + 1, y + 3, 5, 1, F.hullD); R(c, x + 2, y, 3, 2, P.o); R(c, x + 3, y, 1, 1, blink(t + x * 37, 900) ? P.p0 : F.magentaD); break;
      case 1: // solar panel
        R(c, x + 3, y + 4, 1, 3, F.hullD); R(c, x, y, 8, 4, P.o); R(c, x + 1, y + 1, 6, 2, F.panel); R(c, x + 1, y + 1, 6, 1, F.panelL); R(c, x + 4, y + 1, 1, 2, F.panelL); break;
      case 2: // small dome
        shadow(c, x + 1, y + 6, 7, 1);
        R(c, x + 2, y + 1, 4, 1, P.o); R(c, x + 1, y + 2, 6, 4, P.o); R(c, x + 2, y + 2, 4, 3, F.glass); R(c, x + 2, y + 2, 2, 1, F.cyanL); R(c, x + 1, y + 5, 6, 1, F.hullD); break;
      case 3: { // maintenance drone, hovering
        const b = blink(t + y * 29, 400) ? 1 : 0;
        shadow(c, x + 2, y + 7, 4, 1);
        R(c, x + 1, y + 1 + b, 6, 3, P.o); R(c, x + 2, y + 2 + b, 4, 1, F.hull); R(c, x + 3, y + 2 + b, 1, 1, F.cyan);
        R(c, x, y + b, 2, 1, F.hullD); R(c, x + 6, y + b, 2, 1, F.hullD); break;
      }
      case 4: // light bollards
        [x + 1, x + 5].forEach(bx => { R(c, bx, y + 2, 2, 4, P.o); R(c, bx, y + 2, 2, 1, blink(t + bx * 13, 1400) ? F.cyanL : F.cyan); });
        R(c, x, y + 6, 8, 1, F.deckL); break;
      default: // vent grille
        R(c, x, y + 1, 8, 5, P.o); for (let k = 0; k < 4; k++) R(c, x + 1, y + 2 + k, 6, 1, k % 2 ? F.deck : F.deckL);
    }
  },
  /* Hull trim with a starfield outside it. */
  border(c, W, H) {
    R(c, 0, H - 3, W, 3, F.space); R(c, 0, 11, 3, H - 11, F.space); R(c, W - 3, 11, 3, H - 11, F.space);
    for (let x = 1; x < W; x += 7) R(c, x, H - 2 - (x % 2), 1, 1, hash(x, H) < 0.5 ? P.white : F.cyanL);
    for (let y = 13; y < H - 3; y += 9) { R(c, 1 + (y % 2), y, 1, 1, P.white); R(c, W - 2 - (y % 2), y + 4, 1, 1, F.cyanL); }
    R(c, 3, H - 3, W - 6, 1, F.magentaD); R(c, 3, 11, 1, H - 14, F.magentaD); R(c, W - 4, 11, 1, H - 14, F.magentaD);
  },
  people: 'future', robots: true,
  steam: [F.cyanL, F.hullD], sign: [P.o, F.navy, F.cyan]
};

registerTheme('future', FUTURE);
