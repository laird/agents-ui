// Links to GitHub for the village's dialogs (src/web/village/ghlinks.js, #491):
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { actionsLink, branchLinks, commitsLink, gatePrLink, prOf, prSearchUrl } from '../../src/web/village/ghlinks.js';

// A recorded /api/village frame: its repo comes from its issues' URLs.
const frame = JSON.parse(readFileSync(new URL('../omarchy-plugin/agents-village/tests/fixtures/village.json', import.meta.url)));
const v = frame.villages[0];
const repo = /^(https:\/\/github\.com\/[^/]+\/[^/]+)\/issues\/\d+$/.exec(v.backlog[0].url)[1];

test('a worker house links its branch and the search for its pull request', () => {
  const h = { ...v.houses[0], branch: 'feature/issue-378' };
  const ls = branchLinks(repo, h.branch, h.links, v.pipeline.integration);
  assert.deepEqual(ls, [
    { label: 'Branch feature/issue-378', url: 'https://github.com/laird/agents-ui/tree/feature/issue-378' },
    { label: 'Pull request for feature/issue-378', url: 'https://github.com/laird/agents-ui/pulls?q=is%3Apr%20head%3Afeature%2Fissue-378' },
  ]);
});

test('a pull request the report links is used instead of the search', () => {
  const links = [{ label: 'PR #12', url: 'https://github.com/laird/agents-ui/pull/12' }];
  assert.equal(prOf(links), links[0]);
  const ls = branchLinks(repo, 'feature/issue-9', links, 'master');
  assert.deepEqual(ls.map(l => l.label), ['Branch feature/issue-9']);
  assert.equal(prOf(v.houses[0].links), null, 'issue and session links are not pull requests');
});

test('no repo, no branch, or the integration branch itself: no branch links', () => {
  assert.deepEqual(branchLinks(null, 'feature/x', [], 'master'), []);
  assert.deepEqual(branchLinks(repo, undefined, [], 'master'), []);
  assert.deepEqual(branchLinks(repo, 'master', [], 'master'), []);
  assert.equal(prSearchUrl(null, 'feature/x'), null);
});

test('a gate entry links its pull request by branch when known, else by issue number', () => {
  const e = v.pipeline.gates[0];
  assert.deepEqual(gatePrLink(repo, e.issue, 'feature/issue-370'),
    { label: '#370 pull request', url: 'https://github.com/laird/agents-ui/pulls?q=is%3Apr%20head%3Afeature%2Fissue-370' });
  assert.deepEqual(gatePrLink(repo, e.issue), { label: '#370 pull request', url: 'https://github.com/laird/agents-ui/pulls?q=is%3Apr%20370' });
  assert.equal(gatePrLink(null, e.issue), null, 'never a guessed repo');
});

test('the hall links the integration branch history; a green build the workflow runs', () => {
  assert.deepEqual(commitsLink(repo, v.pipeline.integration), { label: 'Commits on master', url: 'https://github.com/laird/agents-ui/commits/master' });
  assert.equal(commitsLink(repo, undefined), null);
  assert.equal(commitsLink(null, 'master'), null);
  assert.deepEqual(actionsLink(repo), { label: 'Workflow runs', url: 'https://github.com/laird/agents-ui/actions' });
  assert.equal(actionsLink(null), null);
});

test('a branch with odd characters stays one path, its slashes kept', () => {
  const [tree] = branchLinks(repo, 'fix/a b#1', [], 'master');
  assert.equal(tree.url, 'https://github.com/laird/agents-ui/tree/fix/a%20b%231');
});
