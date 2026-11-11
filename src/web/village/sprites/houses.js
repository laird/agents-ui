/* Houses, one sprite per kind, and what is drawn on them. Every house is
 * drawn at 2× (#463), in detail pixels (detail.js) from its origin, out of
 * the parts in houseparts.js; drawn2x() tells registerTheme's hd() to leave
 * it be. */
import { P, SHADE, SHINE, blink, glassOf, hash, lit } from '../palette.js';
import { detail } from '../detail.js';
import { bricks, px } from '../art2x.js';
import { chimney, door2, drawn2x, line2, roof2, wall2, win2 } from './houseparts.js';
/* Smoke from a chimney at (x, y): three puffs rising and drifting, `a`
 * while fresh and `b` as they thin. Drawn at 2× (#461): round puffs with a
 * lit top that shrink as they rise, moving half a pixel at a time. */
export function puffs(c, x, y, t, a, b) {
  detail(c, x, y, g => {
    for (let k = 0; k < 3; k++) {
      const p = ((t / 110) + k * 7) % 21, fresh = p < 12, s = p < 5 ? 4 : p < 9 ? 3 : 2;
      const px0 = Math.round(2 * Math.sin((p + k * 2) / 3)), py0 = -Math.round(p * 1.2);
      px(g, px0 + 1, py0, s - 1 > 0 ? s - 1 : 1, 1, fresh ? a : b);
      px(g, px0, py0 + 1, s, s - 1 || 1, fresh ? a : b);
      if (s > 2) px(g, px0 + 1, py0 + 1, 1, 1, P.white);
    }
  });
}


/* ---------- Houses: each draws in x..x+15, y-4..y+16 ----------
 * In detail pixels that is 0..32 across and -8..32 down. */
