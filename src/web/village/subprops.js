/* What a researcher, an analyst or an auditor sub-agent holds at its left
 * side, one drawing per theme (#553): a map, a chart or ledger, a seal or
 * clipboard. person() draws it for `opts.prop`; other sub-agents hold
 * nothing. Each is 6 detail pixels wide; '.' is clear, any other letter a
 * colour from INK. */
import { P, R } from './palette.js';
import { detail } from './detail.js';

const INK = {
  o: P.o, w: P.white, p: P.parch, d: P.woodD, l: P.wood, t: P.teal, r: P.p0, R: P.redD, g: P.gold,
  v: P.purpleD, S: P.stoneL, k: P.dark, G: P.seal, b: '#3878c8', c: '#40e0f0', C: '#b8f8ff', m: '#f048c0'
};
export const SUB_PROPS = {
  researcher: {
    // A map between two wooden rollers: a river and a red mark.
    medieval: ['oooooo', 'oddddo', 'opptpo', 'optppo', 'optrpo', 'opptpo', 'oddddo', 'oooooo'],
    // A folded street map: roads, a park, a pin.
    modern: ['oooooo', 'owbwGo', 'obbbbo', 'owbwwo', 'oGbwwo', 'owbwro', 'oooooo'],
    // A holo-map on an emitter, a pin on it.
    future: ['oooooo', 'ocCcco', 'oCcmCo', 'occCco', 'oooooo', '..oo..', '.okko.']
  },
  analyst: {
    // A leather ledger with a gold clasp and a ribbon marker.
    medieval: ['oooooo', 'ovvvvp', 'ovggvp', 'ovvvvp', 'ovvvvp', 'ovvRvp', 'oooooo', '...R..'],
    // A bar chart on a board.
    modern: ['oooooo', 'owwwGo', 'owwbGo', 'owrbGo', 'owrbGo', 'oooooo'],
    // Hologram bars over their emitter.
    future: ['....C.', '..c.C.', 'c.c.C.', 'c.c.C.', 'CCCCCC', '.okko.']
  },
  auditor: {
    // A red wax seal, gold-stamped, on two ribbons.
    medieval: ['.oooo.', 'orrrro', 'orgrro', 'orrgro', 'orrrro', '.oRRo.', '.R..R.', 'R....R'],
    // A clipboard of ticked lines.
    modern: ['.oSSo.', 'olSSlo', 'olwwlo', 'olGwlo', 'olwwlo', 'olGwlo', 'ollllo', 'oooooo'],
    // A scanner pad showing a green tick.
    future: ['oooooo', 'okkkGo', 'oGkGko', 'okGkko', 'oooooo', '..cc..']
  }
};
/* Each drawing as rectangles, one per run of a letter along a row. */
const RUNS = Object.fromEntries(Object.entries(SUB_PROPS).map(([name, looks]) => [name, Object.fromEntries(Object.entries(looks).map(([style, rows]) => {
  const out = [];
  rows.forEach((row, y) => {
    for (let x = 0; x < row.length;) {
      let w = 1;
      while (row[x + w] === row[x]) w++;
      if (row[x] !== '.') out.push([x, y, w, INK[row[x]]]);
      x += w;
    }
  });
  return [style, out];
}))]));
/* Draw `name`'s prop in `style` (a theme's people) beside the body whose
 * top left is scene (x, y): its left hand holds it. */
export function subProp(c, x, y, name, style) {
  const runs = RUNS[name] && RUNS[name][style]; if (!runs) return;
  detail(c, x, y, g => runs.forEach(([px, py, w, col]) => R(g, px - 6, py + 10, w, 1, col)));
}
