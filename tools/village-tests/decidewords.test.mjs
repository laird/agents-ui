// Deciding a blocked (non-proposal) issue, in words (src/web/village/decidewords.js, #565):
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { decideAnswerWords, decideGroup, decideUrl } from '../../src/web/village/decidewords.js';

test('the decide url is per project and issue', () => {
  assert.equal(decideUrl('shop', 7), 'api/swarms/shop/issues/7/decide');
});

test('each label is grouped the same way the server groups it', () => {
  assert.equal(decideGroup(['P1', 'needs-approval']), 'approve');
  assert.equal(decideGroup(['too-complex']), 'approve');
  assert.equal(decideGroup(['future']), 'approve');
  assert.equal(decideGroup(['blocked']), 'approve');
  assert.equal(decideGroup(['needs-design']), 'answer');
  assert.equal(decideGroup(['needs-clarification']), 'answer');
  assert.equal(decideGroup(['needs-feedback']), 'answer');
  // A proposal's label is decide.js's/proposals.js's, not this one's.
  assert.equal(decideGroup(['proposal']), null);
  assert.equal(decideGroup(['bug']), null);
  assert.equal(decideGroup([]), null);
  assert.equal(decideGroup(undefined), null);
});

test('what the dialog says after the server answers', () => {
  assert.equal(decideAnswerWords(200, {}, 'approve'), 'Approved: on its way to the docks');
  assert.equal(decideAnswerWords(200, {}, 'answer'), 'Answered: unblocked for the next worker');
  assert.equal(decideAnswerWords(200, {}, 'reject'), 'Rejected: closed');
  assert.equal(decideAnswerWords(403, {}, 'approve'), 'This dashboard is read-only: decide on GitHub.');
  assert.equal(decideAnswerWords(409, {}, 'approve'), 'No longer blocked: someone decided already.');
  assert.equal(decideAnswerWords(422, { error: 'say what the answer is' }, 'answer'), 'say what the answer is');
  assert.equal(decideAnswerWords(500, {}, 'approve'), 'Could not decide (500)');
});
