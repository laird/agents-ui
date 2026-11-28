// An issue's own dialog, in words (src/web/village/issuewords.js, #564):
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { blockedReasonWords, issueLineWords, issueTitleWords, issueUrl, labelChips, recentComments } from '../../src/web/village/issuewords.js';

test('the detail url is per project and issue', () => {
  assert.equal(issueUrl('shop', 7), 'api/swarms/shop/issues/7');
  assert.equal(issueUrl('a/b', 7), 'api/swarms/a%2Fb/issues/7');
});

test('a blocking label leads, with its colour and glyph', () => {
  const chips = labelChips(['bug', 'needs-design', 'P1']);
  assert.deepEqual(chips.map(c => c.name), ['needs-design', 'bug', 'P1']);
  assert.equal(chips[0].blocking, true);
  assert.equal(chips[0].color, '#f8c800');
  assert.equal(chips[0].glyph, '✎');
  // A plain label (not one of the human-judgement gates) has neither.
  assert.equal(chips[1].blocking, false);
  assert.equal(chips[1].color, null);
});

/* #565's review: decisions.js offers Approve/Reject or Answer/Reject for
 * every label decidewords.js groups, so each of those must also be
 * "blocking" here, or the dialog shows the buttons with no stated reason. */
test('every decidable label (#565) is also a blocking one here', () => {
  for (const name of ['needs-approval', 'too-complex', 'future', 'blocked', 'needs-clarification', 'needs-design', 'needs-feedback']) {
    assert.equal(labelChips([name])[0].blocking, true, name);
  }
});

test('why it is blocked, or the stand-in words', () => {
  assert.equal(blockedReasonWords({ blocked_reason: '🚫 **Blocked**: need a decision' }), '🚫 **Blocked**: need a decision');
  assert.equal(blockedReasonWords({ blocked_reason: null }), 'no reason recorded');
  assert.equal(blockedReasonWords({}), 'no reason recorded');
});

test('the title and line', () => {
  assert.equal(issueTitleWords({ number: 7, title: 'Saved carts' }), '#7 Saved carts');
  assert.equal(issueLineWords({ priority: 'P1', state: 'OPEN' }), 'P1 · OPEN');
  assert.equal(issueLineWords({ state: 'OPEN' }), 'OPEN');
});

test('only the last few comments are kept', () => {
  const comments = Array.from({ length: 8 }, (_, i) => ({ body: `c${i}` }));
  const kept = recentComments({ comments });
  assert.equal(kept.length, 5);
  assert.equal(kept[0].body, 'c3');
  assert.equal(kept[4].body, 'c7');
  assert.deepEqual(recentComments({ comments: [] }), []);
  assert.deepEqual(recentComments({}), []);
});
