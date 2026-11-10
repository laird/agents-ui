/* ---------- The environment row's art at 2× (#454) ----------
 * No DOM, so node tests it (tools/village-tests/envart.test.mjs).
 *
 * Each environment village's warehouse, sign and keep, its health marks
 * (the failed flag, the incident's fire) and its feature houses' flags.
 * Each is drawn as it was at 1× (its outline and fills, so it keeps its
 * footprint, its hit areas and its labels) and then given finer detail on
 * the 2× grid (#418) with art2x.js's helpers, as #451 did the scenery. */
import { state } from './state.js';
import { F, P, R, blink, shadow } from './palette.js';
import { detail } from './detail.js';
import { planks, px } from './art2x.js';
import { MODERN, M } from './themes/modern.js';
import { FUTURE } from './themes/future.js';

/* The row's colours in the theme shown. */
export const envLook = (T = state.T) => {
  const mod = T === MODERN, fut = T === FUTURE;
  return { roof: fut ? F.purple : mod ? M.steel : P.barn, roofD: fut ? F.purpleD : mod ? M.steelD : P.barnD,
    roofL: fut ? F.plasmaL : mod ? M.concrete : P.red,
    wall: fut ? F.hullD : mod ? M.concrete : P.wood, wallD: fut ? F.deckD : mod ? M.concreteD : P.woodD,
    bay: fut ? F.navy : mod ? M.asphaltD : P.woodD,
    keep: fut ? F.hull : mod ? M.glass : P.stone, keepD: fut ? F.hullD : mod ? M.glassD : P.stoneD,
    mod, fut };
};

/* The warehouse, w×h scene pixels at (x, y): a ribbed roof, a planked (or
 * panelled, or corrugated) wall round the open bay. */
export function envShed(c, x, y, w, h) {
  const k = envLook();
  shadow(c, x + 2, y + h, w, 2);
  R(c, x, y, w, h, P.o);
  R(c, x + 1, y + 1, w - 2, 7, k.roof); R(c, x + 1, y + 7, w - 2, 1, k.roofD);
  R(c, x + 1, y + 8, w - 2, h - 9, k.wall);
  R(c, x + 2, y + 9, w - 4, h - 10, k.bay);
  detail(c, x, y, g => {
    const W = w * 2, H = h * 2;
    // The roof: a light ridge under the outline, ribs a detail pixel wide.
    px(g, 2, 2, W - 4, 1, k.roofL);
    for (let xx = 7; xx < W - 4; xx += 6) px(g, xx, 3, 1, 11, k.roofD);
    // The eave's shadow line, then the wall's grain round the bay.
    px(g, 2, 15, W - 4, 1, k.roofD);
    if (k.fut) for (let xx = 10; xx < W - 4; xx += 12) px(g, xx, 16, 1, 2, k.wallD);
    else if (k.mod) for (let xx = 4; xx < W - 3; xx += 3) px(g, xx, 16, 1, 2, k.wallD);
    else planks(g, 2, 16, W - 4, 2, 1, k.wallD);
    // The bay's door rails down each side.
    px(g, 4, 18, 1, H - 20, k.wallD); px(g, W - 5, 18, 1, H - 20, k.wallD);
  });
}

/* The capital's keep (a tower downtown, a spire in the future), h tall at
 * (x, y), its flag in the health colour. */
export function envKeep(c, x, y, h, flag, t) {
  const k = envLook(), wave = blink(t, 700) ? 1 : 0;
  shadow(c, x + 2, y + h, 12, 2);
  R(c, x, y, 12, h, P.o); R(c, x + 1, y + 1, 10, h - 2, k.keep); R(c, x + 1, y + 1, 2, h - 2, k.keepD);
  for (let xx = x; xx < x + 12; xx += 4) R(c, xx, y - 2, 2, 2, P.o);
  for (let yy = y + 5; yy < y + h - 6; yy += 7) R(c, x + 5, yy, 2, 3, P.dark);
  R(c, x + 6, y - 10 + wave, 5, 3, flag);
  detail(c, x, y, g => {
    const H = h * 2;
    // Courses of stone (or glass panels, or hull plates) up the tower.
    for (let yy = 6; yy < H - 2; yy += k.fut ? 10 : 4) px(g, 6, yy, 15, 1, k.keepD);
    // Each window: a mullion and a sill.
    for (let yy = 10; yy < H - 12; yy += 14) { px(g, 11, yy, 1, 6, k.keep); px(g, 9, yy + 6, 6, 1, k.keepD); }
    // The flagpole a detail pixel wide, the flag's fold in shadow.
    px(g, 10, -20, 1, 16, P.woodD);
    px(g, 12, -20 + 2 * wave + 4, 10, 1, 'rgba(12,20,36,.28)');
  });
}

/* The sign by the road, at (x, y), its board in `face` (grey when nothing
 * says what runs there): a framed board on a post, two lines of lettering. */
export function envSign(c, x, y, face) {
  const [frame, board, ink] = state.T.sign || [P.woodD, P.parch, P.woodD];
  R(c, x + 6, y + 8, 2, 12, frame);
  R(c, x, y, 15, 9, P.o); R(c, x + 1, y + 1, 13, 7, face || board);
  detail(c, x, y, g => {
    // Lettering in detail pixels: a heading and a shorter line under it.
    for (let k = 0; k < 18; k += 3) px(g, 5 + k, 5, 2, 2, ink);
    px(g, 5, 10, 14, 1, ink); px(g, 5, 12, 10, 1, ink);
    // A nail at each top corner, and the post's grain.
    px(g, 3, 3, 1, 1, frame); px(g, 26, 3, 1, 1, frame);
    px(g, 13, 18, 1, 22, P.o);
  });
}

/* The red flag over a warehouse whose newest deployment failed, its pole at x. */
export function failFlag(c, x, y) {
  R(c, x, y, 1, 9, P.woodD); R(c, x + 1, y, 5, 3, P.p0);
  detail(c, x, y, g => { px(g, 2, 0, 10, 1, '#f06050'); px(g, 2, 5, 10, 1, P.redD); });
}

/* An open incident: a flame at (x, y), flickering (`f` 0 or 1). */
export function flame(c, x, y, f) {
  R(c, x, y - f, 3, 3 + f, P.p0); R(c, x + 1, y - 1 - f, 1, 2, P.gold);
  detail(c, x, y, g => { px(g, 2, -2 * f + 1, 2, 2, P.lamp); px(g, 1, 4, 4, 1, '#f07020'); });
}

/* A flag on a pole `pole` tall at x, its cloth `w` wide in `col`, raised by
 * `wave` (0 or 1): a feature house's flag, a failed ship's red one. */
export function poleFlag(c, x, y, col, wave, pole = 9, w = 4) {
  R(c, x, y, 1, pole, P.woodD); R(c, x + 1, y + wave, w, 3, col);
  detail(c, x, y, g => { px(g, 2, 2 * wave, w * 2, 1, 'rgba(255,255,240,.45)'); px(g, 2, 2 * wave + 5, w * 2, 1, 'rgba(12,20,36,.28)'); });
}
