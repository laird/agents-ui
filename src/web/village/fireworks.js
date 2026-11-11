/* ---------- Fireworks over a destination at 2× (#472) ----------
 * No DOM, so each theme's destination can draw them without loading the
 * page's state (the Medieval harbour, the Modern launch pad and the Future
 * spaceport). */
import { P } from './palette.js';
import { detail } from './detail.js';
import { px } from './art2x.js';

/* Fireworks over a destination for 1.6 s after a release ships: a burst per
 * [dx, colour] in `bursts`, rising from `top` scene pixels above cy. Each
 * spark has a white-hot core while young and leaves a trail as it fades. */
export function wfFireworks(c, x, cy, t, fw, bursts, top) {
  const ft = t - fw;
  if (ft < 0 || ft >= 1600) return;
  const p = ft / 1600;
  detail(c, x, cy, g => bursts.forEach(([dx, col], i) => {
    const fx = 2 * dx, fy = -2 * (top + p * 18 + i * 3);
    for (let k = 0; k < 8; k++) {
      const a = k / 8 * Math.PI * 2, at = r => [Math.round(fx + Math.cos(a) * r), Math.round(fy + Math.sin(a) * r)];
      const [sx, sy] = at(p * 18);
      px(g, sx, sy, 2, 2, col);
      if (p < 0.5) px(g, sx, sy, 1, 1, P.white);
      else { const [tx, ty] = at(p * 12); px(g, tx, ty, 1, 1, col); }
    }
  }));
}
