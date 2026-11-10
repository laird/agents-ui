/* ---------- What each object is ----------
 * One dialog per kind of thing on the map: what it is in plain words, and
 * links to what is behind it. Each kind registers its own (registerHit);
 * explain() and act() only dispatch on the kind, and the hit test only finds
 * what paint() recorded. Links come from the report (house `links`, issue
 * `url`s); the repo's own pages (commits, deployments, releases) only when a
 * real issue URL shows which GitHub repo this is, never a guess. */
import { state } from './state.js';
import { HITS, hitAt, registerOverlay } from './registry.js';
import { SIDE, cv, otherTheme, shownScenes, themeAt, worldGeom } from './layout.js';
import { esc } from './panels.js';

export const dialog = document.getElementById('dialog');
document.getElementById('close').addEventListener('click', () => { dialog.hidden = true; });
/* Drill in: each link opens the tool behind the picture (the agent's session,
 * its issue, its workflow) in a new tab. */
export function links(list) {
  document.getElementById('d-links').innerHTML = (list || [])
    .map(l => `<a href="${esc(l.url)}" target="_blank" rel="noopener">${esc(l.label)} ↗</a>`).join('');
}
/* A structured question the agent is waiting on (a numbered menu or y/n):
 * one button per option. The server checks the same question is still on
 * screen before it types the answer; anything open-ended goes through the
 * agent's session link instead. */
export function ask(house) {
  const box = document.getElementById('d-q'), q = house.question;
  if (!q || !house.answer_url) { box.innerHTML = ''; return; }
  box.innerHTML = `<p class="q">${esc(q.prompt)}</p><div class="opts">${q.options.map((o, i) =>
      `<button data-opt="${i}">${q.kind === 'menu' ? esc(o.key) + '. ' : ''}${esc(o.label)}</button>`).join('')}</div><p class="dim" id="d-q-msg" role="status"></p>`;
  const buttons = [...box.querySelectorAll('[data-opt]')], msg = document.getElementById('d-q-msg');
  const busy = on => buttons.forEach(b => { b.disabled = on; });
  buttons.forEach(b => b.addEventListener('click', async () => {
    busy(true); msg.textContent = 'Sending…';
    try {
      const res = await fetch(house.answer_url, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ prompt: q.prompt, option: Number(b.dataset.opt) }) });
      const body = await res.json().catch(() => ({}));
      if (res.ok) { msg.textContent = `Answered: ${body.answered || b.textContent}`; setTimeout(() => { dialog.hidden = true; }, 1200); return; }
      msg.textContent = res.status === 403 ? 'This dashboard is read-only: answer in the agent session.' : (body.error || `Could not answer (${res.status})`);
    } catch (e) { msg.textContent = "Can't reach agents-tui."; }
    busy(false);
  }));
}

