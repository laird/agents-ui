/* ---------- Weather art at 2× (#458) ----------
 * No DOM, so node tests it (tools/village-tests/weatherart.test.mjs).
 *
 * What weather.js draws over the village, in detail pixels (detail.js):
 * rain, cloud banks, the lightning bolt, the roofs' alarm lamps, the BUILD
 * BROKEN sign and the blamed commit's crate. Every function still takes
 * scene pixels, and covers the same scene pixels as it always did. */
import { state } from './state.js';
import { P, R, blink } from './palette.js';
import { detail } from './detail.js';
import { disc, px } from './art2x.js';

/* Precipitation over a box (scene pixels): deterministic streaks falling
 * with `t`, each `len` scene pixels long, a lighter drop at its foot. */
export function rain(c, x0, y0, w, h, n, col, t, len = 3) {
  detail(c, 0, 0, d => {
    for (let i = 0; i < n; i++) {
      const x = x0 + ((i * 37 + (i >> 3) * 11) % w);
      const y = y0 + Math.floor((i * 53 + t / (9 + (i % 5))) % h);
      // Alternate streaks sit half a scene pixel over, so the rain reads finer than the grid.
      const dx = 2 * x + (i % 2);
      px(d, dx, 2 * y, 1, 2 * len - 1, col);
      px(d, dx, 2 * y + 2 * len - 1, 1, 1, 'rgba(255,255,255,.45)');
    }
  });
}

/* `hex` a quarter of the way to white: a cloud's sunlit side. */
export function lighter(hex) {
  const n = parseInt(hex.slice(1), 16), up = v => Math.round(v + (255 - v) / 4);
  return '#' + [n >> 16, (n >> 8) & 255, n & 255].map(v => up(v).toString(16).padStart(2, '0')).join('');
}
/* One cloud puff, its bank 14 by 6 scene pixels at (x, y) and its crown
 * above it: a flat dark base under two shaded billows. */
export function puff(c, x, y, wx, high = 0) {
  detail(c, x, y, d => {
    const cols = { outline: wx.cloudD, fill: wx.cloud, light: lighter(wx.cloud), dark: wx.cloudD };
    px(d, 1, 2 * high + 2, 26, 9, wx.cloudD); px(d, 0, 2 * high + 4, 28, 6, wx.cloudD);
    disc(d, 4, 2 * high - 3, 12, cols);
    disc(d, 12, -4, 12, cols);
    px(d, 3, 2 * high + 9, 22, 1, 'rgba(0,0,0,.18)');
  });
}

/* A row of cloud puffs from x0 over w, at y (scene pixels). */
export function clouds(c, x0, y, w, wx, t) {
  const drift = state.reduce ? 0 : Math.floor(t / 400) % 6;
  for (let x = x0 - 6 + drift; x < x0 + w; x += 11) puff(c, x, y, wx, Math.floor(x / 11) % 3 === 1 ? 1 : 0);
}

/* A lightning bolt from scene (x, y) down, 14 scene pixels: a gold zigzag
 * with a white-hot core. */
export function bolt(c, x, y) {
  detail(c, x, y, d => {
    [[0, 0, 2, 8], [-2, 8, 4, 2], [-4, 10, 2, 8], [-6, 18, 4, 2], [-6, 20, 2, 8]].forEach(([dx, dy, w, h]) => {
      px(d, dx, dy, w, h, P.gold);
      px(d, dx + (w > 2 ? 1 : 0), dy + (h > 2 ? 1 : 0), w > 2 ? w - 2 : 1, h > 2 ? h - 2 : 1, P.white);
    });
  });
}

/* An alarm lamp, 3 by 2 scene pixels at (x, y): a dark housing, the lens
 * lit `col` or dimmed, a glint on it when lit. */
export function alarmLamp(c, x, y, on, col) {
  detail(c, x, y, d => {
    px(d, 0, 0, 6, 4, P.o); px(d, 1, 3, 4, 1, P.dark);
    px(d, 2, 0, 2, 3, on ? col : P.redD);
    if (on) px(d, 2, 0, 1, 1, P.white);
  });
}

/* Blinking red lights: one on each house's roof, and at the hall. */
export function alarms(c, lay, wx, t) {
  // Neighbours alternate; with motion reduced every lamp stays lit.
  const lit = k => state.reduce || blink(t, 420) === (k % 2 === 0);
  (lay.lots || []).forEach((l, k) => alarmLamp(c, l.x + 7, l.y - 13, lit(k), wx.alarm));
  alarmLamp(c, lay.hallX + 15, 5, lit(0), wx.alarm);
}

/* The BUILD BROKEN sign at scene (x, y): a red board 26 by 9 on two posts
 * standing 13 tall, a row of white letters across it, bolted at its corners. */
export const SIGN_W = 26, SIGN_H = 13;
export function brokenSign(c, x, y) {
  detail(c, x, y, d => {
    [6, 42].forEach(px0 => { px(d, px0, 16, 4, 10, P.woodD); px(d, px0 + 1, 16, 1, 10, P.wood); });
    px(d, 0, 0, 52, 18, P.o); px(d, 2, 2, 48, 14, '#d02828');
    px(d, 2, 2, 48, 2, '#f05050'); px(d, 2, 14, 48, 2, '#a01818');
    [[3, 3], [47, 3], [3, 13], [47, 13]].forEach(([bx, by]) => px(d, bx, by, 2, 2, '#f0a0a0'));
    // The words, a word's worth of letters at a time: tall strokes, a gap between words.
    for (let k = 0; k < 5; k++) for (let j = 0; j < 3; j++) px(d, 6 + k * 8 + j * 2, 7, 1, 4, P.white);
  });
}

/* The commit that turned the build red: a crate 7 by 7 at scene (x, y)
 * under a red flag on a pole reaching 7 above it. */
export function blameCrate(c, x, y) {
  detail(c, x, y, d => {
    px(d, 0, 0, 14, 14, P.o); px(d, 2, 2, 10, 10, P.wood);
    [5, 9].forEach(sx => px(d, sx, 2, 1, 10, P.woodD)); px(d, 2, 6, 10, 2, P.woodD); px(d, 2, 2, 10, 1, P.woodL);
    px(d, 10, -14, 2, 16, P.woodD); px(d, 10, -14, 1, 16, P.woodL);
    px(d, 2, -14, 8, 6, '#e02828'); px(d, 2, -14, 8, 1, '#f06050'); px(d, 2, -9, 8, 1, '#a01818');
    px(d, 2, -10, 2, 1, 'rgba(0,0,0,.25)');
  });
}
