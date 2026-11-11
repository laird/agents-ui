/* ---------- The Medieval waterfront at 2× (#472) ----------
 * The lock, the harbour and its lighthouse, the ship that carries a release
 * and the supply boat, each drawn in detail pixels (detail.js) on the scene
 * pixels its 1× art covered, so layout, taps and labels don't move
 * (tools/village-tests/art2x.test.mjs holds the footprints). The theme
 * slots: gate, destination, vessel and supply (themes/medieval.js). */
import { state } from './state.js';
import { P, ROPE, SHADE, SHINE, blink } from './palette.js';
import { crate } from './crates.js';
import { detail } from './detail.js';
import { bricks, pixmap, planks, px } from './art2x.js';
import { wfFireworks } from './fireworks.js';

/* A lock gate between the stops: a gatehouse with a tiled roof and the
 * stop's flag over the canal, quay stones either side, and two wooden
 * leaves across the water while it is shut. */
export function lock(c, x, cy, open, flag) {
  detail(c, x, cy, g => {
    // Quay stones either side of the canal, each with a mooring ring.
    [-8, 28].forEach(Y => {
      px(g, -2, Y, 16, 8, P.o); px(g, -1, Y + 1, 14, 6, P.stone);
      px(g, -1, Y + 1, 14, 1, P.stoneL); px(g, -1, Y + 6, 14, 1, P.stoneD);
      px(g, 5, Y + 2, 1, 4, P.stoneD);
      px(g, 9, Y + 3, 2, 2, P.o); px(g, 9, Y + 3, 1, 1, P.stoneL);
    });
    // The leaves: planked, iron-banded, meeting mid-stream. Open, they lie
    // back against the walls and only their edges show.
    if (!open) {
      px(g, 2, 0, 8, 28, P.o);
      [3, 6].forEach(X => { px(g, X, 1, 3, 26, P.wood); px(g, X, 1, 1, 26, P.woodL); });
      px(g, 5, 1, 1, 26, P.woodD);
      [5, 13, 21].forEach(Y => { px(g, 3, Y, 6, 1, P.stoneD); px(g, 4, Y, 1, 1, P.stoneL); });
    } else { px(g, 2, 0, 1, 28, P.woodD); px(g, 9, 0, 1, 28, P.woodD); }
    // The gatehouse: coursed stone shaded on the right, an arched door with
    // a gold latch, and a lit slit window.
    px(g, -8, -30, 24, 20, P.o); px(g, -7, -29, 22, 18, P.stone);
    bricks(g, -7, -28, 22, 17, 6, 4, P.stoneD);
    px(g, -7, -29, 22, 1, P.stoneL); px(g, 11, -29, 4, 18, SHADE);
    px(g, 1, -21, 4, 1, P.o); px(g, 0, -20, 6, 9, P.o);
    px(g, 1, -20, 4, 9, P.woodD); px(g, 2, -20, 1, 9, P.wood); px(g, 4, -16, 1, 1, P.gold);
    px(g, 8, -25, 3, 5, P.o); px(g, 9, -24, 1, 3, P.lamp);
    // The roof: red tiles in courses, hipped at the top, lit on its ridge.
    for (let j = 0; j < 10; j++) {
      const a = Math.max(0, 4 - j), y = -36 + j;
      px(g, -10 + a, y, 28 - 2 * a, 1, P.o);
      if (j === 0 || j === 9) continue;
      px(g, -9 + a, y, 26 - 2 * a, 1, j === 1 ? '#e07060' : j % 3 === 0 ? P.redD : P.red);
      px(g, 12 - a, y, 5, 1, SHADE);
      if (j % 3 === 0) for (let k = -8 + a + (j % 2) * 2; k < 16 - a; k += 4) px(g, k, y, 1, 1, P.red);
    }
    // The stop's flag on its pole: a swallowtail, lit along the top.
    px(g, 16, -48, 1, 18, P.stoneD); px(g, 17, -48, 1, 18, P.stone); px(g, 16, -50, 2, 2, P.gold);
    px(g, 18, -50, 10, 1, P.o); px(g, 18, -43, 10, 1, P.o);
    [9, 8, 6, 6, 8, 9].forEach((w, j) => { px(g, 18, -49 + j, w, 1, flag); px(g, 18 + w, -49 + j, 1, 1, P.o); });
    px(g, 18, -49, 8, 1, SHINE); px(g, 18, -44, 8, 1, SHADE);
  });
}

/* Where finished work ships out: a quay with a lighthouse at the end of
 * the canal. Its lamp flares when a release goes out. The ship moored in it
 * is drawn by ship() below, so it can load cargo and sail. */
