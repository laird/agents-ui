/* ---------- What each object is ----------
 * One dialog per kind of thing on the map: what it is in plain words, and
 * links to what is behind it. Each kind registers its own (registerHit);
 * explain() and act() only dispatch on the kind, and the hit test only finds
 * what paint() recorded. Links come from the report (house `links`, issue
 * `url`s); the repo's own pages (commits, deployments, releases) only when a
 * real issue URL shows which GitHub repo this is, never a guess. */
import { state } from './state.js';
import { DIALOG_PARTS, HITS, hitAt, registerOverlay } from './registry.js';
import { cv, shownScale, shownScenes, themeAt } from './layout.js';
import { toScene } from './detail.js';
import { esc } from './panels.js';
import { ATTACH_WAIT, HANDLER_KEY, SESSION, attachPlan } from './herdrlink.js';
import { IOS_SSH_KEY, choiceOf, iosPlan, isIOS, parseChoice } from './iossh.js';

export const dialog = document.getElementById('dialog');
document.getElementById('close').addEventListener('click', () => { dialog.hidden = true; });
/* Drill in: each link opens the tool behind the picture (the agent's session,
 * its issue, its workflow) in a new tab. */
export function links(list) {
  const box = document.getElementById('d-links');
  list = list || [];
  box.innerHTML = list.map((l, i) => (l.attach
    ? `<a href="${esc(l.url)}" data-attach="${i}">${esc(l.label)}</a>`
    : `<a href="${esc(l.url)}" target="_blank" rel="noopener">${esc(l.label)} ↗</a>`)).join('');
  list.forEach((l, i) => { if (l.attach) box.querySelector(`[data-attach="${i}"]`).addEventListener('click', e => attachClick(e, l.attach, box)); });
}
/* "Open in herdr" (herdrlink.js, #575): hand the agents-ui:// link to the
 * handler `agents-tui open` registers. With none, the page keeps its focus:
 * after ATTACH_WAIT it opens the web terminal instead and shows the
 * commands to attach by hand. Which happened is remembered. */
const attachKnown = () => { try { return localStorage.getItem(HANDLER_KEY); } catch (e) { return null; } };
const attachMemo = x => { try { if (x) localStorage.setItem(HANDLER_KEY, x); else localStorage.removeItem(HANDLER_KEY); } catch (e) { /* private mode */ } };
function attachClick(e, a, box) {
  if (isIOS(navigator)) { e.preventDefault(); iosAttach(a, box, true); return; }
  const plan = attachPlan(attachKnown());
  if (plan === 'open') return;
  if (plan === 'fallback') { e.preventDefault(); attachFallback(a, box, true); return; }
  let answered = false;
  const on = () => { answered = true; };
  window.addEventListener('blur', on); document.addEventListener('visibilitychange', on);
  setTimeout(() => {
    window.removeEventListener('blur', on); document.removeEventListener('visibilitychange', on);
    attachMemo(answered ? 'yes' : 'none');
    if (!answered) attachFallback(a, box, false);
  }, ATTACH_WAIT);
}
function attachFallback(a, box, now) {
  if (a.web) window.open(a.web, '_blank', 'noopener');
  const el = attachBox(box);
  el.innerHTML = `<p class="dim">${now ? 'No agents-ui:// handler answered last time' : 'No agents-ui:// handler answered'}${a.web ? ', so this opens in the browser' : ''}. To attach from a terminal:</p>`
    + cmdRows(a.cmds)
    + '<button type="button" class="retry">Try the handler again</button>';
  wireCopy(el);
  el.querySelector('.retry').addEventListener('click', () => { attachMemo(null); el.remove(); });
}
const attachBox = box => box.querySelector('.attach-cmds') || box.appendChild(Object.assign(document.createElement('div'), { className: 'attach-cmds' }));
const cmdRows = cmds => cmds.map(c => `<div class="cmd"><code>${esc(c)}</code><button type="button" data-copy="${esc(c)}">Copy</button></div>`).join('');
function wireCopy(el) {
  el.querySelectorAll('[data-copy]').forEach(b => b.addEventListener('click', () => {
    const done = () => { b.textContent = 'Copied'; };
    if (navigator.clipboard) navigator.clipboard.writeText(b.dataset.copy).then(done, () => { b.textContent = 'Select and copy'; });
  }));
}
/* On iOS (iossh.js, #591) there is no agents-ui:// handler: the pane opens
 * in the SSH app chosen here (Blink runs the command, Termius only opens the
 * host), or in the browser until one is chosen. The choice is remembered. */
