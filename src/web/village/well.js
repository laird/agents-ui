/* ---------- The dry well ----------
 * A worker held by a usage or rate limit (#484, limits.js) stands at a dry
 * well beside its house, under an hourglass, instead of sitting asleep: it is
 * not idle and not waiting on you. Downtown and in the future the well is a
 * dry pump. When most of the village is limited, a sign at the hall says when
 * it resumes. Drawn at 2×, inside detail() (#477). */
import { state } from './state.js';
import { P } from './palette.js';
import { detail } from './detail.js';
import { px } from './art2x.js';
import { bitmap } from './glyphs2x.js';
import { speech } from './scenery.js';
import { person } from './people.js';
import { registerKey, registerLabels, registerLayer, tap } from './registry.js';
import { MODERN } from './themes/modern.js';
import { FUTURE } from './themes/future.js';
import { limited, signWords, villageLimit } from './limits.js';

/* The well's footprint in scene pixels, and where it stands by a lot. */
export const WELL = { w: 6, h: 8, dx: 10, dy: 10 };
/* Where its worker stands: a step left of the door, so the well beside it shows. */
export const AT_WELL = { dx: 2, dy: 8 };
/* The hourglass in the worker's bubble: 10 by 10 detail pixels. */
export const HOURGLASS = ['##########', '.#......#.', '..#....#..', '...#..#...', '....##....', '....##....', '...#..#...', '..#.##.#..', '.#.####.#.', '##########'];
export const LIMIT_COL = '#3890e0';
const WELL_ROOF = '#c04830', WELL_ROOF_D = '#802818';

/* A dry well 6×8 at scene (x, y): a stone ring with nothing in it, a roof on
 * two posts, the bucket hauled up empty. A dry pump in the other themes. */
export function well(c, x, y, theme = state.T) {
  detail(c, x, y, d => {
    if (theme === MODERN || theme === FUTURE) {
      // A pump: a body, a spout with no drip, the hose hung on its side.
      const body = theme === FUTURE ? '#262b4c' : '#a8a8b0', lit = theme === FUTURE ? '#40e0f0' : P.white;
      px(d, 2, 1, 8, 15, P.o); px(d, 3, 2, 6, 13, body);
      px(d, 4, 3, 4, 3, P.o); px(d, 5, 4, 2, 1, lit);
      px(d, 9, 5, 3, 2, P.o); px(d, 10, 7, 1, 5, P.o);
      px(d, 1, 15, 10, 1, P.o);
      return;
    }
    // A red gabled roof.
    px(d, 4, 0, 4, 1, P.o); px(d, 2, 1, 8, 1, P.o); px(d, 0, 2, 12, 2, P.o);
    px(d, 4, 1, 4, 1, WELL_ROOF); px(d, 1, 2, 10, 1, WELL_ROOF); px(d, 1, 3, 10, 1, WELL_ROOF_D);
    // Its posts, the rope, and the bucket hauled up empty.
    px(d, 1, 4, 2, 6, P.o); px(d, 9, 4, 2, 6, P.o); px(d, 1, 4, 1, 6, P.wood); px(d, 9, 4, 1, 6, P.wood);
    px(d, 6, 4, 1, 2, P.woodL); px(d, 4, 6, 4, 3, P.o); px(d, 5, 6, 2, 2, P.wood);
    // The stone ring, its mouth dark and dry.
    px(d, 0, 9, 12, 7, P.o); px(d, 1, 10, 10, 5, P.stone); px(d, 1, 10, 10, 1, P.stoneL);
    px(d, 2, 10, 8, 2, '#1a1410');
    [[3, 13], [7, 13], [5, 14], [9, 14]].forEach(([bx, by]) => px(d, bx, by, 1, 1, P.stoneD));
  });
}

/* The hourglass bubble over a limited worker. */
export function limitBubble(c, x, y) {
  detail(c, x, y, g => {
    speech(g, { outline: P.o, fill: P.white, dark: '#d0d4e0' });
    bitmap(g, 4, 4, HOURGLASS, LIMIT_COL);
  });
}

/* A sign on a post, for the hall: the words are its label. */
export function limitSign(c, x, y) {
  detail(c, x, y, d => {
    px(d, 7, 8, 2, 12, P.o); px(d, 7, 8, 1, 12, P.wood);
    px(d, 0, 0, 16, 9, P.o); px(d, 1, 1, 14, 7, LIMIT_COL);
    bitmap(d, 3, 1, ['#######...', '.#...#....', '..#.#.....', '...#......', '..#.#.....', '.#...#....', '#######...'], P.white);
  });
}

/* The well and its worker, after the houses (lifecycle.js leaves a limited
 * worker to this, rest.js), and the bubble over everything. */
registerLayer({ z: 72, draw(c, scene, t) {
  const { v, lay, at } = scene, out = scene.limitBubbles = [];
  v.houses.forEach((h, i) => {
    const lot = lay.lots[i]; if (!lot || !limited(h)) return;
    well(c, lot.x + WELL.dx, lot.y + WELL.dy);
    tap(lot.x + WELL.dx, lot.y + WELL.dy, WELL.w, WELL.h, { kind: 'house', i });
    const away = state.porters.some(p => p.seed === h.id && t >= p.t0 && at && at.get(p));
    if (away) return;
    person(c, lot.x + AT_WELL.dx, lot.y + AT_WELL.dy, h.id, 'stand', { status: 'idle' });
    tap(lot.x + AT_WELL.dx, lot.y + AT_WELL.dy, 8, 12, { kind: 'person', i, who: h.crew[0] });
    out.push([lot.x + AT_WELL.dx, lot.y - 3]);
  });
  scene.villageLimit = villageLimit(v);
  if (scene.villageLimit) {
    limitSign(c, lay.hallX - 10, 30);
    tap(lay.hallX - 10, 30, 8, 10, { kind: 'hall' });
  }
} });
registerLayer({ z: 131, draw(c, scene) {
  (scene.limitBubbles || []).forEach(([x, y]) => limitBubble(c, x, y));
} });

/* The hall's sign says when the village resumes. */
registerLabels(sc => {
  const s = villageLimit(sc.v);
  if (!s) return [];
  return [{ x: sc.L.hallX - 6, y: 29, anchor: 'bottom', text: signWords(s), sub: `${s.n} of ${s.of} limited`, cls: 'limitsign' }];
}, 20);

/* The Key: the dry well and the hourglass. */
registerKey({ group: 'Beside a house: a usage limit', rows: theme => [
  [c => { well(c, 9, 4, theme); person(c, 1, 4, 'key:limited', 'stand', { status: 'idle' }); }, 'At a dry well: its runtime hit a usage or rate limit, and waits for it to reset', 18],
  [c => limitBubble(c, 3, 1), 'The hourglass: rate limited; its label says when it resets'],
  [c => limitSign(c, 4, 2), 'A sign at the hall: most of the village is limited, and when it resumes', 18]
] });
