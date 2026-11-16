/* ---------- Ropes, caravans and barges on the valley (#530) ----------
 * What ties the group's repos together on GitHub, drawn over the valley:
 * - a rope between two repos: an open issue in one waits on an open issue
 *   in the other (crossrepo.rs, #528);
 * - a caravan below the valley: a parent issue with subtasks in several
 *   repos, one crate per repo, full once that repo's part has landed; it
 *   travels its row as crates load and flies a flag when it has arrived;
 * - a barge going back up a river: a consumer's red build names the
 *   library upstream (breakage.rs, #529).
 * Where each sits and what a click says is decided in valleymodel.js;
 * valley.js draws and places them with these. */
import { P, R } from './palette.js';
import { esc } from './panels.js';
import { CARAVAN_CRATE, CARAVAN_LEAD, bargeWords, caravanWords, ropeWords } from './valleymodel.js';

const LINK_ROPE = '#c8a060', LINK_ROPE_D = '#7a5a28', LINK_HIDE = '#8a5a30', LINK_WOOD = '#a87038', LINK_WOOD_D = '#6b4426', LINK_TRACK = 'rgba(80,56,32,.45)';

/* A rope: a twisted line that sags a little at its middle, a knot at each end. */
export function drawValleyRope(c, rp) {
  const { x0, y0, x1, y1 } = rp, steps = Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0), 1);
  for (let k = 0; k <= steps; k++) {
    const f = k / steps, sag = Math.round(3 * Math.sin(Math.PI * f));
    const x = Math.round(x0 + (x1 - x0) * f), y = Math.round(y0 + (y1 - y0) * f) + sag;
    R(c, x, y, 1, 1, k % 3 === 0 ? LINK_ROPE_D : LINK_ROPE);
  }
  [[x0, y0], [x1, y1]].forEach(([x, y]) => { R(c, x - 1, y - 1, 3, 3, P.o); R(c, x, y, 1, 1, LINK_ROPE); });
}

/* A caravan: a horse at its head, then a crate per repo (full once landed),
 * on a track across its row; a flag over the last crate once it has arrived. */
export function drawValleyCaravan(c, tr, W, margin) {
  const { x, y, caravan } = tr, ground = y + 10;
  R(c, margin, ground, W - 2 * margin, 1, LINK_TRACK);
  // The horse, facing east.
  R(c, x + 1, ground - 6, 7, 4, P.o); R(c, x + 2, ground - 5, 5, 2, LINK_HIDE);
  R(c, x + 6, ground - 8, 3, 3, P.o); R(c, x + 7, ground - 7, 1, 1, LINK_HIDE);
  R(c, x + 2, ground - 2, 1, 2, P.o); R(c, x + 6, ground - 2, 1, 2, P.o);
  caravan.crates.forEach((cr, k) => {
    const cx = x + CARAVAN_LEAD + k * CARAVAN_CRATE;
    R(c, cx, ground - 6, 6, 6, P.o);
    if (cr.loaded) { R(c, cx + 1, ground - 5, 4, 4, LINK_WOOD); R(c, cx + 1, ground - 3, 4, 1, LINK_WOOD_D); }
    else R(c, cx + 1, ground - 5, 4, 4, 'rgba(0,0,0,.18)');
  });
  if (caravan.arrived) {
    const fx = x + CARAVAN_LEAD + (caravan.crates.length - 1) * CARAVAN_CRATE + 2;
    R(c, fx, ground - 13, 1, 7, P.o); R(c, fx + 1, ground - 13, 4, 3, '#38c848');
  }
}

/* A barge: a flat hull loaded with red crates, on the river going upstream. */
export function drawValleyBarge(c, b) {
  const x = b.x - 6, y = b.y - 1;
  R(c, x, y, 13, 3, P.o); R(c, x + 1, y, 11, 2, LINK_WOOD_D);
  R(c, x + 2, y - 4, 4, 4, P.o); R(c, x + 3, y - 3, 2, 3, P.p0);
  R(c, x + 7, y - 4, 4, 4, P.o); R(c, x + 8, y - 3, 2, 3, P.p0);
}

/* Over the map: each caravan's parent and how much has landed. */
export function caravanLabels(trains, ox, oy, s) {
  return trains.map(t => `<div class="al ov-grid" aria-hidden="true" style="left:${ox + (t.x + t.w + 4) * s}px;top:${oy + t.y * s}px;transform:none">` +
    `${esc(`${t.caravan.parent.repo}#${t.caravan.parent.number}`)} ${esc(t.caravan.arrived ? 'arrived' : `${t.loaded}/${t.total}`)}</div>`).join('');
}

/* One keyboard target per rope, caravan and barge, opening what a click would. */
export function linkKeys(box, { ropes, trains, barges }, ox, oy, s, open) {
  const keep = new Set();
  const key = (name, words, rect) => {
    let b = [...box.children].find(x => x.dataset.name === name);
    if (!b) { b = document.createElement('button'); b.className = 'hk'; b.dataset.name = name; box.appendChild(b); }
    b.onclick = () => open(words);
    b.setAttribute('aria-label', `${words.title}. ${words.line}`);
    Object.assign(b.style, { left: ox + rect.x * s + 'px', top: oy + rect.y * s + 'px', width: rect.w * s + 'px', height: rect.h * s + 'px' });
    keep.add(b);
  };
  ropes.forEach(rp => key(`rope:${rp.rope.from.repo}#${rp.rope.from.number}>${rp.rope.to.repo}#${rp.rope.to.number}`, ropeWords(rp.rope),
    { x: Math.min(rp.x0, rp.x1) - 2, y: Math.min(rp.y0, rp.y1) - 2, w: Math.abs(rp.x1 - rp.x0) + 4, h: Math.abs(rp.y1 - rp.y0) + 7 }));
  trains.forEach(t => key(`caravan:${t.caravan.parent.repo}#${t.caravan.parent.number}`, caravanWords(t), { x: t.x, y: t.y, w: t.w, h: 12 }));
  barges.forEach((b, k) => key(`barge:${b.barge.from}>${b.barge.to}:${k}`, bargeWords(b), { x: b.x - 7, y: b.y - 6, w: 14, h: 9 }));
  return keep;
}
