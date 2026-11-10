/* ---------- People at 2× (#419) ----------
 * No DOM, so node tests it (tools/village-tests/people2x.test.mjs).
 *
 * A person is still one of people.js's 8×12 templates, posed and coloured
 * per seed. lift() puts it on the 2× detail grid (#418), 16×24 detail
 * pixels in the same 8×12 scene pixels, and adds what only fits there:
 *   faces      narrow eyes, a nose, a mouth;
 *   hair       a highlight along the top;
 *   clothes    buttons down a jacket, a buckle on a belt, shoes with a toe cap;
 *   roles      the manager's gold sash, a sub-agent's armband and badge in its
 *              specialty's colour (a worker is dressed as before);
 *   robots     LED eyes with a glint, rivets on the shell, a lit chest light.
 * The result is a grid of colours (null: nothing), people.js paints it. */

/* A colour mixed `k` of the way to white. */
export function lighten(hex, k = 0.35) {
  const n = parseInt(String(hex).slice(1), 16);
  if (!/^#[0-9a-f]{6}$/i.test(hex) || Number.isNaN(n)) return hex;
  const ch = s => Math.round(((n >> s) & 255) + (255 - ((n >> s) & 255)) * k);
  return '#' + [16, 8, 0].map(s => ch(s).toString(16).padStart(2, '0')).join('');
}

const MOUTH = '#8a4a3a', SHOE = '#4a3a30', OUTLINE = '#202030';

/* Lift posed 8-wide `rows` (people.js's letters) onto the 2× grid, coloured
 * by `map`, and refine it. opts: { robot, mayor, sub, accent, sash }. */
export function lift(rows, map, opts = {}) {
  const H = rows.length * 2, W = 16;
  const g = Array.from({ length: H }, () => Array(W).fill(null));
  const at = (r, k) => (rows[r] && rows[r][k]) || '.';
  rows.forEach((row, r) => [...row].forEach((ch, k) => {
    if (ch === '.') return;
    for (let i = 0; i < 2; i++) for (let j = 0; j < 2; j++) g[2 * r + i][2 * k + j] = map[ch];
  }));
  const set = (y, x, col) => { if (g[y] && x >= 0 && x < W && col) g[y][x] = col; };
  const clear = (y, x) => { if (g[y] && x >= 0 && x < W) g[y][x] = null; };

  if (opts.robot) {
    rows.forEach((row, r) => [...row].forEach((ch, k) => {
      if (ch === 'e') set(2 * r, 2 * k + 1, '#ffffff');                 // a glint in each LED
      if (ch === 'a') { clear(2 * r, 2 * k); clear(2 * r + 1, 2 * k); } // a thinner antenna tip
      if (ch === 'C' && (r + k) % 2 === 0) set(2 * r, 2 * k + (k < 4 ? 1 : 0), lighten(map.C, 0.45)); // rivets
      if (ch === 't') set(2 * r, 2 * k, '#ffffff');                     // the chest light, lit
    }));
    return g;
  }

  // The face: an eye is an outline cell with skin on both sides.
  const skin = ch => ch === 's' || ch === 'S';
  const eyeRow = rows.findIndex((row, r) => [...row].some((ch, k) => ch === 'o' && skin(at(r, k - 1)) && skin(at(r, k + 1))));
  if (eyeRow >= 0) {
    [...rows[eyeRow]].forEach((ch, k) => {
      if (!(ch === 'o' && skin(at(eyeRow, k - 1)) && skin(at(eyeRow, k + 1)))) return;
      // Narrow eyes, set towards the nose.
      const inner = k < 4 ? 2 * k + 1 : 2 * k, outer = k < 4 ? 2 * k : 2 * k + 1;
      set(2 * eyeRow, outer, map.s); set(2 * eyeRow + 1, outer, map.s);
      set(2 * eyeRow, inner, OUTLINE); set(2 * eyeRow + 1, inner, OUTLINE);
    });
    if (skin(at(eyeRow, 3)) && skin(at(eyeRow, 4))) set(2 * eyeRow + 1, 8, map.S); // the nose
    const m = eyeRow + 1;
    if (skin(at(m, 3)) && skin(at(m, 4))) { set(2 * m + 1, 7, MOUTH); set(2 * m + 1, 8, MOUTH); } // the mouth
  }
  // Hair: a highlight on its top edge.
  rows.forEach((row, r) => [...row].forEach((ch, k) => {
    if (ch === 'h' && (eyeRow < 0 || r < eyeRow) && at(r - 1, k) !== 'h') set(2 * r, 2 * k + (k % 2), lighten(map.h, 0.3));
  }));
  // Clothes: a belt (three accent cells or more) gets a buckle; a jacket with
  // a shirt down its front, buttons; shoes, a toe cap.
  rows.forEach((row, r) => {
    if (r <= eyeRow) return;
    const t = [...row].filter(ch => ch === 't').length;
    if (t >= 3 && at(r, 3) === 't' && at(r, 4) === 't') { set(2 * r, 7, '#f8d040'); set(2 * r, 8, '#f8d040'); set(2 * r + 1, 7, '#f8d040'); set(2 * r + 1, 8, OUTLINE); }
    if (at(r, 3) === 'w' && at(r, 4) === 'w') set(2 * r + 1, 8, OUTLINE);
  });
  const last = rows.length - 1;
  [...rows[last]].forEach((ch, k) => { if (ch === 'o') set(2 * last, 2 * k, SHOE); });

  // Roles. The manager wears a gold sash from shoulder to hip.
  const torso = rows.findIndex((row, r) => r > eyeRow + 1 && /[cC]/.test(row));
  if (opts.mayor && torso >= 0) {
    for (let i = 0; i < 6; i++) {
      const y = 2 * torso + i, x = 4 + i;
      if (g[y] && g[y][x] && g[y][x] !== OUTLINE) { set(y, x, opts.sash || '#f8d040'); set(y, x + 1, opts.sash || '#f8d040'); }
    }
  }
  // A sub-agent: an armband and a badge in its specialty's colour.
  if (opts.sub && opts.accent && torso >= 0) {
    set(2 * torso + 2, 2, opts.accent); set(2 * torso + 2, 3, opts.accent);
    set(2 * torso + 1, 10, opts.accent);
  }
  return g;
}

/* The grid as rectangles, one per run of a colour along a row:
 * [x, y, w, colour], in detail pixels. */
export function runs(g) {
  const out = [];
  g.forEach((row, y) => {
    for (let x = 0; x < row.length;) {
      const col = row[x];
      if (!col) { x++; continue; }
      let w = 1;
      while (x + w < row.length && row[x + w] === col) w++;
      out.push([x, y, w, col]);
      x += w;
    }
  });
  return out;
}
