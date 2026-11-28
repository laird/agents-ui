/* ---------- Deciding proposals on the board (#483) ----------
 * Each proposal in the board's dialog gets Approve, Reject (asked twice)
 * and Ask for changes (a short text box that posts a comment); under the
 * list, "Approve all P3 and below" asks first, listing what it would
 * approve. Each posts to the server (proposals.rs), which removes the
 * `proposal` label as the TUI's approve key does, closes the issue, or
 * comments. An approved crate leaves the board for the docks on the next
 * poll (through autocoder-ctl when installed, #541). A read-only server
 * gets none of these buttons at all (#565, readonly.js), not just a 403
 * after a click. */
import { registerDialogPart } from './registry.js';
import { esc } from './panels.js';
import { answerWords, bulkPicks, bulkWords, decisionUrl, isProposal, pickWords } from './decide.js';
import { canWrite } from './readonly.js';

async function decide(project, n, action, comment) {
  try {
    const res = await fetch(decisionUrl(project, n), { method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(comment ? { action, comment } : { action }) });
    const body = await res.json().catch(() => ({}));
    return { ok: res.ok, words: answerWords(res.status, body, action) };
  } catch (e) {
    return { ok: false, words: "Can't reach agents-tui." };
  }
}

/* One proposal's row: its buttons, then what happened. */
function decideRow(li, project, note) {
  const box = document.createElement('div');
  box.className = 'decide';
  box.innerHTML = '<div class="opts"><button data-a="approve">Approve</button><button data-a="reject">Reject</button>' +
    '<button data-a="changes">Ask for changes</button></div><div class="ask" hidden><textarea rows="2" maxlength="4000" ' +
    `aria-label="What should change in #${note.number}"></textarea><button data-a="send">Send</button></div><p class="dim" role="status"></p>`;
  li.appendChild(box);
  const msg = box.querySelector('[role=status]'), ask = box.querySelector('.ask'), text = box.querySelector('textarea');
  const buttons = [...box.querySelectorAll('button')], reject = box.querySelector('[data-a=reject]');
  const busy = on => buttons.forEach(b => { b.disabled = on; });
  const run = async (action, comment) => {
    busy(true); msg.textContent = 'Sending…';
    const r = await decide(project, note.number, action, comment);
    msg.textContent = r.words;
    if (r.ok) { box.querySelector('.opts').remove(); ask.remove(); li.classList.add('decided'); return; }
    busy(false);
  };
  box.addEventListener('click', e => {
    const a = e.target.dataset && e.target.dataset.a;
    if (!a) return;
    if (a === 'approve') run('approve');
    else if (a === 'reject' && reject.dataset.sure) run('reject');
    else if (a === 'reject') { reject.dataset.sure = '1'; reject.textContent = 'Reject: close it?'; msg.textContent = 'Click again to close the issue.'; }
    else if (a === 'changes') { ask.hidden = false; text.focus(); }
    else if (a === 'send') { if (text.value.trim()) run('changes', text.value.trim()); else msg.textContent = 'Say what should change.'; }
  });
}

/* "Approve all P3 and below", with a confirmation listing them. */
function bulkBox(root, project, picks, rows) {
  if (!picks.length) return;
  const offer = () => { root.innerHTML = `<div class="opts"><button data-b="ask">Approve all P3 and below (${picks.length})</button></div>`; };
  offer();
  root.addEventListener('click', async e => {
    const b = e.target.dataset && e.target.dataset.b;
    if (b === 'ask') {
      root.innerHTML = `<p class="q">${esc(bulkWords(picks))}</p><ul>${picks.map(n => `<li>${esc(pickWords(n))}</li>`).join('')}</ul>` +
        `<div class="opts"><button data-b="go">Approve ${picks.length}</button><button data-b="no">Cancel</button></div><p class="dim" role="status"></p>`;
    } else if (b === 'no') {
      offer();
    } else if (b === 'go') {
      root.querySelectorAll('button').forEach(x => { x.disabled = true; });
      const msg = root.querySelector('[role=status]');
      let done = 0, failed = '';
      for (const n of picks) {
        if (rows.get(n.number)?.classList.contains('decided')) continue; // decided from its own row meanwhile
        msg.textContent = `Approving #${n.number}…`;
        const r = await decide(project, n.number, 'approve');
        const row = rows.get(n.number);
        if (r.ok) {
          done++;
          if (row) { row.querySelector('.decide .opts')?.remove(); row.querySelector('.decide [role=status]').textContent = r.words; row.classList.add('decided'); }
        } else {
          failed = `#${n.number}: ${r.words}`;
          if (/read-only|reach/.test(r.words)) break; // the rest would fail the same way
        }
      }
      msg.textContent = `Approved ${done} of ${picks.length}${failed ? ' · ' + failed : ''}`;
      root.querySelector('.opts')?.remove();
    }
  });
}

/* The dialog this ask last filled: a slow answer for an earlier one is dropped. */
let proposalsToken = 0;

registerDialogPart(info => {
  // A fresh box each time, so no listener is left from the last dialog.
  const old = document.getElementById('d-act');
  const box = old.cloneNode(false);
  old.replaceWith(box);
  if (!info.decide) return;
  const mine = ++proposalsToken;
  canWrite().then(writable => {
    if (!writable || mine !== proposalsToken) return;
    const project = info.decide, notes = (info.items || []).filter(isProposal), rows = new Map();
    for (const note of notes) {
      const li = document.querySelector(`#d-meta li[data-n="${note.number}"]`);
      if (li) { decideRow(li, project, note); rows.set(note.number, li); }
    }
    bulkBox(box, project, bulkPicks(notes), rows);
  });
}, 20);
