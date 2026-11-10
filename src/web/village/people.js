/* People, robots and drones, and what they act out. Bodies are drawn at 2×
 * (#419): people2x.js lifts each 8×12 template onto the detail grid (#418)
 * with faces, hair, buttons, shoes and role clothing; tools and props are
 * drawn over them in scene pixels, as before. */
import { state, now } from './state.js';
import { F, P, R, blink, shadow } from './palette.js';
import { detail } from './detail.js';
import { lift, runs } from './people2x.js';

/* A posed body at 2×, its top-left at scene (x, y). */
function body2x(c, x, y, rows, map, opts) {
  const parts = runs(lift(rows, map, opts));
  detail(c, x, y, g => parts.forEach(([px, py, w, col]) => R(g, px, py, w, 1, col)));
}

/* ---------- People ----------
 * Four body templates per theme, two women and two men, alternating so a
 * village is a mix. Letters: o outline, h hair, s/S skin and its shade,
 * c/C clothing and its shade, t accent (belt, apron, tie), w shirt, p legs
 * or skirt. Who a villager is (template, skin, hair, clothes) comes from a
 * hash of their agent, so a person looks the same on every refresh. */
const BODY = {
  medieval: [
    // man in a belted tunic
    ['..oooo..', '.ohhhho.', 'ohhhhhho', 'ohssssho', 'osossoso', '.oSssSo.', '.occcco.', 'oCccccCo', 'sCttttCs', '.occcco.', '.opoopo.', '.oo..oo.'],
    // woman with long hair in a dress
    ['..oooo..', '.ohhhho.', 'ohhhhhho', 'ohssssho', 'hsossosh', 'hoSssSoh', 'hoctccoh', 'oCccccCo', 'sCccccCs', 'oCccccCo', 'oCCCCCCo', '.oo..oo.'],
    // bearded man in a hood
    ['..oooo..', '.occcco.', 'occcccco', 'ocssssco', 'osossoso', 'ochhhhco', '.ochhco.', 'oCccccCo', 'sCttttCs', '.occcco.', '.opoopo.', '.oo..oo.'],
    // woman with a bun and an apron
    ['...oo...', '..ohho..', '.ohhhho.', 'ohssssho', 'osossoso', '.oSssSo.', '.occcco.', 'oCttttCo', 'sCttttCs', 'oCttttCo', 'oCCCCCCo', '.oo..oo.']
  ],
  modern: [
    // man in a suit and tie
    ['..oooo..', '.ohhhho.', 'ohhhhhho', 'ohssssho', 'osossoso', '.oSssSo.', '.owttwo.', 'oCcttcCo', 'sCcttcCs', '.occcco.', '.opoopo.', '.oo..oo.'],
    // woman with long hair in a blazer and skirt
    ['..oooo..', '.ohhhho.', 'ohhhhhho', 'ohssssho', 'hsossosh', 'hoSssSoh', 'hocwwcoh', 'oCcwwcCo', 'sCccccCs', '.oppppo.', '.os..so.', '.oo..oo.'],
    // woman with a bob in a trouser suit
    ['..oooo..', '.ohhhho.', 'ohhhhhho', 'hhssssho', 'hsossosh', '.hSssSh.', '.ocwwco.', 'oCcwwcCo', 'sCccccCs', '.occcco.', '.opoopo.', '.oo..oo.'],
    // bearded man in a jacket, open collar
    ['..oooo..', '.ohhhho.', 'ohhhhhho', 'ohssssho', 'osossoso', '.ohhhho.', '.ocwwco.', 'oCcwwcCo', 'sCccccCs', '.occcco.', '.opoopo.', '.oo..oo.']
  ],
  /* Robots. Same grid and the same hand and foot cells, so poses still
   * apply. Letters: h head plate, e eye LED, a antenna tip, c/C shell and
   * its shade, t chest light, s claw, p legs or hover skirt. */
  future: [
    // round bot on legs
    ['....a...', '....o...', '.oooooo.', 'ohhhhhho', 'ohehheho', 'ohhhhhho', '.oooooo.', 'oCcttcCo', 'sCccccCs', '.oCccCo.', '.opoopo.', '.oo..oo.'],
    // twin-antenna bot with a visor, hovering on a skirt
    ['.a....a.', '.o....o.', '.oooooo.', 'ohhhhhho', 'oheeeeho', 'ohhhhhho', '.oooooo.', 'oCcttcCo', 'sCccccCs', 'oCccccCo', '.oppppo.', '..oooo..'],
    // tall can bot
    ['...aa...', '...oo...', '..oooo..', '.ohhhho.', '.oehheo.', '.ohhhho.', '.oooooo.', 'oCcttcCo', 'sCcttcCs', '.occcco.', '.opoopo.', '.oo..oo.'],
    // wide one-eyed bot
    ['...a....', '...o....', 'oooooooo', 'ohhhhhho', 'ohheehho', 'ohhhhhho', 'oooooooo', '.oCttCo.', 'sCccccCs', '.occcco.', '.opoopo.', '.oo..oo.']
  ]
};
const SKIN = [['#f8d0a8', '#d8a880'], ['#e8b088', '#c08860'], ['#c88858', '#a06838'], ['#94603c', '#704428'], ['#603c28', '#482818']];
const HAIR = ['#302018', '#704830', '#c89040', '#a03820', '#181820', '#d0c8c0', '#583828'];
const DRESS = {
  medieval: { cloth: [['#a87848', '#785030'], ['#4878a0', '#305878'], ['#7a8a3a', '#56622a'], ['#8a4a6a', '#62324a'], ['#a85838', '#783a24']], accent: ['#e8dcc0', '#583820', '#f8f8f8'], legs: ['#585060', '#685038'],
    mayor: { c: '#804098', C: '#582868', t: '#f8d040', p: '#302838' } },
  modern: { cloth: [['#283048', '#181c30'], ['#3a3a44', '#262630'], ['#4a3a30', '#33281f'], ['#5a6070', '#40465a'], ['#2a4a70', '#1c3450']], accent: ['#c03838', '#3878c8', '#f8d040', '#48a878'], legs: ['#262630', '#383850'],
    mayor: { c: '#1c2030', C: '#101420', t: '#f8d040', p: '#181c28' } },
  future: { cloth: [['#40c8e0', '#2890a8'], ['#f070c8', '#b04898'], ['#f8c040', '#c08820'], ['#78e070', '#48a848'], ['#a890f8', '#7058c8'], ['#f08858', '#b85830']],
    accent: ['#40e0f0', '#f048c0', '#f8d040'], legs: ['#585c80', '#40446a'],
    heads: ['#d8dcf0', '#b8c0dc', '#e8e0f8'],
    mayor: { c: '#7a4ae0', C: '#4a2ca0', t: '#f8d040', p: '#40446a' } }
};
/* A robot's eyes and antenna say its status: cyan at work, red and
 * blinking when it needs you, dim when stalled or asleep, dark when unknown. */
