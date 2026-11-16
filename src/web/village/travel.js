/* ---------- An issue's travel log in its dialogs (#495) ----------
 * A crate's dialog and a house's (its claimed issue) get a "Travel log"
 * button, and every issue listed in a dialog (the quay, the stops, the
 * ship, the castle: landed work) a small "log" one. A click asks the server
 * for the log (travel.rs) and shows it in the dialog: each stage, how long
 * it took against its 14-day median, the slowest marked, the gate's
 * hand-backs, and a footer saying where the swarm's time goes. The words
 * are in travelwords.js. A runtime village (a platform's workloads) has no
 * issues to follow, so it gets no buttons. */
import { state } from './state.js';
import { registerDialogPart } from './registry.js';
import { esc } from './panels.js';
import { houseName } from './dialogs.js';
import { bounceWords, medianWords, rowWords, travelRows, travelUrl } from './travelwords.js';

/* The dialog this part last filled: a slow answer for an earlier one is dropped. */
let travelToken = 0;

async function showTravel(box, project, n) {
  // Each click is its own ask: a slower answer for an earlier click is dropped.
  const mine = ++travelToken;
  box.innerHTML = `<p class="dim" role="status">Reading the travel log of #${n}…</p>`;
  let log = null, status = 0;
  try {
    const res = await fetch(travelUrl(project, n));
    status = res.status;
    if (res.ok) log = await res.json();
  } catch { /* unreachable: said below */ }
  if (mine !== travelToken) return;
  if (!log) {
    box.innerHTML = `<p class="dim" role="status">${status ? `Couldn't read the travel log of #${n} from GitHub.` : "Can't reach agents-tui."}</p>`;
    return;
  }
  const v = (state.scenes[project] || {}).v || {};
  const g = v.github || {}, medians = g.stage_medians || {};
  const h = (v.houses || []).find(h => h.issue === n);
  const rows = travelRows(log, medians, Date.now(), h ? houseName(v, h) : null);
  const foot = medianWords(medians), gate = bounceWords(log.bounces);
  box.innerHTML = `<h3>Travel log · #${n}</h3><ol class="travel">${rows.map(r =>
    `<li class="${r.slow ? 'slow' : ''}${r.key === 'now' ? ' now' : ''}"${r.at ? ` title="${esc(new Date(r.at).toLocaleString())}"` : ''}>${esc(rowWords(r))}</li>`).join('')}</ol>` +
    (gate ? `<p class="fact">${esc(gate)}</p>` : '') +
    (foot ? `<p class="dim">${esc(foot.line)}. ${esc(foot.most)}.</p>` : '');
}

registerDialogPart(info => {
  // A fresh box each time, so no listener or answer is left from the last dialog.
  const old = document.getElementById('d-travel');
  const box = old.cloneNode(false);
  old.replaceWith(box);
  ++travelToken;
  const project = info.project, v = project && (state.scenes[project] || {}).v;
  if (!v || v.plane === 'runtime') return;
  const n = info.travel || (info.house && info.house.issue) || null;
  if (n) {
    box.innerHTML = '<div class="opts"><button data-travel="1">Travel log</button></div>';
    box.querySelector('button').addEventListener('click', () => showTravel(box, project, n));
  }
  document.querySelectorAll('#d-meta li[data-n]').forEach(li => {
    const k = Number(li.dataset.n);
    if (!k) return;
    const b = document.createElement('button');
    b.className = 'travel-log'; b.textContent = 'log';
    b.setAttribute('aria-label', `Travel log of #${k}`);
    b.addEventListener('click', () => showTravel(box, project, k));
    li.appendChild(b);
  });
});
