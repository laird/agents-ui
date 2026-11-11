/* ---------- Buildings at 2× (#421) ----------
 * No DOM, so node tests it (tools/village-tests/upscale.test.mjs).
 *
 * A building is drawn as it always was, in scene pixels with fillRect, but
 * into a recorder rather than the map. Its pixels are then scaled to the 2×
 * detail grid (#418) with Scale2x, which keeps every straight edge and
 * flat colour, and rounds corners and smooths diagonals (roofs, the castle's
 * crenels, the gatehouse arch, cobweb strands) the way pixel art is drawn at
 * that size. Then it is painted with detail(). A building animates as
 * before (lit windows, smoke, the gatehouse's tables), because it is
 * recorded afresh every frame; it is still clickable where it was, because
 * what it taps is unchanged.
 *
 * hd(fn) wraps one of a theme's drawing slots this way (themes/registry.js). */
import { detail } from './detail.js';
import { runs } from './people2x.js';

/* A colour that hides what is under it: hex, rgb(), or rgba() at full alpha. */
export function opaque(col) {
  const m = /^rgba\(([^)]*)\)$/.exec(String(col).replace(/\s/g, ''));
  return !m || parseFloat(m[1].split(',')[3]) >= 1;
}

/* Run `draw(ctx)` against a context that only keeps what fillRect paints:
 * { x0, y0, grid }, the grid holding each scene pixel's colour (null:
 * nothing), with its top-left at (x0, y0). A see-through colour over
 * another keeps both, as "under|over", so painting lays them down in order. */
export function record(draw) {
  const px = new Map(), over = [];
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  // A building may draw a little at 2× itself, through detail() (a note's
  // glyph on the board, #443): scale and translate are followed, and what is
  // drawn finer than a scene pixel is kept apart, in `over`, to be painted
  // over the scaled building as it was drawn.
  let m = { s: 1, x: 0, y: 0 };
  const stack = [];
  const ctx = {
    fillStyle: '#000',
    save() { stack.push({ ...m }); }, restore() { m = stack.pop() || { s: 1, x: 0, y: 0 }; },
    translate(x, y) { m = { s: m.s, x: m.x + m.s * x, y: m.y + m.s * y }; },
    scale(sx) { m = { s: m.s * sx, x: m.x, y: m.y }; },
    fillRect(x, y, w, h) {
      if (m.s !== 1) { over.push([m.x + m.s * x, m.y + m.s * y, m.s * w, m.s * h, ctx.fillStyle]); return; }
      x += m.x; y += m.y;
      const xa = Math.round(x), ya = Math.round(y);
      const col = ctx.fillStyle, see = !opaque(col);
      for (let j = 0; j < h; j++) for (let i = 0; i < w; i++) {
        const k = `${xa + i},${ya + j}`, under = px.get(k);
        px.set(k, see && under ? `${under}|${col}` : col);
      }
      x0 = Math.min(x0, xa); y0 = Math.min(y0, ya); x1 = Math.max(x1, xa + w); y1 = Math.max(y1, ya + h);
    },
  };
  draw(ctx);
  if (!px.size) return { x0: 0, y0: 0, grid: [], over };
  const grid = Array.from({ length: y1 - y0 }, (_, j) => Array.from({ length: x1 - x0 }, (_, i) => px.get(`${x0 + i},${y0 + j}`) ?? null));
  return { x0, y0, grid, over };
}

/* Scale2x (EPX): each pixel becomes four. A corner of the four takes the
 * colour of the two neighbours it touches when they agree and the pixel's
 * other neighbours don't; otherwise it keeps the pixel's own colour. */
export function scale2x(grid) {
  const H = grid.length, W = H ? grid[0].length : 0;
  const at = (y, x) => (y >= 0 && y < H && x >= 0 && x < W ? grid[y][x] : null);
  const out = Array.from({ length: H * 2 }, () => Array(W * 2).fill(null));
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const p = grid[y][x], a = at(y - 1, x), b = at(y, x + 1), c = at(y, x - 1), d = at(y + 1, x);
    out[2 * y][2 * x] = c === a && c !== d && a !== b ? a : p;
    out[2 * y][2 * x + 1] = a === b && a !== c && b !== d ? b : p;
    out[2 * y + 1][2 * x] = d === c && d !== b && c !== a ? c : p;
    out[2 * y + 1][2 * x + 1] = b === d && b !== a && d !== c ? d : p;
  }
  return out;
}

/* Inside a recording, a wrapped slot draws plainly: one building drawing
 * another (a hall and its tower) is scaled once, with the rest. */
let recording = false;

/* Draw `fn(c, ...args)` at 2×: recorded, Scale2x'd, painted on the detail grid. */
export function hd(fn) {
  if (typeof fn !== 'function' || fn.hd) return fn;
  const wrapped = (c, ...args) => {
    if (recording) return fn(c, ...args);
    recording = true;
    let rec;
    try { rec = record(r => fn(r, ...args)); } finally { recording = false; }
    // A slot redrawn at 2× (#464) draws only on the detail grid: nothing to scale, all of it in `over`.
    if (rec.grid.length) {
      const parts = runs(scale2x(rec.grid));
      // Two detail pixels per scene pixel: x and y in detail pixels from (x0, y0).
      detail(c, rec.x0, rec.y0, g => parts.forEach(([x, y, w, col]) => col.split('|').forEach(layer => { g.fillStyle = layer; g.fillRect(x, y, w, 1); })));
    }
    // What the building drew at 2× itself, on top, where it drew it.
    if (rec.over.length) detail(c, 0, 0, g => rec.over.forEach(([x, y, w, h, col]) => { g.fillStyle = col; g.fillRect(x * 2, y * 2, w * 2, h * 2); }));
  };
  wrapped.hd = true;
  // What it draws before it is scaled: a test checks a redrawn slot draws nothing in scene pixels.
  wrapped.raw = fn;
  return wrapped;
}
