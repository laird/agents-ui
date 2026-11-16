/* ---------- The valley's environment grid (#512) ----------
 * The repos joined by `api` edges, by environment, below the valley: each
 * cell a plot with what that repo runs there, the prod column drawn as a
 * city. A cell outside what a neighbour in the same environment expects is
 * marked red, and its dialog says which neighbour, which versions and which
 * environment. Where everything sits, and what a click says, is decided in
 * valleymodel.js (gridLayout, gridWords); valley.js draws and places it
 * with these. */
import { P, R } from './palette.js';
import { esc } from './panels.js';
import { gridWords } from './valleymodel.js';

const MARK = '#e02828', TOWER = '#5a6478', TOWER_L = '#7a869c', LIT = '#f8e088', ROOF = '#a04030', WALL = '#d8c8a0';

/* One cell: a plot, and on it a city block (prod) or a cottage. */
function drawCell(c, g, state) {
  const { x, y, w, h } = g, marked = (g.cell.mismatches || []).length > 0;
  R(c, x - 1, y - 1, w + 2, h + 2, marked ? MARK : P.o);
  state.T.ground(c, x, y, w, h);
  if (g.city) {
    // A skyline: three towers of different heights, a lit window or two.
    [[4, 14], [16, 22], [30, 17]].forEach(([dx, th]) => {
      R(c, x + dx, y + h - 4 - th, 10, th, P.o);
      R(c, x + dx + 1, y + h - 3 - th, 8, th - 1, TOWER);
      R(c, x + dx + 1, y + h - 3 - th, 2, th - 1, TOWER_L);
      for (let wy = y + h - th; wy < y + h - 6; wy += 4) R(c, x + dx + 4, wy, 2, 2, LIT);
    });
  } else {
    R(c, x + 6, y + h - 14, 14, 10, P.o);
    R(c, x + 7, y + h - 13, 12, 8, WALL);
    R(c, x + 5, y + h - 17, 16, 4, ROOF);
  }
  if (marked) R(c, x + w - 7, y + 2, 4, 9, MARK), R(c, x + w - 7, y + 13, 4, 3, MARK);
}

/* Every cell of the grid. */
export function drawGrid(c, lay, state) {
  (lay.grid ? lay.grid.cells : []).forEach(g => drawCell(c, g, state));
}

/* Over the map: each column's environment, each row's repo, and each cell's version. */
export function gridLabels(lay, ox, oy, s) {
  const g = lay.grid;
  if (!g) return '';
  const at = (x, y, cls, text, center = true) =>
    `<div class="al ${cls}" aria-hidden="true" style="left:${ox + x * s}px;top:${oy + y * s}px${center ? '' : ';transform:none'}">${esc(text)}</div>`;
  return [
    ...g.cols.map(col => at(col.x + 28, g.y, 'ov-grid', col.city ? `${col.env} (city)` : col.env)),
    ...g.rows.map(row => at(g.x - 60, row.y + 8, 'ov-grid', row.repo, false)),
    ...g.cells.map(cell => at(cell.x + 28, cell.y + cell.h - 2, (cell.cell.mismatches || []).length ? 'ov-ship' : 'ov-grid', cell.cell.version || '?')),
  ].join('');
}

/* A keyboard target for each cell, opening what a click would. */
export function gridKeys(box, lay, ox, oy, s, open) {
  const keep = new Set();
  (lay.grid ? lay.grid.cells : []).forEach(g => {
    const key = `grid:${g.cell.repo}@${g.cell.env}`, words = gridWords(g.cell);
    let b = [...box.children].find(x => x.dataset.name === key);
    if (!b) {
      b = document.createElement('button'); b.className = 'hk'; b.dataset.name = key;
      box.appendChild(b);
    }
    b.onclick = () => open(g.cell);
    b.setAttribute('aria-label', `${words.title}. ${words.line}`);
    Object.assign(b.style, { left: ox + g.x * s + 'px', top: oy + g.y * s + 'px', width: g.w * s + 'px', height: g.h * s + 'px' });
    keep.add(b);
  });
  return keep;
}