export function harbour(c, x, cy, t, fw) {
  const lit = blink(t, 700) || t - fw < 1600;
  detail(c, x, cy, g => {
    // The quay wall: coursed stone under a coping, weed at the waterline,
    // three iron bollards with a rope slung between two of them.
    px(g, 0, -40, 76, 40, P.o); px(g, 1, -39, 74, 38, P.stone);
    bricks(g, 2, -36, 72, 32, 8, 4, P.stoneD);
    px(g, 1, -39, 74, 2, P.stoneL); px(g, 1, -37, 74, 1, P.stoneD);
    px(g, 1, -3, 74, 2, P.stoneD);
    for (let k = 3; k < 72; k += 7) px(g, k, -4, 2, 2, P.leafD);
    [6, 24, 42].forEach(bx => {
      px(g, bx + 1, -9, 4, 1, P.o); px(g, bx, -8, 6, 6, P.o);
      px(g, bx + 1, -8, 4, 5, P.dark); px(g, bx + 1, -8, 1, 3, P.stone);
    });
    for (let k = 0; k < 13; k++) px(g, 31 + k, -6 + (k > 2 && k < 10 ? 1 : 0) + (k > 4 && k < 8 ? 1 : 0), 1, 1, ROPE);
    // The lighthouse, tapering: red and white bands, lit on the left and
    // shaded on the right, two windows and a door at its foot.
    for (let y = -100; y < -36; y += 2) {
      const a = Math.round((-36 - y) / 32), white = Math.floor((y + 100) / 10) % 2 === 1;
      px(g, 54 + a, y, 18 - 2 * a, 2, P.o);
      px(g, 55 + a, y, 16 - 2 * a, 2, white ? P.white : P.red);
      px(g, 56 + a, y, 1, 2, white ? '#ffffff' : '#f07070');
      px(g, 68 - a, y, 3, 2, white ? P.stoneL : P.redD);
    }
    [-88, -66].forEach(Y => { px(g, 61, Y, 4, 5, P.o); px(g, 62, Y + 1, 2, 3, lit ? P.lamp : P.glass); });
    px(g, 61, -47, 4, 1, P.o); px(g, 60, -46, 6, 10, P.o); px(g, 61, -46, 4, 10, P.woodD); px(g, 62, -46, 1, 10, P.wood);
    // The lamp room: glazed between bars, bright with a white core when
    // lit, over a railed gallery.
    px(g, 52, -112, 22, 14, P.o);
    px(g, 54, -110, 18, 8, lit ? P.lamp : P.glass);
    if (lit) { px(g, 60, -109, 6, 6, '#fff8d8'); px(g, 62, -108, 2, 4, P.white); }
    else px(g, 55, -109, 2, 2, P.white);
    px(g, 58, -110, 1, 8, P.o); px(g, 67, -110, 1, 8, P.o);
    px(g, 53, -101, 20, 2, P.stoneD);
    for (let k = 54; k < 73; k += 4) px(g, k, -101, 1, 2, P.stone);
    // The cap: a red dome with a gold finial.
    px(g, 62, -118, 2, 2, P.gold);
    px(g, 59, -116, 8, 1, P.o);
    px(g, 58, -115, 10, 1, P.o); px(g, 59, -115, 8, 1, P.red);
    px(g, 57, -114, 12, 1, P.o); px(g, 58, -114, 10, 1, P.red); px(g, 59, -114, 3, 1, '#f07070');
    px(g, 56, -113, 14, 1, P.o); px(g, 57, -113, 12, 1, P.red); px(g, 65, -113, 4, 1, P.redD);
  });
  wfFireworks(c, x, cy, t, fw, [[8, P.gold], [20, P.fizzA], [31, P.fizzB]], 62);
}

/* The ship that carries the next release, moored at harbour x, moved by
 * (ox, oy) while it sails. Its cargo is drawn on deck at the theme's hold. */
