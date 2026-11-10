/* ---------- Area labels ----------
 * A name over each part of the map and under each house, from what each
 * feature registers (registerLabels). Each label is placed in canvas pixels
 * with an anchor: 'top' hangs below the point, 'bottom' sits above it,
 * 'right' ends at it. A label with `parts` may wrap between them, never
 * inside one. */
import { LABELS, registerOverlay } from './registry.js';
import { cv, shownScenes, themeAt } from './layout.js';
import { esc } from './panels.js';

const abox = document.getElementById('alabels'), labelsOn = document.getElementById('labels-on');
try { labelsOn.checked = localStorage.getItem('village-labels') !== 'off'; } catch {}
labelsOn.addEventListener('change', () => {
  try { localStorage.setItem('village-labels', labelsOn.checked ? 'on' : 'off'); } catch {}
  placeLabels();
});
/* Move labels that cover an avoided sprite or an earlier label: up for those
 * that sit above their point, down for those that hang below it. Measured on
 * screen, so it holds at any scale. */
function unclutter(els, blocked) {
  const hits = (a, b) => a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom;
  // A fixed label stays on its point; the others keep clear of it.
  const fixed = els.filter(el => el.dataset.fixed);
  const placed = [...blocked, ...fixed.map(el => el.getBoundingClientRect())];
  els.filter(el => !el.dataset.fixed).forEach(el => {
    const up = el.dataset.anchor !== 'top';
    for (let k = 0; k < 8; k++) {
      const r = el.getBoundingClientRect(), o = placed.find(p => hits(r, p));
      if (!o) break;
      el.style.top = parseFloat(el.style.top) + (up ? o.top - r.bottom - 1 : o.bottom - r.top + 1) + 'px';
    }
    placed.push(el.getBoundingClientRect());
  });
}
/* Keep every label within the map's left and right edges (#437): an
 * end-anchored label near the edge would otherwise be cut off ("erge gate"). */
function keepInside(els, map) {
  els.forEach(el => {
    const r = el.getBoundingClientRect();
    const dx = r.left < map.left ? map.left - r.left : r.right > map.right ? Math.max(map.left - r.left, map.right - r.right) : 0;
    if (dx) el.style.left = parseFloat(el.style.left) + dx + 'px';
  });
}
export function placeLabels() {
  abox.hidden = !labelsOn.checked || cv.hidden;
  if (abox.hidden) return;
  const vp = document.getElementById('viewport'), r = cv.getBoundingClientRect(), vr = vp.getBoundingClientRect();
  const s = r.width / cv.width, ox0 = r.left - vr.left + vp.scrollLeft, oy0 = r.top - vr.top + vp.scrollTop;
  const shift = { top: 'translate(-50%, 0)', bottom: 'translate(-50%, -100%)', right: 'translate(-100%, -50%)', center: 'translate(-50%, -50%)' };
  // Text grows with the map; below 3x the subtitles go, so names stay readable.
  abox.style.setProperty('--lf', Math.round(Math.max(9, Math.min(14, 2 + 2.5 * s))) + 'px');
  abox.classList.toggle('compact', s < 3);
  let html = '';
  const blocked = [];
  shownScenes().forEach(([sc, ox, oy]) => {
    const theme = themeAt(sc);
    LABELS.forEach(({ fn }) => fn(sc, theme).forEach(l => {
      const a = l.avoid;
      if (a) { blocked.push({ left: r.left + (ox + a.x) * s, top: r.top + (oy + a.y) * s, right: r.left + (ox + a.x + a.w) * s, bottom: r.top + (oy + a.y + a.h) * s }); return; }
      // A label given a width (scene pixels) is no wider than that.
      const max = l.width ? `max-width:${Math.max(28, l.width * s)}px;` : '';
      // A label given a size (scene pixels) scales with the map, no smaller
      // than 6px: when that is tight it gives its `short` text, and zoomed out
      // too far to fit at all it is left out.
      if (l.size && l.size * s < 3.5) return;
      const tight = l.size && l.size * s < 8 && l.short;
      const size = l.size ? `font-size:${Math.max(6, Math.min(16, l.size * s))}px;` : '';
      // A label given an edge colour is bordered in it (#441).
      const edge = l.edge && /^#[0-9a-f]{3,8}$/i.test(l.edge) ? `border-color:${l.edge};` : '';
      html += `<div class="al${l.cls ? ' ' + esc(l.cls) : ''}" aria-hidden="true" data-anchor="${l.anchor}"${l.fixed ? ' data-fixed="1"' : ''} style="${size}${edge}left:${ox0 + (ox + l.x) * s}px;top:${oy0 + (oy + l.y) * s}px;transform:${shift[l.anchor]};${max}">` +
        `${tight ? esc(l.short) : l.parts ? l.parts.map(p => `<span>${esc(p)}</span>`).join(' ') : esc(l.text)}${l.sub ? ` <small>${esc(l.sub)}</small>` : ''}</div>`;
    }));
  });
  // Redo it when the labels or the sprites to avoid change (a sprite's rect may
  // only be known once a frame has drawn it).
  const drawn = html + blocked.map(b => [b.left, b.top, b.right, b.bottom].map(Math.round).join(',')).join(';');
  if (abox.drawn === drawn) return;
  abox.innerHTML = html; abox.drawn = drawn;
  unclutter([...abox.children], blocked);
  keepInside([...abox.children], cv.getBoundingClientRect());
}
registerOverlay(placeLabels, 20);
