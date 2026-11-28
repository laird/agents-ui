/* ---------- Swarm controls: the words and choices (#567) ----------
 * DOM-free, so node can test it (tools/village-tests/swarmwords.test.mjs);
 * swarmcontrols.js puts the buttons on the castle (hall) dialog. These are
 * the same routes the plain dashboard's launch panel uses (ui.html), not
 * the per-agent control endpoint (controlwords.js): Stop swarm, Resume,
 * Add worker and Switch agent act on the whole swarm, not one house. */

/* Where a swarm's own actions are: `server.rs` (#558-era routes, long
 * since shipped for the dashboard). */
export const swarmUrl = project => `api/swarms/${encodeURIComponent(project)}`;
export const resumeUrl = project => `${swarmUrl(project)}/resume`;
export const workersUrl = project => `${swarmUrl(project)}/workers`;

/* The runtimes `/api/agent-types` can name (server.rs's static list), used
 * before that answer is in, and if it never comes. */
export const DEFAULT_AGENT_TYPES = ['Claude', 'Codex', 'Droid', 'Gemini'];

/* The confirmation's question, before Stop or Resume touch anything. Add
 * worker and Switch agent need no confirmation (as little in the dashboard
 * today): one only adds, the other only changes what the *next* worker runs. */
export function swarmConfirmWords(action, project) {
  if (action === 'stop') return `Stop all workers in ${project}? This shuts the whole swarm down.`;
  if (action === 'resume') return `Resume ${project}? Each agent will be seeded with its last handoff.`;
  return `${action} ${project}?`;
}

/* What the dialog says after the server answers. */
export function swarmAnswer(status, body, action) {
  if (status >= 200 && status < 300) {
    if (action === 'stop') return 'Stopping: tearing the swarm down.';
    if (action === 'resume') return 'Resuming: each agent is seeded with its last handoff.';
    if (action === 'add-worker') return 'Adding a worker.';
    if (action === 'switch-agent') return 'Switched: new workers use this agent type.';
    return 'Done.';
  }
  if (status === 403) return 'This dashboard is read-only: use the dashboard or the TUI.';
  if (status === 404) return 'That swarm is gone.';
  return (body && body.error) || `Could not do that (${status})`;
}
