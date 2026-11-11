/* Houses, one sprite per kind, and what is drawn on them. */
import { P, R, SHADE, SHINE, blink, glassOf, hash, lit } from '../palette.js';
import { detail } from '../detail.js';
import { px } from '../art2x.js';
function win(c, x, y, st) {
  R(c, x, y, 4, 4, P.woodD); R(c, x + 1, y + 1, 2, 2, glassOf(st));
  if (st === 'working') R(c, x + 1, y + 1, 1, 1, P.white);
  // A flower box under every window.
  R(c, x, y + 4, 4, 1, P.woodL); R(c, x, y + 3, 1, 1, '#e05050'); R(c, x + 3, y + 3, 1, 1, P.gold);
}
function door(c, x, y, st) {
  if (st === 'blocked') { R(c, x, y, 4, 5, P.o); return; }
  R(c, x, y, 4, 5, P.woodD); R(c, x + 2, y + 2, 1, 1, P.gold);
}
function gable(c, x, y, w, h, a, d) {
  R(c, x, y, w, h, P.o);
  for (let r = 1; r < h - 1; r++) R(c, x + 1, y + r, w - 2, 1, (r % 3 === 0) ? d : a);
  R(c, x + 1, y + 1, w - 2, 1, d);
  // Shingles on the lit slope, shade on the far one, a sunlit ridge.
  for (let r = 2; r < h - 1; r += 3) for (let k = x + 2 + (r % 2) * 2; k < x + w - 2; k += 4) R(c, k, y + r, 1, 1, d);
  R(c, x + 1 + Math.ceil((w - 2) * 0.62), y + 1, Math.floor((w - 2) * 0.38), h - 2, SHADE);
  R(c, x + 1, y + 1, Math.ceil((w - 2) * 0.62), 1, SHINE);
  R(c, x + 1, y + h - 1, w - 2, 1, SHADE);
}
function wall(c, x, y, w, h, col) {
  R(c, x, y, w, h, P.o); R(c, x + 1, y, w - 2, h - 1, col);
  R(c, x + 1, y + h - 2, w - 2, 1, P.stoneD);
  R(c, x + w - 3, y, 2, h - 2, SHADE);
}
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

