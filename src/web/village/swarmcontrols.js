/* ---------- Swarm controls on the castle (hall) dialog (#567) ----------
 * The hall's dialog gets what the plain dashboard's launch panel already
 * does (ui.html): Stop swarm (with Resume once it is stopped), Add worker
 * and Switch agent (for the *next* worker). These act on the whole swarm
 * (server.rs), not one house, so they are a separate box (`d-swarmctl`)
 * from the per-agent controls (controls.js). A read-only server shows
 * nothing, same check as controls.js (`canWrite`, readonly.js, #565, so the
 * two boxes never disagree). The words and routes are in swarmwords.js. */
import { registerDialogPart } from './registry.js';
import { esc } from './panels.js';
import { askServer } from './controls.js';
import { canWrite } from './readonly.js';
import { DEFAULT_AGENT_TYPES, resumeUrl, swarmAnswer, swarmConfirmWords, swarmUrl, workersUrl } from './swarmwords.js';

/* The swarm's own record from `/api/swarms` (not the village model, which
 * carries no agent_type): fetched fresh each time the dialog opens, so
 * Stop/Resume and the agent type select always match what is actually
 * running. */
async function findSwarm(project) {
  const r = await askServer('api/swarms');
  const list = (r && r.status === 200 && r.body && r.body.swarms) || [];
  return list.find(s => s.project_name === project) || null;
}

async function agentTypes() {
  const r = await askServer('api/agent-types');
  return (r && r.body && Array.isArray(r.body.agent_types) && r.body.agent_types) || DEFAULT_AGENT_TYPES;
}

/* The dialog this part last filled: a slow answer for an earlier one is dropped. */
let token = 0;

/* The route each action sends to: Stop and Switch agent act on the swarm
 * itself, Resume and Add worker on their own sub-paths. */
const ROUTE = { stop: swarmUrl, 'switch-agent': swarmUrl, resume: resumeUrl, 'add-worker': workersUrl };

async function send(box, msg, project, action, init) {
  box.querySelectorAll('button, select').forEach(x => { x.disabled = true; });
  msg.textContent = 'Sending…';
  const r = await askServer(ROUTE[action](project), init);
  msg.textContent = r ? swarmAnswer(r.status, r.body, action) : "Can't reach agents-tui.";
  box.querySelectorAll('button, select').forEach(x => { x.disabled = false; });
  return r;
}

function render(box, project, swarm, types, mine) {
  const stopped = !!(swarm && swarm.stopped);
  const type = types.includes(mine.type) ? mine.type : (swarm && swarm.agent_type) || types[0];
  mine.type = type;
  const selector = `<select data-type aria-label="Agent type for the next worker">${types.map(t =>
    `<option${t === type ? ' selected' : ''}>${esc(t)}</option>`).join('')}</select>`;
  box.innerHTML = stopped
    ? `<div class="opts"><button data-ask="resume">Resume</button></div><p class="dim" role="status"></p>`
    : `<div class="opts"><button data-ask="stop">Stop swarm</button>${selector}<button data-add="1">Add worker</button></div><p class="dim" role="status"></p>`;
  const msg = box.querySelector('[role=status]');
  box.querySelector('[data-type]')?.addEventListener('change', async e => {
    mine.type = e.target.value;
    await send(box, msg, project, 'switch-agent', { method: 'PATCH', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ agent_type: mine.type }) });
  });
  box.querySelector('[data-add]')?.addEventListener('click', async () => {
    await send(box, msg, project, 'add-worker', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ agent_type: mine.type }) });
  });
  box.querySelector('[data-ask]')?.addEventListener('click', e => {
    const action = e.target.dataset.ask;
    box.innerHTML = `<p class="q">${esc(swarmConfirmWords(action, project))}</p>` +
      `<div class="opts"><button data-go="${esc(action)}">${action === 'stop' ? 'Stop' : 'Resume'}</button><button data-no="1">Cancel</button></div><p class="dim" role="status"></p>`;
    box.querySelector('[data-no]').addEventListener('click', () => render(box, project, swarm, types, mine));
    box.querySelector('[data-go]').addEventListener('click', async () => {
      await send(box, box.querySelector('[role=status]'), project, action, { method: action === 'stop' ? 'DELETE' : 'POST' });
      box.querySelector('.opts')?.remove();
    });
  });
}

async function fillSwarmControls(box, project, t) {
  if (!(await canWrite()) || t !== token) return;
  const [swarm, types] = await Promise.all([findSwarm(project), agentTypes()]);
  if (t !== token || !swarm) return;
  render(box, project, swarm, types, {});
}

registerDialogPart(info => {
  // A fresh box each time, so no listener is left from the last dialog.
  const old = document.getElementById('d-swarmctl');
  const box = old.cloneNode(false);
  old.replaceWith(box);
  const t = ++token;
  if (info.swarmControls) fillSwarmControls(box, info.swarmControls.project, t);
}, 31);
