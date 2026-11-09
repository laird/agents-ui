/* The Medieval village theme. */
import { P, R, hash, shadow } from '../palette.js';
import { B, scaffold } from '../sprites/houses.js';
import { FLOWERS, board, boat, crate, grass, hall, harbour, lock, path, ship, tree, water } from '../scenery.js';
import { registerTheme } from './registry.js';

/* ---------- Decoration ----------
 * Free ground is never left bare: bushes, flowerbeds, rocks, hay and
 * logs in the village; planters, benches, lamps and bike racks downtown.
 * Placed once per layout on a grid of free cells (layout().decor). */
const medievalDecor = (c, x, y, k, t) => {
  switch (k) {
    case 0: // bush
      shadow(c, x + 1, y + 5, 7, 1);
      R(c, x, y + 1, 7, 4, P.o); R(c, x + 1, y, 5, 1, P.o);
      R(c, x + 1, y + 1, 5, 3, P.leaf); R(c, x + 1, y + 1, 2, 1, P.leafL); R(c, x + 4, y + 3, 2, 1, P.leafD);
      R(c, x + 2, y + 2, 1, 1, '#e05050'); break;
    case 1: // flowerbed
      R(c, x, y + 3, 8, 2, P.woodD); R(c, x + 1, y + 2, 6, 1, '#6a4a2a');
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
      R(c, x + 1, y, 2, 1, '#e05050'); R(c, x + 2, y + 1, 1, 1, P.white); break;
    default: // fence
      for (let i = 0; i < 8; i += 3) R(c, x + i, y + 1, 1, 4, P.woodD);
      R(c, x, y + 2, 8, 1, P.woodL); R(c, x, y + 4, 8, 1, P.woodL);
  }
};
/* A hedge around the village, as in the mockups (the low fence downtown is in modern.js). */
const medievalBorder = (c, W, H) => {
  const leaf = (x, y, w, h) => { R(c, x, y, w, h, P.leafD); for (let yy = y; yy < y + h; yy += 2) for (let xx = x + ((yy >> 1) % 2); xx < x + w; xx += 2) R(c, xx, yy, 1, 1, P.leaf); };
  leaf(0, H - 3, W, 3); leaf(0, 11, 3, H - 11); leaf(W - 3, 11, 3, H - 11);
  R(c, 3, H - 3, W - 6, 1, P.o); R(c, 3, 11, 1, H - 14, P.o); R(c, W - 4, 11, 1, H - 14, P.o);
};

/* The gatehouse on the road to the quay: a stone arch over the road, and one
 * inspection table per gate slot, its lamp lit while busy. `g` is
 * layout().gate; busy holds the busy slots (1-based). */
function gatehouse(c, g, t, busy) {
  const { x0, y, w, rx, rw } = g;
  R(c, rx - 3, y + 2, 2, 20, P.o); R(c, rx - 2, y + 3, 1, 18, P.stone);
  R(c, rx + rw + 1, y + 2, 2, 20, P.o); R(c, rx + rw + 1, y + 3, 1, 18, P.stone);
  R(c, rx - 4, y, rw + 8, 4, P.o); R(c, rx - 3, y + 1, rw + 6, 2, P.stoneL);
  R(c, x0, y + 4, w, 19, P.o); R(c, x0 + 1, y + 5, w - 2, 17, P.stone);
  for (let yy = y + 7; yy < y + 21; yy += 3) for (let xx = x0 + 2 + ((yy % 2) ? 2 : 0); xx < x0 + w - 2; xx += 5) R(c, xx, yy, 1, 1, P.stoneD);
  R(c, x0 - 1, y, w + 2, 5, P.o); R(c, x0, y + 1, w, 3, P.red);
  g.tables.forEach(([tx, ty], k) => {
    const on = busy.has(k + 1);
    R(c, tx + 4, ty - 9, 6, 6, P.o); R(c, tx + 5, ty - 8, 4, 4, on ? P.lamp : P.dark);
    R(c, tx, ty + 5, 9, 2, P.o); R(c, tx + 1, ty + 5, 7, 1, P.wood); R(c, tx + 1, ty + 7, 1, 2, P.woodD); R(c, tx + 7, ty + 7, 1, 2, P.woodD);
  });
}

export const MEDIEVAL = {
  name: 'Medieval village',
  // In a world the platform is drawn in another theme, so the two read apart.
  pair: 'modern',
  labels: { docks: 'Docks', board: 'Notice board', houses: 'Houses', quay: 'Quay', castle: 'Harbour', hall: 'Castle', ship: 'Ship', sails: 'sails', gate: 'Gatehouse' },
  ground: grass, road: path, yard: path, channel: water,
  dock(c, x, y, w) { R(c, x, y, w, 7, P.o); R(c, x + 1, y, w - 2, 6, P.plank); },
  tree, hall, board, crate, B, scaffold, gate: lock, destination: harbour, vessel: ship, supply: boat, gatehouse, decor: medievalDecor, border: medievalBorder,
  // Deck slots for cargo (from the harbour's x and the canal's y), the "+N"
  // badge past them, and the box the ship takes up.
  hold: [[3, -2], [8, -2], [14, -2], [19, -2], [2, -7], [20, -7]], holdBadge: [23, -12], hull: [2, -17, 23, 29],
  // The destination's tall parts, [dx, dy, w, h] from (castleX, canalY): labels keep off them.
  towers: [[26, -60, 12, 42]],
  people: 'medieval', steam: [P.white, P.stoneL], sign: [P.woodD, P.parch, P.woodD]
};

registerTheme('medieval', MEDIEVAL);
