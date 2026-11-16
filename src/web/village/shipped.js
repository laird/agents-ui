/* ---------- An epic reaching production (#482) ----------
 * Feature flags fly only while a feature has work in flight (#401), so an
 * epic used to finish unnoticed. When its last subtask docks in the last
 * stop, the server lists it in `v.shipped` for the rest of the day, and:
 *   - fireworks burst over the quay and the harbour (production), in the
 *     epic's flag colour, once, as the page sees it arrive; none in the
 *     screensaver's night hours (`dim`), nor with reduced motion;
 *   - its flag flies on the hall's pole until midnight, its title on hover
 *     and in the dialog a click opens;
 *   - the screensaver's strip says "Shipped: <epic>" (screensaver.js).
 * What fires and when is decided in shipdecide.js. */
import { state } from './state.js';
import { P, featureColour } from './palette.js';
import { detail } from './detail.js';
import { px } from './art2x.js';
import { wfFireworks } from './fireworks.js';
import { registerHit, registerKey, registerLayer, tap } from './registry.js';
import { link, objAt } from './dialogs.js';
import { cv } from './layout.js';
import { agoWords } from './envlabels.js';
import { dimmedAt, saverOptions } from './saver.js';
import { MODERN } from './themes/modern.js';
import { FUTURE } from './themes/future.js';
import { hallFlag, hallFlagWords, newCelebrations } from './shipdecide.js';

const SHIP_NIGHT = saverOptions(location.search, location.hash).dim;
const SHIP_ROUNDS = 3, SHIP_GAP = 700, SHIP_BURST = 1600;
const shipSeen = new Map(), shipShows = new Map(); // per project: numbers seen; bursts going off

/* An epic's colour: its flag's, as if it still flew among the village's. */
const shipColour = (s, v) => featureColour(s, [...(v.features || []), s]);

/* Where the hall's flag hangs, in detail pixels from the hall's origin
 * (hallX - 4, 18): the castle keep's own flag, beside the tower's spire, on
 * the dome's mast. */
const shipPole = () => (state.T === MODERN ? [42, -34] : state.T === FUTURE ? [42, -31] : [42, -22]);

/* The flag's cloth, 10×6 detail pixels: an outline, the colour, a light hem. */
function shipCloth(g, X, Y, col) {
  px(g, X, Y, 10, 6, P.o);
  px(g, X, Y + 1, 9, 4, col);
  px(g, X, Y + 1, 9, 1, 'rgba(255,255,240,.45)');
}

/* On the hall (z 50), the newest shipped epic's flag. */
registerLayer({ z: 55, draw(c, { v, lay }) {
  const s = hallFlag(v.shipped);
  if (!s) return;
  const [X, Y] = shipPole();
  detail(c, lay.hallX - 4, 18, g => shipCloth(g, X, Y, shipColour(s, v)));
  tap(lay.hallX - 4 + X / 2, 18 + Y / 2 - 1, 7, 5, { kind: 'shipped' });
} });

/* Over everything, fireworks for a shipping just seen: three rounds of two
 * bursts each over the harbour and the quay. */
registerLayer({ z: 145, draw(c, { v, lay }, t) {
  let seen = shipSeen.get(v.project);
  if (!seen) shipSeen.set(v.project, seen = new Set());
  const quiet = state.reduce || dimmedAt(new Date().getHours(), SHIP_NIGHT);
  const fresh = newCelebrations(v.shipped, seen, Date.now(), quiet).map(s => ({ col: shipColour(s, v), t }));
  const shows = [...(shipShows.get(v.project) || []), ...fresh].filter(b => t - b.t < SHIP_BURST + SHIP_GAP * (SHIP_ROUNDS - 1));
  shipShows.set(v.project, shows);
  shows.forEach(b => {
    for (let k = 0; k < SHIP_ROUNDS; k++) {
      const fw = b.t + k * SHIP_GAP;
      wfFireworks(c, lay.castleX, lay.canalY, t, fw, [[6 + k * 9, b.col], [30 - k * 9, b.col]], 58 + k * 4);
      wfFireworks(c, lay.roadX - 8, lay.quayY, t, fw, [[4 + k * 7, b.col], [22 - k * 5, b.col]], 22 + k * 4);
    }
  });
} });

registerHit({ kind: 'shipped', dialog(o, sc, theme) {
  const list = sc.v.shipped || [], s = list[0], stops = sc.v.pipeline.stops, last = stops[stops.length - 1];
  if (!s) return null;
  const when = agoWords(s.at);
  return {
    title: `Shipped: #${s.number} ${s.title}`,
    line: `Every subtask reached ${last ? last.name : 'production'}${when ? ' ' + when : ''}. Its flag flies on the ${(theme || state.T).labels.hall.toLowerCase()} for the rest of the day.`,
    meta: list.slice(1).map(x => `Also shipped today: #${x.number} ${x.title}`),
    links: list.map(x => link(`Epic #${x.number}`, x.url)),
  };
} });

/* Hovering the flag names the epic. */
cv.addEventListener('mousemove', e => {
  const h = objAt(e), words = h && h.o.kind === 'shipped' ? hallFlagWords(h.sc.v.shipped) : '';
  if (words || cv.title.startsWith('Shipped today')) cv.title = words;
});

registerKey({ group: 'other', rows: () => [
  [c => detail(c, 0, 4, g => shipCloth(g, 4, 0, P.gold)), 'A flag on the hall: an epic reached production today (fireworks when it arrives)']] });
