/* ---------- Filing an issue from the village (#490) ----------
 * "New issue" in the top bar (beside the repo link) and on the board's
 * dialog opens a small form: title, body, type and priority (P3 unless
 * changed). It posts to the server (newissue.rs), which runs the tracker's
 * create in the swarm's repo. No `proposal` label, so the issue goes straight
 * into the queue, and its crate joins the backlog on the next poll. A
 * read-only server answers 403, and the form links to GitHub instead. */
import { state } from './state.js';
import { registerDialogPart, registerOverlay } from './registry.js';
import { esc } from './panels.js';
import { openInfo, repoUrl } from './dialogs.js';
import { ISSUE_PRIORITIES, ISSUE_TYPES, filedWords, issueRequest, newIssueUrl } from './issueform.js';

/* The village "New issue" files into: the shown development village, else none. */
function filingVillage() {
  const view = state.view, sc = view && state.scenes[view.world ? (state.worlds.find(w => w.name === view.world) || {}).dev : view.village];
  return sc && sc.v && sc.v.plane !== 'runtime' ? sc.v : null;
}

const topButton = document.getElementById('new-issue');
topButton.addEventListener('click', () => {
  const v = filingVillage();
  if (v) openInfo({ title: 'New issue', line: `Filed in ${v.project}, straight into the queue`, newIssue: v });
});
registerOverlay(() => { topButton.hidden = !filingVillage(); });

const options = (list, chosen) => list.map(x => `<option${x === chosen ? ' selected' : ''}>${x}</option>`).join('');

/* The form, in `box`, filing into village `v`. */
function issueForm(box, v) {
  box.innerHTML = `<form class="new-issue"><label>Title <input name="title" required></label>` +
    '<label>Body <textarea name="body" rows="4"></textarea></label>' +
    `<div class="row"><label>Type <select name="type">${options(ISSUE_TYPES, 'enhancement')}</select></label>` +
    `<label>Priority <select name="priority">${options(ISSUE_PRIORITIES, 'P3')}</select></label>` +
    '<button type="submit">File it</button></div><p class="dim" role="status"></p></form>';
  const form = box.querySelector('form'), msg = form.querySelector('[role=status]'), send = form.querySelector('button');
  // By name through `elements`: `form.title` is the form's own title attribute.
  const field = n => form.elements.namedItem(n);
  // After the frame: opened from the top bar, the dialog is shown only once the parts have run.
  requestAnimationFrame(() => field('title').focus());
  form.addEventListener('submit', async e => {
    e.preventDefault();
    const req = issueRequest({ title: field('title').value, body: field('body').value, type: field('type').value, priority: field('priority').value });
    if (req.error) { msg.textContent = req.error; return; }
    send.disabled = true; msg.textContent = 'Filing…';
    let said;
    try {
      const res = await fetch(newIssueUrl(v.project), { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(req) });
      said = { ...filedWords(res.status, await res.json().catch(() => ({})), repoUrl(v)), ok: res.ok };
    } catch (err) {
      said = { words: "Can't reach agents-tui." };
    }
    msg.innerHTML = esc(said.words) + (said.url ? ` <a href="${esc(said.url)}" target="_blank" rel="noopener">${said.ok ? 'Open it' : 'New issue on GitHub'} ↗</a>` : '');
    if (said.ok) form.querySelectorAll('input, textarea, select, button').forEach(x => { x.disabled = true; });
    else send.disabled = false;
  });
}

registerDialogPart(info => {
  // A fresh box each time, so no listener is left from the last dialog.
  const old = document.getElementById('d-form');
  const box = old.cloneNode(false);
  old.replaceWith(box);
  if (info.newIssue) { issueForm(box, info.newIssue); return; }
  // The board's dialog offers it too, under its list.
  const v = info.decide && state.villages.find(x => x.project === info.decide);
  if (!v) return;
  box.innerHTML = '<div class="opts"><button data-new="1">New issue</button></div>';
  box.querySelector('[data-new]').addEventListener('click', () => issueForm(box, v));
});
