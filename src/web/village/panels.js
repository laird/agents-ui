/* ---------- Panels ----------
 * Around the map: the flow strip (one step per registered counter), the
 * toast, and the zoom. */
import { COUNTERS, placeOverlays } from './registry.js';
import { cv } from './layout.js';

export const esc = s => String(s).replace(/[&<>"]/g, ch => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[ch]));

/* The flow strip for a village, or for a world (plane 'world'): each
 * counter of that plane, in order. */
export function renderFlow(v, plane = v.plane === 'runtime' ? 'runtime' : 'development') {
  const items = COUNTERS.filter(k => k.plane === plane).flatMap(k => {
    const got = k.render(v);
    return !got ? [] : got.length && !Array.isArray(got[0]) ? [got] : got;
  });
  document.getElementById('flow').innerHTML = items.map(([l, s, n]) =>
    `<div class="step"><b>${esc(n)}</b><span>${esc(l)}<br>${esc(s)}</span></div>`).join('');
}

let toastTimer = 0;
export function toast(text) {
  const el = document.getElementById('toast');
  el.textContent = text; el.hidden = false;
  clearTimeout(toastTimer); toastTimer = setTimeout(() => { el.hidden = true; }, 5000);
}

/* The map fills the panel's width; a tall map scrolls the page rather than
 * shrinking. Whole-number scales keep the pixels square; below 1× (a phone)
 * it shrinks to the panel so the page never scrolls sideways. +/− step the
 * scale from the fitted one, and the step is remembered. */
const ZOOM_MAX_SCALE = 16;
let zoom = 0;
try { zoom = Math.max(-ZOOM_MAX_SCALE, Math.min(ZOOM_MAX_SCALE, parseInt(localStorage.getItem('village-zoom'), 10) || 0)); } catch {}
function fitScale() {
  const room = (document.getElementById('viewport').clientWidth - 8) / cv.width;
  return room >= 1 ? Math.floor(room) : Math.max(0.1, room);
}
function mapScale() {
  const f = fitScale();
  return zoom ? Math.max(Math.min(1, f), Math.min(ZOOM_MAX_SCALE, Math.max(1, Math.floor(f)) + zoom)) : f;
}
export function fit() {
  const W = cv.width, H = cv.height, s = mapScale();
  cv.style.width = W * s + 'px'; cv.style.height = H * s + 'px';
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
window.addEventListener('resize', () => { fit(); placeOverlays(); });
