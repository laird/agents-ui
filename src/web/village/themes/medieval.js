/* The Medieval village theme. */
import { P, R, SHADE, SHINE, hash, shadow } from '../palette.js';
import { B, scaffold } from '../sprites/houses.js';
import { crate } from '../crates.js';
import { FLOWERS, boat, grass, harbour, lock, path, ship, tree, water } from '../scenery.js';
import { board, dock, hall } from './medievalbuildings.js';
import { detail } from '../detail.js';
import { bricks, dither, px } from '../art2x.js';
import { registerTheme } from './registry.js';

/* ---------- Decoration ----------
 * Free ground is never left bare: bushes, flowerbeds, rocks, hay and
 * logs in the village; planters, benches, lamps and bike racks downtown.
 * Placed once per layout on a grid of free cells (layout().decor). */
const BERRY = '#e05050', GLINT = '#f8b0a8', SOIL = '#6a4a2a', HEDGE_D = '#183c1e';
/* The 2× detail over each decoration, in detail pixels from its (x, y); all
 * of it inside the 1× sprite, so the footprint is unchanged (#462). */
const DECOR2X = [
  (g, x, y) => { // bush: leaf clusters, a second berry, glints
    px(g, 5, 3, 2, 1, P.leafL); px(g, 9, 2, 2, 1, P.leafL); px(g, 7, 5, 1, 1, P.leafL);
    px(g, 3, 6, 2, 1, P.leafD); px(g, 7, 4, 2, 1, P.leafD); px(g, 11, 4, 1, 2, P.leafD);
    px(g, 4, 4, 1, 1, GLINT); px(g, 9, 5, 2, 2, BERRY); px(g, 9, 5, 1, 1, GLINT);
  },
  (g, x, y) => { // flowerbed: a lit edge, clods of soil, a heart in each flower, a leaf on each stem
    px(g, 0, 6, 16, 1, P.wood); px(g, 4, 5, 1, 1, P.woodD); px(g, 8, 4, 1, 1, P.woodD); px(g, 11, 5, 1, 1, P.woodD);
    for (let i = 0; i < 4; i++) {
      const col = FLOWERS[(i + Math.floor(hash(x, y) * 5)) % FLOWERS.length];
      px(g, 3 + i * 4, 1, 1, 1, col === FLOWERS[1] ? '#c07020' : FLOWERS[1]);
      px(g, 2 + i * 4, 3, 1, 1, P.leafL);
    }
  },
  (g) => { // rocks: a crack, speckles, a glint, moss
    px(g, 5, 4, 1, 2, P.stoneD); px(g, 6, 5, 1, 1, P.stoneD); px(g, 3, 5, 1, 1, P.stoneD);
    px(g, 3, 2, 1, 1, P.white); px(g, 6, 2, 2, 1, P.leafL);
    px(g, 10, 4, 1, 1, P.stoneL); px(g, 11, 5, 1, 1, P.stoneD);
  },
  (g) => { // haystack: straws, and the twine round it
    for (let k = 3; k < 12; k += 3) { px(g, k, 4 + (k % 2), 1, 2, P.thatchD); px(g, k + 1, 3, 1, 2, P.gold); }
    px(g, 2, 8, 10, 1, P.woodD); px(g, 6, 9, 1, 1, P.woodD);
  },
  (g) => { // log: bark lines, rings in the cut end, spots on the mushroom
    px(g, 2, 6, 3, 1, P.woodD); px(g, 6, 7, 3, 1, P.woodD);
    px(g, 11, 5, 3, 1, P.wood); px(g, 11, 8, 3, 1, P.wood); px(g, 13, 7, 1, 1, P.plank);
    px(g, 3, 0, 1, 1, P.white); px(g, 5, 1, 1, 1, GLINT);
  },
  (g) => { // fence: grain on the posts, a lit top on each rail, nails
    for (const X of [0, 6, 12]) { px(g, X + 1, 2, 1, 2, P.wood); px(g, X + 1, 7, 1, 1, P.wood); }
    px(g, 0, 4, 16, 1, P.plank); px(g, 0, 8, 16, 1, P.plank);
    for (const X of [0, 6, 12]) { px(g, X, 5, 1, 1, P.stoneD); px(g, X, 9, 1, 1, P.stoneD); }
  },
];
const medievalDecor = (c, x, y, k, t) => {
  switch (k) {
    case 0: // bush
      shadow(c, x + 1, y + 5, 7, 1);
      R(c, x, y + 1, 7, 4, P.o); R(c, x + 1, y, 5, 1, P.o);
      R(c, x + 1, y + 1, 5, 3, P.leaf); R(c, x + 1, y + 1, 2, 1, P.leafL); R(c, x + 4, y + 3, 2, 1, P.leafD);
      R(c, x + 2, y + 2, 1, 1, BERRY); break;
    case 1: // flowerbed
      R(c, x, y + 3, 8, 2, P.woodD); R(c, x + 1, y + 2, 6, 1, SOIL);
      for (let i = 0; i < 4; i++) { R(c, x + 1 + i * 2, y + 1, 1, 1, P.leaf); R(c, x + 1 + i * 2, y, 1, 1, FLOWERS[(i + Math.floor(hash(x, y) * 5)) % FLOWERS.length]); } break;
    case 2: // rocks
      shadow(c, x + 1, y + 4, 6, 1);
      R(c, x, y + 1, 5, 3, P.o); R(c, x + 1, y + 1, 3, 2, P.stone); R(c, x + 1, y + 1, 2, 1, P.stoneL);
      R(c, x + 4, y + 2, 3, 2, P.o); R(c, x + 5, y + 2, 1, 1, P.stone); break;
    case 3: // haystack
      shadow(c, x + 2, y + 6, 7, 1);
      R(c, x + 1, y, 5, 1, P.o); R(c, x, y + 1, 7, 5, P.o); R(c, x + 1, y + 1, 5, 4, P.thatch); R(c, x + 1, y + 1, 3, 1, P.gold);
      R(c, x + 2, y + 3, 1, 1, P.thatchD); R(c, x + 4, y + 2, 1, 1, P.thatchD); break;
    case 4: // log with a mushroom
      R(c, x, y + 2, 7, 3, P.o); R(c, x + 1, y + 3, 5, 1, P.wood); R(c, x + 5, y + 2, 2, 3, P.plank); R(c, x + 6, y + 3, 1, 1, P.woodD);
      R(c, x + 1, y, 2, 1, BERRY); R(c, x + 2, y + 1, 1, 1, P.white); break;
    default: // fence
      for (let i = 0; i < 8; i += 3) R(c, x + i, y + 1, 1, 4, P.woodD);
      R(c, x, y + 2, 8, 1, P.woodL); R(c, x, y + 4, 8, 1, P.woodL);
  }
  const art = DECOR2X[Math.min(k, DECOR2X.length - 1)];
  detail(c, x, y, g => art(g, x, y));
};
/* A hedge around the village, as in the mockups (the low fence downtown is in
 * modern.js). At 2×, single leaves: lit ones, shaded ones and the odd berry. */
const HEDGE = (x, y) => { const h = hash(x, y); return h < 0.3 ? P.leafL : h < 0.5 ? HEDGE_D : h > 0.97 ? BERRY : null; };
const medievalBorder = (c, W, H) => {
  const leaf = (x, y, w, h) => { R(c, x, y, w, h, P.leafD); for (let yy = y; yy < y + h; yy += 2) for (let xx = x + ((yy >> 1) % 2); xx < x + w; xx += 2) R(c, xx, yy, 1, 1, P.leaf); };
  leaf(0, H - 3, W, 3); leaf(0, 11, 3, H - 11); leaf(W - 3, 11, 3, H - 11);
  R(c, 3, H - 3, W - 6, 1, P.o); R(c, 3, 11, 1, H - 14, P.o); R(c, W - 4, 11, 1, H - 14, P.o);
  detail(c, 0, 0, g => {
    const strip = (x, y, w, h) => dither(g, x, y, w, h, 4, (dx, dy) => HEDGE(x + dx, y + dy));
    strip(0, 2 * (H - 3) + 2, 2 * W, 4); strip(0, 22, 5, 2 * (H - 14)); strip(2 * W - 5, 22, 5, 2 * (H - 14));
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
  ground: grass, road: path, yard: path, channel: water,
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