const ROBOT_EYE = { working: '#b8f8ff', idle: '#40e0f0', asleep: '#24606c', blocked: '#e02828', done: '#3878e0', stalled: '#585870', unknown: '#383850', stopped: '#383850' };
function strHash(s) {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619);
  return h >>> 0;
}
/* Poses are edits of the standing body: a raised hand, both hands up under
 * a crate, a foot lifted mid-step, sitting down. */
const setAt = (row, i, ch) => row.slice(0, i) + ch + row.slice(i + 1);
function pose(rows, kind) {
  const r = rows.slice();
  // Waving: the right arm is raised (drawn in person()), so its hand leaves the side.
  if (kind === 'wave' || kind === 'wave2' || kind === 'work' || kind === 'work2') { r[7] = setAt(r[7], 7, '.'); r[8] = setAt(r[8], 7, '.'); }
  if (kind === 'carry' || kind === 'carrystep') {
    r[5] = setAt(setAt(r[5], 0, 's'), 7, 's');
    r[8] = setAt(setAt(r[8], 0, '.'), 7, '.');
  }
  if (kind === 'carrystep') r[11] = '.oo...o.';
  // Sitting on the doorstep (#411): two rows lower, the legs folded away, the feet out front.
  if (kind === 'sit') return ['........', '........', ...r.slice(0, 9), r[11]];
  return r;
}
/* Draw a person. `seed` names who they are; `accent` (a specialist's color)
 * overrides their tie, apron or belt; `mayor` dresses them for the hall. */