const iosKnown = () => { try { return parseChoice(localStorage.getItem(IOS_SSH_KEY)); } catch (e) { return null; } };
const iosMemo = x => { try { if (x) localStorage.setItem(IOS_SSH_KEY, x); else localStorage.removeItem(IOS_SSH_KEY); return true; } catch (e) { return false; } };
function iosAttach(a, box, go) {
  const p = iosPlan(iosKnown(), a), el = attachBox(box);
  if (go) { if (p.kind === 'app') window.location.href = p.url; else if (a.web) window.open(a.web, '_blank', 'noopener'); }
  el.innerHTML = (p.kind === 'app'
    ? `<p class="dim">${go ? 'Opening' : 'Opens'} in ${esc(p.app)}${p.app === 'Termius' ? ', which can only open the host: paste this there' : ', which runs'}:</p>${cmdRows([p.cmd])}`
      + '<button type="button" class="ios-change">Change SSH app</button>'
    : `<p class="dim">${go && a.web ? 'This opens in the browser. ' : ''}To open panes in an SSH app instead, choose one (Blink's key is in its Settings &gt; x-callback-url):</p>${cmdRows([p.cmd])}`
      + '<div class="cmd"><input class="ios-key" type="text" autocomplete="off" autocapitalize="off" spellcheck="false" placeholder="Blink key">'
      + '<button type="button" data-app="blink">Use Blink</button><button type="button" data-app="termius">Use Termius</button></div>'
      + '<p class="dim ios-msg" role="status"></p>');
  wireCopy(el);
  const change = el.querySelector('.ios-change');
  if (change) change.addEventListener('click', () => { iosMemo(null); iosAttach(a, box, false); });
  el.querySelectorAll('[data-app]').forEach(b => b.addEventListener('click', () => {
    const c = choiceOf(b.dataset.app, el.querySelector('.ios-key').value);
    if (!c) { el.querySelector('.ios-msg').textContent = 'Enter the key from Blink\'s Settings > x-callback-url.'; return; }
    if (!iosMemo(c)) { el.querySelector('.ios-msg').textContent = 'This browser would not save the choice (site data is blocked).'; return; }
    iosAttach(a, box, true);
  }));
}
/* A structured question the agent is waiting on (a numbered menu or y/n):
 * one button per option. The server checks the same question is still on
 * screen before it types the answer; anything open-ended goes through the
 * agent's session link instead. */
