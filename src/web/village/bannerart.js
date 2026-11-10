/* ---------- A feature's flag (banner) ----------
 * No DOM, so node tests it (tools/village-tests/envart.test.mjs). Drawn by
 * features.js in the banner strip and by the Key. At 2× (#454) it keeps its
 * 1× footprint and gains finer detail (art2x.js). */
import { state } from './state.js';
import { P, R, blink } from './palette.js';
import { digits } from './scenery.js';
import { detail } from './detail.js';
import { px } from './art2x.js';
import { MEDIEVAL } from './themes/medieval.js';
import { M } from './themes/modern.js';

/* A flag on a pole: swallowtail cloth in the feature's own colour, always
 * (#406), a red pennant when a subtask is stuck, and once live a gold finial
 * and sparkles, never a gold cloth. The spec lies at its foot: a scroll in
 * the village, a clipboard downtown. */
export function drawBanner(c, g, on, t) {
  const { x, y, f } = g, col = g.col, wave = !state.reduce && blink(t + f.number * 7, 320) ? 1 : 0;
  const stuck = (f.stuck || []).length > 0, top = stuck ? y - 7 : y - 1, modern = state.T !== MEDIEVAL;
  R(c, x - 2, y + 23, 5, 2, P.o); R(c, x - 1, y + 23, 3, 1, modern ? M.concrete : P.stoneD);
  R(c, x, top, 1, y + 23 - top, modern ? M.steelD : P.o);
  if (f.live) { R(c, x - 1, top - 3, 3, 3, P.o); R(c, x, top - 2, 1, 1, P.gold); R(c, x - 1, top - 2, 3, 1, P.gold); R(c, x, top - 3, 1, 1, P.gold); }
  else R(c, x, top - 1, 1, 1, modern ? M.steel : P.stoneL);
  R(c, x + 1, y, 14, 10, P.o); R(c, x + 1, y + 1, 13, 8, col); R(c, x + 1, y + 1, 13, 1, P.white);
  if (!modern) R(c, x + 13, y + 4 + wave, 2, 2, P.o);
  // Its issue number on the cloth (#441), when three digits fit; else two lines.
  if (String(f.number).length <= 3 && f.number >= 0) digits(c, x + 2, y + 2, f.number, P.white);
  else { R(c, x + 4, y + 4, 6, 1, P.white); R(c, x + 4, y + 6, 4, 1, P.white); }
  if (stuck) { const w = !state.reduce && blink(t, 260) ? 1 : 0; R(c, x + 1, y - 7, 7 + w, 4, P.o); R(c, x + 1, y - 6, 6 + w, 2, P.p0); }
  if (modern) { R(c, x + 4, y + 18, 7, 7, P.o); R(c, x + 5, y + 19, 5, 5, P.white); R(c, x + 6, y + 18, 3, 1, M.steel); R(c, x + 6, y + 21, 3, 1, M.concreteD); R(c, x + 6, y + 23, 2, 1, M.concreteD); }
  else { R(c, x + 3, y + 19, 8, 5, P.o); R(c, x + 4, y + 20, 6, 3, P.parch); R(c, x + 5, y + 21, 4, 1, P.pathS); }
  if (f.live && !state.reduce) for (let k = 0; k < 3; k++) if (blink(t + k * 170, 220)) R(c, x + 3 + k * 5, y - 3 - (k % 2) * 2, 1, 1, P.gold);
  // At 2× (#454): the pole a detail pixel wide with a highlight, the cloth's
  // fold in shadow down its middle and a hem along its foot. The number on it
  // stays the 1× stencil, so it reads at every zoom.
  detail(c, x, y, d => {
    px(d, 1, (top - y) * 2, 1, (y + 23 - top) * 2, modern ? M.steel : P.stoneD);
    px(d, 15, 4, 1, 12, 'rgba(12,20,36,.22)');
    px(d, 3, 16, 24, 1, 'rgba(12,20,36,.28)');
  });
  if (on) { const o = !state.reduce && blink(t, 400) ? col : P.white; R(c, x - 3, y - 9, 20, 1, o); R(c, x - 3, y + 26, 20, 1, o); R(c, x - 3, y - 9, 1, 36, o); R(c, x + 16, y - 9, 1, 36, o); }
}