export function person(c, x, y, seed, kind = 'stand', opts = {}) {
  const style = state.T.people, h = strHash(seed + '|' + style);
  if (state.T.robots) return robot(c, x, y, seed, h, kind, opts);
  const bodies = BODY[style], dress = DRESS[style];
  const rows = pose(bodies[h % bodies.length], kind);
  const [skin, skinS] = SKIN[(h >>> 3) % SKIN.length];
  const cloth = opts.mayor ? [dress.mayor.c, dress.mayor.C] : dress.cloth[(h >>> 7) % dress.cloth.length];
  const map = {
    o: P.o, s: skin, S: skinS, w: P.white,
    h: HAIR[(h >>> 11) % HAIR.length],
    c: cloth[0], C: cloth[1],
    t: opts.accent || (opts.mayor ? dress.mayor.t : dress.accent[(h >>> 15) % dress.accent.length]),
    p: opts.mayor ? dress.mayor.p : dress.legs[(h >>> 19) % dress.legs.length]
  };
  x = Math.round(x); y = Math.round(y);
  shadow(c, x + 1, y + 11, 7, 1);
  // The manager's sash is medieval dress; downtown its gold tie says it already.
  body2x(c, x, y, rows, map, { mayor: opts.mayor && style === 'medieval', sash: dress.mayor.t, sub: opts.sub, accent: opts.accent });
  if (kind === 'work' || kind === 'work2') {
    // Busy: a hammer swinging between raised and struck, so a working
    // agent reads as working even with no action sign.
    R(c, x + 7, y + 5, 2, 2, P.o); R(c, x + 7, y + 5, 1, 1, map.s);
    if (kind === 'work') { R(c, x + 8, y + 1, 1, 5, P.woodD); R(c, x + 7, y, 3, 2, P.o); R(c, x + 7, y, 3, 1, P.stoneD); }
    else { R(c, x + 8, y + 6, 4, 1, P.woodD); R(c, x + 11, y + 4, 2, 4, P.o); R(c, x + 11, y + 5, 1, 2, P.stoneD); R(c, x + 13, y + 7, 1, 1, P.gold); }
  }
  if (kind === 'wave' || kind === 'wave2') {
    // An arm raised above the head, swinging between upright and tilted out,
    // with motion ticks on the outswing: readable at a glance.
    R(c, x + 7, y + 6, 2, 2, P.o); R(c, x + 7, y + 6, 1, 1, map.C);
    if (kind === 'wave') {
      R(c, x + 8, y + 1, 2, 6, P.o); R(c, x + 8, y + 2, 1, 5, map.c);
      R(c, x + 7, y - 2, 4, 4, P.o); R(c, x + 8, y - 1, 2, 2, map.s);
    } else {
      R(c, x + 9, y + 4, 2, 3, P.o); R(c, x + 10, y + 2, 2, 3, P.o); R(c, x + 9, y + 5, 1, 1, map.c); R(c, x + 10, y + 3, 1, 2, map.c);
      R(c, x + 10, y - 1, 4, 4, P.o); R(c, x + 11, y, 2, 2, map.s);
      R(c, x + 14, y - 1, 1, 1, P.white); R(c, x + 15, y + 1, 1, 1, P.white); R(c, x + 14, y + 3, 1, 1, P.white);
    }
  }
}

/* A sub-agent in Future is a mini drone in its specialty's colour. */
function drone(c, x, y, col, t) {
  const b = !state.reduce && blink(t, 300) ? 1 : 0;
  x = Math.round(x) + 1; y = Math.round(y) + 3 + b;
  shadow(c, x + 1, y + 8 - b, 5, 1);
  R(c, x, y + 1, 7, 4, P.o); R(c, x + 1, y + 2, 5, 2, col); R(c, x + 2, y + 2, 1, 1, P.white); R(c, x + 4, y + 2, 1, 1, P.white);
  R(c, x - 1, y, 3, 1, P.o); R(c, x + 5, y, 3, 1, P.o);
  R(c, x + 3, y + 5, 1, 1, '#40e0f0');
}
/* Draw a robot: the person() of the Future theme. `opts.status` picks its
 * look; `opts.sub` makes it a drone; `mayor` gives it a crown and a cape. */
