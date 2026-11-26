// Filing an issue from the village (src/web/village/issueform.js, #490), under node:
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { BODY_MAX, filedWords, issueRequest, newIssueUrl } from '../../src/web/village/issueform.js';

test('the form posts under its swarm', () => {
  assert.equal(newIssueUrl('agents ui'), 'api/swarms/agents%20ui/issues');
});

test('a title is required, one line, at most 256 characters', () => {
  assert.match(issueRequest({ title: '  ' }).error, /title/);
  assert.match(issueRequest({ title: 'a\nb' }).error, /one line/);
  assert.equal(issueRequest({ title: 'é'.repeat(256) }).error, undefined);
  assert.match(issueRequest({ title: 'é'.repeat(257) }).error, /256/);
});

test('the body is optional and at most 64 KB', () => {
  assert.deepEqual(issueRequest({ title: 'T', body: '  ' }), { title: 'T', labels: ['enhancement', 'P3'] });
  assert.equal(issueRequest({ title: 'T', body: 'x'.repeat(BODY_MAX) }).body.length, BODY_MAX);
  assert.match(issueRequest({ title: 'T', body: 'x'.repeat(BODY_MAX + 1) }).error, /64 KB/);
});

test('a type and a priority, P3 enhancement unless chosen, never a proposal', () => {
  assert.deepEqual(issueRequest({ title: 'T', type: 'bug', priority: 'P1' }).labels, ['bug', 'P1']);
  assert.deepEqual(issueRequest({ title: 'T', type: 'proposal', priority: 'P9' }).labels, ['enhancement', 'P3']);
});

test('what the form says: the new issue, read-only with a GitHub link, or the error', () => {
  assert.deepEqual(filedWords(200, { number: 512, url: 'u' }, null), { words: 'Filed #512: it joins the backlog on the next refresh.', url: 'u' });
  assert.deepEqual(filedWords(403, {}, 'https://github.com/o/r'), { words: 'This dashboard is read-only: file it on GitHub.', url: 'https://github.com/o/r/issues/new' });
  assert.equal(filedWords(403, {}, null).url, null);
  assert.equal(filedWords(422, { error: 'give it a title' }, null).words, 'give it a title');
});
