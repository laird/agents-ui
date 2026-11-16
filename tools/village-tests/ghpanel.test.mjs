// The GitHub panel's lines and links (src/web/village/ghpanellines.js, #492):
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ageWords, asOfWords, blockedLabels, panelDialog, panelLines, panelSub } from '../../src/web/village/ghpanellines.js';

const repo = 'https://github.com/laird/agents-ui';
// `github` on /api/village as github.rs serializes it (its test fixtures).
const g = {
  as_of: '2026-11-16T06:00:00Z',
  days: [{ date: '2026-11-15', landed: 4 }, { date: '2026-11-16', landed: 2 }],
  closed_today: 3,
  open_prs: 3, oldest_pr_at: '2026-11-02T18:30:00Z', review_requests: 1,
  open_issues: 5, open_by_label: { proposal: 2, P2: 1, P3: 2, P1: 1, 'needs-clarification': 1, working: 1 },
  commits_24h: 3,
};
const q = s => encodeURIComponent(s);

test('a line per count, each linking its GitHub search', () => {
  assert.deepEqual(panelLines(g, repo, 'master'), [
    { text: '3 open pull requests, the oldest 13 d old', url: `${repo}/pulls?q=${q('is:pr is:open')}` },
    { text: "1 review request for agents-tui's GitHub account", url: `${repo}/pulls?q=${q('is:pr is:open review-requested:@me')}`, mine: true },
    { text: '2 proposals awaiting approval', url: `${repo}/issues?q=${q('is:issue is:open label:proposal')}`, mine: true },
    { text: 'Blocked: 1 needs-clarification', url: `${repo}/issues?q=${q('is:issue is:open label:needs-clarification')}` },
    { text: 'By priority: P1 1 · P2 1 · P3 2', url: `${repo}/issues?q=${q('is:issue is:open label:P1,P2,P3')}` },
    { text: '5 open issues in all', url: `${repo}/issues?q=${q('is:issue is:open')}` },
    { text: '3 commits landed on master in the last 24 h', url: `${repo}/commits/master` },
    { text: '3 issues closed today', url: `${repo}/issues?q=${q('is:issue is:closed closed:>=2026-11-16')}` },
  ]);
});

test('the review-requests search is the one the issue names', () => {
  const line = panelLines(g, repo, 'master')[1];
  assert.equal(decodeURIComponent(line.url), `${repo}/pulls?q=is:pr is:open review-requested:@me`);
});

test('with no repo known there are no links, never a guessed one', () => {
  const lines = panelLines(g, null, 'master');
  assert.equal(lines.length, 8);
  assert.ok(lines.every(l => l.url === null));
  const d = panelDialog(g, null, 'master', Date.parse('2026-11-16T06:02:00Z'));
  assert.deepEqual(d.links, []);
  assert.equal(d.meta.length, 8, 'every line still shown, as words');
});

test("what the server couldn't read says so instead of a zero", () => {
  const unread = { as_of: g.as_of, days: g.days, closed_today: 0 };
  const texts = panelLines(unread, repo, 'master').map(l => l.text);
  assert.deepEqual(texts, [
    "Open pull requests: GitHub couldn't be read",
    "Review requests: GitHub couldn't be read",
    "Open issues by label: GitHub couldn't be read",
    '0 issues closed today',
  ]);
  assert.equal(panelSub(unread), 'not read');
  // Read and empty is zero, though.
  const none = { ...unread, open_prs: 0, review_requests: 0, open_issues: 0, open_by_label: {}, commits_24h: 0 };
  const lines = panelLines(none, repo, 'main');
  assert.equal(lines[0].text, '0 open pull requests');
  assert.equal(lines[1].mine, undefined, 'nothing waits on you');
  assert.equal(lines[3].text, 'Nothing blocked (needs-*)');
  assert.equal(lines[4].text, 'By priority: none labelled');
  assert.equal(panelSub(none), '0 PRs');
});

test('no summary: no panel', () => {
  assert.deepEqual(panelLines(undefined, repo, 'master'), []);
  assert.equal(panelSub(null), '');
});

test('ages and as_of read as short words', () => {
  assert.equal(ageWords('2026-11-16T05:20:00Z', '2026-11-16T06:00:00Z'), '40 min');
  assert.equal(ageWords('2026-11-15T09:00:00Z', '2026-11-16T06:00:00Z'), '21 h');
  assert.equal(ageWords('2026-11-02T18:30:00Z', '2026-11-16T06:00:00Z'), '13 d');
  assert.equal(ageWords('soon', '2026-11-16T06:00:00Z'), null);
  assert.equal(asOfWords(g, Date.parse('2026-11-16T06:00:20Z')), 'as of just now');
  assert.equal(asOfWords(g, Date.parse('2026-11-16T06:02:00Z')), 'as of 2 min ago');
  const d = panelDialog(g, repo, 'master', Date.parse('2026-11-16T06:02:00Z'));
  assert.equal(d.title, 'On GitHub');
  assert.equal(d.line, 'What waits on GitHub for this repo, as of 2 min ago.');
  assert.equal(d.links.length, 8);
  assert.deepEqual(d.meta, []);
});

test('the sign says what waits on you first', () => {
  assert.equal(panelSub(g), '1 to review · 2 proposals · 3 PRs');
  assert.deepEqual(blockedLabels({ 'needs-design': 1, P3: 2, 'needs-clarification': 1 }), ['needs-clarification', 'needs-design']);
});
