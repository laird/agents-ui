/* ---------- Keyboard order (#478) ----------
 * Where Tab goes next and where an arrow key lands, with no DOM: keynav.js
 * hands in one entry per keyboard target ({ o, v, scene, x, y, w, h }: the
 * object behind it, its village, which shown village it is in, and its box
 * on the page) and puts the buttons in the order given back.
 *
 * Tab order, in each shown village: whatever is waiting on you first (a
 * blocked house or castle, one with a question, or one whose sub-agent is
 * blocked), then the houses left to right, the hall, the board, the gate,
 * the quay, the docks and the environment row. Everything belonging to a
 * house (its people, sub-agents and crate) follows that house, so Tab
 * finishes one house before the next. */

/* The area of the map each kind belongs to, in Tab order. Houses and the
 * hall are placed by what they belong to (owner()), crates by where they are. */
const AREA = {
  board: 3,
  gatehouse: 4, train: 4, redbase: 4, gatecrate: 4, bounce: 4, scoreboard: 4, hotspots: 4,
  quay: 5, stop: 5, build: 5, blame: 5, harbour: 5, ship: 5,
  docks: 6,
  env: 7, envhouse: 7, voyage: 7, barge: 7
};
const CRATE_AREA = { gate: 4, quay: 5, ship: 5, backlog: 6, env: 7, voyage: 7 };
const OTHER = 8;
/* Within a house: its "!" (#571), the house, then who is in it, then its work. */
const WITHIN = { alarm: -1, house: 0, hall: 0, person: 1, sub: 2, crate: 3, gatecrate: 4 };

const HALL = -1;
/* Which house an object belongs to (HALL for the castle), or null. */
export function owner(o, v) {
  if (o.kind === 'hall') return HALL;
  if (o.kind === 'alarm') return o.i < 0 ? HALL : (o.i ?? null);
  if (o.kind === 'sub') return o.house ?? null;
  if (o.kind === 'house' || o.kind === 'person' || (o.kind === 'crate' && o.at === 'house')) return o.i ?? null;
  if (o.kind === 'gatecrate' && o.at === 'house') {
    const i = ((v && v.houses) || []).findIndex(h => h.issue === o.n);
    return i < 0 ? null : i;
  }
  return null;
}

/* A house (or the castle's manager) waiting on you: blocked, asking a
 * question, or with a sub-agent that is blocked. */
export function waitsOnYou(h) {
  return !!h && (h.status === 'blocked' || !!h.question || (h.subagents || []).some(sa => sa.status === 'blocked'));
}

function keyRank(e, anchors) {
  const v = e.v || {}, who = owner(e.o, v);
  if (who !== null) {
    const h = who === HALL ? v.manager : (v.houses || [])[who];
    const a = anchors.get(`${e.scene}|${who}`) || e;
    const area = waitsOnYou(h) ? 0 : who === HALL ? 2 : 1;
    return [e.scene, area, a.x, a.y, WITHIN[e.o.kind] ?? 5, e.y, e.x];
  }
  const area = e.o.kind === 'crate' ? (CRATE_AREA[e.o.at] ?? OTHER) : (AREA[e.o.kind] ?? OTHER);
  return [e.scene, area, e.x, e.y, 0, e.y, e.x];
}
const byRank = (a, b) => { for (let k = 0; k < a.length; k++) if (a[k] !== b[k]) return a[k] - b[k]; return 0; };

/* The entries in Tab order (a new array; the input is left as it is). */
export function keyOrder(entries) {
  // Each house is placed where its building is drawn: the leftmost of its own targets.
  const anchors = new Map();
  for (const e of entries) {
    if (e.o.kind !== 'house' && e.o.kind !== 'hall') continue;
    const k = `${e.scene}|${owner(e.o, e.v)}`, a = anchors.get(k);
    if (!a || e.x < a.x) anchors.set(k, { x: e.x, y: e.y });
  }
  return entries.map((e, k) => ({ e, r: keyRank(e, anchors), k }))
    .sort((a, b) => byRank(a.r, b.r) || a.k - b.k)
    .map(({ e }) => e);
}

/* The nearest target from `from` in a direction ('left', 'right', 'up',
 * 'down'), by the centres of their boxes, or null when nothing lies that
 * way. Straying sideways costs twice as much as going further, so Right
 * from a house finds its neighbour, not a crate down by the quay. */
export function nearest(from, all, dir) {
  const c = r => [r.x + r.w / 2, r.y + r.h / 2], [fx, fy] = c(from);
  let best = null, score = Infinity;
  for (const r of all) {
    if (r === from) continue;
    const [x, y] = c(r), dx = x - fx, dy = y - fy;
    const along = dir === 'right' ? dx : dir === 'left' ? -dx : dir === 'down' ? dy : -dy;
    const side = dir === 'left' || dir === 'right' ? Math.abs(dy) : Math.abs(dx);
    if (along <= 0) continue;
    const s = along + 2 * side;
    if (s < score) { score = s; best = r; }
  }
  return best;
}

/* Which answer a digit picks in a question dialog, or -1: in a numbered
 * menu the option with that number (a menu numbered 1, 2, 4 has no 3);
 * otherwise (yes/no) the digit's place, 1 for the first. */
export function optionFor(digit, keys) {
  if (!/^[1-9]$/.test(digit)) return -1;
  if (keys.some(k => /^[1-9]$/.test(k))) return keys.indexOf(digit);
  const k = Number(digit) - 1;
  return k < keys.length ? k : -1;
}
