/* ---------- Opening and rendering the "Needs you" list ----------
 * `#needs` or `#<project>&needs` opens every item waiting on the user
 * across the villages the client knows about (#580, for #570), grouped by
 * village. A row opens that item's own dialog over the list (dialogs.js's
 * Back slot, #573, so the list stays open behind it); `&open=<key>` opens
 * one already, over the list. Not on the screensaver, which has no
 * dialogs. needswords.js has the model, with no DOM, so node tests it. */
import { state } from './state.js';
import { explain, openInfo, pushForDetail } from './dialogs.js';
import { esc } from './panels.js';
import { needsGroups, needsListInfo, needsOpenKey, needsProject, needsWanted } from './needswords.js';
import { registerDialogPart } from './registry.js';

let needsLinkOpened = '';
window.addEventListener('village:polled', e => {
  if (!e.detail || !e.detail.ok || state.screensaver || needsLinkOpened === location.hash || !needsWanted(location.hash)) return;
  const scope = needsProject(location.hash);
  const villages = Object.values(state.scenes).map(sc => sc.v).filter(Boolean);
  const groups = needsGroups(scope ? villages.filter(v => v.project === scope) : villages);
  needsLinkOpened = location.hash;
  openInfo(needsListInfo(groups, scope));
  const key = needsOpenKey(location.hash);
  const item = key && groups.flatMap(g => g.items).find(i => i.key === key);
  if (item) openItem(item);
});

/* Opens `item`'s own dialog (the same one its kind's alarm, gate, build or
 * env dialog already is), keeping the list as the Back slot. */
export function openItem(item) {
  const sc = state.scenes[item.project];
  if (!sc || !sc.v) return;
  const info = explain(item.target, sc, state.T);
  if (!info) return;
  pushForDetail();
  openInfo(info, true);
}

registerDialogPart(info => {
  const box = document.getElementById('d-meta');
  if (!info.needsGroups) return;
  box.innerHTML = info.needsGroups.length
    ? `<ul>${info.needsGroups.map(g => `<li class="need-group">${esc(g.project)}</li>` +
        g.items.map(it => `<li data-need-key="${esc(it.key)}">${esc(it.title)}${it.detail ? ' · ' + esc(it.detail) : ''}</li>`).join('')).join('')}</ul>`
    : '';
  box.querySelectorAll('li[data-need-key]').forEach(li => li.addEventListener('click', () => {
    const item = info.needsGroups.flatMap(g => g.items).find(i => i.key === li.dataset.needKey);
    if (item) openItem(item);
  }));
}, 41);
