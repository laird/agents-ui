/* The Medieval village theme. */
import { P, SHADE, SHINE, hash } from '../palette.js';
import { B, scaffold } from '../sprites/houses.js';
import { crate } from '../crates.js';
import { FLOWERS, cobbles, grass, path, tree, water } from '../scenery.js';
import { boat, harbour, lock, ship } from '../waterfront.js';
import { board, dock, hall } from './medievalbuildings.js';
import { detail } from '../detail.js';
import { bricks, lobes, px } from '../art2x.js';
import { registerTheme } from './registry.js';

/* ---------- Decoration ----------
 * Free ground is never left bare: bushes, flowerbeds, rocks, hay and
 * logs in the village; planters, benches, lamps and bike racks downtown.
 * Placed once per layout on a grid of free cells (layout().decor). */
const BERRY = '#e05050', GLINT = '#f8b0a8', SOIL = '#6a4a2a', HEDGE_D = '#183c1e';
/* Rows of a shape, [x, w] from its top: outlined in P.o, filled inside. */
const rowsOf = (g, x0, y0, rows, outline, fill) => {
  rows.forEach(([x, w], j) => px(g, x0 + x, y0 + j, w, 1, outline));
  rows.forEach(([x, w], j) => { if (j && j < rows.length - 1 && w > 2) px(g, x0 + x + 1, y0 + j, w - 2, 1, fill); });
};
/* A soft shadow on the ground: an oval of shade, `w` wide and two rows deep. */
const oval = (g, x, y, w) => { px(g, x + 1, y, w - 2, 1, SHADE); px(g, x, y + 1, w, 1, SHADE); };
/* Each decoration at 2× (#473), in detail pixels from its (x, y), in the box
 * it filled at 1× (#462 put 2× detail over a 1× base; this redraws the base). */