/* ---------- Houses: each draws in x..x+15, y-4..y+16 ---------- */
export const B = {
  cottage(c, x, y, st, t, roof = P.thatch, roofD = P.thatchD) {
    R(c, x + 11, y - 2, 4, 5, P.o); R(c, x + 12, y - 1, 2, 4, P.stone);
    gable(c, x, y + 1, 16, 9, roof, roofD);
    wall(c, x + 1, y + 9, 14, 7, P.wood); R(c, x + 2, y + 9, 12, 1, P.woodD);
    win(c, x + 2, y + 10, st); win(c, x + 10, y + 10, st); door(c, x + 6, y + 11, st);
    if (lit(st)) puffs(c, x + 12, y - 4, t, P.white, P.stoneL);
  },
  workshop(c, x, y, st, t) {
    B.cottage(c, x, y, st, t, P.red, P.redD);
    R(c, x + 4, y + 4, 6, 3, P.o); R(c, x + 5, y + 4, 4, 1, P.stoneL); R(c, x + 6, y + 5, 2, 1, P.stoneL);
  },
  feature(c, x, y, st, t) {
    gable(c, x, y + 1, 11, 9, P.red, P.redD);
    wall(c, x + 1, y + 9, 10, 7, P.wood); R(c, x + 2, y + 9, 8, 1, P.woodD);
    win(c, x + 2, y + 10, st); door(c, x + 6, y + 11, st);
    R(c, x + 11, y + 2, 1, 14, P.plank); R(c, x + 15, y + 2, 1, 14, P.plank);
    R(c, x + 10, y + 2, 6, 1, P.plank); R(c, x + 11, y + 7, 5, 1, P.plank);
    for (let k = 0; k < 4; k++) R(c, x + 12 + k, y + 3 + k, 1, 1, P.woodL);
    R(c, x + 11, y + 12, 5, 1, P.plank); R(c, x + 11, y + 13, 5, 1, P.woodD); R(c, x + 11, y + 14, 5, 1, P.plank); R(c, x + 11, y + 15, 5, 1, P.o);
    if (lit(st) && blink(t, 140)) { R(c, x + 12 + (Math.floor(t / 280) % 3), y + 6, 1, 1, P.white); R(c, x + 13, y + 5, 1, 1, P.gold); }
  },
  bug(c, x, y, st, t) {
    gable(c, x, y + 1, 16, 9, P.thatch, P.thatchD);
    R(c, x + 3, y + 3, 5, 4, P.woodD); R(c, x + 4, y + 4, 3, 2, P.plank); R(c, x + 5, y + 4, 1, 2, P.woodD);
    wall(c, x + 1, y + 9, 14, 7, P.wood); R(c, x + 2, y + 9, 12, 1, P.woodD);
    win(c, x + 2, y + 10, st); door(c, x + 6, y + 11, st);
    R(c, x + 11, y + 2, 1, 14, P.woodL); R(c, x + 14, y + 2, 1, 14, P.woodL);
    for (let yy = y + 3; yy < y + 16; yy += 3) R(c, x + 11, yy, 4, 1, P.woodD);
    R(c, x + 9, y - 1, 1, 2, P.o);
    R(c, x + 6, y - 4, 7, 4, P.o); R(c, x + 7, y - 3, 5, 2, P.bug); R(c, x + 8, y - 3, 1, 1, P.o); R(c, x + 10, y - 2, 1, 1, P.o); R(c, x + 9, y - 3, 1, 2, P.o);
    if (lit(st)) R(c, x + 2 + Math.floor(t / 160) % 8, y + 7, 2, 1, P.o);
  },
  security(c, x, y, st, t) {
    R(c, x, y + 10, 4, 6, P.o); R(c, x + 1, y + 11, 2, 4, P.stone);
    R(c, x + 12, y + 10, 4, 6, P.o); R(c, x + 13, y + 11, 2, 4, P.stone);
    R(c, x + 3, y - 2, 10, 18, P.o); R(c, x + 4, y - 1, 8, 16, P.stone); R(c, x + 4, y - 1, 8, 1, P.stoneL);
    for (let yy = y + 1; yy < y + 14; yy += 3) for (let xx = x + 5 + ((yy - y) % 2 ? 2 : 0); xx < x + 11; xx += 4) R(c, xx, yy, 1, 1, P.stoneD);
    [3, 7, 11].forEach(dx => R(c, x + dx, y - 4, 2, 2, P.o));
    R(c, x + 5, y + 1, 1, 3, glassOf(st)); R(c, x + 10, y + 1, 1, 3, glassOf(st));
    R(c, x + 6, y + 4, 4, 4, P.gold); R(c, x + 7, y + 5, 2, 2, P.shield); R(c, x + 7, y + 8, 2, 1, P.gold);
    door(c, x + 6, y + 11, st);
    if (lit(st)) R(c, x + 7, y - 5, 2, 1, blink(t, 160) ? P.gold : P.lamp);
  },
  tests(c, x, y, st, t) {
    gable(c, x, y + 1, 16, 9, P.purple, P.purpleD);
    R(c, x + 2, y - 2, 5, 1, P.o); R(c, x + 3, y - 1, 3, 3, P.o); R(c, x + 4, y - 1, 1, 3, P.copper);
    R(c, x + 9, y + 2, 2, 1, P.o); R(c, x + 8, y + 3, 4, 4, P.o); R(c, x + 9, y + 4, 2, 2, lit(st) ? (blink(t, 300) ? P.fizzA : P.fizzB) : P.glass);
    wall(c, x + 1, y + 9, 14, 7, P.wood); R(c, x + 2, y + 9, 12, 1, P.woodD);
    win(c, x + 2, y + 10, st); win(c, x + 10, y + 10, st); door(c, x + 6, y + 11, st);
    if (lit(st)) puffs(c, x + 4, y - 4, t, P.fizzA, P.fizzB);
  },
  research(c, x, y, st, t) {
    for (let dy = -7; dy <= 0; dy++) for (let dx = -7; dx <= 7; dx++) {
      const d = dx * dx + dy * dy; if (d > 49) continue;
      R(c, x + 8 + dx, y + 9 + dy, 1, 1, d > 36 ? P.o : dx < -1 ? P.dome : P.domeD);
    }
    R(c, x + 8, y + 3, 2, 6, P.dark);
    const a = -Math.PI / 4 + (lit(st) ? Math.sin(t / 900) * 0.35 : 0);
    for (let d = 0; d < 8; d++) {
      const px = Math.round(x + 9 + Math.cos(a) * d), py = Math.round(y + 6 + Math.sin(a) * d);
      R(c, px, py, 2, 2, P.o); if (d > 1) R(c, px, py, 1, 1, P.stoneD);
    }
    wall(c, x + 1, y + 9, 14, 7, P.stone);
    win(c, x + 2, y + 10, st); win(c, x + 10, y + 10, st); door(c, x + 6, y + 11, st);
  },
  docs(c, x, y, st, t) {
    for (let r = 0; r < 7; r++) R(c, x + 7 - r, y + r, 2 + 2 * r, 1, P.o);
    for (let r = 1; r < 7; r++) R(c, x + 8 - r, y + r, 2 * r, 1, r % 2 ? P.teal : P.tealD);
    R(c, x + 7, y + 3, 2, 2, P.gold);
    R(c, x, y + 7, 16, 2, P.o); R(c, x + 1, y + 7, 14, 1, P.stoneL);
    wall(c, x + 1, y + 9, 14, 7, P.parch);
    const books = ['#c04838', '#3870c0', '#58a048', '#f8d040'];
    [x + 2, x + 10].forEach((wx, i) => {
      R(c, wx, y + 9, 4, 6, P.woodD); R(c, wx + 1, y + 10, 2, 4, glassOf(st));
      R(c, wx + 1, y + 12, 1, 2, books[i * 2]); R(c, wx + 2, y + 12, 1, 2, books[i * 2 + 1]);
    });
    door(c, x + 6, y + 11, st);
  },
  refactor(c, x, y, st, t) {
    gable(c, x, y + 1, 16, 9, P.red, P.redD);
    for (let r = 1; r < 8; r++) R(c, x + 8, y + 1 + r, 7, 1, (r % 3 === 0) ? P.slateD : P.slate);
    wall(c, x + 1, y + 9, 14, 7, P.wood); R(c, x + 2, y + 9, 12, 1, P.woodD);
    win(c, x + 2, y + 10, st); door(c, x + 6, y + 11, st);
    R(c, x + 11, y + 12, 5, 4, P.o); R(c, x + 12, y + 13, 1, 2, P.stoneL); R(c, x + 14, y + 13, 1, 2, P.stoneL);
    R(c, x + 14, y - 4, 1, 6, P.woodD); R(c, x + 9, y - 4, 6, 1, P.woodD);
    const drop = lit(st) ? Math.round((Math.sin(t / 300) + 1) * 1.5) : 0;
    R(c, x + 10, y - 3, 1, 1 + drop, P.stoneD);
    R(c, x + 9, y - 2 + drop, 3, 2, P.o); R(c, x + 10, y - 2 + drop, 1, 1, P.stoneL);
  },
  perf(c, x, y, st, t) {
    for (let r = 0; r < 13; r++) {
      const hw = 3 + Math.floor(r / 4);
      R(c, x + 8 - hw - 1, y + 3 + r, 2 * hw + 2, 1, P.o);
      R(c, x + 8 - hw, y + 3 + r, 2 * hw, 1, r === 12 ? P.o : (r % 4 === 3 ? P.millD : P.mill));
    }
    R(c, x + 4, y + 1, 8, 3, P.o); R(c, x + 5, y + 1, 6, 2, P.red);
    win(c, x + 6, y + 5, st); door(c, x + 6, y + 10, st);
    const a = lit(st) ? t / 260 : Math.PI / 4, hx = x + 8, hy = y + 3;
    for (let k = 0; k < 4; k++) {
      const ang = a + k * Math.PI / 2, cs = Math.cos(ang), sn = Math.sin(ang);
      for (let d = 2; d <= 7; d++) {
        const px = Math.round(hx + cs * d), py = Math.round(hy + sn * d);
        if (d > 2) R(c, Math.round(px - sn * 1.2), Math.round(py + cs * 1.2), 1, 1, P.sail);
        R(c, px, py, 1, 1, P.woodD);
      }
    }
    R(c, hx - 1, hy - 1, 2, 2, P.o);
  },
  chores(c, x, y, st, t) {
    for (let r = 0; r < 9; r++) { const i = Math.max(0, 3 - r); R(c, x + i, y + 1 + r, 16 - 2 * i, 1, P.o); }
    for (let r = 1; r < 8; r++) { const i = Math.max(0, 3 - r) + 1; R(c, x + i, y + 1 + r, 16 - 2 * i, 1, r % 3 === 0 ? P.barnD : P.barn); }
    R(c, x + 6, y + 3, 4, 4, P.white); R(c, x + 7, y + 4, 2, 2, glassOf(st));
    wall(c, x + 1, y + 9, 14, 7, P.barn); R(c, x + 2, y + 9, 12, 1, P.barnD);
    if (st === 'blocked') R(c, x + 4, y + 10, 8, 6, P.o);
    else {
      R(c, x + 4, y + 10, 8, 6, P.white); R(c, x + 5, y + 11, 6, 4, P.woodL);
      for (let k = 0; k < 4; k++) { R(c, x + 5 + k, y + 11 + k, 1, 1, P.white); R(c, x + 10 - k, y + 11 + k, 1, 1, P.white); }
    }
  }
};
export function scaffold(c, x, y, t) {
  R(c, x + 1, y + 2, 1, 14, P.woodL); R(c, x + 14, y + 2, 1, 14, P.woodL);
  for (let yy = y + 3; yy < y + 16; yy += 5) R(c, x + 1, yy, 14, 1, P.woodL);
  for (let k = 0; k < 12; k++) R(c, x + 2 + k, y + 15 - k, 1, 1, P.woodD);
  if (blink(t, 90)) R(c, x + 4 + Math.floor(hash(Math.floor(t / 90), 3) * 8), y + 4, 1, 1, P.gold);
}
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
