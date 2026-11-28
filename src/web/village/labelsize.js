/* ---------- Map label size ----------
 * No DOM, so node tests it (tools/village-tests/labelsize.test.mjs).
 *
 * The map-label font size (--lf, labels.js) grows with the fit scale but
 * never drops below the readable floor, nor past the point a banner or house
 * outgrows its room. A class that shrinks further still (village.css's
 * `max(11px, calc(var(--lf, 11px) - Npx))`) keeps its own floor below that,
 * so every label on the map stays readable at the smallest fit scale (#568).
 */
export const LABEL_FLOOR = 12;
export const LABEL_CEIL = 14;
export const CLASS_FLOOR = 11;

export function lfPx(s) {
  return Math.round(Math.max(LABEL_FLOOR, Math.min(LABEL_CEIL, 2 + 2.5 * s)));
}

// What village.css's `max(11px, calc(var(--lf, 11px) - offset))` resolves to.
export function classPx(s, offset) {
  return Math.max(CLASS_FLOOR, lfPx(s) - offset);
}
