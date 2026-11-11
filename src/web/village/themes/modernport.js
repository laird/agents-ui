/* ---------- The Modern theme's checkpoint, launch pad, rocket and truck at 2× (#472) ----------
 * Each draws in detail pixels (detail.js) on the scene pixels its 1× art
 * covered (tools/village-tests/modern2x.test.mjs holds the footprints).
 * themes/modern.js puts them in its gate, destination, vessel and supply. */
import { P, blink } from '../palette.js';
import { detail } from '../detail.js';
import { pixmap, px } from '../art2x.js';
import { MC } from './modernhouses.js';
import { wfFireworks } from '../fireworks.js';

const RED_L = '#f07060';
/* A toll booth over the road between stops, the stop's signal on its roof
 * and a striped boom across the road while it is shut. */
export function modernGate(c, x, cy, open, flag) {
  detail(c, x, cy, g => {
    // The booth: a lit front, a shaded side, blinds in its window, a teal stripe.
    px(g, -8, -30, 24, 20, P.o); px(g, -7, -29, 22, 18, MC.concreteL);
    px(g, 11, -29, 4, 18, MC.concrete); px(g, -7, -13, 22, 2, MC.concreteD);
    px(g, -7, -15, 22, 1, MC.awnB);
    px(g, -4, -25, 16, 8, P.o); px(g, -3, -24, 14, 6, MC.glassL);
    for (let k = -23; k < -18; k += 2) px(g, -3, k, 14, 1, MC.glass);
    px(g, -2, -24, 2, 1, P.white);
    // The roof slab, its edge lit; the signal in a hooded housing.
    px(g, -9, -32, 26, 1, P.o); px(g, -10, -31, 28, 3, P.o);
    px(g, -9, -31, 26, 1, MC.concreteL); px(g, -9, -30, 26, 1, MC.steel);
    px(g, 3, -40, 6, 1, P.o); px(g, 2, -39, 8, 7, P.o);
    px(g, 4, -38, 4, 4, flag); px(g, 4, -38, 1, 1, P.white);
    // The boom's post, and the boom: red and white bands, lit down one side.
    px(g, 4, -8, 4, 8, P.o); px(g, 5, -7, 2, 7, MC.steel); px(g, 5, -7, 1, 7, MC.concreteL);
    if (!open) {
      px(g, 4, 0, 4, 27, P.o); px(g, 5, 27, 2, 1, P.o);
      for (let y = 0; y < 27; y++) [5, 6].forEach(X => px(g, X, y, 1, 1, Math.floor((y + X) / 4) % 2 ? P.white : P.p0));
      px(g, 5, 25, 1, 1, P.gold);
    } else px(g, 2, -6, 2, 4, MC.steelD);
  });
}

/* The launch pad: a slab with hazard stripes, a red gantry tower braced
 * with service arms and a beacon on top, and the flame trench in front. */
export function modernPad(c, x, cy, t, fw) {
  const lit = blink(t, 700) || t - fw < 1600;
  detail(c, x, cy, g => {
    px(g, 0, -16, 76, 16, P.o); px(g, 1, -15, 74, 14, MC.concrete);
    px(g, 1, -15, 74, 1, MC.concreteL); px(g, 1, -3, 74, 2, MC.concreteD);
    px(g, 1, -14, 74, 3, P.dark);
    for (let k = 2; k < 74; k += 6) px(g, k, -14, 3, 3, P.gold);
    for (let k = 4; k < 72; k += 12) px(g, k, -8, 6, 2, MC.dash);
    // The gantry: lit and shaded edges, cross-bracing, rivets.
    px(g, 8, -100, 8, 84, P.o); px(g, 9, -98, 6, 81, P.red);
    px(g, 9, -98, 1, 81, RED_L); px(g, 14, -98, 1, 81, P.redD);
    for (let y = -92; y < -18; y += 12) for (let k = 0; k < 6; k++) { px(g, 10 + k, y + k * 2, 1, 1, P.redD); px(g, 15 - k, y + k * 2, 1, 1, P.redD); }
    for (let y = -92; y < -16; y += 6) px(g, 11, y, 1, 1, '#f8a0a0');
    px(g, 10, -100, 4, 2, lit ? P.p0 : P.redD); if (lit) px(g, 11, -100, 1, 1, '#ffd0c0');
    // Service arms reaching out to the rocket, each with a lit top.
    for (let y = -92; y < -16; y += 12) { px(g, 16, y, 12, 2, MC.steelD); px(g, 16, y, 12, 1, MC.steel); px(g, 26, y, 2, 2, P.o); }
  });
  wfFireworks(c, x, cy, t, fw, [[6, P.gold], [21, P.fizzA], [34, P.fizzB]], 58);
  detail(c, x, cy, g => {
    // The flame trench: a grating over a warm glow.
    px(g, 16, -4, 40, 36, P.o); px(g, 18, 0, 36, 30, P.dark);
    px(g, 20, 22, 32, 6, 'rgba(240,144,48,.22)');
    for (let k = 21; k < 54; k += 4) px(g, k, 0, 1, 30, '#2a2a3c');
    px(g, 18, 0, 36, 1, '#50506a');
  });
}

/* The rocket for the next release, with its cargo rack, moved by (ox, oy).
 * Its engines fire while it climbs. */
