/* ---------- The Modern theme's offices, at 2× (#464) ----------
 * One office block per kind of house, drawn on the detail grid (detail.js)
 * with the helpers in art2x.js: half a scene pixel per detail pixel, so a
 * 16×16 lot is 32×32 here. Each keeps the footprint, the status language
 * (lit windows for working, an open door for blocked, dark glass for
 * unknown) and the animation it had at 1×, and gains what a pixel was too
 * coarse for: window mullions, sills and blinds, brick bond, a lattice
 * crane, a ribbed observatory dome, fan blades, a corrugated warehouse.
 *
 * The colours live here too (MC; themes/modern.js exports them as M), so
 * this module needs nothing from modern.js. */
import { P, blink, hash } from '../palette.js';
import { puffs } from '../sprites/houses.js';
import { detail } from '../detail.js';
import { bricks, dither, px } from '../art2x.js';

export const MC = {
  lawn: '#68b050', lawnD: '#58a040', asphalt: '#505868', asphaltD: '#383c48', dash: '#f0d850',
  concrete: '#c0c0c8', concreteD: '#9898a8', concreteL: '#e0e0e8',
  glass: '#5890c8', glassD: '#386898', glassL: '#a8d0f0', brick: '#a85840', brickD: '#783828',
  steel: '#8890a0', steelD: '#586070', crane: '#f0c020', box: '#c89858', boxD: '#987040',
  tarp: '#48a068', tarpD: '#307848', awnA: '#f8f8f8', awnB: '#2890a0', led: '#58e878'
};
/* See-through shade and light, laid over whatever is under them. */
export const MSHADE = 'rgba(32,32,48,0.28)', MLIT = 'rgba(255,255,255,0.32)';
const CRANE_D = '#b88810', BLIND = '#e8c060';

export const mWin = st => st === 'working' ? P.lamp : (st === 'unknown' || st === 'stopped') ? P.dark : MC.glassL;

/* A window of w×h detail pixels at (x, y), in its status colour: a lamp-lit
 * office has its blind half down, clear glass a glint; a wide one has a
 * mullion. A sill under it. */
export function mWindow(g, x, y, w, h, st) {
  const col = mWin(st);
  px(g, x, y, w, h, col);
  if (st === 'working') px(g, x, y, w, 1, BLIND);
  else if (col === MC.glassL) px(g, x + 1, y + 1, 1, Math.max(1, h - 2), P.white);
  if (w >= 6) px(g, x + (w >> 1), y, 1, h, MC.glassD);
  px(g, x - 1, y + h, w + 2, 1, MC.concreteL);
}

/* Ink a small bitmap: each character is a key of `cols` ('.' and unknown
 * characters are left clear). */
export function mInk(g, x, y, rows, cols) {
  rows.forEach((row, j) => { for (let i = 0; i < row.length; i++) if (cols[row[i]]) px(g, x + i, y + j, 1, 1, cols[row[i]]); });
}

/* The flat-roofed block every office starts from, filling the 32×32 lot
 * from row 2·top: a parapet with a lit coping over a gravel roof, the
 * facade (o.face draws on it), a row of windows and the glass door, which
 * stands open and dark when the office is blocked. */
function mBlock(g, st, o) {
  const T = 2 * (o.top ?? 1);
  px(g, 0, T, 32, 32 - T, P.o);
  px(g, 2, T + 2, 28, 6, o.roof || MC.concreteD);
  px(g, 2, T + 2, 28, 2, o.roofL || MC.concrete);
  dither(g, 2, T + 4, 28, 4, 3, (i, j) => (hash(i + 3 * j, T) < 0.5 ? MSHADE : null));
  px(g, 2, T + 8, 28, 22 - T, o.wall);
  px(g, 2, T + 8, 28, 1, MSHADE);
  px(g, 29, T + 9, 1, 21 - T, MSHADE);
  px(g, 2, 29, 28, 1, MSHADE);
  if (o.face) o.face(g);
  if (o.rows !== 0) for (let wy = 2 * (o.top ?? 1) + 12; wy < 20; wy += 6) [4, 12, 20].forEach(wx => mWindow(g, wx, wy, 6, 3, st));
  if (o.door !== false) {
    px(g, 11, 21, 10, 1, MC.steelD);
    px(g, 12, 22, 8, 10, P.o);
    if (st === 'blocked') px(g, 18, 23, 1, 8, MC.glassD);
    else {
      px(g, 13, 23, 6, 8, MC.glassD); px(g, 14, 24, 1, 5, MC.glass); px(g, 16, 23, 1, 8, P.o);
      px(g, 15, 27, 1, 2, MC.steel); px(g, 17, 27, 1, 2, MC.steel);
    }
  }
}

