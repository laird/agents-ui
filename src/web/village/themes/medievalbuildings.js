/* ---------- The Medieval theme's castle, notice board and docks at 2× (#463) ----------
 * No DOM, so node tests it (tools/village-tests/medievalbuildings.test.mjs).
 * themes/medieval.js draws with these (its gatehouse is there, redrawn the
 * same way; the houses are in sprites/houses.js). Each draws in detail
 * pixels (detail.js) on the scene pixels its 1× art covered. */
import { P, SHADE, SHINE, glassOf } from '../palette.js';
import { note } from '../cvdmarks.js';
import { detail } from '../detail.js';
import { bricks, px } from '../art2x.js';
import { drawn2x } from '../sprites/houseparts.js';

const PURPLE = '#804098', PURPLE_D = '#603078';
/* A conical tower roof with a gold finial, its widest row 16 across at
 * (X, Y + 7): red and dark courses, the far side in shade. */
function cone(g, X, Y) {
  for (let j = 0; j < 8; j++) {
    px(g, X + 7 - j, Y + j, 2 + 2 * j, 1, P.o);
    if (j) { px(g, X + 8 - j, Y + j, 2 * j, 1, (j >> 1) % 2 ? P.red : P.redD); px(g, X + 8 + Math.ceil(j / 3), Y + j, Math.floor(j * 2 / 3), 1, SHADE); }
  }
  px(g, X + 7, Y, 2, 1, P.gold);
}
/* The manager's castle: a keep with banners between two round-roofed
 * towers, behind a crenellated curtain wall. 40 wide, centred on the
 * 32-wide hall slot; the gate opens when the manager waits on you. */
export const hall = drawn2x((c, x, y, st) => detail(c, x - 4, y, g => {
  const glass = glassOf(st);
  // The keep: dressed stone, crenels, two arched windows, the banners and the flag.
  px(g, 24, -4, 32, 40, P.o); px(g, 25, -3, 30, 39, P.stoneL);
  bricks(g, 25, -1, 30, 37, 8, 4, P.stone);
  px(g, 51, -3, 4, 39, SHADE);
  for (let k = 0; k < 32; k += 6) { px(g, 24 + k, -8, 4, 4, P.o); px(g, 25 + k, -7, 2, 3, P.stoneL); }
  [32, 44].forEach(X => {
    px(g, X + 1, 6, 2, 1, P.o); px(g, X, 7, 4, 7, P.o); px(g, X + 1, 8, 2, 6, glass);
    if (st === 'working') px(g, X + 1, 8, 1, 2, P.white);
  });
  [28, 50].forEach(X => {
    px(g, X, 2, 4, 14, PURPLE); px(g, X + 3, 4, 1, 12, PURPLE_D); px(g, X, 2, 4, 2, P.gold);
    px(g, X + 1, 8, 2, 2, P.gold); px(g, X, 16, 1, 2, PURPLE); px(g, X + 3, 16, 1, 2, PURPLE_D);
  });
  px(g, 40, -22, 1, 14, P.stoneD); px(g, 41, -22, 1, 14, P.stone);
  px(g, 42, -22, 10, 6, P.o); px(g, 42, -21, 8, 4, P.gold); px(g, 42, -18, 8, 1, '#c09020'); px(g, 47, -21, 1, 3, '#c09020');
  // The curtain wall, crenellated, its stones coursed.
  px(g, 0, 20, 80, 24, P.o); px(g, 1, 21, 78, 22, P.stone);
  bricks(g, 1, 22, 78, 21, 8, 4, P.stoneD);
  px(g, 1, 21, 78, 1, P.stoneL);
  for (let k = 0; k < 80; k += 8) { px(g, k, 16, 4, 4, P.o); px(g, k + 1, 17, 2, 3, P.stone); px(g, k + 1, 17, 2, 1, P.stoneL); }
  // The towers, each under its cone.
  [0, 64].forEach(X => {
    px(g, X, -8, 16, 52, P.o); px(g, X + 1, -7, 14, 50, P.stoneL);
    bricks(g, X + 1, -5, 14, 47, 7, 4, P.stone);
    px(g, X + 11, -7, 4, 50, SHADE);
    px(g, X + 7, 6, 2, 1, P.o); px(g, X + 6, 7, 4, 6, P.o); px(g, X + 7, 8, 2, 5, glass);
    cone(g, X, -16);
  });
  // The gate: a portcullis, down and dark while the manager waits on you.
  px(g, 33, 26, 14, 1, P.o); px(g, 32, 27, 16, 17, P.o);
  px(g, 38, 25, 4, 2, P.stoneL);
  if (st !== 'blocked') {
    px(g, 34, 28, 12, 16, P.woodD);
    for (let k = 35; k < 46; k += 3) px(g, k, 28, 1, 16, P.stoneD);
    for (let j = 31; j < 44; j += 4) px(g, 34, j, 12, 1, P.stoneD);
  }
}));

/* The notice board: a framed plank board on two posts, a note pinned on it
 * for each issue that waits on a person (eight at most). */
export const board = drawn2x((c, x, y, notes) => {
  detail(c, x, y, g => {
    [2, 32].forEach(X => { px(g, X, 18, 2, 12, P.woodD); px(g, X, 18, 1, 12, P.wood); });
    px(g, 0, 0, 36, 22, P.o); px(g, 1, 1, 34, 20, P.woodD); px(g, 1, 1, 34, 1, P.woodL);
    px(g, 2, 2, 32, 18, P.wood);
    for (let j = 6; j < 20; j += 4) px(g, 2, j, 32, 1, 'rgba(0,0,0,.14)');
    px(g, 9, 9, 2, 1, P.woodD); px(g, 25, 15, 2, 1, P.woodD);
    px(g, 2, 2, 32, 1, SHINE);
  });
  notes.slice(0, 8).forEach((n, i) => note(c, x + 2 + (i % 4) * 4, y + 2 + Math.floor(i / 4) * 4, n.label, P.white));
  // A pin at the top of each note.
  detail(c, x, y, g => notes.slice(0, 8).forEach((n, i) => px(g, 6 + (i % 4) * 8, 4 + Math.floor(i / 4) * 8, 1, 1, P.stoneD)));
});

/* A dock: boards laid along it, their butt joints staggered and nailed. */
export const dock = drawn2x((c, x, y, w) => detail(c, x, y, g => {
  const W = 2 * w;
  px(g, 0, 0, W, 14, P.o); px(g, 1, 0, W - 2, 13, P.plank);
  [4, 8].forEach(j => px(g, 1, j, W - 2, 1, P.wood));
  for (let n = 0; n < 3; n++) for (let k = 3 + (n * 5) % 12; k < W - 2; k += 12) { px(g, k, n * 4 + (n ? 1 : 0), 1, n ? 3 : 4, P.wood); px(g, k + 1, n * 4 + 2, 1, 1, P.stoneD); }
  px(g, 1, 0, W - 2, 1, P.sail); px(g, 1, 12, W - 2, 1, P.woodD);
}));