const house2x = draw => drawn2x((c, x, y, st, t) => detail(c, x, y, g => draw(g, st, t)));
/* The plain cottage under a roof of `roof`: a chimney, two windows, a door. */
function cottage2x(g, st, roof, roofD) {
  chimney(g, 22, -4, 8, 10);
  roof2(g, 0, 2, 32, 18, roof, roofD);
  wall2(g, 2, 18, 28, 14, P.wood);
  win2(g, 4, 20, st); win2(g, 20, 20, st); door2(g, 12, 22, st);
}
const BOOKS = ['#c04838', '#3870c0', '#58a048', '#f8d040'];
export const B = {
  cottage: drawn2x((c, x, y, st, t, roof = P.thatch, roofD = P.thatchD) => {
    detail(c, x, y, g => cottage2x(g, st, roof, roofD));
    if (lit(st)) puffs(c, x + 12, y - 4, t, P.white, P.stoneL);
  }),
  // A smithy: the cottage under a red roof, an anvil on the sign in its gable.
  workshop: drawn2x((c, x, y, st, t) => {
    detail(c, x, y, g => {
      cottage2x(g, st, P.red, P.redD);
      px(g, 8, 8, 12, 6, P.o); px(g, 9, 9, 10, 4, P.woodD);
      px(g, 10, 9, 8, 2, P.stoneL); px(g, 9, 9, 1, 1, P.stoneL); px(g, 13, 11, 3, 1, P.stoneD); px(g, 11, 12, 7, 1, P.stoneL);
    });
    if (lit(st)) puffs(c, x + 12, y - 4, t, P.white, P.stoneL);
  }),
  // Building out: a narrow house and the scaffold tower going up beside it, sparks while working.
  feature: house2x((g, st, t) => {
    roof2(g, 0, 2, 22, 18, P.red, P.redD);
    wall2(g, 2, 18, 20, 14, P.wood);
    win2(g, 4, 20, st); door2(g, 12, 22, st);
    [22, 30].forEach(k => { px(g, k, 4, 2, 28, P.woodD); px(g, k, 4, 1, 28, P.plank); });
    [[20, 4, 12], [22, 14, 10]].forEach(([X, Y, w]) => { px(g, X, Y, w, 2, P.plank); px(g, X, Y + 1, w, 1, P.woodD); });
    line2(g, 24, 6, 29, 13, P.woodL);
    [[24, P.plank], [26, P.woodD], [28, P.plank]].forEach(([j, col]) => { px(g, 22, j, 10, 2, col); px(g, 22, j, 10, 1, col === P.plank ? P.sail : P.wood); });
    px(g, 22, 30, 10, 2, P.o);
    if (lit(st) && blink(t, 140)) {
      const s = 24 + 2 * (Math.floor(t / 280) % 3);
      px(g, s, 12, 1, 1, P.white); px(g, s + 1, 11, 1, 1, P.gold); px(g, s - 1, 13, 1, 1, P.gold); px(g, 26, 10, 1, 1, P.gold);
    }
  }),
  // The bug hunter's: a thatched house with a ladybird on its sign, a trellis, and a bug on the run.
  bug: house2x((g, st, t) => {
    roof2(g, 0, 2, 32, 18, P.thatch, P.thatchD);
    px(g, 6, 6, 10, 8, P.o); px(g, 7, 7, 8, 6, P.woodD); px(g, 8, 8, 6, 4, P.plank); px(g, 10, 8, 1, 4, P.woodD); px(g, 8, 8, 6, 1, P.sail);
    wall2(g, 2, 18, 28, 14, P.wood);
    win2(g, 4, 20, st); door2(g, 12, 22, st);
    [22, 28].forEach(k => { px(g, k, 4, 2, 28, P.woodD); px(g, k, 4, 1, 28, P.woodL); });
    for (let j = 6; j < 32; j += 6) { px(g, 22, j, 8, 2, P.woodD); px(g, 24, j, 4, 1, P.wood); }
    px(g, 18, -2, 2, 4, P.o);
    px(g, 12, -8, 14, 8, P.o); px(g, 13, -7, 12, 6, P.parch);
    px(g, 16, -7, 5, 1, P.bug); px(g, 15, -6, 7, 4, P.bug); px(g, 16, -2, 5, 1, P.bug); px(g, 16, -6, 1, 1, '#f8a0a0');
    px(g, 18, -7, 1, 6, P.o); [[16, -4], [20, -5], [20, -3]].forEach(([X, Y]) => px(g, X, Y, 1, 1, P.o));
    px(g, 22, -5, 2, 2, P.o); px(g, 24, -6, 1, 1, P.o);
    if (lit(st)) { const bx = 4 + 2 * (Math.floor(t / 160) % 8); px(g, bx, 14, 3, 2, P.o); px(g, bx + 3, 14, 1, 1, P.o); px(g, bx + 1, 14, 1, 1, P.bug); }
  }),
  // The watch: a crenellated keep with arrow slits and a shield, a low tower either side, its beacon lit while working.
  security: house2x((g, st, t) => {
    [0, 24].forEach(X => {
      px(g, X, 20, 8, 12, P.o); px(g, X + 1, 21, 6, 10, P.stone); px(g, X + 1, 21, 6, 1, P.stoneL);
      px(g, X + 1, 25, 6, 1, P.stoneD); px(g, X + 3, 22, 1, 3, P.stoneD); px(g, X + 5, 26, 1, 3, P.stoneD);
    });
    px(g, 6, -4, 20, 36, P.o); px(g, 7, -3, 18, 34, P.stone);
    bricks(g, 7, -2, 18, 33, 6, 3, P.stoneD);
    px(g, 7, -3, 18, 1, P.stoneL); px(g, 21, -2, 4, 33, SHADE);
    [6, 14, 22].forEach(X => { px(g, X, -8, 4, 4, P.o); px(g, X + 1, -7, 2, 3, P.stone); px(g, X + 1, -7, 2, 1, P.stoneL); });
    [10, 20].forEach(X => { px(g, X, 1, 2, 8, P.o); px(g, X - 1, 4, 4, 1, P.o); px(g, X, 2, 1, 6, glassOf(st)); });
    px(g, 12, 8, 8, 6, P.gold); px(g, 13, 14, 6, 2, P.gold); px(g, 14, 16, 4, 1, P.gold); px(g, 15, 17, 2, 1, P.gold);
    px(g, 13, 9, 6, 5, P.shield); px(g, 14, 14, 4, 2, P.shield); px(g, 15, 16, 2, 1, P.shield);
    px(g, 15, 9, 2, 7, P.gold); px(g, 13, 11, 6, 1, P.gold); px(g, 13, 9, 1, 1, '#5078d0');
    door2(g, 12, 22, st);
    if (lit(st)) { px(g, 14, -10, 4, 2, blink(t, 160) ? P.gold : P.lamp); if (blink(t, 160)) px(g, 15, -10, 2, 1, P.white); }
  }),
  // The test lab: a purple roof with a copper flue, a flask in the gable that fizzes while working.
  tests: drawn2x((c, x, y, st, t) => {
    detail(c, x, y, g => {
      roof2(g, 0, 2, 32, 18, P.purple, P.purpleD);
      px(g, 4, -4, 10, 2, P.o); px(g, 5, -4, 8, 1, P.copper);
      px(g, 6, -2, 6, 6, P.o); px(g, 8, -2, 2, 6, P.copper); px(g, 8, -2, 1, 6, '#f0b070');
      const fizz = lit(st) ? (blink(t, 300) ? P.fizzA : P.fizzB) : P.glass;
      px(g, 18, 4, 4, 3, P.o); px(g, 19, 4, 2, 2, P.glass);
      px(g, 17, 7, 6, 1, P.o); px(g, 16, 8, 8, 5, P.o); px(g, 17, 13, 6, 1, P.o);
      px(g, 17, 8, 6, 1, P.glass); px(g, 17, 9, 6, 3, fizz); px(g, 18, 12, 4, 1, fizz);
      if (lit(st)) { px(g, 18, 10, 1, 1, P.white); px(g, 21, 9, 1, 1, P.white); }
      wall2(g, 2, 18, 28, 14, P.wood);
      win2(g, 4, 20, st); win2(g, 20, 20, st); door2(g, 12, 22, st);
    });
    if (lit(st)) puffs(c, x + 4, y - 4, t, P.fizzA, P.fizzB);
  }),
  // The observatory: a dome lit from the left, its shutter open, and a telescope that sweeps the sky while working.
  research: house2x((g, st, t) => {
    for (let j = 4; j < 20; j++) {
      const dy = 19 - (j + 0.5), hw = Math.sqrt(225 - dy * dy), a = Math.round(17 - hw), b = Math.round(17 + hw);
      px(g, a, j, b - a, 1, P.o);
      if (j > 4 && b - a > 2) { px(g, a + 1, j, b - a - 2, 1, P.domeD); px(g, a + 1, j, Math.max(0, 15 - a), 1, P.dome); }
      if (j > 6 && j < 14) px(g, a + 2, j, 1, 1, P.white);
    }
    px(g, 16, 6, 4, 12, P.o); px(g, 17, 7, 2, 11, P.dark);
    const ang = -Math.PI / 4 + (lit(st) ? Math.sin(t / 900) * 0.35 : 0), cs = Math.cos(ang), sn = Math.sin(ang);
    for (let d = 0; d <= 10; d++) px(g, Math.round(18 + cs * d) - 1, Math.round(12 + sn * d) - 1, 3, 3, P.o);
    for (let d = 2; d <= 10; d++) px(g, Math.round(18 + cs * d), Math.round(12 + sn * d), 1, 1, d > 8 ? P.gold : P.stoneL);
    wall2(g, 2, 18, 28, 14, P.stone);
    win2(g, 4, 20, st); win2(g, 20, 20, st); door2(g, 12, 22, st);
  }),
  // The library: a tiled pediment with an open book, a cornice, and shelves of books in its windows.
  docs: house2x((g, st) => {
    for (let j = 0; j < 14; j++) {
      px(g, 14 - j, j, 4 + 2 * j, 1, P.o);
      if (j > 1) px(g, 15 - j, j, 2 + 2 * j, 1, (j >> 1) % 2 ? P.teal : P.tealD);
    }
    for (let j = 4; j < 14; j += 2) for (let k = 15 - j + (j % 4 ? 2 : 0); k < 17 + j; k += 4) px(g, k, j, 1, 1, P.tealD);
    px(g, 13, 6, 6, 4, P.o); px(g, 14, 6, 2, 3, P.parch); px(g, 17, 6, 2, 3, P.parch); px(g, 16, 6, 1, 4, P.gold);
    px(g, 0, 14, 32, 4, P.o); px(g, 1, 14, 30, 2, P.stoneL); px(g, 1, 16, 30, 1, P.stone);
    for (let k = 2; k < 31; k += 4) px(g, k, 16, 1, 1, P.stoneL);
    wall2(g, 2, 18, 28, 14, P.parch);
    [4, 20].forEach((X, i) => {
      px(g, X, 18, 8, 12, P.woodD); px(g, X + 1, 19, 6, 5, glassOf(st)); px(g, X + 1, 24, 6, 1, P.woodL);
      for (let k = 0; k < 6; k++) px(g, X + 1 + k, 25 + (k % 3 === 1 ? 1 : 0), 1, 4 - (k % 3 === 1 ? 1 : 0), BOOKS[(i * 2 + k) % 4]);
      if (st === 'working') px(g, X + 2, 20, 1, 2, P.white);
    });
    door2(g, 12, 22, st);
  }),
  // Rebuilding: half the roof re-slated, cut stone stacked by the wall, a crane lowering a block while working.
  refactor: house2x((g, st, t) => {
    roof2(g, 0, 2, 32, 18, P.red, P.redD);
    for (let j = 4; j < 18; j++) px(g, 16, j, 14, 1, j % 4 === 3 ? P.slateD : P.slate);
    for (let j = 4, n = 0; j < 18; j += 4, n++) for (let k = 17 + (n % 2) * 2; k < 30; k += 4) px(g, k, j, 1, 3, P.slateD);
    px(g, 16, 4, 1, 14, P.o);
    wall2(g, 2, 18, 28, 14, P.wood);
    win2(g, 4, 20, st); door2(g, 12, 22, st);
    px(g, 22, 24, 10, 8, P.o);
    [[23, 25], [27, 25], [25, 28]].forEach(([X, Y]) => { px(g, X, Y, 4, 3, P.stone); px(g, X, Y, 4, 1, P.stoneL); });
    px(g, 28, -8, 2, 12, P.woodD); px(g, 28, -8, 1, 12, P.wood);
    px(g, 18, -8, 12, 2, P.woodD); px(g, 18, -8, 12, 1, P.wood); line2(g, 24, -6, 27, -3, P.woodD);
    const drop = lit(st) ? Math.round((Math.sin(t / 300) + 1) * 3) : 0;
    px(g, 20, -6, 1, 2 + drop, P.stoneD);
    px(g, 18, -4 + drop, 6, 4, P.o); px(g, 19, -3 + drop, 4, 2, P.stone); px(g, 19, -3 + drop, 4, 1, P.stoneL);
  }),
  // The windmill: a clapboard tower under a red cap, its sails turning while working.
  perf: house2x((g, st, t) => {
    for (let j = 6; j < 32; j++) {
      const hw = Math.round(6 + (j - 6) / 4);
      px(g, 15 - hw, j, 2 * hw + 2, 1, P.o);
      if (j < 31) { px(g, 16 - hw, j, 2 * hw, 1, (j - 6) % 4 === 3 ? P.millD : P.mill); px(g, 13 + hw, j, 3, 1, SHADE); }
    }
    px(g, 10, 2, 12, 1, P.o); px(g, 8, 3, 16, 5, P.o);
    px(g, 10, 3, 12, 1, P.red); px(g, 9, 4, 14, 3, P.red); px(g, 9, 6, 14, 1, P.redD); px(g, 10, 4, 4, 1, '#e07060');
    win2(g, 12, 10, st); door2(g, 12, 20, st);
    const a = lit(st) ? t / 260 : Math.PI / 4;
    for (let k = 0; k < 4; k++) {
      const cs = Math.cos(a + k * Math.PI / 2), sn = Math.sin(a + k * Math.PI / 2);
      // Half steps, so a sail at any angle is cloth without gaps; a lath across it every few.
      for (let d = 3; d <= 14; d += 0.5) {
        const X = Math.round(16 + cs * d), Y = Math.round(6 + sn * d);
        if (d > 4) for (const o of [1, 1.5, 2, 2.5]) px(g, Math.round(16 + cs * d - sn * o), Math.round(6 + sn * d + cs * o), 1, 1, Math.round(d) % 3 ? P.sail : P.millD);
        px(g, X, Y, 1, 1, P.woodD);
      }
    }
    px(g, 15, 5, 3, 3, P.o); px(g, 16, 6, 1, 1, P.stoneL);
  }),
  // The barn: a gambrel roof with a loft window, boarded walls, and big doors braced with an X (shut when blocked).
  chores: house2x((g, st) => {
    for (let j = 2; j < 20; j++) {
      const i = Math.max(0, 6 - (j - 2));
      px(g, i, j, 32 - 2 * i, 1, P.o);
      if (j > 3 && j < 18) px(g, i + 1, j, 30 - 2 * i, 1, j % 6 === 5 ? P.barnD : P.barn);
    }
    px(g, 5, 4, 22, 1, SHINE);
    px(g, 12, 6, 8, 8, P.white); px(g, 13, 7, 6, 6, P.woodD); px(g, 14, 8, 4, 4, glassOf(st)); px(g, 14, 10, 4, 1, P.woodD);
    wall2(g, 2, 18, 28, 14, P.barn);
    px(g, 3, 18, 26, 2, P.barnD);
    for (let k = 4; k < 28; k += 3) px(g, k, 20, 1, 9, P.barnD);
    if (st === 'blocked') px(g, 8, 20, 16, 12, P.o);
    else {
      px(g, 8, 20, 16, 12, P.white); px(g, 10, 22, 12, 8, P.woodL); px(g, 15, 22, 2, 8, P.white);
      for (let k = 11; k < 22; k += 2) px(g, k, 22, 1, 8, 'rgba(0,0,0,.12)');
      [[10, 14], [17, 21]].forEach(([a, b]) => { line2(g, a, 22, b, 29, P.white); line2(g, b, 22, a, 29, P.white); });
    }
  })
};
/* A house going up: two posts, three rails, a brace, and the odd spark. */
export const scaffold = drawn2x((c, x, y, t) => detail(c, x, y, g => {
  [2, 28].forEach(k => { px(g, k, 4, 2, 28, P.woodD); px(g, k, 4, 1, 28, P.woodL); });
  for (let j = 6; j < 32; j += 10) { px(g, 2, j, 28, 2, P.wood); px(g, 2, j, 28, 1, P.woodL); }
  line2(g, 4, 30, 27, 7, P.woodD); line2(g, 5, 30, 28, 7, P.wood);
  if (blink(t, 90)) {
    const s = 8 + Math.floor(hash(Math.floor(t / 90), 3) * 16);
    px(g, s, 8, 1, 1, P.gold); px(g, s + 1, 7, 1, 1, P.white); px(g, s - 1, 9, 1, 1, P.gold);
  }
}));
/* Cobwebs in a stalled house's two upper corners, each 3 by 3 scene
 * pixels from (x + 1, y + 9) and (x + 12, y + 9). Drawn at 2× (#461): fine
 * strands from the corner and two threads across them; the right-hand web
 * is the left one mirrored. */
