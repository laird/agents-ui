/* ---------- Start swarm from the empty state (#578) ----------
 * When /api/village shows no villages at all, the page has no repo to be
 * "in" — data.js's poll() calls fillEmpty() on `#empty` instead of just
 * setting its text. A writable server gets a small form (repo path,
 * agent type, workers) that posts to the dashboard's own launch route; a
 * read-only one keeps the plain message. Rendered once and left alone
 * while villages stay empty, so the 3s poll never wipes out what someone
 * is mid-typing (ui.html's launch panel has the same problem, solved the
 * same way: don't touch the DOM while it already has the form up). */
import { esc } from './panels.js';
import { askServer } from './controls.js';
import { canWrite } from './readonly.js';
import { AGENT_TYPES_URL, DEFAULT_WORKERS, LAUNCH_URL, LOOKING, READ_ONLY, REPOS_URL, launchAnswer, launchRequest } from './startswarmwords.js';
import { DEFAULT_AGENT_TYPES } from './swarmwords.js';

async function agentTypeOptions() {
  const r = await askServer(AGENT_TYPES_URL);
  return (r && r.body && Array.isArray(r.body.agent_types) && r.body.agent_types) || DEFAULT_AGENT_TYPES;
}

async function repos() {
  const r = await askServer(REPOS_URL);
  return (r && r.body && Array.isArray(r.body.repos) && r.body.repos) || [];
}

function form(box, types, repoList) {
  const options = types.map(t => `<option${t === types[0] ? ' selected' : ''}>${esc(t)}</option>`).join('');
  const datalist = repoList.map(r => `<option value="${esc(r.path)}">${esc(r.name)} — ${esc(r.path)}</option>`).join('');
  box.innerHTML = `<p>${esc(LOOKING)}</p>` +
    `<form class="start-swarm"><label>Repository path <input name="repo" list="start-swarm-repos" placeholder="/path/to/your/repo" autocomplete="off" required></label>` +
    `<datalist id="start-swarm-repos">${datalist}</datalist>` +
    `<div class="row"><label>Agent type <select name="type">${options}</select></label>` +
    `<label>Workers <input name="workers" type="number" min="1" max="20" value="${DEFAULT_WORKERS}"></label>` +
    `<button type="submit">Start swarm</button></div><p class="dim" role="status"></p></form>`;
  const el = box.querySelector('form'), msg = el.querySelector('[role=status]'), send = el.querySelector('button');
  const field = n => el.elements.namedItem(n);
  el.addEventListener('submit', async e => {
    e.preventDefault();
    const req = launchRequest(field('repo').value, field('type').value, field('workers').value);
    if (req.error) { msg.textContent = req.error; return; }
    send.disabled = true; msg.textContent = 'Starting…';
    const r = await askServer(LAUNCH_URL, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(req.body) });
    const said = r ? launchAnswer(r.status, r.body) : { ok: false, words: "Can't reach agents-tui." };
    msg.textContent = said.words;
    send.disabled = false;
    if (said.ok) el.querySelectorAll('input, select, button').forEach(x => { x.disabled = true; });
  });
}

/* `started` makes a second call for the same empty spell a no-op, so a poll
 * firing again before (or after) the first build never touches the box
 * again. `token` is bumped by resetEmpty() to invalidate a build still in
 * flight from a spell that already ended — villages appearing and
 * disappearing within one poll's fetches must never let a stale build land
 * on top of a fresh one (or on top of what someone is mid-typing). */
let emptyStarted = false;
let emptyToken = 0;

export async function fillEmpty(box) {
  if (emptyStarted) return;
  emptyStarted = true;
  const mine = ++emptyToken;
  if (!(await canWrite())) {
    if (mine === emptyToken) box.textContent = READ_ONLY;
    return;
  }
  const [types, repoList] = await Promise.all([agentTypeOptions(), repos()]);
  if (mine !== emptyToken) return;
  form(box, types, repoList);
}

/* Villages came back: the next empty spell gets a fresh build (a
 * different read-only answer, a freshly scanned repo list), and any build
 * still in flight from this one is discarded rather than landing late. */
export function resetEmpty() {
  emptyStarted = false;
  emptyToken++;
}
