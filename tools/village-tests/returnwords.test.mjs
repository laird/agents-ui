// Returned goods (#497): what the quay's red-tagged crates say
// (src/web/village/returnwords.js).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { returnedGoods, returnedLine, returnedShare } from '../../src/web/village/returnwords.js';

const gh = {
  returned_of: 40,
  returned: [
    { number: 10, title: 'Saved carts', how: 'reopened' },
    { number: 13, title: 'x', how: 'reverted', at: '2026-11-08T10:00:00Z', revert: 'Revert "Fix #13: x"' },
    { number: 12, title: 'y', how: 'regression', report: 20 },
  ],
};

test('the goods come from the GitHub summary, and none without one', () => {
  assert.deepEqual(returnedGoods({ github: gh }).map(r => r.number), [10, 13, 12]);
  assert.deepEqual(returnedGoods({}), []);
  assert.deepEqual(returnedGoods({ github: { returned: [{ how: 'reopened' }] } }), [], 'no number, no crate');
});

test('each says why it came back', () => {
  assert.equal(returnedLine(gh.returned[0]), 'Reopened after it landed');
  assert.equal(returnedLine(gh.returned[1]), 'Reverted on the integration branch: Revert "Fix #13: x"');
  assert.equal(returnedLine(gh.returned[2]), 'Named in a regression report, #20');
});

test('the share is out of what landed, and absent with nothing landed', () => {
  assert.deepEqual(returnedShare(gh), { n: 3, of: 40, pct: 8, words: '3 of 40 landed came back (8%)' });
  assert.equal(returnedShare({ returned: [], returned_of: 12 }).words, '0 of 12 landed came back (0%)');
  assert.equal(returnedShare({ returned_of: 0 }), null);
  assert.equal(returnedShare(null), null);
});
