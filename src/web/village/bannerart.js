/* ---------- A feature's flag (banner) ----------
 * No DOM, so node tests it (tools/village-tests/envart.test.mjs). Drawn by
 * features.js in the banner strip and by the Key. Redrawn at 2× (#475), it
 * keeps its 1× footprint (art2x.js). */
import { state } from './state.js';
import { F, P, SHADE, blink } from './palette.js';
import { digits } from './scenery.js';
import { detail } from './detail.js';
import { disc, px } from './art2x.js';
import { MEDIEVAL } from './themes/medieval.js';
import { M } from './themes/modern.js';
import { FUTURE } from './themes/future.js';

/* A flag on a pole, drawn in detail pixels (detail.js) on the scene pixels
 * its 1× art covered (#475): swallowtail cloth in the feature's own colour,
 * always (#406), rippling gently with folds in shadow; a red pennant when a
 * subtask is stuck; and once live a gold finial and sparkles, never a gold
 * cloth. The spec lies at its foot: a scroll in the village, a clipboard
 * downtown, a data pad in the future. Its issue number is on the cloth in
 * the 2× font (#441, #456). */
export function drawBanner(c, g, on, t) {
  const { x, y, f } = g, col = g.col, still = state.reduce, th = state.T;
  const stuck = (f.stuck || []).length > 0, top = stuck ? y - 7 : y - 1, T = (top - y) * 2;
  const look = th === FUTURE ? 'future' : th === MEDIEVAL ? 'medieval' : 'modern';
  detail(c, x, y, d => {
    // The foot of the pole, and the pole.
    if (look === 'medieval') { px(d, -3, 46, 8, 1, P.o); px(d, -4, 47, 10, 3, P.o); px(d, -2, 47, 6, 2, P.stone); px(d, -2, 47, 3, 1, P.stoneL); }
    else { px(d, -4, 46, 10, 4, P.o); px(d, -3, 47, 8, 2, look === 'future' ? F.deck : M.concrete); px(d, -3, 47, 8, 1, look === 'future' ? F.cyan : M.concreteL); }
    px(d, 0, T, 2, 46 - T, P.o); px(d, 1, T + 1, 1, 45 - T, look === 'medieval' ? P.woodL : look === 'future' ? F.hullD : M.steel);
    if (f.live) { disc(d, -2, T - 6, 6, { outline: P.o, fill: P.gold, light: '#fff4b0', dark: '#c09020' }); px(d, 0, T - 5, 1, 1, P.white); }
    else px(d, 0, T - 2, 2, 2, look === 'medieval' ? P.stoneL : look === 'future' ? F.cyan : M.steel);
    // The cloth, a column at a time: in a trough of the ripple a column's top
    // dips a detail pixel and it is in shadow; its foot stays on the 1× hem.
    // The swallowtail is cut out.
    const phase = still ? 0 : Math.floor(t / 180) % 12;
    for (let cx = 2; cx < 30; cx++) {
      const s = cx > 6 && (cx + 12 - phase) % 12 < 6 ? 1 : 0, cut = look === 'medieval' ? Math.max(0, cx - 26) : 0;
      const runs = cut ? [[s, 10 + s - cut], [10 + s + cut, 20]] : [[s, 20]];
      for (const [a, b] of runs) {
        px(d, cx, a, 1, b - a, P.o);
        if (cx === 29 || b - a < 3) continue;
        px(d, cx, a + 1, 1, b - a - 2, col);
        if (a === s) px(d, cx, a + 1, 1, 1, P.white);
        if (b === 20) px(d, cx, b - 3, 1, 1, look === 'future' ? F.cyanL : SHADE);
        if (s) px(d, cx, a + 2, 1, b - a - 5, 'rgba(12,20,36,.16)');
      }
    }
    if (String(f.number).length > 3 || f.number < 0) { px(d, 8, 8, 12, 2, P.white); px(d, 8, 12, 8, 2, P.white); }
    if (stuck) {
      const L = 14 + (!still && blink(t, 260) ? 2 : 0);
      for (let j = 0; j < 8; j++) {
        const len = L - Math.max(0, Math.abs(j - 3.5) - 0.5) * 3;
        px(d, 2, -14 + j, len, 1, P.o);
        if (j > 0 && j < 7) px(d, 2, -14 + j, len - 2, 1, j === 1 ? '#ff7060' : P.p0);
      }
    }
    // The spec at its foot.
    if (look === 'medieval') {
      px(d, 6, 38, 16, 10, P.o); px(d, 8, 39, 12, 8, P.parch);
      px(d, 7, 39, 2, 8, P.mill); px(d, 19, 39, 2, 8, P.mill); px(d, 8, 39, 1, 8, P.millD); px(d, 20, 39, 1, 8, P.millD);
      px(d, 10, 41, 8, 1, P.pathS); px(d, 10, 43, 5, 1, P.pathS); px(d, 15, 44, 3, 3, P.red); px(d, 15, 44, 1, 1, '#e07060');
    } else if (look === 'future') {
      px(d, 8, 36, 14, 14, P.o); px(d, 9, 37, 12, 12, F.navy); px(d, 10, 38, 10, 8, F.panel);
      px(d, 11, 40, 7, 1, F.cyan); px(d, 11, 42, 5, 1, F.cyan); px(d, 11, 44, 6, 1, F.cyanD); px(d, 14, 47, 2, 1, F.cyanL);
    } else {
      px(d, 8, 36, 14, 14, P.o); px(d, 9, 37, 12, 12, M.boxD); px(d, 10, 39, 10, 10, P.white);
      px(d, 12, 36, 6, 3, M.steel); px(d, 13, 36, 4, 1, M.concreteL);
      px(d, 11, 41, 7, 1, M.concreteD); px(d, 11, 43, 8, 1, M.concreteD); px(d, 11, 45, 4, 1, M.concreteD); px(d, 16, 46, 2, 1, P.seal);
    }
    if (f.live && !still) for (let k = 0; k < 3; k++) if (blink(t + k * 170, 220)) { const X = 6 + k * 10, Y = -6 - (k % 2) * 4; px(d, X, Y, 2, 2, P.gold); px(d, X, Y, 1, 1, P.white); }
    if (on) {
      const o = !still && blink(t, 400) ? col : P.white;
      px(d, -6, -18, 40, 2, o); px(d, -6, 52, 40, 2, o); px(d, -6, -18, 2, 72, o); px(d, 32, -18, 2, 72, o);
    }
  });
  if (String(f.number).length <= 3 && f.number >= 0) digits(c, x + 2, y + 2, f.number, P.white);
}
