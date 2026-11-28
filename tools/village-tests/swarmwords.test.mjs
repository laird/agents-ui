// Swarm controls on the castle (hall) dialog (src/web/village/swarmwords.js, #567):
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { DEFAULT_AGENT_TYPES, resumeUrl, swarmAnswer, swarmConfirmWords, swarmUrl, workersUrl } from '../../src/web/village/swarmwords.js';

test('the swarm routes match the dashboard\'s own, its project escaped', () => {
  assert.equal(swarmUrl('shop'), 'api/swarms/shop');
  assert.equal(swarmUrl('my shop'), 'api/swarms/my%20shop');
  assert.equal(resumeUrl('shop'), 'api/swarms/shop/resume');
  assert.equal(workersUrl('shop'), 'api/swarms/shop/workers');
  assert.deepEqual(DEFAULT_AGENT_TYPES, ['Claude', 'Codex', 'Droid', 'Gemini']);
});

test('stop and resume ask before touching anything; add worker and switch agent need no question', () => {
  assert.equal(swarmConfirmWords('stop', 'shop'), 'Stop all workers in shop? This shuts the whole swarm down.');
  assert.equal(swarmConfirmWords('resume', 'shop'), 'Resume shop? Each agent will be seeded with its last handoff.');
});

test('what the dialog says after the server answers', () => {
  assert.equal(swarmAnswer(202, { ok: true }, 'stop'), 'Stopping: tearing the swarm down.');
  assert.equal(swarmAnswer(200, { ok: true }, 'resume'), 'Resuming: each agent is seeded with its last handoff.');
  assert.equal(swarmAnswer(202, { ok: true }, 'add-worker'), 'Adding a worker.');
  assert.equal(swarmAnswer(200, { ok: true }, 'switch-agent'), 'Switched: new workers use this agent type.');
  assert.equal(swarmAnswer(403, {}, 'stop'), 'This dashboard is read-only: use the dashboard or the TUI.');
  assert.equal(swarmAnswer(404, {}, 'stop'), 'That swarm is gone.');
  assert.equal(swarmAnswer(502, { error: 'teardown failed' }, 'stop'), 'teardown failed');
  assert.equal(swarmAnswer(502, {}, 'stop'), 'Could not do that (502)');
});