export function ask(house) {
  const box = document.getElementById('d-q'), q = house.question;
  if (!q || !house.answer_url) { box.innerHTML = ''; return; }
  box.innerHTML = `<p class="q">${esc(q.prompt)}</p><div class="opts">${q.options.map((o, i) =>
      `<button data-opt="${i}" data-key="${esc(o.key || '')}">${q.kind === 'menu' ? esc(o.key) + '. ' : ''}${esc(o.label)}</button>`).join('')}</div><p class="dim" id="d-q-msg" role="status"></p>`;
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
export const sessionOf = h => (h.links || []).find(l => SESSION.test(l.url));
export const isSha = s => /^[0-9a-f]{7,40}$/i.test(String(s || ''));
/* A house's name: its worktree's folder, less the project prefix (shop-wt-2 → wt-2). */
export function houseName(v, h) {
  const id = String(h.id || '');
  if (!id.includes('/')) return id || h.role;
  const base = id.replace(/\/+$/, '').split('/').pop();
  const short = base.startsWith(v.project + '-') ? base.slice(v.project.length + 1) : base;
  return short || h.role;
}

/* What an object on the map is: { title, line, meta (list items), items (issues), links, house, project }. */
export function explain(o, sc, theme) {
  const hit = HITS[o.kind];
  const info = hit && hit.dialog ? hit.dialog(o, sc, theme) : null;
  // Which village it is in, for the parts that ask the server about it.
  if (info && !info.project && sc && sc.v) info.project = sc.v.project;
  return info;
}
/* One level of "back" (#573): a list dialog an issue's own detail (#564)
 * was opened from, so Back/Esc returns to it, scrolled where it was, and
 * Next/Previous can move through it without losing this slot. A plain
 * `openInfo` call (any map click or hover) clears it, since that is always
 * a fresh root, not a continuation; only `pushForDetail` followed by a
 * `keepHistory` call (issuedialog.js) keeps it. */
let current = null, previous = null;
const backBtn = document.getElementById('d-back');
backBtn.addEventListener('click', () => goBack());
/* The list `info` an issue detail is about to be opened from, with where
 * its (scrollable) `<ul>` was scrolled, so it looks the same on Back. */
export function pushForDetail() {
  previous = { info: current, scroll: (document.querySelector('#d-meta ul') || {}).scrollTop || 0 };
}
/* `info.items` of the list a detail view came from, for Next/Previous
 * (issuedialog.js) — null outside of one (a single-subject's own issue, or
 * no detail open at all). */
export function previousItems() {
  return (previous && previous.info.items) || null;
}
export function goBack() {
  if (!previous) { dialog.hidden = true; return; }
  const p = previous;
  previous = null;
  openInfo(p.info);
  const ul = document.querySelector('#d-meta ul');
  if (ul) ul.scrollTop = p.scroll;
}
/* The dialog for one object, or a list such as the board. `keepHistory`
 * (issuedialog.js only) keeps the Back slot `pushForDetail` just set,
 * instead of clearing it as a fresh root normally does. */
export function openInfo(info, keepHistory) {
  if (!keepHistory) previous = null;
  current = info;
  backBtn.hidden = !previous;
  document.getElementById('d-title').textContent = info.title;
  document.getElementById('d-line').textContent = info.line || '';
  // Each fact on its own line, then the list: joined, they ran together
  // ("Runs: no Deployment and no branchNothing here.").
  const meta = (info.meta || []).filter(Boolean).map(m => `<span class="fact">${esc(m)}</span>`).join('');
  // An item with a URL opens the issue dialog in the village (issuedialog.js,
  // #564); its GitHub link stays, now secondary. One with none (a runtime
  // village's workstream, not a GitHub issue) stays plain text, as before.
  // The whole row is the click target (#573), at least 32px tall on touch
  // (village.css); the GitHub link keeps its own target inside it.
  const items = info.items && (info.items.length
    ? `<ul>${info.items.map(i => `<li data-n="${esc(i.number)}">${i.url ? `<button class="issue-open" type="button" data-n="${esc(i.number)}">#${i.number}</button>` : '#' + i.number} ${esc(i.title || '')}${i.label ? ' · ' + esc(i.label) : ''}${i.priority ? ' · ' + esc(i.priority) : ''}${(i.waits_on || []).length ? ' · waits on ' + i.waits_on.map(d => '#' + d).join(', ') : ''}${i.url ? ` <a class="sub-link" href="${esc(i.url)}" target="_blank" rel="noopener">on GitHub ↗</a>` : ''}</li>`).join('')}</ul>`
    : '<span class="fact">Nothing here.</span>');
  document.getElementById('d-meta').innerHTML = [meta, items].filter(Boolean).join('');
  ask(info.house || {});
  DIALOG_PARTS.forEach(part => part(info));
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
  const { view, scenes } = state;
  if (!view) return null;
  const r = cv.getBoundingClientRect();
  const x = toScene((e.clientX - r.left) / r.width * cv.width), y = toScene((e.clientY - r.top) / r.height * cv.height);
  if (!view.world) { const sc = scenes[view.village], o = sc && sc.v && hitAt(sc, x, y); return o ? { o, sc, theme: state.T } : null; }
  // A world: the scene on top at that point (the platform over its slot) answers.
  const at = shownScenes().reverse().find(([sc, ox, oy]) => x >= ox && y >= oy && x < ox + sc.L.W && y < oy + sc.L.H);
  if (!at) return null;
  const [sc, ox, oy] = at, o = hitAt(sc, x - ox, y - oy);
  return o ? { o, sc, theme: themeAt(sc) } : null;
}
cv.addEventListener('click', e => {
  if (!state.view || state.view.overview || state.view.valley) return; // overview.js and valley.js answer their clicks
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
document.addEventListener('keydown', e => { if (e.key === 'Escape') goBack(); });

/* One keyboard target per object drawn in the last frame, kept across polls
 * (by what it is, not where) so focus survives a refresh. Kinds registered
 * with `key: false` (walkers, the banners with their own buttons) are left
 * to the mouse. */
export const kbox = document.getElementById('hitkeys'), kbuttons = new Map();
const keyOf = o => [o.kind, o.i ?? '', o.n ?? '', o.at ?? '', o.who ?? '', o.house ?? '', o.id ?? '', o.f ?? ''].join(':');
export function placeKeys() {
  const vp = document.getElementById('viewport'), r = cv.getBoundingClientRect(), vr = vp.getBoundingClientRect();
  const s = shownScale(), ox0 = r.left - vr.left + vp.scrollLeft, oy0 = r.top - vr.top + vp.scrollTop, keep = new Set();
  if (!cv.hidden) shownScenes().forEach(([sc, ox, oy]) => {
    const theme = themeAt(sc);
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
      const label = [info.title, info.line].filter(Boolean).join(': ');
      b.setAttribute('aria-label', label);
      b.title = label;
      // A small sprite still gets a real keyboard target: at least 24 CSS px
      // (#568), centred over its drawn hit rect rather than just its corner.
      const w = Math.max(24, h.w * s), ht = Math.max(24, h.h * s);
      Object.assign(b.style, {
        left: ox0 + (ox + h.x) * s - (w - h.w * s) / 2 + 'px',
        top: oy0 + (oy + h.y) * s - (ht - h.h * s) / 2 + 'px',
        width: w + 'px', height: ht + 'px',
      });
    });
  });
  for (const [key, b] of kbuttons) if (!keep.has(key)) { b.remove(); kbuttons.delete(key); }
}
registerOverlay(placeKeys, 30);
