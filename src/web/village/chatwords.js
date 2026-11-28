/* ---------- What the chat panel says and sends (#566, for #563) ----------
 * No DOM, so node tests it (tools/village-tests/chatwords.test.mjs); chat.js
 * draws the panel. It reuses the web terminal's own endpoints (ui.html):
 * /pane and /pane/stream to read, /input to type and /keys for Enter, Esc
 * and Ctrl-C. */
import { labelChips } from './issuewords.js';

const agentUrl = (project, role) => `api/swarms/${encodeURIComponent(project)}/agents/${encodeURIComponent(role)}`;
export const paneUrl = (project, role) => `${agentUrl(project, role)}/pane`;
export const streamUrl = (project, role) => `${agentUrl(project, role)}/pane/stream`;
export const inputUrl = (project, role) => `${agentUrl(project, role)}/input`;
export const keysUrl = (project, role) => `${agentUrl(project, role)}/keys`;

/* A pane's text as plain text: colours, cursor moves and other escapes gone. */
// eslint-disable-next-line no-control-regex
const ANSI = /\x1b\[[0-?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[@-Z\\-_]|\r/g;
export const plainPane = text => String(text || '').replace(ANSI, '').replace(/\s+$/, '');

/* A trust or consent prompt on screen: the panel never answers one for the
 * user, and says so (the swarm's own rule: worker-start never accepts one). */
const TRUST = /Do you trust the files in this folder|Is this a project you (?:created|trust)|Bypass Permissions mode|dangerously-skip-permissions|Yes, I accept/i;
export function trustPrompt(text) {
  const tail = plainPane(text).split('\n').slice(-30).join('\n');
  return TRUST.test(tail);
}

/* What "Discuss with manager" types for the user to finish: "About #N
 * (<label>): <the worker's question> — ". The label is the blocking one
 * when there is one, else the first; the question is the server's "why it's
 * blocked", on one line. */
export function discussPrefill(d) {
  const chips = labelChips(d.labels), label = (chips.find(c => c.blocking) || chips[0] || {}).name;
  const why = String(d.blocked_reason || '').replace(/\s+/g, ' ').trim();
  return `About #${d.number}${label ? ` (${label})` : ''}: ${why ? why + ' ' : ''}— `;
}

/* What sending `text` posts, in order: the text (no Enter), then Enter. */
export function sendSteps(project, role, text) {
  return [
    { url: inputUrl(project, role), body: { text, enter: false } },
    { url: keysUrl(project, role), body: { items: [{ type: 'key', key: 'Enter' }] } },
  ];
}
/* The panel's Esc and Ctrl-C buttons. */
export const CHAT_KEYS = [{ label: 'Esc', key: 'Escape' }, { label: 'Ctrl-C', key: 'C-c' }];
export const keyStep = (project, role, key) => ({ url: keysUrl(project, role), body: { items: [{ type: 'key', key }] } });
