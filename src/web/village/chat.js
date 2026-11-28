/* ---------- Chat with an agent, in the village (#566, for #563) ----------
 * Every agent's dialog (the hall's manager, a house's worker, a person)
 * gets "Chat", and an issue's own dialog (#564) "Discuss with manager",
 * with the line prefilled with the issue and its worker's question. Both
 * open this panel over the map, not a new tab: the session's recent output,
 * live (/pane/stream, else /pane every 2s), as plain text kept scrolled to
 * the bottom, and a line that types into the session, then Enter; Esc and
 * Ctrl-C buttons. A trust or consent prompt on screen is never answered
 * from here: the panel says so and won't send. A read-only server (/api/config)
 * shows the output only. The words and requests are in chatwords.js. */
import { state } from './state.js';
import { registerDialogPart } from './registry.js';
import { esc } from './panels.js';
import { CHAT_KEYS, discussPrefill, keyStep, paneUrl, plainPane, sendSteps, streamUrl, trustPrompt } from './chatwords.js';

const panel = document.createElement('div');
panel.className = 'win chat';
panel.id = 'chat';
panel.hidden = true;
panel.setAttribute('role', 'dialog');
panel.innerHTML = `<button type="button" class="chat-close" aria-label="Close chat">×</button>
  <h2 id="chat-title"></h2>
  <pre id="chat-out" aria-live="polite"></pre>
  <p class="chat-warn" id="chat-warn" hidden></p>
  <form id="chat-form" hidden><input id="chat-in" type="text" autocomplete="off" aria-label="Message">
    <button type="submit">Send</button>${CHAT_KEYS.map(k => `<button type="button" data-key="${esc(k.key)}">${esc(k.label)}</button>`).join('')}</form>
  <p class="dim" id="chat-msg" role="status"></p>`;
document.body.appendChild(panel);
const out = panel.querySelector('#chat-out'), form = panel.querySelector('#chat-form'), input = panel.querySelector('#chat-in');
const warn = panel.querySelector('#chat-warn'), msg = panel.querySelector('#chat-msg');

/* Whether this server takes input: asked once it answers. */
let chatWritable = null;
function chatCanWrite() {
  if (!chatWritable) chatWritable = fetch('api/config').then(r => (r.ok ? r.json() : Promise.reject(r.status))).then(c => !c.read_only)
    .catch(() => { chatWritable = null; return false; });
  return chatWritable;
}

let chatWho = null, chatStream = null, chatPoll = 0, chatTrust = false;
function showPane(text) {
  const atBottom = out.scrollHeight - out.scrollTop <= out.clientHeight + 20;
  out.textContent = plainPane(text) || '(nothing on screen yet)';
  if (atBottom) out.scrollTop = out.scrollHeight;
  chatTrust = trustPrompt(text);
  warn.hidden = !chatTrust;
  warn.textContent = chatTrust ? 'The session is showing a trust or consent prompt. Answer it in the agent\'s own session: this panel never answers it for you.' : '';
}
function stopChat() {
  if (chatStream) { chatStream.close(); chatStream = null; }
  clearInterval(chatPoll); chatPoll = 0;
}
async function pollPane(who) {
  try {
    const res = await fetch(paneUrl(who.project, who.role));
    if (who === chatWho && res.ok) showPane((await res.json()).pane_content);
    else if (who === chatWho) out.textContent = `Couldn't read the session (${res.status}).`;
  } catch { if (who === chatWho) out.textContent = "Can't reach agents-tui."; }
}
function watch(who) {
  stopChat();
  pollPane(who);
  const poll = () => { if (!chatPoll && who === chatWho) chatPoll = setInterval(() => pollPane(who), 2000); };
  if (typeof EventSource === 'undefined') { poll(); return; }
  chatStream = new EventSource(streamUrl(who.project, who.role));
  chatStream.onmessage = ev => {
    clearInterval(chatPoll); chatPoll = 0;
    try { if (who === chatWho) showPane(JSON.parse(ev.data).pane_content); } catch { /* a bad frame: wait for the next */ }
  };
  chatStream.onerror = poll;
}

/* Open the panel on `role` of `project`, its line prefilled with `text`. */
export async function openChat(project, role, title, text = '') {
  const who = chatWho = { project, role };
  panel.querySelector('#chat-title').textContent = title;
  msg.textContent = '';
  out.textContent = 'Loading…';
  panel.hidden = false;
  document.getElementById('dialog').hidden = true;
  watch(who);
  const ok = await chatCanWrite();
  if (who !== chatWho) return;
  form.hidden = !ok;
  if (!ok) { msg.textContent = 'This dashboard is read-only: output only.'; return; }
  input.value = text;
  input.focus();
  input.setSelectionRange(text.length, text.length);
}
function closeChat() { chatWho = null; stopChat(); panel.hidden = true; }
panel.querySelector('.chat-close').addEventListener('click', closeChat);
panel.addEventListener('keydown', e => { if (e.key === 'Escape' && e.target !== input) closeChat(); });

async function post(steps) {
  for (const s of steps) {
    const res = await fetch(s.url, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(s.body) });
    if (!res.ok) throw new Error(res.status === 403 ? 'This dashboard is read-only.' : `The server said ${res.status}.`);
  }
}
form.addEventListener('submit', async e => {
  e.preventDefault();
  const who = chatWho, text = input.value;
  if (!who || !text.trim()) return;
  if (chatTrust) { msg.textContent = 'Not sent: answer the trust prompt in the agent\'s own session first.'; return; }
  msg.textContent = 'Sending…';
  try { await post(sendSteps(who.project, who.role, text)); input.value = ''; msg.textContent = 'Sent.'; }
  catch (err) { msg.textContent = err.message || "Can't reach agents-tui."; }
});
form.querySelectorAll('[data-key]').forEach(b => b.addEventListener('click', async () => {
  const who = chatWho;
  if (!who) return;
  try { await post([keyStep(who.project, who.role, b.dataset.key)]); msg.textContent = `Sent ${b.textContent}.`; }
  catch (err) { msg.textContent = err.message || "Can't reach agents-tui."; }
}));

/* The dialog's buttons: "Chat" on an agent's dialog (`info.chat`), and
 * "Discuss with manager" on an issue's own (`info.issueDetail`). */
registerDialogPart(info => {
  const old = document.getElementById('d-chat'), box = old.cloneNode(false);
  old.replaceWith(box);
  const project = info.project, v = project && (state.scenes[project] || {}).v;
  if (!v) return;
  const buttons = [];
  if (info.chat) buttons.push([info.chat.label || 'Chat', () => openChat(project, info.chat.role, `Chat · ${info.chat.name}`)]);
  if (info.issueDetail && v.plane !== 'runtime' && v.manager && v.manager.role)
    buttons.push(['Discuss with manager', () => openChat(project, v.manager.role, `Chat · manager · about #${info.issueDetail.number}`, discussPrefill(info.issueDetail))]);
  if (!buttons.length) return;
  box.innerHTML = `<div class="opts">${buttons.map(([label], k) => `<button type="button" data-chat="${k}">${esc(label)}</button>`).join('')}</div>`;
  buttons.forEach(([, go], k) => box.querySelector(`[data-chat="${k}"]`).addEventListener('click', go));
}, 47);