/* ---------- Words the dialogs share ---------- */
const ISSUE_URL = /^(https:\/\/github\.com\/[^/]+\/[^/]+)\/issues\/\d+$/;
export function repoUrl(v) {
  const urls = [...v.backlog.map(c => c.url), ...v.board.map(n => n.url), ...(v.features || []).map(f => f.url),
    ...[v.manager, ...v.houses].flatMap(h => (h.links || []).map(l => l.url))];
  for (const u of urls) { const m = ISSUE_URL.exec(u || ''); if (m) return m[1]; }
  return null;
}
export const link = (label, url) => (url ? { label, url } : null);
/* An issue's title, priority and URL from wherever the village knows it. */
export function issueOf(v, n) {
  const c = v.backlog.find(c => c.number === n) || v.board.find(b => b.number === n);
  const h = v.houses.find(h => h.issue === n);
  const sub = (v.features || []).flatMap(f => f.subtasks || []).find(s => s.number === n);
  const own = h && (h.links || []).find(l => new RegExp(`/issues/${n}$`).test(l.url));
  const repo = repoUrl(v);
  return {
    title: (c && c.title) || (h && h.issue_title) || (sub && sub.title) || (v.pipeline.titles || {})[n] || '',
    priority: (c && c.priority) || (h && h.priority) || '',
    url: (c && c.url) || (own && own.url) || (repo ? `${repo}/issues/${n}` : null)
  };
}
export const issueLabel = n => `GitHub issue #${n}`;
export const issueItems = (v, ns) => ns.map(n => ({ number: n, ...issueOf(v, n) }));
export const sessionOf = h => (h.links || []).find(l => /^\/#\/swarm\/[^/]+\/agent\//.test(l.url));
export const isSha = s => /^[0-9a-f]{7,40}$/i.test(String(s || ''));
/* A house's name: its worktree's folder, less the project prefix (shop-wt-2 → wt-2). */
export function houseName(v, h) {
  const id = String(h.id || '');
  if (!id.includes('/')) return id || h.role;
  const base = id.replace(/\/+$/, '').split('/').pop();
  const short = base.startsWith(v.project + '-') ? base.slice(v.project.length + 1) : base;
  return short || h.role;
}

/* What an object on the map is: { title, line, meta (list items), items (issues), links, house }. */
export function explain(o, sc, theme) {
  const hit = HITS[o.kind];
  return hit && hit.dialog ? hit.dialog(o, sc, theme) : null;
}
/* The dialog for one object, or a list such as the board. */
export function openInfo(info) {
  document.getElementById('d-title').textContent = info.title;
  document.getElementById('d-line').textContent = info.line || '';
  // Each fact on its own line, then the list: joined, they ran together
  // ("Runs: no Deployment and no branchNothing here.").
  const meta = (info.meta || []).filter(Boolean).map(m => `<span class="fact">${esc(m)}</span>`).join('');
  const items = info.items && (info.items.length
    ? `<ul>${info.items.map(i => `<li>${i.url ? `<a href="${esc(i.url)}" target="_blank" rel="noopener">#${i.number}</a>` : '#' + i.number} ${esc(i.title || '')}${i.label ? ' · ' + esc(i.label) : ''}${i.priority ? ' · ' + esc(i.priority) : ''}${(i.waits_on || []).length ? ' · waits on ' + i.waits_on.map(d => '#' + d).join(', ') : ''}</li>`).join('')}</ul>`
    : '<span class="fact">Nothing here.</span>');
  document.getElementById('d-meta').innerHTML = [meta, items].filter(Boolean).join('');
  ask(info.house || {});
  links((info.links || []).filter(Boolean));
  dialog.hidden = false;
}
/* Do what a click on `o` does; false when it does nothing (open ground). A
 * kind may act instead of opening its dialog (a flag follows its feature). */
export function act(o, sc, theme) {
  const hit = HITS[o.kind];
  if (hit && hit.act) return hit.act(o, sc, theme);
  const info = explain(o, sc, theme);
  if (info) openInfo(info);
  return !!info;
}
/* What is under a pointer event: the object, its scene and theme; or null. */
export function objAt(e) {
  const { view, scenes, worlds } = state;
  if (!view) return null;
  const r = cv.getBoundingClientRect();
  const x = (e.clientX - r.left) / r.width * cv.width, y = (e.clientY - r.top) / r.height * cv.height;
  if (!view.world) { const sc = scenes[view.village], o = sc && sc.v && hitAt(sc, x, y); return o ? { o, sc, theme: state.T } : null; }
  const w = worlds.find(x => x.name === view.world), Ps = w && scenes[w.platform], Ds = w && scenes[w.dev];
  if (!Ps || !Ds || !Ps.v || !Ds.v) return null;
  const g = worldGeom(Ps, Ds);
  const [sc, theme, top] = y < Ps.L.H ? [Ps, otherTheme(), 0] : y >= g.devTop ? [Ds, state.T, g.devTop] : [];
  const o = sc && hitAt(sc, x - SIDE, y - top);
  return o ? { o, sc, theme } : null;
}
cv.addEventListener('click', e => {
  if (!state.view) return;
  const h = objAt(e);
  if (h && act(h.o, h.sc, h.theme)) hovered = false;
  else groundHint(e);
});
/* Hovering with a mouse over a kind registered with `hover: true` opens its
 * dialog, as a click does, and it closes when the pointer leaves; a click
 * keeps it open, and a dialog a click opened is never replaced by a hover.
 * A short delay, restarted on each new object, keeps it from flickering
 * between neighbours. Touch has no hover: a tap opens it as before. */
let hovered = false, hoverKey = '', hoverTimer = 0;
function hoverTo(h) {
  const key = h ? `${h.sc.v.project}|${keyOf(h.o)}` : '';
  if (key === hoverKey) return;
  hoverKey = key; clearTimeout(hoverTimer);
  hoverTimer = setTimeout(() => {
    if (h) {
      if (!dialog.hidden && !hovered) return;
      const info = explain(h.o, h.sc, h.theme);
      if (info) { openInfo(info); hovered = true; }
    } else if (hovered) { dialog.hidden = true; hovered = false; }
  }, h ? 120 : 250);
}
cv.addEventListener('pointermove', e => {
  if (e.pointerType !== 'mouse' || !state.view) return;
  const h = objAt(e);
  hoverTo(h && (HITS[h.o.kind] || {}).hover ? h : null);
});
cv.addEventListener('pointerleave', () => hoverTo(null));
/* Open ground does nothing, so say what does, where the click was. */
const hintEl = document.getElementById('hint');
let hintTimer = 0;
function groundHint(e) {
  const vp = document.getElementById('viewport'), vr = vp.getBoundingClientRect();
  hintEl.hidden = false;
  const half = hintEl.offsetWidth / 2 + 4, y = e.clientY - vr.top;
  const x = Math.max(vp.scrollLeft + half, Math.min(vp.scrollLeft + vp.clientWidth - half, e.clientX - vr.left + vp.scrollLeft));
  hintEl.classList.toggle('below', y < 40);
  hintEl.style.left = x + 'px'; hintEl.style.top = y + vp.scrollTop + 'px';
  clearTimeout(hintTimer);
  hintTimer = setTimeout(() => { hintEl.hidden = true; }, 2500);
}
document.addEventListener('keydown', e => { if (e.key === 'Escape') dialog.hidden = true; });

/* One keyboard target per object drawn in the last frame, kept across polls
 * (by what it is, not where) so focus survives a refresh. Kinds registered
 * with `key: false` (walkers, the banners with their own buttons) are left
 * to the mouse. */
export const kbox = document.getElementById('hitkeys'), kbuttons = new Map();
const keyOf = o => [o.kind, o.i ?? '', o.n ?? '', o.at ?? '', o.who ?? '', o.house ?? '', o.id ?? '', o.f ?? ''].join(':');
export function placeKeys() {
  const vp = document.getElementById('viewport'), r = cv.getBoundingClientRect(), vr = vp.getBoundingClientRect();
  const s = r.width / cv.width, ox0 = r.left - vr.left + vp.scrollLeft, oy0 = r.top - vr.top + vp.scrollTop, keep = new Set();
  if (!cv.hidden) shownScenes().forEach(([sc, ox, oy]) => {
    const theme = themeAt(oy);
    (sc.hits || []).forEach(h => {
      if ((HITS[h.o.kind] || {}).key === false || h.o.at === 'transit') return;
      const key = `${sc.v.project}|${keyOf(h.o)}`;
      if (keep.has(key)) return;
      const info = explain(h.o, sc, theme);
      if (!info) return;
      keep.add(key);
      let b = kbuttons.get(key);
      if (!b) {
        b = document.createElement('button'); b.className = 'hk';
        b.addEventListener('click', () => { const sc2 = state.scenes[b.project]; if (sc2 && sc2.v) act(b.obj, sc2, b.theme); });
        kbuttons.set(key, b); kbox.appendChild(b);
      }
      b.project = sc.v.project; b.obj = h.o; b.theme = theme;
      b.setAttribute('aria-label', [info.title, info.line].filter(Boolean).join(': '));
      Object.assign(b.style, { left: ox0 + (ox + h.x) * s + 'px', top: oy0 + (oy + h.y) * s + 'px', width: h.w * s + 'px', height: h.h * s + 'px' });
    });
  });
  for (const [key, b] of kbuttons) if (!keep.has(key)) { b.remove(); kbuttons.delete(key); }
}
registerOverlay(placeKeys, 30);
