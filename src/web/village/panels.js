/* ---------- Panels ----------
 * Around the map: the toast and the zoom. The map itself shows the counts
 * (its labels and dialogs), so there is no row of counters under it (#405). */
import { placeOverlays } from './registry.js';
import { cv } from './layout.js';
import { state } from './state.js';
import { chooseScale, phoneScale } from './fitscale.js';

export const esc = s => String(s).replace(/[&<>"]/g, ch => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[ch]));

let toastTimer = 0;
export function toast(text) {
  const el = document.getElementById('toast');
  el.textContent = text; el.hidden = false;
  clearTimeout(toastTimer); toastTimer = setTimeout(() => { el.hidden = true; }, 5000);
}

/* Fit (#400): the map fills the window, width and height, with no scrolling.
 * The page is a column the window's height (village.css), so the viewport is
 * exactly the room left after the bar, minimap and strips; chooseScale picks
 * the scale for it. On a phone (NARROW, #437) the map keeps a readable
 * minimum scale (phoneScale) and scrolls sideways instead of shrinking.
 * +/− step the scale from the fitted one,
 * scrolling inside the viewport, and the step is remembered. */
const ZOOM_MAX_SCALE = 16;
const NARROW = '(max-width: 640px)';
let zoom = 0;
try { zoom = Math.max(-ZOOM_MAX_SCALE, Math.min(ZOOM_MAX_SCALE, parseInt(localStorage.getItem('village-zoom'), 10) || 0)); } catch {}
function fitScale() {
  const vp = document.getElementById('viewport');
  const tall = state.screensaver || !matchMedia(NARROW).matches;
  // A phone (#437): never a sliver; at PHONE_MIN the map scrolls sideways, from the dev village.
  if (!tall) return phoneScale(cv.width, cv.height, vp.clientWidth - 8);
  return chooseScale(cv.width, cv.height, vp.clientWidth - 8, vp.clientHeight - 8);
}
function mapScale() {
  const f = fitScale();
  if (state.screensaver) return f;
  return zoom ? Math.max(Math.min(1, f), Math.min(ZOOM_MAX_SCALE, Math.max(1, Math.floor(f)) + zoom)) : f;
}
export function fit() {
  const W = cv.width, H = cv.height, s = mapScale();
  // Whole CSS pixels, rounded down, so a fractional fit never overflows by one.
  cv.style.width = Math.floor(W * s) + 'px'; cv.style.height = Math.floor(H * s) + 'px';
  const fitBtn = document.getElementById('zoom-fit');
  fitBtn.textContent = zoom ? (Number.isInteger(s) ? s : s.toFixed(1)) + '×' : 'Fit';
  fitBtn.disabled = !zoom;
  document.getElementById('zoom-out').disabled = s <= Math.min(1, fitScale());
  document.getElementById('zoom-in').disabled = s >= ZOOM_MAX_SCALE;
}
function stepZoom(d) {
  const before = mapScale();
  zoom = d ? zoom + d : 0;
  if (d && mapScale() === before) zoom -= d;  // already at the smallest or largest scale
  try { localStorage.setItem('village-zoom', String(zoom)); } catch {}
  fit(); placeOverlays();
}
document.getElementById('zoom-out').addEventListener('click', () => stepZoom(-1));
document.getElementById('zoom-in').addEventListener('click', () => stepZoom(1));
document.getElementById('zoom-fit').addEventListener('click', () => stepZoom(0));
/* Fit again when the room changes: the window, fullscreen, or the chrome
 * around the map (the minimap or feature strip appearing, the flow strip
 * wrapping). The viewport's size doesn't follow the canvas (village.css), so
 * fitting never feeds back into the observer. Debounced: a drag-resize fits
 * once it settles. A new map size (houses added) fits at once (data.js). */
let refitSoonTimer = 0;
const refitSoon = () => {
  clearTimeout(refitSoonTimer);
  refitSoonTimer = setTimeout(() => { if (!cv.hidden) { fit(); placeOverlays(); } }, 80);
};
window.addEventListener('resize', refitSoon);
document.addEventListener('fullscreenchange', refitSoon);
if (typeof ResizeObserver === 'function') new ResizeObserver(refitSoon).observe(document.getElementById('viewport'));
