// Start swarm from the empty state (src/web/village/startswarmwords.js, #578):
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { AGENT_TYPES_URL, DEFAULT_WORKERS, LAUNCH_URL, LOOKING, READ_ONLY, REPOS_URL, launchAnswer, launchRequest } from '../../src/web/village/startswarmwords.js';

test('the routes and defaults match the dashboard\'s own launch panel', () => {
  assert.equal(LAUNCH_URL, 'api/swarms');
  assert.equal(AGENT_TYPES_URL, 'api/agent-types');
  assert.equal(REPOS_URL, 'api/repos');
  assert.equal(DEFAULT_WORKERS, 3);
});

test('an empty repo path is refused before anything is sent', () => {
  assert.deepEqual(launchRequest('', 'Claude', 3), { error: 'Enter a repository path.' });
  assert.deepEqual(launchRequest('   ', 'Claude', 3), { error: 'Enter a repository path.' });
});

test('the request trims the path and falls back to the default workers', () => {
  assert.deepEqual(launchRequest('  /repos/shop  ', 'Codex', 5), { body: { repo_path: '/repos/shop', agent_type: 'Codex', num_workers: 5 } });
  assert.deepEqual(launchRequest('/repos/shop', 'Claude', 0), { body: { repo_path: '/repos/shop', agent_type: 'Claude', num_workers: 3 } });
  assert.deepEqual(launchRequest('/repos/shop', 'Claude', 'not a number'), { body: { repo_path: '/repos/shop', agent_type: 'Claude', num_workers: 3 } });
});

test('what the form says after the server answers', () => {
  assert.deepEqual(launchAnswer(200, { ok: true, project_name: 'shop' }), { ok: true, words: 'Launching for shop! Refresh in a few seconds.' });
  assert.deepEqual(launchAnswer(200, { ok: true }), { ok: true, words: 'Launching! Refresh in a few seconds.' });
  assert.deepEqual(launchAnswer(403, {}), { ok: false, words: 'This dashboard is read-only: use the dashboard or the TUI.' });
  // api_launch_swarm_handler sends a bare 400 with no body for a bad path or agent type.
  assert.deepEqual(launchAnswer(400, {}), { ok: false, words: "That repository path doesn't exist on this server, or the agent type is unknown." });
  assert.deepEqual(launchAnswer(502, { error: 'boom' }), { ok: false, words: 'boom' });
  assert.deepEqual(launchAnswer(502, {}), { ok: false, words: 'Launch failed (HTTP 502)' });
});

test('the looking and read-only messages name the same two ways to start one', () => {
  assert.ok(LOOKING.includes('the dashboard or the agents-tui terminal'));
  assert.ok(READ_ONLY.includes('the dashboard or the agents-tui terminal'));
  assert.ok(READ_ONLY.includes('read-only'));
});