const HULL_IN = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 3, 4, 6, 8];
const PENNANT = [
  ['oooooooo..', 'oppppppoo.', 'opppppppo.', 'oppppoo...', 'opppppppo.', 'ooooooooo.'],
  ['ooooooooo.', 'oppppppppo', 'oppppppoo.', 'oppppo....', 'oppppppoo.', 'oooooooo..'],
];
export function ship(c, x, cy, t, ox, oy) {
  x = Math.round(x + ox); cy = Math.round(cy + oy);
  const flap = !state.reduce && blink(t, 450) ? 1 : 0;
  detail(c, x, cy, g => {
    // The mast and its pennant, fluttering.
    px(g, 24, -32, 2, 40, P.woodD); px(g, 24, -32, 1, 40, P.wood);
    pixmap(g, 26, -34, PENNANT[flap], { o: P.o, p: '#804098' });
    px(g, 27, -33, 5, 1, '#a060c0');
    // Rigging: stays from the masthead to bow and stern.
    for (let k = 0; k < 14; k++) { px(g, 23 - k, -26 + 2 * k, 1, 1, P.woodD); px(g, 26 + k, -26 + 2 * k, 1, 1, P.woodD); }
    // The yard, then the sail bellying out to leeward: lit near the mast,
    // shaded where it curves away, seamed, with a row of reef points.
    px(g, 11, -26, 30, 2, P.woodD); px(g, 11, -26, 30, 1, P.wood);
    for (let j = 0; j < 24; j++) {
      const y = -24 + j, b = Math.round(2 * Math.sin(Math.PI * j / 23)), r = 36 + b;
      px(g, 12, y, r - 12, 1, P.o);
      if (j === 0 || j === 23) continue;
      px(g, 13, y, r - 14, 1, P.sail);
      px(g, 15, y, 1, 1, '#fffaf0');
      px(g, r - 4, y, 3, 1, P.millD);
    }
    px(g, 25, -23, 1, 22, P.millD); px(g, 14, -16, 21, 1, P.millD); px(g, 14, -9, 22, 1, P.millD);
    for (let k = 16; k < 34; k += 4) px(g, k, -20, 1, 1, P.millD);
    // The hull, rounded below: a lit rail, planks, a painted strake with
    // portholes, a dark keel, and a lantern at the stern.
    HULL_IN.forEach((a, j) => {
      const y = 6 + j, l = 4 + a, w = 44 - 2 * a;
      if (j === 0 || j === HULL_IN.length - 1) { px(g, l, y, w, 1, P.o); return; }
      px(g, l, y, 1, 1, P.o); px(g, l + w - 1, y, 1, 1, P.o);
      const col = j < 3 ? P.woodL : j === 3 ? P.woodD : j < 6 ? P.wood : j < 9 ? P.redD : j < 13 ? P.wood : P.woodD;
      px(g, l + 1, y, w - 2, 1, col);
    });
    planks(g, 5, 15, 42, 4, 2, P.woodD);
    [12, 20, 28, 36].forEach(k => { px(g, k, 12, 3, 2, P.o); px(g, k + 1, 12, 1, 1, P.glass); });
    px(g, 5, 7, 1, 1, P.white);
    px(g, 4, 1, 3, 5, P.o); px(g, 5, 2, 1, 3, P.lamp);
    // The water lapping at the bow and the stern.
    if (!state.reduce) {
      const w = blink(t, 500) ? 1 : 0;
      px(g, 42 + w, 21, 3, 1, P.white); px(g, 44, 22, 3, 1, 'rgba(248,248,248,.6)');
      px(g, 7 - w, 22, 3, 1, 'rgba(248,248,248,.6)');
    }
  });
}

/* The supply boat that brings new issues down the river to the docks,
 * with `load` crates still on deck. The river is the map's top 11 rows. */
const BOAT_IN = [0, 0, 0, 0, 0, 1, 2, 3, 5, 7];
export function boat(c, x, t, load) {
  x = Math.round(x);
  const y = !state.reduce && blink(t, 600) ? 1 : 0, Y = 2 * y;
  detail(c, x, 0, g => {
    // The mast and a red pennant, the stay to the bow, a little lug sail.
    px(g, 44, 0, 1, Y + 10, P.woodD); px(g, 45, 0, 3, 2, P.red);
    for (let k = 0; k < 6; k++) px(g, 45 + k, 2 + k + (k > 2 ? y : 0), 1, 1, P.woodD);
    for (let j = 0; j < 8; j++) {
      const w = 3 + j, yy = 2 + j;
      px(g, 44 - w, yy, w, 1, P.o);
      if (j && j < 7) { px(g, 45 - w, yy, w - 1, 1, P.sail); px(g, 42, yy, 2, 1, P.millD); }
    }
    // The hull, rounded below and planked, nails along the strake, a
    // curved stem at the bow.
    BOAT_IN.forEach((a, j) => {
      const yy = Y + 10 + j, l = a, w = 56 - 2 * a;
      if (j === BOAT_IN.length - 1) { px(g, l, yy, w, 1, P.o); return; }
      px(g, l, yy, 1, 1, P.o); px(g, l + w - 1, yy, 1, 1, P.o);
      px(g, l + 1, yy, w - 2, 1, j === 0 ? P.woodL : j < 7 ? P.wood : P.woodD);
    });
    planks(g, 1, Y + 10, 54, 7, 3, P.woodD);
    for (let k = 6; k < 52; k += 10) px(g, k, Y + 15, 1, 1, P.o);
    px(g, 54, Y + 8, 4, 2, P.o); px(g, 56, Y + 10, 2, 4, P.o); px(g, 54, Y + 10, 2, 3, P.woodL);
    if (!state.reduce) px(g, 1 + y, Y + 18, 3, 1, P.white);
  });
  for (let k = 0; k < Math.min(load, 3); k++) crate(c, x + 2 + k * 5, y, 'workshop', null);
}