const FLASK = ['.oooo.', '.oggo.', '.oggo.', 'occcco', 'occcco', 'oooooo'];
const BUG_ICON = ['.o....o.', '..oooo..', 'oorrorro', '.rrrorr.', 'oorrorro', '..rrrr..'];
const BADGE = ['.gggggg.', 'gssssssg', 'gsswwssg', 'gsswwssg', 'gssssssg', '.gssssg.', '..gssg..', '...gg...'];

export const MB = {
  cottage(c, x, y, st, t) {
    detail(c, x, y, g => {
      mBlock(g, st, { wall: MC.concreteL, rows: 0, top: 3 });
      // The café: its sign, and a striped awning scalloped along its edge.
      px(g, 10, 14, 12, 3, MC.awnB);
      for (let i = 0; i < 5; i++) px(g, 11 + 2 * i, 15, 1, 1, P.white);
      px(g, 2, 18, 28, 6, P.o);
      for (let k = 0; k < 7; k++) { const col = k % 2 ? MC.awnA : MC.awnB; px(g, 4 + k * 4, 19, 4, 3, col); px(g, 5 + k * 4, 22, 2, 1, col); }
      px(g, 4, 19, 28, 1, MLIT);
      mWindow(g, 4, 26, 6, 3, st); mWindow(g, 22, 26, 6, 3, st);
    });
    if (st === 'working') puffs(c, x + 12, y - 1, t, P.white, MC.concreteL);
  },
  workshop(c, x, y, st, t) {
    detail(c, x, y, g => mBlock(g, st, { wall: MC.brick, rows: 0, face: f => {
      bricks(f, 2, 10, 28, 20, 6, 3, MC.brickD);
      // Tall windows under stone lintels, a transom across each.
      [4, 20].forEach(wx => { px(f, wx - 1, 11, 10, 1, MC.concreteL); mWindow(f, wx, 12, 8, 4, st); mWindow(f, wx, 18, 8, 2, st); });
    } }));
  },
  feature(c, x, y, st, t) {
    detail(c, x, y, g => {
      mBlock(g, st, { wall: MC.concrete, top: 4, rows: 0 });
      mWindow(g, 4, 20, 6, 3, st);
      // A new wing going up in steel: beams with a lit top, braced bays.
      for (let yy = 10; yy < 32; yy += 8) {
        px(g, 18, yy, 14, 2, MC.steel); px(g, 18, yy, 14, 1, MC.concreteL);
        if (yy + 8 < 32) for (let k = 0; k < 6; k++) px(g, 20 + Math.round(k * 9 / 5), yy + 2 + k, 1, 1, MC.steelD);
      }
      px(g, 18, 10, 2, 22, MC.steelD); px(g, 30, 10, 2, 22, MC.steelD);
      // The tower crane: a lattice mast and jib, the cab, a trolley and hook.
      px(g, 26, -8, 2, 40, P.o); px(g, 28, -8, 2, 40, MC.crane);
      for (let yy = -4; yy < 32; yy += 4) { px(g, 28, yy, 1, 1, P.o); px(g, 29, yy + 2, 1, 1, P.o); }
      px(g, 6, -8, 26, 2, MC.crane); px(g, 6, -8, 26, 1, '#f8e070'); px(g, 6, -6, 26, 2, P.o);
      for (let xx = 8; xx < 26; xx += 4) px(g, xx, -7, 2, 1, CRANE_D);
      px(g, 23, -4, 4, 3, P.o); px(g, 24, -3, 2, 1, MC.glassL);
      const hook = st === 'working' ? 5 + Math.floor((Math.sin(t / 400) + 1) * 2) : 6;
      const hx = 10 + 2 * (st === 'working' ? Math.floor(t / 700) % 3 : 0);
      px(g, hx - 1, -5, 3, 1, MC.steelD);
      px(g, hx, -4, 1, 2 * hook, P.o);
      px(g, hx - 1, 2 * hook - 6, 3, 2, MC.steelD);
    });
  },
  bug(c, x, y, st, t) {
    detail(c, x, y, g => {
      mBlock(g, st, { wall: MC.concreteL, rows: 0 });
      // A wide screen with scan lines; a sweep runs across it while it is worked on.
      px(g, 4, 12, 24, 8, P.o); px(g, 6, 14, 20, 4, mWin(st));
      px(g, 6, 15, 20, 1, MSHADE); px(g, 6, 17, 20, 1, MSHADE);
      if (st === 'working') px(g, 6 + 2 * (Math.floor(t / 160) % 9), 14, 2, 4, P.o);
      // The billboard on the roof, with the bug on it.
      px(g, 12, 4, 2, 4, P.o); px(g, 18, 4, 2, 4, P.o);
      px(g, 8, -6, 16, 10, P.o); px(g, 10, -4, 12, 6, P.white);
      mInk(g, 12, -4, BUG_ICON, { o: P.o, r: P.bug });
    });
  },
  security(c, x, y, st, t) {
    detail(c, x, y, g => {
      mBlock(g, st, { wall: MC.concreteD, rows: 0, top: 2, roof: MC.steelD, roofL: MC.steel });
      [6, 22].forEach(wx => mWindow(g, wx, 16, 4, 2, st));
      mInk(g, 12, 12, BADGE, { g: P.gold, s: P.shield, w: P.white });
      // The radar on its mast, sweeping while it works; its arm ends where it did at 1×.
      px(g, 6, -4, 2, 10, P.o);
      const a = st === 'working' ? t / 300 : 0.6;
      const tx = 2 * (3 + Math.round(Math.cos(a) * 4)), ty = 2 * (-2 + Math.round(Math.sin(a) * 2));
      for (let k = 0; k <= 8; k++) px(g, Math.round(6 + (tx - 6) * k / 8), Math.round(-4 + (ty + 4) * k / 8), 1, 1, MC.steelD);
      px(g, tx, ty, 2, 2, MC.steelD);
      // The camera, its lens red when it blinks.
      px(g, 24, -2, 6, 4, P.o); px(g, 25, -1, 4, 2, MC.steelD);
      px(g, 26, -1, 2, 2, st === 'working' && blink(t, 400) ? P.p0 : MC.steel);
    });
  },
  tests(c, x, y, st, t) {
    detail(c, x, y, g => {
      mBlock(g, st, { wall: MC.glass, rows: 0 });
      // A glass lab: lit bands of benches, bubbles rising while it runs.
      for (let wy = 10; wy < 22; wy += 4) { px(g, 2, wy, 28, 2, st === 'working' ? P.fizzB : MC.glassL); px(g, 2, wy + 1, 28, 1, MSHADE); }
      px(g, 4, 12, 2, 8, MC.glassD); px(g, 26, 12, 2, 8, MC.glassD);
      if (st === 'working') for (let k = 0; k < 5; k++) px(g, 7 + k * 4, 20 - (Math.floor(t / 120) + k * 3) % 9, 1, 1, P.white);
      [6, 20].forEach(fx => mInk(g, fx, -4, FLASK, { o: P.o, g: MC.glassL, c: P.copper }));
    });
    if (st === 'working') { puffs(c, x + 4, y - 3, t, P.fizzA, P.fizzB); puffs(c, x + 11, y - 3, t + 500, P.fizzB, P.fizzA); }
  },
  research(c, x, y, st, t) {
    detail(c, x, y, g => {
      mBlock(g, st, { wall: P.white, top: 2 });
      // The observatory: a ribbed dome lit from the left, its telescope
      // swinging while it works.
      const a = -2.2 + (st === 'working' ? Math.sin(t / 900) * 0.5 : 0);
      for (let j = -4; j < 6; j++) for (let i = 8; i < 26; i++) {
        const dx = (i + 0.5) / 2 - 8.5, dy = (j + 0.5) / 2 - 2.5, r2 = dx * dx + dy * dy;
        if (r2 > 18.5 || dy > 0.25) continue;
        px(g, i, j, 1, 1, r2 > 14 ? P.o : dx + dy < -3.2 ? P.white : i % 4 === 0 ? MC.concrete : MC.concreteL);
      }
      const gx = 2 * (8 + Math.round(Math.cos(a) * 4)), gy = 2 * (1 + Math.round(Math.sin(a) * 4));
      for (let k = 0; k < 6; k++) px(g, Math.round(16 + (gx - 16) * k / 6), Math.round(4 + (gy - 4) * k / 6), 1, 1, MC.steelD);
      px(g, gx, gy, 2, 2, P.gold); px(g, gx, gy, 1, 1, P.white);
      px(g, 16, 4, 2, 4, P.o);
    });
  },
  docs(c, x, y, st, t) {
    detail(c, x, y, g => {
      mBlock(g, st, { wall: P.parch, rows: 0 });
      [4, 20].forEach(wx => { mWindow(g, wx, 12, 8, 8, st); px(g, wx, 15, 8, 1, MC.glassD); });
      // Books on a shelf on the roof, each spine banded, one leaning shorter.
      ['#c04838', '#3870c0', '#58a048', '#f8d040'].forEach((col, k) => {
        const bx = 8 + k * 4, top = [-4, -3, -2, -4][k];
        px(g, bx, top, 4, 8 - (top + 4), col);
        px(g, bx, top, 1, 4 - top, MLIT); px(g, bx + 3, top, 1, 4 - top, MSHADE);
        px(g, bx + 1, top + 2, 2, 1, k === 3 ? P.o : P.gold);
      });
      px(g, 6, 4, 20, 2, P.o); px(g, 8, 6, 1, 2, P.o); px(g, 23, 6, 1, 2, P.o);
    });
  },
  refactor(c, x, y, st, t) {
    detail(c, x, y, g => {
      mBlock(g, st, { wall: MC.brick, rows: 0, face: f => { bricks(f, 2, 10, 28, 20, 6, 3, MC.brickD); mWindow(f, 4, 14, 4, 4, st); mWindow(f, 10, 14, 4, 4, st); } });
      // Half the building under a tarp, creased and tied to the scaffold rails.
      px(g, 16, 2, 16, 30, P.o); px(g, 18, 4, 12, 26, MC.tarp);
      for (let yy = 8; yy < 30; yy += 8) px(g, 18, yy, 12, 1, MC.tarpD);
      dither(g, 18, 4, 12, 26, 3, (i, j) => (hash(i, j + 7) < 0.25 ? MC.tarpD : null));
      for (let yy = 4; yy < 32; yy += 10) { px(g, 14, yy, 18, 2, MC.steel); px(g, 14, yy, 18, 1, MC.concreteL); px(g, 24, yy + 2, 1, 2, P.woodL); }
      if (st === 'working' && blink(t, 140)) {
        const sy = 2 * (6 + Math.floor(t / 280) % 6);
        px(g, 22, sy - 1, 1, 3, P.white); px(g, 21, sy, 3, 1, P.white); px(g, 22, sy, 1, 1, P.gold);
      }
    });
  },
  perf(c, x, y, st, t) {
    detail(c, x, y, g => {
      mBlock(g, st, { wall: MC.steelD, rows: 0, top: 2, roof: MC.steel, roofL: MC.concrete });
      // Vents across the wall, and a rack's LEDs blinking while it runs.
      [12, 14].forEach(yy => px(g, 4, yy, 24, 1, MC.steel));
      px(g, 3, 17, 24, 4, P.o);
      for (let k = 0; k < 6; k++) px(g, 4 + 4 * k, 18, 2, 2, st === 'working' && hash(k, Math.floor(t / 200)) < 0.6 ? MC.led : P.dark);
      // Two cooling fans on the roof, four blades each, spinning while it runs.
      [4, 11].forEach((fx, k) => {
        const cx = 2 * fx;
        px(g, cx - 6, -2, 14, 12, P.o); px(g, cx - 4, 0, 10, 8, MC.steelD); px(g, cx - 4, 0, 10, 1, MC.steel);
        const a = st === 'working' ? t / 90 + k : 0.3;
        for (let d = -4; d <= 4; d++) {
          px(g, Math.round(cx + 0.5 + Math.cos(a) * d), Math.round(4.5 + Math.sin(a) * d * 0.7), 1, 1, MC.concreteL);
          px(g, Math.round(cx + 0.5 - Math.sin(a) * d), Math.round(4.5 + Math.cos(a) * d * 0.7), 1, 1, MC.steel);
        }
        px(g, cx, 4, 1, 1, P.o);
      });
    });
  },
  chores(c, x, y, st, t) {
    detail(c, x, y, g => {
      // A corrugated warehouse with a roller door and a crate out front.
      px(g, 0, 6, 32, 26, P.o); px(g, 2, 8, 28, 22, MC.steel);
      for (let xx = 4; xx < 30; xx += 4) { px(g, xx, 8, 1, 22, MC.steelD); px(g, xx + 2, 8, 1, 22, MC.concreteL); }
      px(g, 2, 8, 28, 4, MC.concreteD); px(g, 2, 8, 28, 1, MC.concrete); px(g, 2, 12, 28, 1, MSHADE);
      px(g, 8, 16, 16, 16, P.o);
      if (st === 'blocked') px(g, 10, 18, 12, 2, MC.concreteD);
      else {
        px(g, 10, 18, 12, 14, MC.concreteL);
        for (let yy = 20; yy < 32; yy += 2) px(g, 10, yy, 12, 1, MC.concreteD);
        px(g, 15, 29, 2, 1, P.o);
      }
      if (st === 'working') { px(g, 10, 18, 12, 2, P.lamp); px(g, 10, 20, 12, 1, 'rgba(248,224,128,0.45)'); }
      px(g, 24, 24, 8, 8, P.o); px(g, 26, 26, 4, 4, MC.box); px(g, 26, 26, 4, 1, '#e0b878'); px(g, 26, 28, 4, 1, MC.boxD);
    });
  }
};
