/* ---------- Themes ----------
 * A theme decides how things look, never what they mean: every theme draws
 * the same slots (ground, roads, buildings per kind, stations) in the same
 * places, and keeps the shared status language -- lit windows for working,
 * an open door and an orange "!" for blocked, dust for stalled, fog for
 * unknown -- so switching themes never means relearning the map.
 *
 * Each theme module registers itself. The Style picker lists the themes in
 * the order they register, by their `name`; `pair` names the theme a world
 * draws its other half in. */
export const THEMES = {};

/* Every theme draws its own sprites at 2×, in detail() (detail.js): its
 * buildings since #463-#465, which retired the Scale2x upscaling (#466).
 * tools/village-tests/scenepixels.test.mjs holds every theme to it. */
export function registerTheme(name, theme) {
  THEMES[name] = theme;
}
