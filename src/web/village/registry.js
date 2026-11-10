/* ---------- Registries ----------
 * The page's core knows no feature: it walks what the features register.
 *
 *   registerLayer({ z, draw(c, scene, t) })   paint() draws the layers in z
 *       order (ties in the order they registered). `scene` is this frame's:
 *       { v, lay } plus whatever earlier layers left on it for later ones.
 *   registerHit({ kind, dialog(obj, scene, theme), act?, key? })   what a
 *       drawn object of that kind is: explain() and act() dispatch on it. A
 *       hit with `act` does that on a click instead of opening its dialog;
 *       `key: false` leaves it to the mouse (no keyboard target).
 *   registerLabels(fn, order)   fn(scene, theme) lists the names over one
 *       village: { x, y, anchor, text, sub, cls, width } in scene pixels, or
 *       { avoid: { x, y, w, h } }, a sprite no label may cover.
 *   registerOverlay(fn, order)   fn() places something over the map (feature
 *       banners, area labels, keyboard targets) after it moves or changes.
 *
 * `order` and `z` are numbers; lower goes first. */
import { state } from './state.js';

const byOrder = list => list.sort((a, b) => a.order - b.order);

export const LAYERS = [];
export function registerLayer(layer) {
  LAYERS.push({ order: layer.z, ...layer });
  byOrder(LAYERS);
}

export const HITS = {};
export function registerHit(hit) {
  HITS[hit.kind] = hit;
}

export const LABELS = [];
export function registerLabels(fn, order = 0) {
  LABELS.push({ fn, order });
  byOrder(LABELS);
}

export const OVERLAYS = [];
export function registerOverlay(fn, order = 0) {
  OVERLAYS.push({ fn, order });
  byOrder(OVERLAYS);
}

/* What is clickable comes from what is drawn: paint() records a rectangle
 * (scene pixels) and the object behind it for every sprite it draws, later
 * ones on top. hit() and the keyboard targets read the list back, so a new
 * or moved sprite stays clickable without copying its coordinates. */
let hits = null;
export const tap = (x, y, w, h, o) => { if (hits) hits.push({ x: Math.round(x), y: Math.round(y), w, h, o }); };

/* One frame of the village in `state`: every layer, bottom to top. */
export function paint(c, t) {
  const scene = { v: state.current, lay: state.L };
  for (const layer of LAYERS) layer.draw(c, scene, t);
}

export function drawVillage(c, sc, t) {
  state.current = sc.v; state.L = sc.L; state.porters = sc.porters; state.floats = sc.floats; state.fireworks = sc.fireworks; state.S = sc;
  hits = [];
  paint(c, t);
  sc.hits = hits; hits = null;
  sc.porters = state.porters; sc.floats = state.floats;
}

/* The topmost drawn object under a point, in scene pixels. */
export function hitAt(sc, x, y) {
  const hs = sc.hits || [];
  for (let k = hs.length - 1; k >= 0; k--) { const r = hs[k]; if (x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h) return r.o; }
  return null;
}

/* Everything over the map, placed again. */
export function placeOverlays() {
  OVERLAYS.forEach(o => o.fn());
}
