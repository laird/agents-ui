/* ---------- Worker controls: the words and choices (#506) ----------
 * DOM-free, so node can test it (tools/village-tests/controlwords.test.mjs);
 * controls.js puts the buttons on the house dialog. The server's side is
 * controls.rs (#504: POST does it) and control_preview.rs (GET: what it
 * would send, sending nothing). */

/* Where a worker's controls are: GET previews, POST does. */
export const controlUrl = (project, role) =>
  `api/swarms/${encodeURIComponent(project)}/agents/${encodeURIComponent(role)}/control`;
export const previewUrl = (project, role, action) => `${controlUrl(project, role)}?action=${encodeURIComponent(action)}`;

/* The actions the dialog offers, in order (#505, #567: Stop and Restart join
 * Nudge and Release). */
export const CONTROLS = [
  { action: 'nudge', label: 'Nudge' },
  { action: 'release', label: 'Release issue' },
  { action: 'stop', label: 'Stop' },
  { action: 'restart', label: 'Restart' },
];

/* The castle dialog offers only these two for the manager (#567): it holds
 * no issue to release, and Stop swarm (swarmwords.js) covers stopping it.
 * Derived from CONTROLS, so a label change there can't drift out of sync. */
export const MANAGER_CONTROLS = CONTROLS.filter(({ action }) => action === 'nudge' || action === 'restart');

/* What each button does, given the server's preview of it: { status, body }
 * (null when the server could not be reached). Shown and ready on a 2xx;
 * shown but disabled, saying why, on a 409; left out when the server has no
 * such worker (404) or can't do it (501), or could not be asked. `actions`
 * is the dialog's own list: CONTROLS for a worker, MANAGER_CONTROLS for the
 * manager (controls.js). */
export function controlChoices(previews, actions = CONTROLS) {
  return actions.flatMap(({ action, label }) => {
    const p = previews && previews[action];
    if (!p) return [];
    if (p.status >= 200 && p.status < 300) {
      const ch = { action, label, ready: true, sent: (p.body && p.body.sent) || '' };
      // autocoder-ctl does it (#540): the confirmation shows the call.
      if (p.body && p.body.via === 'autocoder-ctl') ch.via = 'autocoder-ctl';
      return [ch];
    }
    if (p.status === 409) return [{ action, label, ready: false, why: (p.body && p.body.error) || 'Not now' }];
    return [];
  });
}

/* The confirmation's question, over what will be sent: what the village
 * types or posts itself, or the autocoder-ctl call that does it (#540). */
export function confirmWords(action, house, issue, via) {
  const ctl = via === 'autocoder-ctl';
  if (action === 'nudge') return `Nudge ${house}? ${ctl ? 'autocoder runs:' : 'This is typed into its session:'}`;
  if (action === 'release') return `Release ${issue ? '#' + issue : 'its issue'} from ${house}? ${ctl ? 'autocoder runs:' : 'This is posted on the issue:'}`;
  if (action === 'stop') return `Stop ${house}? ${ctl ? 'autocoder runs:' : 'This is sent to its pane:'}`;
  if (action === 'restart') return `Restart ${house}? ${ctl ? 'autocoder runs:' : 'This is sent to its pane:'}`;
  return `${action} ${house}?`;
}

/* What the dialog says after the server answers the POST. */
export function controlAnswer(status, body, action, issue) {
  if (status >= 200 && status < 300) {
    // What autocoder-ctl said it did (#540), after the village's own words.
    const said = body && body.via === 'autocoder-ctl' && body.said ? ` autocoder-ctl: ${body.said}` : '';
    if (action === 'nudge') return 'Nudged: its loop is starting.' + said;
    if (action === 'release') return `Released ${issue || (body && body.number) ? '#' + (issue || body.number) : 'its issue'}: the crate goes back to the docks.` + said;
    if (action === 'stop') return 'Stopped.' + (body && body.note ? ` ${body.note}.` : '') + said;
    if (action === 'restart') return 'Restarted: its runtime is relaunching.' + said;
    return 'Done.' + said;
  }
  if (status === 403) return 'This dashboard is read-only: use the TUI.';
  // Generic wording (#567): this serves the manager too, on MANAGER_CONTROLS.
  if (status === 404) return 'That agent is gone.';
  if (status === 501) return "This server can't do that here.";
  return (body && body.error) || `Could not ${action} it (${status})`;
}
