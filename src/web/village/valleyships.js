/* ---------- Ships on the valley's rivers (#511) ----------
 * A library's release flows downstream: a new one is a ship sailing down
 * the river to each consumer, and a consumer pinned to an older version has
 * a ship waiting at its dock, labelled with the version it is waiting to
 * take. Where the ships are, and what a click on one says, is decided in
 * valleymodel.js (valleyShips, shipWords); valley.js draws and places them
 * with these. */
import { P, R } from './palette.js';
import { esc } from './panels.js';
import { shipWords } from './valleymodel.js';

const HULL = '#6b4426', HULL_D = '#4a2e18';

/* A ship 11 scene pixels wide, its waterline at (x, y). A waiting ship
 * flies a red pennant: something downstream hasn't taken it yet. */
export function drawShip(c, ship) {
  const x = ship.x - 5, y = ship.y - 1;
  R(c, x, y, 11, 3, P.o);
  R(c, x + 1, y, 9, 2, HULL);
  R(c, x + 2, y + 2, 7, 1, HULL_D);
  R(c, x + 5, y - 8, 1, 8, P.o);
  R(c, x + 6, y - 7, 4, 5, P.o);
  R(c, x + 6, y - 7, 3, 4, P.parch);
  if (ship.waiting) R(c, x + 3, y - 8, 2, 2, P.p0);
}

/* Each waiting ship's version, over the map, beside it. */
export function shipLabels(ships, ox, oy, s) {
  return ships.filter(sh => sh.waiting).map(sh =>
    `<div class="al ov-ship" aria-hidden="true" style="left:${ox + sh.x * s}px;top:${oy + (sh.y + 3) * s}px">${esc(sh.edge.latest)}</div>`).join('');
}

/* A keyboard target for each waiting ship, opening what a click would. */
export function shipKeys(box, ships, ox, oy, s, open) {
  const keep = new Set();
  ships.filter(sh => sh.waiting).forEach(sh => {
    const key = `ship:${sh.edge.from}>${sh.edge.to}`, words = shipWords(sh);
    let b = [...box.children].find(x => x.dataset.name === key);
    if (!b) {
      b = document.createElement('button'); b.className = 'hk'; b.dataset.name = key;
      box.appendChild(b);
    }
    b.onclick = () => open(sh);
    b.setAttribute('aria-label', `${words.title}. ${words.line}`);
    Object.assign(b.style, { left: ox + (sh.x - 6) * s + 'px', top: oy + (sh.y - 9) * s + 'px', width: 12 * s + 'px', height: 13 * s + 'px' });
    keep.add(b);
  });
  return keep;
}