const WEB = ['######', '##..#.', '#.#.#.', '#..##.', '####..', '#.....'];
export function cobwebs(c, x, y) {
  [[x + 1, false], [x + 12, true]].forEach(([wx, flip]) => detail(c, wx, y + 9, g => WEB.forEach((row, j) => {
    for (let i = 0; i < 6; i++) if (row[i] === '#') px(g, flip ? 5 - i : i, j, 1, 1, P.web);
  })));
}
/* A priority pennant on a pole by the house at (x, y): P0 a red flag that
 * waves, P1 gold, P2 white, P3 none. Drawn at 2× (#461): a capped pole, a
 * swallowtail cloth with a fold, and a ripple that runs along the P0 flag. */
export function pennant(c, x, y, pr, t) {
  if (!pr || pr === 'P3') return;
  detail(c, x - 1, y - 5, g => {
    px(g, 0, 1, 2, 17, P.stoneD); px(g, 1, 2, 1, 16, P.stone); px(g, 0, 0, 2, 1, P.gold);
    if (pr === 'P0') {
      const w = blink(t, 300) ? 1 : 0;
      for (let j = 0; j < 8; j++) {
        const len = 8 + w * (j % 2) - (j === 3 || j === 4 ? 2 : 0), ripple = (j + (w ? 1 : 0)) % 4 === 0;
        px(g, 2, j, len + 1, 1, P.o);
        if (j > 0 && j < 7) px(g, 2, j, len, 1, ripple ? '#a01818' : P.p0);
      }
      px(g, 2, 1, 6, 1, '#f06050');
    } else {
      const cloth = pr === 'P1' ? P.gold : P.white, fold = pr === 'P1' ? '#c09020' : '#c8c8d0';
      [7, 7, 6, 6, 5, 4].forEach((len, j) => { px(g, 2, j, len + 1, 1, P.o); if (j > 0 && j < 5) px(g, 2, j, len, 1, j === 3 ? fold : cloth); });
    }
  });
}
