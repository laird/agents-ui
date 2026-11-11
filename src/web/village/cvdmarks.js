/* ---------- Marks that don't rely on colour ----------
 * Every colour-coded state also has a shape (#443), so the village reads
 * under colour blindness: a board note carries a glyph for its type, and a
 * crate's priority band has a pattern per level. Both are drawn on the 2×
 * detail grid (#418), where a 3×3 note has room for a 4×4 glyph and a
 * crate's band (#420's 2× crates) has room for a pattern. No DOM, so
 * its tests run under node (tools/village-tests/cvdmarks.test.mjs). */
import { NOTECOL, P, R } from './palette.js';
import { detail } from './detail.js';

/* Each note type's glyph, 4×4 detail pixels ('1' is ink). */
export const NOTE_GLYPHS = {
  failed: ['1001', '0110', '0110', '1001'],                // ✗
  conflict: ['0010', '1111', '1111', '0100'],              // ⇄
  'needs-clarification': ['1110', '0010', '0000', '0100'], // ?
  'needs-design': ['0011', '0110', '1100', '1000'],        // ✎
  proposal: ['1110', '1110', '1000', '1000'],              // ⚑
  'needs-approval': ['0000', '0001', '1010', '0100'],      // ✓
  'too-complex': ['1010', '1010', '0000', '1010'],         // !!
  future: ['0010', '1111', '0010', '0000'],                // →
  unreachable: ['0000', '0000', '1111', '0000']            // –
};
/* What each glyph stands for, for the Key. */
export const GLYPH_WORDS = { failed: '✗', conflict: '⇄', 'needs-clarification': '?', 'needs-design': '✎', proposal: '⚑',
  'needs-approval': '✓', 'too-complex': '!!', future: '→', unreachable: '–' };

/* Dark ink on a light note, white on a dark one, by perceived lightness. */
export function inkOn(hex) {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex || '');
  if (!m) return P.o;
  const n = parseInt(m[1], 16), r = n >> 16, g = (n >> 8) & 255, b = n & 255;
  return 0.299 * r + 0.587 * g + 0.114 * b > 140 ? P.o : P.white;
}

/* A note on a board: its colour, 3×3 scene pixels at (x, y), with its glyph
 * on it, all in detail pixels (#466). An unknown label is a plain note in
 * `fallback`. */
export function note(c, x, y, label, fallback) {
  const col = NOTECOL[label] || fallback;
  const g = NOTE_GLYPHS[label];
  const ink = inkOn(col);
  detail(c, x, y, d => {
    R(d, 0, 0, 6, 6, col);
    if (g) g.forEach((row, j) => { for (let i = 0; i < 4; i++) if (row[i] === '1') R(d, 1 + i, 1 + j, 1, 1, ink); });
  });
}

/* Which cells of a priority band of `len` detail pixels are painted: P0
 * solid, P1 both ends, P2 dashed, P3 the middle. Null when unprioritised. */
export function prioCells(pr, len) {
  const end = Math.max(1, Math.floor(len / 3));
  const at = {
    P0: () => true,
    P1: k => k < end || k >= len - end,
    P2: k => k % 2 === 0,
    P3: k => Math.abs(k - (len - 1) / 2) < 1
  }[pr];
  return at ? Array.from({ length: len }, (_, k) => at(k)) : null;
}

/* A priority band `len` detail pixels long and one high at (x, y), drawn
 * with `d` already on the detail grid (a 2× crate, crates.js). */
export function prioRun(d, x, y, len, pr, col) {
  (prioCells(pr, len) || []).forEach((on, k) => { if (on) R(d, x + k, y, 1, 1, col); });
}
