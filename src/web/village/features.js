/* ---------- Features ----------
 * A feature (a parent issue split into subtasks) is a banner in the strip
 * cleared east of the houses, with its spec scroll at its foot. Following
 * one draws a marching trail to each counted subtask. In whole-feature
 * projects a feature's subtasks ride one barge (a truck downtown) once
 * they are all at the same stop; an environment nobody can answer for is
 * in fog. The server works out every count; the renderer only draws them.
 * Each banner has a label (a button, so it can be followed by keyboard), and
 * the one followed has the strip under the map. */
import { state } from './state.js';
import { F, P, R, blink } from './palette.js';
import { puffs } from './sprites/houses.js';
import { bubble } from './scenery.js';
import { flagLabel } from './houselabel.js';
import { drawBanner } from './bannerart.js';
import { M, MODERN } from './themes/modern.js';
import { FUTURE } from './themes/future.js';
import { placeOverlays, registerHit, registerKey, registerLabels, registerLayer, registerOverlay, tap } from './registry.js';
import { cv, shownScale, shownScenes, slotPos } from './layout.js';
import { SHIP_CAP, cargoOf, holdPos, placeOf, sourceWords } from './ship.js';
import { esc } from './panels.js';
import { ask, dialog, links, objAt } from './dialogs.js';
import { draw } from './worlds.js';

let followed = null; // { project, number }
const isFollowed = (v, f) => !!followed && followed.project === v.project && followed.number === f.number;
const counted = f => (f.subtasks || []).filter(s => !s.dropped);
function bargesFor(v, lay) {
  const last = v.pipeline.stops.length - 1, queued = {};
  return lay.banners.map(g => {
    if (g.f.promotion !== 'feature') return null;
    const ns = counted(g.f).map(s => s.number), at = ns.map(n => placeOf(n, v));
    if (!ns.length || at.some(a => a === null || a !== at[0])) return null;
    // Barges at the same stop queue up: off the quay they line up downstream,
    // at a lock or the harbour they wait upstream of it.
    const i = at[0], k = queued[i] = (queued[i] ?? -1) + 1;
    const x = i === -1 ? lay.roadX - 14 + k * 26
      : Math.max(lay.roadX - 14, (i === last ? lay.castleX - 26 : lay.lockStart + i * 34 + 8) - k * 26);
    return { f: g.f, col: g.col, issues: ns, stop: i, x, y: lay.canalY + 1 };
  }).filter(Boolean);
}
/* The map point a trail leads to for subtask n: its house, its crate, or its barge. */
function whereIs(n, v, lay, barges) {
  const hi = v.houses.findIndex(h => h.issue === n);
  if (hi >= 0) { const lot = lay.lots[hi]; if (lot) return { x: lot.x + 8, y: lot.y + 18 }; }
  const bi = v.backlog.findIndex(c => c.number === n);
  if (bi >= 0) { const s = slotPos(Math.min(bi, 39), v.backlog.length); return { x: s.x + 2, y: s.y + 2 }; }
  const ni = v.board.findIndex(b => b.number === n);
  if (ni >= 0) { const k = Math.min(ni, 7); return { x: lay.hallX + 45 + (k % 4) * 4, y: 31 + Math.floor(k / 4) * 4 }; }
  const b = barges.find(b => b.issues.includes(n));
  if (b) { const k = Math.min(b.issues.indexOf(n), 3); return { x: b.x + 4 + k * 5, y: b.y + 3 }; }
  const hid = new Set(barges.flatMap(b => b.issues));
  const ci = cargoOf(v).filter(m => !hid.has(m)).indexOf(n);
  if (ci >= 0) { const [x, y] = holdPos(lay, Math.min(ci, SHIP_CAP - 1)); return { x: x + 2, y: y + 2 }; }
  const qi = v.pipeline.quay.filter(m => !hid.has(m)).indexOf(n);
  if (qi >= 0) { const k = Math.min(qi, 5); return { x: lay.roadX + 8 + (k % 3) * 6, y: lay.quayY - 1 - Math.floor(k / 3) * 6 }; }
  const stops = v.pipeline.stops;
  for (let i = 0; i < stops.length; i++) {
    let k = stops[i].issues.filter(m => !hid.has(m)).indexOf(n);
    if (k < 0) continue;
    // Shipped work has sailed: the trail ends at the harbour's lighthouse.
    return i < stops.length - 1
      ? (k = Math.min(k, 3), { x: lay.lockStart + i * 34 + 10 + (k % 2) * 6, y: lay.canalY + 20 + Math.floor(k / 2) * 6 })
      : { x: lay.castleX + 31, y: lay.canalY - 24 };
  }
  return null;
}
/* A barge on the canal, or a flatbed truck on the district's road. */
function drawBarge(c, b, t) {
  const x = b.x, y = b.y + (!state.reduce && blink(t, 700) ? 1 : 0);
  if (state.T === FUTURE) {
    // A hover-barge: hull, cockpit, a glow under it.
    R(c, x, y + 6, 24, 4, P.o); R(c, x + 1, y + 6, 22, 2, F.hull); R(c, x + 1, y + 6, 22, 1, b.col);
    R(c, x + 18, y + 1, 6, 6, P.o); R(c, x + 19, y + 2, 4, 3, F.glass);
    R(c, x + 2, y + 10, 20, 1, !state.reduce && blink(t, 200) ? F.cyanL : F.cyan);
  } else if (state.T === MODERN) {
    R(c, x, y + 6, 20, 4, P.o); R(c, x + 1, y + 6, 18, 2, M.steel); R(c, x + 1, y + 6, 18, 1, b.col);
    R(c, x + 19, y + 1, 6, 9, P.o); R(c, x + 20, y + 2, 4, 6, P.white); R(c, x + 21, y + 3, 3, 2, M.glassL);
    [x + 3, x + 14, x + 21].forEach(wx => { R(c, wx, y + 9, 3, 3, P.o); R(c, wx + 1, y + 10, 1, 1, M.steel); });
  } else {
    R(c, x, y + 6, 24, 5, P.o); R(c, x + 1, y + 6, 22, 3, P.wood); R(c, x + 1, y + 6, 22, 1, P.woodL); R(c, x + 3, y + 9, 18, 1, P.woodD);
    R(c, x + 22, y - 6, 1, 8, P.o); R(c, x + 23, y - 6, 5, 4, P.o); R(c, x + 23, y - 5, 4, 2, b.col);
  }
  b.issues.slice(0, 4).forEach((n, k) => state.T.crate(c, x + 2 + k * 5, y + 1, 'workshop', null, true, n));
  tap(b.x, b.y - 6, 28, 18, { kind: 'barge', f: b.f ? b.f.number : null, issues: b.issues });
}
/* The fog wash per theme, shared with the key. */
export const fogCol = th => th === MODERN ? 'rgba(170,176,196,.75)' : th === FUTURE ? 'rgba(60,52,110,.78)' : 'rgba(206,210,230,.72)';
registerKey({ group: 'other', rows: theme => [
  [c => { R(c, 0, 0, 14, 14, fogCol(theme)); bubble(c, 2, 1, 'unknown'); }, 'Grey fog over a stage: nothing says what runs there (no Deployment, no branch)']] });
