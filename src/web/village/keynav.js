/* ---------- Getting around the map by keyboard (#478) ----------
 * dialogs.js puts one invisible button over each object (placeKeys); this
 * puts them in a useful Tab order (keyorder.js), moves between them with the
 * arrow keys, and lets 1-9 answer a question dialog. Enter opens a target's
 * dialog because it is a button: Enter clicks it, and the click does what a
 * click on the map does. The focus ring is drawn in each target's theme
 * colour (`focus` on the theme). The screensaver (`dismiss=1`) still closes
 * on any key: screensaver.js listens first, in the capture phase. */
import { state } from './state.js';
import { registerKey, registerOverlay } from './registry.js';
import { R } from './palette.js';
import { shownScenes } from './layout.js';
import { dialog, kbox, kbuttons } from './dialogs.js';
import { keyOrder, nearest, optionFor } from './keyorder.js';

const box = b => ({ b, x: parseFloat(b.style.left) || 0, y: parseFloat(b.style.top) || 0, w: parseFloat(b.style.width) || 0, h: parseFloat(b.style.height) || 0 });

/* Put the targets in Tab order: the DOM order is the Tab order. A button
 * moved while it has focus loses it, so focus is put back. */
export function orderKeys() {
  const at = new Map(shownScenes().map(([sc], k) => [sc.v.project, k]));
  const entries = [...kbuttons.values()].map(b => {
    if (b.theme && b.theme.focus) b.style.setProperty('--focus', b.theme.focus);
    const sc = state.scenes[b.project];
    return { ...box(b), o: b.obj, v: sc && sc.v, scene: at.get(b.project) ?? at.size };
  });
  const want = keyOrder(entries).map(e => e.b), now = [...kbox.children];
  if (want.length === now.length && want.every((b, k) => b === now[k])) return;
  const focused = document.activeElement;
  want.forEach(b => kbox.appendChild(b));
  if (focused && focused !== document.activeElement && focused.isConnected) focused.focus({ preventScroll: true });
}
registerOverlay(orderKeys, 31);

const DIRS = { ArrowLeft: 'left', ArrowRight: 'right', ArrowUp: 'up', ArrowDown: 'down' };
const typing = el => !!el && (/^(INPUT|SELECT|TEXTAREA)$/.test(el.tagName) || el.isContentEditable);

document.addEventListener('keydown', e => {
  if (e.altKey || e.ctrlKey || e.metaKey) return;
  // An arrow from a target on the map: the nearest target that way.
  const dir = DIRS[e.key], el = document.activeElement;
  if (dir && dialog.hidden && el && el.parentNode === kbox) {
    const all = [...kbox.children].map(box), from = all.find(r => r.b === el);
    const to = from && nearest(from, all, dir);
    if (to) { e.preventDefault(); to.b.focus(); }
    return;
  }
  // A question dialog: 1-9 answers as the button with that number does.
  if (!dialog.hidden && !typing(el)) {
    const buttons = [...document.querySelectorAll('#d-q [data-opt]')];
    const k = optionFor(e.key, buttons.map(b => b.dataset.key || ''));
    if (k >= 0 && !buttons[k].disabled) { e.preventDefault(); buttons[k].click(); }
  }
});

/* The Key says so. */
registerKey({ group: 'Keyboard', order: 90, rows: theme => [
  [c => { const f = theme.focus || '#f8d040'; R(c, 1, 1, 10, 1, f); R(c, 1, 10, 10, 1, f); R(c, 1, 1, 1, 10, f); R(c, 10, 1, 1, 10, f); },
    'Tab / Shift+Tab: the next thing on the map, waiting-on-you first, then the houses left to right, the board, gate, quay, docks and environments', 12],
  [c => { const f = theme.focus || '#f8d040'; R(c, 5, 1, 2, 10, f); R(c, 1, 5, 10, 2, f); }, 'Arrow keys: the nearest thing that way', 12],
  [c => { const f = theme.focus || '#f8d040'; R(c, 2, 5, 8, 2, f); R(c, 8, 2, 2, 4, f); }, 'Enter: open it, as a click does. Esc closes; 1-9 answer a question', 12]
] });
