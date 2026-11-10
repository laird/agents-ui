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
import { hd } from '../upscale.js';

export const THEMES = {};

/* The buildings each theme draws at 2× (#421, upscale.js): houses of every
 * kind, the hall, the notice board, the gatehouse, docks and quay, and the
 * scaffolding a house goes up in. Ground, water and scenery stay 1×. */
export const HD_SLOTS = ['hall', 'board', 'gatehouse', 'dock', 'scaffold'];

export function registerTheme(name, theme) {
  HD_SLOTS.forEach(slot => { theme[slot] = hd(theme[slot]); });
  if (theme.B) theme.B = Object.fromEntries(Object.entries(theme.B).map(([kind, fn]) => [kind, hd(fn)]));
  THEMES[name] = theme;
}