/* Fog over a stop with neither a Deployment nor a branch: "?", never "not live". */
function fogStop(c, v, i, lay, t) {
  const last = v.pipeline.stops.length - 1, x = i === last ? lay.castleX - 4 : lay.lockStart + i * 34 - 7, w = i === last ? 46 : 32;
  R(c, x, lay.canalY - 28, w, lay.devH - lay.canalY + 28, fogCol(state.T));
  if (!state.reduce) for (let k = 0; k < w / 6 - 1; k++) puffs(c, x + 4 + k * 6, lay.canalY - 18 + (k % 2) * 22, t + k * 330, '#e0e4f0', '#b8c0d8');
  bubble(c, x + (w >> 1) - 4, lay.canalY - 6, 'unknown');
}
/* Under each flag, with LABELS on: "#417 Hi-res · 1/6", edged in its colour (#441). */
registerLabels(sc => (sc.L.banners || []).map(g => {
  const f = g.f, b = f.build || { landed: 0, total: counted(f).length };
  return { x: g.x + 8, y: g.y + 27, anchor: 'top', text: flagLabel(f.number, f.title, b), cls: 'flag', edge: g.col, width: 29 };
}), 15);
function drawTrail(c, x0, y0, x1, y1, col, t) {
  const off = state.reduce ? 0 : Math.floor(t / 110) % 4, pts = [];
  x1 = Math.round(x1); y1 = Math.round(y1);
  for (let y = y0; y !== y1; y += Math.sign(y1 - y0)) pts.push([x0, y]);
  for (let x = x0; x !== x1; x += Math.sign(x1 - x0)) pts.push([x, y1]);
  pts.forEach(([px, py], i) => { if ((i + off) % 4 < 2) { R(c, px, py + 1, 1, 1, P.o); R(c, px, py, 1, 1, col); } });
  const o = !state.reduce && blink(t, 300) ? col : P.white;
  R(c, x1 - 4, y1 - 4, 9, 1, o); R(c, x1 - 4, y1 + 4, 9, 1, o); R(c, x1 - 4, y1 - 4, 1, 9, o); R(c, x1 + 4, y1 - 4, 1, 9, o);
}

