/* ---------- Start swarm from the empty state (#578) ----------
 * DOM-free words and the request for the village's own "no swarms
 * running" state: the same POST /api/swarms the dashboard's launch panel
 * already uses (server.rs's api_launch_swarm_handler). startswarm.js
 * renders the form in `#empty`; this is tested under node
 * (tools/village-tests/startswarmwords.test.mjs). */

export const LAUNCH_URL = 'api/swarms';
export const AGENT_TYPES_URL = 'api/agent-types';
export const REPOS_URL = 'api/repos';
export const DEFAULT_WORKERS = 3;

/* The text the empty state shows before anyone knows whether this server
 * takes writes, and the read-only fallback once it answers. */
export const LOOKING = 'No swarms running. Start one from the dashboard or the agents-tui terminal.';
export const READ_ONLY = 'No swarms running. This dashboard is read-only: start one from the dashboard or the agents-tui terminal.';

/* The POST body for what was typed, or the error to show instead of
 * sending anything (never an empty or relative-looking path). */
export function launchRequest(repoPath, agentType, numWorkers) {
  const repo_path = (repoPath || '').trim();
  if (!repo_path) return { error: 'Enter a repository path.' };
  const n = parseInt(numWorkers, 10);
  return { body: { repo_path, agent_type: agentType, num_workers: n > 0 ? n : DEFAULT_WORKERS } };
}

/* What the form says once the server answers. `api_launch_swarm_handler`
 * sends a bare 400 (no body) for a repo path that doesn't exist on the
 * server or an agent type it doesn't know. A start through autocoder-ctl
 * (#583, web/launch.rs) answers `message` (the session, the agent it is on
 * when autocoder chose another, any prompt waiting) or `error`. */
export function launchAnswer(status, body) {
  if (status === 200 && body && body.ok && body.message) return { ok: true, words: body.message };
  if (status === 200 && body && body.ok) {
    return { ok: true, words: `Launching${body.project_name ? ' for ' + body.project_name : ''}! Refresh in a few seconds.` };
  }
  if (status === 403) return { ok: false, words: 'This dashboard is read-only: use the dashboard or the TUI.' };
  if (status === 400) return { ok: false, words: "That repository path doesn't exist on this server, or the agent type is unknown." };
  return { ok: false, words: (body && body.error) || `Launch failed (HTTP ${status})` };
}
