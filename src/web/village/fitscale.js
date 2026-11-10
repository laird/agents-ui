/* ---------- The Fit scale ----------
 * No DOM, so node tests it (tools/village-tests/fitscale.test.mjs).
 *
 * The largest scale at which the whole map (w × h scene pixels) fits the room
 * left for it (roomW × roomH CSS pixels). A room of Infinity height bounds the
 * map by width alone (see phoneScale for a phone).
 *
 * Whole-number scales keep every scene pixel the same size. A fractional
 * scale (drawn `image-rendering: pixelated`, so a few pixel rows come out a
 * pixel wider) is taken only when the best whole one would leave more than
 * FIT_SLACK of the room empty on both axes: a map that fits at 1.9× shown at
 * 1× fills half the window, and a slightly uneven pixel grid is the smaller
 * cost. Below 1× the map shrinks to fit, so nothing scrolls. */
export const FIT_SLACK = 0.15;
export const FIT_MIN = 0.1;
/* The smallest scale on a phone (#437): houses (16 scene pixels) at least
 * 32 px tall, in whole pixels. A map wider than the screen at this scale
 * scrolls sideways inside its viewport instead of shrinking to a sliver. */
export const PHONE_MIN = 2;

export function chooseScale(w, h, roomW, roomH = Infinity) {
  if (!(w > 0 && h > 0)) return 1;
  const most = Math.min(roomW / w, roomH / h);
  if (!(most > 0)) return FIT_MIN;
  if (most < 1) return Math.max(FIT_MIN, most);
  const whole = Math.floor(most);
  const bareW = 1 - (whole * w) / roomW, bareH = 1 - (whole * h) / roomH;
  return bareW > FIT_SLACK && bareH > FIT_SLACK ? most : whole;
}

/* The scale on a narrow screen: the width fits the map when it can be read
 * there, and PHONE_MIN when it can't (the viewport then scrolls sideways). */
export function phoneScale(w, h, roomW) {
  return Math.max(PHONE_MIN, chooseScale(w, h, roomW, Infinity));
}