const DECOR2X = [
  (g, x, y) => { // bush: three lobes lit from the top left, leaf clusters, berries with a glint
    oval(g, 2, 10, 14);
    lobes(g, [[0, 2, 9], [6, 2, 9], [3, 0, 9]], { outline: P.o, fill: P.leaf, light: P.leafL, dark: P.leafD });
    px(g, 4, 3, 2, 1, P.leafL); px(g, 8, 2, 2, 1, P.leafL); px(g, 2, 5, 1, 1, P.leafL);
    px(g, 6, 7, 2, 1, P.leafD); px(g, 11, 6, 1, 2, P.leafD);
    [[4, 6], [10, 4]].forEach(([bx, by]) => { px(g, bx, by, 2, 2, BERRY); px(g, bx, by, 1, 1, GLINT); });
  },
  (g, x, y) => { // flowerbed: a planked box of soil, four flowers on leafy stems
    px(g, 0, 5, 16, 5, P.o); px(g, 1, 6, 14, 3, P.woodD); px(g, 1, 6, 14, 1, P.wood); px(g, 8, 7, 1, 2, P.o);
    px(g, 2, 4, 12, 2, SOIL); px(g, 4, 4, 1, 1, P.woodD); px(g, 10, 5, 1, 1, P.woodD);
    for (let i = 0; i < 4; i++) {
      const col = FLOWERS[(i + Math.floor(hash(x, y) * 5)) % FLOWERS.length], X = 2 + i * 4;
      px(g, X, 2, 1, 3, P.leafD); px(g, X + 1, 3, 1, 1, P.leafL);
      px(g, X - 1, 1, 3, 1, col); px(g, X, 0, 1, 3, col);
      px(g, X, 1, 1, 1, col === FLOWERS[1] ? '#c07020' : FLOWERS[1]);
    }
  },
  (g) => { // rocks: a big stone and a small one, lit on top, a crack, moss and a glint
    oval(g, 2, 8, 12);
    rowsOf(g, 0, 2, [[1, 8], [0, 10], [0, 10], [0, 10], [0, 10], [1, 8]], P.o, P.stone);
    px(g, 1, 3, 6, 1, P.stoneL); px(g, 1, 4, 2, 1, P.stoneL); px(g, 2, 6, 7, 1, P.stoneD);
    px(g, 5, 4, 1, 2, P.stoneD); px(g, 6, 5, 1, 1, P.stoneD); px(g, 2, 3, 1, 1, P.white);
    px(g, 3, 2, 3, 1, P.leafL); px(g, 4, 2, 1, 1, P.leaf);
    rowsOf(g, 8, 4, [[1, 4], [0, 6], [0, 6], [1, 4]], P.o, P.stone);
    px(g, 9, 5, 3, 1, P.stoneL); px(g, 10, 6, 3, 1, P.stoneD); px(g, 9, 5, 1, 1, P.white);
  },
  (g) => { // haystack: a rounded stack, lit on top, straws, twine round it
    oval(g, 4, 12, 14);
    rowsOf(g, 0, 0, [[4, 6], [2, 10], [1, 12], [0, 14], [0, 14], [0, 14], [0, 14], [0, 14], [0, 14], [0, 14], [0, 14], [1, 12]], P.o, P.thatch);
    px(g, 4, 1, 4, 1, P.gold); px(g, 2, 2, 5, 1, P.gold); px(g, 1, 3, 3, 2, P.gold);
    px(g, 12, 4, 1, 7, P.thatchD); px(g, 2, 10, 10, 1, P.thatchD);
    for (let k = 3; k < 12; k += 3) { px(g, k, 4 + (k % 2), 1, 2, P.thatchD); px(g, k + 1, 3, 1, 2, P.gold); }
    px(g, 1, 7, 12, 1, P.woodD); px(g, 6, 8, 1, 1, P.woodD); px(g, 0, 5, 1, 1, P.gold); px(g, 13, 2, 1, 1, P.gold);
  },
  (g) => { // log with a mushroom: bark lines, moss, rings in the cut end, a spotted cap
    rowsOf(g, 0, 4, [[1, 10], [0, 11], [0, 11], [0, 11], [0, 11], [1, 10]], P.o, P.wood);
    px(g, 1, 5, 9, 1, P.woodL); px(g, 1, 8, 9, 1, P.woodD);
    px(g, 2, 6, 3, 1, P.woodD); px(g, 6, 7, 3, 1, P.woodD); px(g, 4, 4, 3, 1, P.leafL);
    rowsOf(g, 9, 4, [[1, 3], [0, 5], [0, 5], [0, 5], [0, 5], [1, 3]], P.o, P.plank);
    px(g, 10, 6, 3, 2, P.wood); px(g, 11, 6, 1, 2, P.plank); px(g, 11, 7, 1, 1, P.woodD);
    px(g, 3, 2, 2, 2, '#f0e8d8'); px(g, 4, 2, 1, 2, P.stoneL);
    rowsOf(g, 1, 0, [[1, 4], [0, 6], [1, 4]], P.o, BERRY);
    px(g, 2, 0, 3, 1, BERRY); px(g, 1, 1, 6, 1, BERRY); px(g, 2, 1, 1, 1, P.white); px(g, 4, 0, 1, 1, P.white); px(g, 5, 1, 1, 1, GLINT);
  },
  (g) => { // fence: pointed posts with grain, two rails with a lit top, nails, its shadow
    px(g, 1, 9, 15, 1, SHADE);
    px(g, 0, 3, 16, 3, P.o); px(g, 0, 6, 16, 3, P.o);
    for (const X of [0, 6, 12]) { px(g, X, 2, 2, 7, P.o); px(g, X, 2, 2, 6, P.woodD); px(g, X, 2, 1, 6, P.wood); px(g, X, 3, 1, 1, P.woodL); }
    for (const Y of [3, 6]) for (const [a, b] of [[2, 6], [8, 12], [14, 16]]) { px(g, a, Y + 1, b - a, 1, P.woodL); px(g, a, Y + 2, b - a, 1, P.wood); }
    for (const X of [0, 6, 12]) { px(g, X + 1, 4, 1, 1, P.stoneD); px(g, X + 1, 7, 1, 1, P.stoneD); }
  },
];
const medievalDecor = (c, x, y, k, t) => {
  const art = DECOR2X[Math.min(k, DECOR2X.length - 1)];
  detail(c, x, y, g => art(g, x, y));
};
/* A hedge around the village, as in the mockups (the low fence downtown is in
 * modern.js). At 2× (#473): a dark base under rows of single leaves, lit ones
 * and shaded ones, the odd berry and flower, and an outline along its inner edge. */
const HEDGE = (x, y) => { const h = hash(x, y); return h < 0.3 ? P.leafL : h < 0.5 ? HEDGE_D : null; };
const hedge = (g, x0, y0, w, h) => {
  px(g, x0, y0, w, h, P.leafD);
  for (let y = 0, r = 0; y < h; y += 2, r++) for (let x = (r % 2) * 2; x < w; x += 4) {
    const X = x0 + x, Y = y0 + y, v = hash(X, Y), col = HEDGE(X, Y);
    px(g, X, Y, Math.min(3, w - x), 1, P.leaf);
    if (col) px(g, X, Y, 1, 1, col);
    if (y + 1 < h) px(g, X + 1, Y + 1, 1, 1, HEDGE_D);
    if (v > 0.97) px(g, X + 1, Y, 1, 1, BERRY);
    else if (v > 0.95) px(g, X + 1, Y, 1, 1, FLOWERS[Math.floor(hash(Y, X) * FLOWERS.length)]);
  }
};
const medievalBorder = (c, W, H) => {
  detail(c, 0, 0, g => {
    const X = 2 * W, Y = 2 * H;
    hedge(g, 0, Y - 4, X, 4); hedge(g, 0, 22, 6, Y - 26); hedge(g, X - 6, 22, 6, Y - 26);
    // The outline is two detail pixels thick, as it was one scene pixel: the leaves keep off it.
    px(g, 0, Y - 6, X, 2, P.leafD); px(g, 6, Y - 6, X - 12, 2, P.o); px(g, 6, 22, 2, Y - 28, P.o); px(g, X - 8, 22, 2, Y - 28, P.o);
  });
};