function robot(c, x, y, seed, h, kind, opts) {
  const t = now(), st = kind === 'work' || kind === 'work2' ? 'working' : opts.status || 'idle';
  if (opts.sub && kind !== 'wave' && kind !== 'wave2') return drone(c, x, y, opts.accent || '#f8f8f8', t + (h % 997));
  const dress = DRESS.future, bodies = BODY.future;
  const slump = st === 'stalled';
  let rows = pose(bodies[h % bodies.length], kind);
  if (slump) rows = ['......a.', '.....o..', ...rows.slice(2)];
  const shell = opts.mayor ? [dress.mayor.c, dress.mayor.C] : dress.cloth[(h >>> 7) % dress.cloth.length];
  const eye = opts.sub && st !== 'blocked' ? '#40e0f0' : ROBOT_EYE[st] || ROBOT_EYE.idle;
  const tip = st === 'blocked' ? (blink(t, 300) ? '#e02828' : '#802828') : st === 'working' && blink(t + (h % 500), 600) ? '#b8f8ff' : slump || st === 'asleep' ? '#585870' : '#f048c0';
  const map = {
    o: P.o, w: P.white, s: '#9098b8', S: '#686e90',
    h: dress.heads[(h >>> 11) % dress.heads.length], e: eye, a: tip,
    c: shell[0], C: shell[1],
    t: opts.accent || (opts.mayor ? dress.mayor.t : dress.accent[(h >>> 15) % dress.accent.length]),
    p: opts.mayor ? dress.mayor.p : dress.legs[(h >>> 19) % dress.legs.length]
  };
  x = Math.round(x); y = Math.round(y);
  // Idle robots hover gently; reduced motion keeps them on the ground.
  const bob = st === 'idle' && kind === 'stand' && !state.reduce && blink(t + (h % 900), 700) ? -1 : 0;
  shadow(c, x + 1, y + 11, 7, 1);
  const yy = y + bob + (slump ? 1 : 0);
  if (opts.mayor) { R(c, x - 1, yy + 7, 1, 4, map.C); R(c, x + 8, yy + 7, 1, 4, map.C); }
  body2x(c, x, yy, rows, map, { robot: true });
  if (opts.mayor) { R(c, x + 2, yy - 2, 5, 2, P.o); R(c, x + 2, yy - 1, 5, 1, P.gold); R(c, x + 2, yy - 2, 1, 1, P.gold); R(c, x + 4, yy - 3, 1, 1, P.gold); R(c, x + 6, yy - 2, 1, 1, P.gold); }
  if (kind === 'wave' || kind === 'wave2') {
    R(c, x + 7, yy + 6, 2, 2, P.o); R(c, x + 7, yy + 6, 1, 1, map.C);
    if (kind === 'wave') { R(c, x + 8, yy + 1, 2, 6, P.o); R(c, x + 8, yy + 2, 1, 5, map.s); R(c, x + 7, yy - 2, 4, 4, P.o); R(c, x + 8, yy - 1, 2, 2, map.s); }
    else {
      R(c, x + 9, yy + 4, 2, 3, P.o); R(c, x + 10, yy + 2, 2, 3, P.o); R(c, x + 9, yy + 5, 1, 1, map.s); R(c, x + 10, yy + 3, 1, 2, map.s);
      R(c, x + 10, yy - 1, 4, 4, P.o); R(c, x + 11, yy, 2, 2, map.s);
    }
    // A red "!" light beside a robot that needs you.
    if (st === 'blocked') { R(c, x - 3, yy, 2, 4, P.o); R(c, x - 3, yy, 1, 3, '#e02828'); R(c, x - 3, yy + 5, 1, 1, '#e02828'); }
  }
  if (st === 'working') {
    // Arms at work with a tiny wrench, sparks at the tip (none with reduced motion).
    const up = !state.reduce && blink(t + (h % 400), 260);
    R(c, x + 8, yy + (up ? 6 : 7), 3, 1, P.o); R(c, x + 10, yy + (up ? 5 : 6), 1, 3, '#9098b8');
    if (!state.reduce && blink(t + (h % 300), 120)) { R(c, x + 11, yy + 4, 1, 1, P.white); R(c, x + 12, yy + 6, 1, 1, P.gold); R(c, x + 12, yy + 3, 1, 1, '#40e0f0'); }
  }
  if (st === 'done') { R(c, x + 3, yy + 7, 1, 1, '#3878e0'); R(c, x + 4, yy + 7, 1, 1, '#3878e0'); }
  // Porters carry their crate on a hover-tray.
  if (kind === 'carry' || kind === 'carrystep') { R(c, x - 1, yy - 1, 10, 2, P.o); R(c, x, yy - 1, 8, 1, F.hull); if (!state.reduce && blink(t, 160)) R(c, x + 1, yy + 1, 6, 1, F.cyan); }
}