const ROCKET_NOSE = [
  '....oooo....', '....olro....', '...olrrdo...', '...olrrdo...', '..olrrrrdo..', '..olrrrrdo..',
  '..olrrrrdo..', '.olrrrrrrdo.', '.olrrrrrrdo.', '.olrrrrrrdo.', 'olrrrrrrrrdo', 'oooooooooooo',
];
export function modernRocket(c, x, cy, t, ox, oy) {
  const rx = Math.round(x + 16 + ox); cy = Math.round(cy + oy);
  detail(c, rx, cy, g => {
    pixmap(g, 4, -100, ROCKET_NOSE, { o: P.o, l: RED_L, r: P.red, d: P.redD });
    // The body: round, lit on the left, shaded on the right, seamed and
    // riveted, a red band, a porthole with a glint, the engine skirt.
    px(g, 1, -88, 18, 1, P.o); px(g, 0, -87, 20, 71, P.o);
    px(g, 1, -87, 18, 70, P.white); px(g, 3, -87, 2, 70, '#ffffff');
    px(g, 14, -87, 5, 70, MC.concreteL); px(g, 17, -87, 2, 70, MC.concreteD);
    [-76, -56, -40].forEach(y => { px(g, 1, y, 18, 1, MC.concreteD); for (let k = 4; k < 18; k += 4) px(g, k, y + 1, 1, 1, MC.steelD); });
    px(g, 1, -50, 18, 3, P.red); px(g, 3, -50, 2, 3, RED_L); px(g, 14, -50, 5, 3, P.redD);
    px(g, 7, -68, 6, 1, P.o); px(g, 6, -67, 8, 4, P.o); px(g, 7, -63, 6, 1, P.o);
    px(g, 7, -67, 6, 4, MC.glassL); px(g, 8, -66, 2, 1, P.white); px(g, 11, -64, 2, 1, MC.glass);
    px(g, 2, -21, 16, 4, P.o); px(g, 3, -20, 14, 2, MC.steel); px(g, 3, -20, 14, 1, MC.concreteL);
    // Fins, swept back, lit on their leading edges.
    for (let j = 0; j < 16; j++) {
      const w = Math.min(6, 1 + (j >> 1)), y = -32 + j, last = j === 15;
      px(g, -w, y, w, 1, P.o); px(g, 20, y, w, 1, P.o);
      if (w > 1 && !last) { px(g, 1 - w, y, w - 1, 1, P.red); px(g, 20, y, w - 1, 1, P.red); px(g, 1 - w, y, 1, 1, RED_L); px(g, 18 + w, y, 1, 1, P.redD); }
    }
    // The cargo rack.
    px(g, 26, -52, 2, 36, MC.steelD); px(g, 26, -52, 1, 36, MC.steel);
    px(g, 26, -18, 22, 2, MC.steelD); px(g, 26, -18, 22, 1, MC.steel);
    for (let k = 30; k < 48; k += 6) px(g, k, -17, 1, 1, MC.concreteL);
    // Engine flames, each with a white-hot core, flickering as it climbs.
    if (oy < 0) for (let k = 0; k < 4; k++) {
      const f = blink(t + k * 60, 80) ? 2 : 0, h = 6 + (k % 2) * 4, X = 4 + 4 * k;
      px(g, X, -16 + f, 4, h, k % 2 ? '#f09030' : P.gold);
      px(g, X + 1, -16 + f, 2, h - 2, k % 2 ? P.gold : '#f8f8f8');
      px(g, X + 1, -16 + f, 2, 2, P.white);
    }
  });
}

/* The delivery truck that brings new issues along the top road, its crates
 * on the flatbed (themes/modern.js draws them). */
export function modernTruck(c, x) {
  detail(c, Math.round(x), 0, g => {
    px(g, 0, 10, 40, 8, P.o); px(g, 1, 11, 38, 3, MC.steel); px(g, 1, 11, 38, 1, MC.concreteL);
    px(g, 1, 14, 38, 3, MC.steelD); px(g, 1, 13, 38, 1, P.red);
    // The cab: rounded at the top, a windscreen with a glint, the door's
    // seam and handle, a headlight and a bumper.
    px(g, 38, 2, 12, 1, P.o); px(g, 38, 3, 14, 15, P.o);
    px(g, 39, 3, 11, 13, P.white); px(g, 39, 14, 11, 2, MC.concreteD); px(g, 50, 4, 1, 10, MC.concreteL);
    px(g, 44, 5, 7, 6, P.o); px(g, 45, 6, 5, 4, MC.glassL); px(g, 46, 6, 1, 2, P.white);
    px(g, 42, 6, 1, 9, MC.concreteD); px(g, 40, 11, 2, 1, MC.steelD);
    px(g, 50, 13, 2, 2, P.gold); px(g, 48, 16, 4, 2, MC.steelD);
    // Round wheels with hubs.
    [6, 28, 42].forEach(wx => {
      px(g, wx + 1, 16, 4, 1, P.o); px(g, wx, 17, 6, 4, P.o); px(g, wx + 1, 21, 4, 1, P.o);
      px(g, wx + 2, 18, 2, 2, MC.steel); px(g, wx + 2, 18, 1, 1, MC.concreteL);
    });
  });
}