/* The gatehouse on the road to the quay: a stone arch over the road, and one
 * inspection table per gate slot, its lantern lit while busy. `g` is
 * layout().gate; busy holds the busy slots (1-based). Drawn at 2× (#463),
 * in detail pixels from the scene's origin. */
function gatehouse(c, g, t, busy) {
  const { x0, y, w, rx, rw } = g;
  detail(c, 0, 0, d => {
    const X = 2 * x0, Y = 2 * y, W = 2 * w, RX = 2 * rx, RW = 2 * rw;
    // The arch: two coursed pillars under a lintel with its keystone.
    [RX - 6, RX + RW + 2].forEach(k => {
      px(d, k, Y + 4, 4, 40, P.o); px(d, k + 1, Y + 5, 2, 38, P.stone); px(d, k + 1, Y + 5, 1, 38, P.stoneL);
      for (let j = Y + 9; j < Y + 42; j += 6) px(d, k + 1, j, 2, 1, P.stoneD);
    });
    px(d, RX - 8, Y, RW + 16, 8, P.o); px(d, RX - 7, Y + 1, RW + 14, 6, P.stoneL);
    for (let k = RX - 4; k < RX + RW + 6; k += 5) px(d, k, Y + 2, 1, 5, P.stone);
    px(d, RX + (RW >> 1) - 2, Y + 1, 4, 6, P.stone); px(d, RX - 7, Y + 6, RW + 14, 1, P.stone);
    // The gatehouse itself: dressed stone under a tiled red roof.
    px(d, X, Y + 8, W, 38, P.o); px(d, X + 1, Y + 9, W - 2, 36, P.stone);
    bricks(d, X + 1, Y + 11, W - 2, 34, 10, 5, P.stoneD);
    px(d, X + W - 5, Y + 9, 4, 36, SHADE);
    px(d, X - 2, Y, W + 4, 10, P.o); px(d, X - 1, Y + 1, W + 2, 8, P.red);
    for (let j = Y + 4, n = 0; j < Y + 9; j += 3, n++) {
      px(d, X - 1, j, W + 2, 1, P.redD);
      for (let k = X + 1 + (n % 2) * 3; k < X + W; k += 6) px(d, k, j - 2, 1, 2, P.redD);
    }
    px(d, X - 1, Y + 1, W + 2, 1, SHINE); px(d, X + 1, Y + 10, W - 2, 1, SHADE);
    g.tables.forEach(([tx, ty], k) => {
      const on = busy.has(k + 1), TX = 2 * tx, TY = 2 * ty;
      // A lantern on the wall over the table, and the table on its legs.
      px(d, TX + 8, TY - 18, 12, 12, P.o); px(d, TX + 10, TY - 16, 8, 8, on ? P.lamp : P.dark);
      px(d, TX + 13, TY - 16, 2, 8, P.o); px(d, TX + 10, TY - 13, 8, 1, P.o);
      if (on) { px(d, TX + 11, TY - 15, 1, 2, P.white); px(d, TX + 10, TY - 9, 8, 1, P.gold); }
      px(d, TX, TY + 10, 18, 4, P.o); px(d, TX + 1, TY + 10, 16, 2, P.wood); px(d, TX + 1, TY + 10, 16, 1, P.woodL);
      [TX + 2, TX + 14].forEach(lx => px(d, lx, TY + 14, 2, 4, P.woodD));
    });
  });
}

export const MEDIEVAL = {
  name: 'Medieval village',
  // In a world the platform is drawn in another theme, so the two read apart.
  pair: 'modern',
  weather: { style: 'storm', cloud: '#50506a', cloudD: '#30303f', rain: '#a8d0f0', tint: 'rgba(16,20,44,0.32)', flash: 'rgba(255,255,236,0.38)', alarm: '#f03030' },
  labels: { docks: 'Docks', board: 'Notice board', houses: 'Houses', quay: 'Quay', castle: 'Harbour', hall: 'Castle', ship: 'Ship', sails: 'sails', gate: 'Gatehouse' },
  ground: grass, road: path, yard: cobbles, channel: water,
  dock,
  tree, hall, board, crate, B, scaffold, gate: lock, destination: harbour, vessel: ship, supply: boat, gatehouse, decor: medievalDecor, border: medievalBorder,
  // Deck slots for cargo (from the harbour's x and the canal's y), the "+N"
  // badge past them, and the box the ship takes up.
  hold: [[3, -2], [8, -2], [14, -2], [19, -2], [2, -7], [20, -7]], holdBadge: [23, -12], hull: [2, -17, 23, 29],
  // The destination's tall parts, [dx, dy, w, h] from (castleX, canalY): labels keep off them.
  towers: [[26, -60, 12, 42]],
  people: 'medieval', steam: [P.white, P.stoneL], sign: [P.woodD, P.parch, P.woodD]
};

registerTheme('medieval', MEDIEVAL);
