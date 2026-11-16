/* ---------- Worker controls on the house dialog (#506, for #488) ----------
 * A worker's house dialog gets Nudge (start its loop now, when it is idle)
 * and Release issue (give up its claim, so the crate goes back to the
 * docks). When the dialog opens it asks the server what each would send
 * (control_preview.rs): an action the server can't run for this house is
 * left out, and one that can't run now is greyed with the reason. A click
 * asks first, showing exactly what will be typed or posted; Send posts it
 * (controls.rs) and says what happened. A read-only server (/api/config)
 * shows no controls. The words and choices are in controlwords.js. */
import { registerDialogPart } from './registry.js';
import { esc } from './panels.js';
import { confirmWords, controlAnswer, controlChoices, controlUrl, previewUrl, CONTROLS } from './controlwords.js';

/* Whether this server takes control requests: asked once it answers (a
 * failed ask is asked again next time, not remembered as read-only). */
let writable = null;
function canWrite() {
  if (!writable) writable = fetch('/api/config').then(r => (r.ok ? r.json() : Promise.reject(r.status))).then(c => !c.read_only)
    .catch(() => { writable = null; return false; });
  return writable;
}

async function askServer(url, init) {
  try {
    const res = await fetch(url, init);
    return { status: res.status, body: await res.json().catch(() => ({})) };
  } catch {
    return null;
  }
}

/* The dialog this part last filled: a slow answer for an earlier one is dropped. */
let ctlToken = 0;

async function fillControls(box, c, token) {
  if (!(await canWrite()) || token !== ctlToken) return;
  const previews = {};
  await Promise.all(CONTROLS.map(async ({ action }) => { previews[action] = await askServer(previewUrl(c.project, c.role, action)); }));
  if (token !== ctlToken) return;
  const choices = controlChoices(previews);
  if (!choices.length) return;
  const offer = () => {
    box.innerHTML = `<div class="opts">${choices.map(ch =>
      `<button data-c="${esc(ch.action)}"${ch.ready ? '' : ` disabled title="${esc(ch.why)}"`}>${esc(ch.label)}</button>`).join('')}</div>` +
      `<p class="dim" role="status">${esc(choices.filter(ch => !ch.ready).map(ch => `${ch.label}: ${ch.why}`).join(' · '))}</p>`;
  };
  offer();
  box.addEventListener('click', async e => {
    const d = e.target.dataset || {};
    if (d.c) {
      const ch = choices.find(x => x.action === d.c);
      if (!ch || !ch.ready) return;
      box.innerHTML = `<p class="q">${esc(confirmWords(ch.action, c.name, c.issue))}</p><pre class="sent">${esc(ch.sent)}</pre>` +
        `<div class="opts"><button data-go="${esc(ch.action)}">Send</button><button data-no="1">Cancel</button></div><p class="dim" role="status"></p>`;
    } else if (d.no) {
      offer();
    } else if (d.go) {
      box.querySelectorAll('button').forEach(b => { b.disabled = true; });
      const msg = box.querySelector('[role=status]');
      msg.textContent = 'Sending…';
      const r = await askServer(controlUrl(c.project, c.role), { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action: d.go }) });
      if (token !== ctlToken) return;
      msg.textContent = r ? controlAnswer(r.status, r.body, d.go, c.issue) : "Can't reach agents-tui.";
      box.querySelector('.opts')?.remove();
    }
  });
}

registerDialogPart(info => {
  // A fresh box each time, so no listener is left from the last dialog.
  const old = document.getElementById('d-ctl');
  const box = old.cloneNode(false);
  old.replaceWith(box);
  const token = ++ctlToken;
  if (info.controls) fillControls(box, info.controls, token);
});
