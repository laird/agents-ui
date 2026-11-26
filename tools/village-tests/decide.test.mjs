// Deciding proposals on the board (src/web/village/decide.js, #483), under node:
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { answerWords, bulkPicks, bulkWords, decisionUrl, isProposal, pickWords } from '../../src/web/village/decide.js';

const note = (number, label, priority) => ({ number, title: `T${number}`, label, ...(priority ? { priority } : {}) });

test('a decision is posted under its swarm and issue', () => {
  assert.equal(decisionUrl('agents ui', 7), 'api/swarms/agents%20ui/issues/7/decision');
});

test('only proposals are decided here, not blocked issues or an unreachable note', () => {
  assert.ok(isProposal(note(4, 'proposal')));
  assert.ok(!isProposal(note(5, 'needs-design')));
  assert.ok(!isProposal(note(0, 'proposal')));
});

test('"approve all P3 and below" takes P3 and unprioritized proposals only', () => {
  const board = [note(1, 'proposal', 'P1'), note(2, 'proposal', 'P2'), note(3, 'proposal', 'P3'), note(4, 'proposal'), note(5, 'needs-design', 'P3')];
  assert.deepEqual(bulkPicks(board).map(n => n.number), [3, 4]);
  assert.deepEqual(bulkPicks(undefined), []);
});

test('the confirmation names what it would approve', () => {
  assert.equal(bulkWords([note(3)]), 'Approve this proposal?');
  assert.equal(bulkWords([note(3), note(4)]), 'Approve these 2 proposals?');
  assert.equal(pickWords(note(3, 'proposal', 'P3')), '#3 T3 · P3');
  assert.equal(pickWords(note(4, 'proposal')), '#4 T4 · no priority');
});

test('the answer in words: done, read-only, decided already, or the error', () => {
  assert.match(answerWords(200, {}, 'approve'), /docks/);
  assert.match(answerWords(200, {}, 'reject'), /closed/);
  assert.match(answerWords(403, {}, 'approve'), /read-only/);
  assert.match(answerWords(409, {}, 'approve'), /no longer a proposal/i);
  assert.equal(answerWords(502, { error: 'gh: boom' }, 'approve'), 'gh: boom');
  assert.equal(answerWords(500, {}, 'approve'), 'Could not decide (500)');
});
