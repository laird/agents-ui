/* ---------- Worker controls: the words and choices (#506) ----------
 * DOM-free, so node can test it (tools/village-tests/controlwords.test.mjs);
 * controls.js puts the buttons on the house dialog. The server's side is
 * controls.rs (#504: POST does it) and control_preview.rs (GET: what it
 * would send, sending nothing). */

/* Where a worker's controls are: GET previews, POST does. */
export const controlUrl = (project, role) =>
  `/api/swarms/${encodeURIComponent(project)}/agents/${encodeURIComponent(role)}/control`;
export const previewUrl = (project, role, action) => `${controlUrl(project, role)}?action=${encodeURIComponent(action)}`;

/* The actions the dialog offers, in order. Restart and Stop come with #505. */
export const CONTROLS = [
  { action: 'nudge', label: 'Nudge' },
  { action: 'release', label: 'Release issue' },
];

/* What each button does, given the server's preview of it: { status, body }
 * (null when the server could not be reached). Shown and ready on a 2xx;
 * shown but disabled, saying why, on a 409; left out when the server has no
 * such worker (404) or can't do it (501), or could not be asked. */
export function controlChoices(previews) {
  return CONTROLS.flatMap(({ action, label }) => {
    const p = previews && previews[action];
    if (!p) return [];
    if (p.status >= 200 && p.status < 300) return [{ action, label, ready: true, sent: (p.body && p.body.sent) || '' }];
    if (p.status === 409) return [{ action, label, ready: false, why: (p.body && p.body.error) || 'Not now' }];
    return [];
  });
}

/* The confirmation's question, over what will be sent. */
export function confirmWords(action, house, issue) {
  if (action === 'nudge') return `Nudge ${house}? This is typed into its session:`;
  if (action === 'release') return `Release ${issue ? '#' + issue : 'its issue'} from ${house}? This is posted on the issue:`;
  return `${action} ${house}?`;
}

/* What the dialog says after the server answers the POST. */
export function controlAnswer(status, body, action, issue) {
  if (status >= 200 && status < 300) {
    if (action === 'nudge') return 'Nudged: its loop is starting.';
    if (action === 'release') return `Released ${issue || (body && body.number) ? '#' + (issue || body.number) : 'its issue'}: the crate goes back to the docks.`;
    return 'Done.';
  }
  if (status === 403) return 'This dashboard is read-only: use the TUI.';
  if (status === 404) return 'That worker is gone.';
  if (status === 501) return "This server can't do that to a worker.";
  return (body && body.error) || `Could not ${action} it (${status})`;
}