/* Before anything is drawn: a whole-feature barge carries its subtasks'
 * crates, so they are not drawn loose (scene.hid). */
registerLayer({ z: -5, draw(c, scene) {
  const barges = scene.barges = bargesFor(scene.v, scene.lay);
  scene.hid = new Set(barges.flatMap(b => b.issues));
} });
/* Over everything: barges, fog, trails and the banners. */
registerLayer({ z: 140, draw(c, { v, lay, barges }, t) {
  if (!lay.banners.length && !v.pipeline.stops.some(s => s.source === 'unknown')) return;
  barges.forEach(b => drawBarge(c, b, t));
  v.pipeline.stops.forEach((s, i) => { if (s.source === 'unknown') fogStop(c, v, i, lay, t); });
  const fade = (state.scenes[v.project] || {}).fade || {};
  lay.banners.forEach(g => {
    const on = isFollowed(v, g.f), a = on ? 1 : (fade[g.f.number] ?? 1);
    if (on) counted(g.f).forEach(s => { const w = whereIs(s.number, v, lay, barges); if (w) drawTrail(c, g.x, g.y + 25, w.x, w.y, g.col, t); });
    c.globalAlpha = a; drawBanner(c, g, on, t); c.globalAlpha = 1;
    tap(g.x - 3, g.y - 9, 20, 36, { kind: 'banner', f: g.f.number });
  });
} });

/* A flag follows its feature (label and strip shown) or stops following it;
 * the strip's Details button opens the feature. A barge opens its feature. */
registerHit({ kind: 'banner', key: false, act(o, sc) { toggleFollow(sc.v.project, o.f); return true; } });
registerHit({ kind: 'barge', act(o, sc) { const v = sc.v, f = (v.features || []).find(f => f.number === o.f); if (f) showFeature(f, v); return !!f; } });

/* A live banner has done its job: it fades over the first hour after this
 * page saw it go live and is gone after LIVE_GONE, unless it is being
 * followed. (The server stops listing a feature once every subtask is live, #401.) */
const LIVE_FADE = 10 * 60e3, LIVE_DIM = 60 * 60e3, LIVE_GONE = 2 * 60 * 60e3;
export function fadeLive(sc, v) {
  const now = Date.now(), seen = {};
  sc.fade = {};
  v.features = (v.features || []).filter(f => {
    if (!f.live) return true;
    const since = seen[f.number] = sc.liveSince[f.number] ?? now, age = now - since;
    if (age > LIVE_GONE && !isFollowed(v, f)) return false;
    sc.fade[f.number] = age <= LIVE_FADE ? 1 : Math.max(0.35, 1 - 0.65 * (age - LIVE_FADE) / (LIVE_DIM - LIVE_FADE));
    return true;
  });
  sc.liveSince = seen;
}