/* ---------- Actions ----------
 * What an agent is doing right now, acted out with a prop: a thought cloud,
 * a book, a swinging hammer, a terminal, a magnifying glass, a letter.
 * Searching and messaging also walk a few steps back and forth. */
export const wander = (action, t, seed) =>
  action === 'searching' || action === 'messaging' ? Math.round(Math.sin(t / 520 + (strHash(seed) % 7)) * 3) : 0;
export function drawAction(c, x, y, action, t) {
  const up = blink(t, 240);
  switch (action) {
    case 'thinking':
      [[8, -2, 1], [10, -4, 1], [12, -7, 2]].forEach(([dx, dy, sz], k) => {
        if (blink(t + k * 160, 480) || state.reduce) { R(c, x + dx - 1, y + dy - 1, sz + 2, sz + 2, P.o); R(c, x + dx, y + dy, sz, sz, P.white); }
      });
      break;
    case 'reading':
      R(c, x + 1, y + 7, 6, 3, P.o); R(c, x + 2, y + 7, 2, 2, P.white); R(c, x + 4, y + 7, 2, 2, P.parch);
      break;
    case 'writing':
      if (up) { R(c, x + 8, y + 4, 1, 4, P.woodD); R(c, x + 7, y + 3, 3, 2, P.stoneD); }
      else { R(c, x + 8, y + 7, 3, 1, P.woodD); R(c, x + 10, y + 6, 2, 3, P.stoneD); R(c, x + 12, y + 10, 1, 1, P.gold); }
      break;
    case 'running':
      R(c, x + 8, y + 8, 5, 4, P.o); R(c, x + 9, y + 9, 3, 2, up ? '#58e878' : P.dark);
      break;
    case 'searching':
      R(c, x + 7, y + 1, 4, 4, P.o); R(c, x + 8, y + 2, 2, 2, P.glass); R(c, x + 10, y + 5, 1, 1, P.o); R(c, x + 11, y + 6, 1, 1, P.o);
      break;
    case 'messaging':
      R(c, x + 6, y + 7, 4, 3, P.o); R(c, x + 7, y + 8, 2, 1, P.white); R(c, x + 8, y + 8, 1, 1, P.p0);
      break;
  }
}
/* Actions with nothing to hammer: no sparks fly off the crate. */
export const QUIET_ACTIONS = new Set(['thinking', 'reading', 'searching', 'messaging']);
const ACTION_ICONS = {
  thinking: ['.###.', '#####', '.###.', '.....', '#....'],
  reading: ['##.##', '#.#.#', '#.#.#', '##.##', '.....'],
  writing: ['###..', '###..', '.#...', '.#...', '.#...'],
  running: ['#.#.#', '.###.', '##.##', '.###.', '#.#.#'],
  searching: ['.##..', '#..#.', '.##..', '...#.', '....#'],
  messaging: ['#####', '##.##', '#.#.#', '#...#', '#####']
};
/* A small sign over a busy house naming what its own agent is doing. */
export function actionSign(c, x, y, action) {
  const icon = ACTION_ICONS[action]; if (!icon) return;
  const [frame, face, ink] = state.T.sign;
  R(c, x, y, 7, 7, frame); R(c, x + 1, y + 1, 5, 5, face);
  for (let r = 0; r < 5; r++) for (let k = 0; k < 5; k++) if (icon[r][k] === '#') R(c, x + 1 + k, y + 1 + r, 1, 1, ink);
}
