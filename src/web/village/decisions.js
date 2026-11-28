/* ---------- Deciding a blocked (non-proposal) issue, from its dialog (#565) ----------
 * The issue dialog (#564) gets Approve/Reject or Answer/Reject, by which
 * blocking label the issue carries (decidewords.js groups it the same way
 * decisions.rs does); a proposal keeps its own flow (proposals.js). Hidden
 * on a --read-only server, as every other control is (readonly.js). */
import { registerDialogPart } from './registry.js';
import { canWrite } from './readonly.js';
import { decideAnswerWords, decideGroup, decideUrl } from './decidewords.js';

async function sendDecision(project, n, action, comment) {
  try {
    const res = await fetch(decideUrl(project, n), { method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(comment ? { action, comment } : { action }) });
    const body = await res.json().catch(() => ({}));
    return { ok: res.ok, words: decideAnswerWords(res.status, body, action) };
  } catch (e) {
    return { ok: false, words: "Can't reach agents-tui." };
  }
}

function fillDecide(box, project, n, group) {
  const answering = group === 'answer';
  box.innerHTML = `<div class="decide">` +
    `<textarea class="note" rows="2" maxlength="4000" aria-label="${answering ? 'Your answer' : 'Optional note'}" placeholder="${answering ? 'Your answer' : 'Optional note'}"></textarea>` +
    `<div class="opts"><button type="button" data-a="${answering ? 'answer' : 'approve'}">${answering ? 'Answer' : 'Approve'}</button><button type="button" data-a="reject">Reject</button></div>` +
    `<p class="dim" role="status"></p></div>`;
  const root = box.firstElementChild, msg = root.querySelector('[role=status]'), note = root.querySelector('textarea');
  const buttons = [...root.querySelectorAll('button')], reject = root.querySelector('[data-a=reject]');
  const busy = on => buttons.forEach(b => { b.disabled = on; });
  const run = async (action, comment) => {
    busy(true); msg.textContent = 'Sending…';
    const r = await sendDecision(project, n, action, comment);
    msg.textContent = r.words;
    if (r.ok) { root.querySelector('.opts').remove(); note.remove(); return; }
    busy(false);
  };
  root.addEventListener('click', e => {
    const a = e.target.dataset && e.target.dataset.a;
    if (!a) return;
    if (a === 'approve') run('approve', note.value.trim());
    else if (a === 'answer') { if (note.value.trim()) run('answer', note.value.trim()); else msg.textContent = 'Say what the answer is.'; }
    else if (a === 'reject' && reject.dataset.sure) run('reject', note.value.trim());
    else { reject.dataset.sure = '1'; reject.textContent = 'Reject: close it?'; msg.textContent = 'Click again to close the issue.'; }
  });
}

/* The dialog this ask last filled: a slow answer for an earlier one is dropped. */
let decideToken = 0;

registerDialogPart(info => {
  // A fresh box each time, so no listener is left from the last dialog.
  const old = document.getElementById('d-decide');
  const box = old.cloneNode(false);
  old.replaceWith(box);
  const d = info.issueDetail; if (!d) return;
  const group = decideGroup(d.labels);
  if (!group) return;
  const mine = ++decideToken;
  canWrite().then(writable => { if (writable && mine === decideToken) fillDecide(box, info.project, d.number, group); });
}, 46);
