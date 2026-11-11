/* ---------- The detail grid ----------
 * No DOM, so node tests it (tools/village-tests/detail.test.mjs).
 *
 * The map's canvas has DETAIL canvas pixels per scene pixel (#418): a crate
 * of 5×5 scene pixels covers 10×10 canvas pixels. Layout stays in scene
 * pixels, and every scene is painted under a ×DETAIL transform (prepare).
 * Sprites draw inside detail(), in canvas pixels, with twice the detail in
 * the same space (#455); what is still drawn in scene pixels under the bare
 * transform (the ground, water and other scenery, shadows) comes out doubled.
 * tools/village-tests/scenepixels.test.mjs keeps sprites off it.
 *
 * Whatever maps between the screen and the scene divides the canvas's size
 * by DETAIL (sceneScale, toScene); Fit scales the canvas, which is DETAIL
 * times bigger, so the map fills the same room as before. */
export const DETAIL = 2;

/* Set a context to draw in scene pixels on a canvas DETAIL times finer. A
 * canvas resized since forgets its transform, so this runs every frame. */
export function prepare(c, d = DETAIL) {
  c.setTransform(d, 0, 0, d, 0, 0);
  c.imageSmoothingEnabled = false;
}
/* Draw a 2× sprite: `fn` draws in detail pixels (half a scene pixel each),
 * with its origin at scene (x, y). */
export function detail(c, x, y, fn, d = DETAIL) {
  c.save();
  c.translate(Math.round(x), Math.round(y));
  c.scale(1 / d, 1 / d);
  fn(c);
  c.restore();
}
/* On-screen (CSS) pixels per scene pixel, for a canvas shown `shownWidth`
 * wide with `canvasWidth` canvas pixels. */
export const sceneScale = (shownWidth, canvasWidth, d = DETAIL) => (shownWidth / canvasWidth) * d;
/* Canvas pixels to scene pixels. */
export const toScene = (v, d = DETAIL) => v / d;