/* ---------- Banner labels, strip, dialog ---------- */
const fbox = document.getElementById('fbanners'), fstrip = document.getElementById('fstrip');
const STAGE_LABEL = { dev: 'Dev', development: 'Dev', uat: 'UAT', qa: 'QA', staging: 'Staging', prod: 'Prod', production: 'Prod' };
const stageLabel = name => STAGE_LABEL[String(name).toLowerCase()] || String(name).charAt(0).toUpperCase() + String(name).slice(1);
const blockedWords = b => typeof b === 'string' && b ? b.replace(/-/g, ' ') : 'blocked';
function stuckWords(f) {
  const n = f.stuck[0], s = (f.subtasks || []).find(s => s.number === n);
  return `#${n} ${blockedWords(s && s.blocked)}`;
}
/* The furthest state a feature has reached, as its banner label reads it. */
function featureSummary(f) {
  if (f.live) return 'Live in prod';
  if ((f.stuck || []).length) return `Stuck · ${stuckWords(f)}`;
  const b = f.build || { landed: 0, total: counted(f).length }, stages = f.stages || [];
  if (b.landed < b.total) return `Build ${b.landed}/${b.total}`;
  const s = stages.find(s => s.unknown || s.reached < s.total) || stages[stages.length - 1];
  if (!s) return `Build ${b.landed}/${b.total}`;
  return s.unknown ? `${stageLabel(s.name)}: unknown` : `${stageLabel(s.name)} ${s.reached}/${s.total}`;
}
/* One button per banner, kept across polls so keyboard focus survives a refresh. */
const fbuttons = new Map();
let peeked = null;
function peek(b) {
  if (peeked === b) return;
  if (peeked) peeked.classList.remove('peek');
  peeked = b || null;
  if (peeked) peeked.classList.add('peek');
}
/* Hovering a flag shows its feature's label while the pointer is on it. */
cv.addEventListener('mousemove', e => {
  const h = objAt(e);
  peek(h && h.o.kind === 'banner' ? fbuttons.get(`${h.sc.v.project}#${h.o.f}`) : null);
});
cv.addEventListener('mouseleave', () => peek(null));
function placeBanners() {
  const vp = document.getElementById('viewport'), r = cv.getBoundingClientRect(), vr = vp.getBoundingClientRect();
  const s = shownScale(), keep = new Set();
  let strip = null;
  if (!cv.hidden) shownScenes().forEach(([sc, ox, oy]) => sc.L.banners.forEach((g, k) => {
    const f = g.f, key = `${sc.v.project}#${f.number}`, on = isFollowed(sc.v, f), col = g.col;
    if (on) strip = { sc, g };
    keep.add(key);
    let b = fbuttons.get(key);
    if (!b) {
      b = document.createElement('button');
      b.addEventListener('click', () => toggleFollow(b.project, b.number));
      fbuttons.set(key, b); fbox.appendChild(b);
    }
    b.project = sc.v.project; b.number = f.number;
    b.className = 'fb' + (f.live ? ' live' : '') + ((f.stuck || []).length ? ' stuck' : '') + (b === peeked ? ' peek' : '');
    b.style.setProperty('--fc', col);
    b.setAttribute('aria-pressed', String(on));
    const sum = featureSummary(f);
    b.innerHTML = `#${f.number} ${esc(f.title)}<small>${esc(sum)}</small>`;
    b.setAttribute('aria-label', `Feature #${f.number} ${f.title}: ${sum}. ${on ? 'Following; press to stop' : 'Press to follow'}.`);
    b.style.setProperty('--fade', on ? 1 : (sc.fade[f.number] ?? 1));
    // Labels sit under their banners; when the map is drawn small they alternate rows.
    const lift = 30 * s < 120 && k % 2 ? 36 : 0;
    b.style.left = (r.left - vr.left + vp.scrollLeft + (ox + g.x + 7) * s) + 'px';
    b.style.top = (r.top - vr.top + vp.scrollTop + (oy + g.y + 28) * s + lift) + 'px';
  }));
  for (const [key, b] of fbuttons) if (!keep.has(key)) { if (b === peeked) peeked = null; b.remove(); fbuttons.delete(key); }
  renderStrip(strip);
}
registerOverlay(placeBanners, 10);
function renderStrip(on) {
  fstrip.hidden = !on;
  if (!on) return;
  const { sc, g } = on, f = g.f, b = f.build || { landed: 0, total: counted(f).length };
  fstrip.style.setProperty('--fc', g.col);
  const chip = (cls, text) => `<span class="chip ${cls}">${esc(text)}</span>`;
  const build = (f.stuck || []).length ? chip('stuck', `Build ${b.landed}/${b.total}: ${stuckWords(f)}`)
    : chip(b.landed === b.total ? 'done' : b.landed ? 'part' : '', `Build ${b.landed}/${b.total}`);
  const stages = (f.stages || []).map(s => s.unknown ? chip('unknown', `${stageLabel(s.name)}: unknown`)
    : chip(s.reached === s.total ? 'done' : s.reached ? 'part' : '', `${stageLabel(s.name)} ${s.reached}/${s.total}`));
  document.getElementById('fs-title').textContent = `Following #${f.number} ${f.title}`;
  const chips = [chip('done', 'Spec ✓'), build, ...stages].join('<span class="sep">›</span>') + (f.live ? ' ' + chip('live', '★ Live in prod') : '');
  // Only touch the live region when the reading changes, so a poll is not re-announced.
  const box = document.getElementById('fs-chips');
  if (box.innerHTML !== chips) box.innerHTML = chips;
  document.getElementById('fs-mode').textContent = f.promotion === 'feature' ? 'Promotes as a whole feature' : 'Promotes per subtask';
  fstrip.feature = { project: sc.v.project, number: f.number };
}
function toggleFollow(project, number) {
  followed = followed && followed.project === project && followed.number === number ? null : { project, number };
  placeOverlays();
  if (state.reduce) draw(0);
}
document.getElementById('fs-stop').addEventListener('click', () => { followed = null; placeOverlays(); if (state.reduce) draw(0); });
document.getElementById('fs-details').addEventListener('click', () => {
  const k = fstrip.feature, sc = k && state.scenes[k.project], f = sc && sc.v && (sc.v.features || []).find(f => f.number === k.number);
  if (f) showFeature(f, sc.v);
});
function whereWords(s, v, f) {
  const extra = s.blocked ? ` · ${blockedWords(s.blocked)}` : '', stops = v.pipeline.stops;
  switch (s.where) {
    case 'house': {
      const h = v.houses.find(h => h.issue === s.number);
      if (!h) return 'being built' + extra;
      return h.status === 'blocked' ? `blocked · ${h.role} is waiting on you` : `being built by ${h.role}${extra}`;
    }
    case 'backlog': return `waiting at the ${state.T.labels.docks.toLowerCase()}${extra}`;
    case 'board': return `on the ${state.T.labels.board.toLowerCase()}${extra}`;
    case 'quay': return f.promotion === 'feature' ? `landed · waiting at the ${state.T.labels.quay.toLowerCase()} for the rest` : `landed · waiting for ${stops.length ? stageLabel(stops[0].name) : 'release'}`;
    case 'stop': {
      const st = stops.find(x => x.name === s.stop);
      return st && st.source === 'unknown' ? `${stageLabel(s.stop)}: unknown` : `in ${stageLabel(s.stop || '?')}`;
    }
    case 'closed': return 'closed';
    default: return s.where || '';
  }
}
export function showFeature(f, v) {
  const subs = f.subtasks || [], url = n => (f.url || '').replace(/\/issues\/\d+$/, '/issues/' + n);
  const issueLink = (n, cls) => f.url ? `<a href="${esc(url(n))}" target="_blank" rel="noopener"${cls ? ` class="${cls}"` : ''}>#${n}</a>` : `<span${cls ? ` class="${cls}"` : ''}>#${n}</span>`;
  document.getElementById('d-title').textContent = `#${f.number} ${f.title} · feature`;
  document.getElementById('d-line').textContent = `${featureSummary(f)} · ${counted(f).length} subtasks counted`;
  document.getElementById('d-meta').innerHTML =
    `<ul class="subs">${subs.map(s => s.dropped
        ? `<li>${issueLink(s.number, 'drop')}<span class="drop">${esc(s.title || '')}</span><span class="w">closed as not planned · not counted</span></li>`
        : `<li>${issueLink(s.number)}<span>${esc(s.title || '')}</span><span class="w">${esc(whereWords(s, v, f))}</span></li>`).join('')}</ul>
      <dl><dt>Subtasks from</dt><dd>${f.linked_by === 'body' ? `"Sub-task of #${f.number}" in each body` : 'GitHub sub-issues'}</dd>
      ${v.pipeline.stops.map(s => `<dt>${esc(stageLabel(s.name))}</dt><dd>${esc(sourceWords(s))}</dd>`).join('')}
      <dt>Promotion</dt><dd>${f.promotion === 'feature' ? 'as a whole feature' : 'per subtask'}</dd>
      <dt>Live means</dt><dd>every counted subtask's code runs in ${esc(v.pipeline.stops.length ? stageLabel(v.pipeline.stops[v.pipeline.stops.length - 1].name) : 'prod')}</dd></dl>`;
  ask({});
  links([
    f.spec_url ? { label: `Spec: ${f.spec_url.split('/').pop()}`, url: f.spec_url } : f.url && { label: `Spec: #${f.number} issue body`, url: f.url },
    f.url && { label: `GitHub issue #${f.number}`, url: f.url }
  ].filter(Boolean));
  dialog.hidden = false;
  document.getElementById('close').focus();
}
