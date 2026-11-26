// The screensaver's "since you last looked" card (src/web/village/recap.js, #479):
//   node --test tools/omarchy-plugin/agents-village-screensaver/tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { recapHeading, recapLines, recapModel, recapSince, waitingItems } from '../../../../src/web/village/recap.js';

const SINCE = Date.parse('2026-11-15T20:00:00Z');

const village = (over = {}) => ({
  project: 'shop',
  manager: { role: 'manager', status: 'working' },
  houses: [],
  board: [],
  pipeline: { stops: [], gates: [], titles: {} },
  ...over,
});

test('since= is unix seconds, and nonsense or the future is no recap', () => {
  const now = Date.parse('2026-11-16T08:00:00Z');
  assert.equal(recapSince('?screensaver=1&since=1794800000', now), 1794800000000);
  assert.equal(recapSince('?screensaver=1', now), null);
  assert.equal(recapSince('?since=abc', now), null);
  assert.equal(recapSince('?since=-5', now), null);
  assert.equal(recapSince('?since=5', now), null);
  assert.equal(recapSince(`?since=${now / 1000 + 3600}`, now), null);
});

test('nothing happened is said, not skipped', () => {
  const changes = { villages: [{ project: 'shop', checked: true, closed: [], merged: [] }] };
  const m = recapModel([village()], changes, SINCE);
  assert.equal(m.quiet, true);
  assert.deepEqual(m.unchecked, []);
  assert.deepEqual(recapLines(m.villages[0]), []);
});

test('landed, merged, closed unfinished, deploys and bounces since the moment', () => {
  const changes = { villages: [{
    project: 'shop', checked: true,
    closed: [
      { number: 5, title: 'A', at: '2026-11-15T22:00:00Z', reason: 'completed' },
      { number: 6, title: 'B', at: '2026-11-15T23:00:00Z', reason: 'not_planned' },
    ],
    merged: [{ number: 9, title: 'PR', at: '2026-11-15T21:00:00Z' }],
  }] };
  const v = village({ pipeline: {
    titles: { 7: 'Hotspot' },
    stops: [
      { kind: 'environment', name: 'deploy', sha: 'abcdef1234567', deployed_at: '2026-11-16T01:00:00Z' },
      { kind: 'environment', name: 'release', sha: 'abcdef1234567', tag: 'v2026.11.16.1', deployed_at: '2026-11-16T01:00:05Z' },
      { kind: 'environment', name: 'old', sha: 'aaa', deployed_at: '2026-11-14T01:00:00Z' },
    ],
    bounces: {
      7: [{ at: '2026-11-15T19:00:00Z', kind: 'conflict' }, { at: '2026-11-15T22:30:00Z', kind: 'conflict' }],
      8: [{ at: '2026-11-15T22:40:00Z', kind: 'push-retry' }],
    },
  } });
  const m = recapModel([v], changes, SINCE);
  assert.equal(m.quiet, false);
  const r = m.villages[0];
  assert.deepEqual(r.landed.map(i => i.number), [5]);
  assert.deepEqual(r.closed.map(i => i.number), [6]);
  assert.deepEqual(r.deploys, [
    { name: 'deploy', version: 'abcdef1', at: '2026-11-16T01:00:00Z' },
    { name: 'release', version: 'v2026.11.16.1', at: '2026-11-16T01:00:05Z' },
  ]);
  assert.deepEqual(r.bounces, [{ issue: 7, kind: 'conflict', title: 'Hotspot' }, { issue: 8, kind: 'push-retry', title: '' }]);
  assert.deepEqual(recapLines(r), [
    { text: '1 issue landed (#5)', red: false },
    { text: '1 pull request merged (#9)', red: false },
    { text: '1 issue closed unfinished (#6)', red: false },
    { text: 'Deployed to deploy · abcdef1', red: false },
    { text: 'Deployed to release · v2026.11.16.1', red: false },
    { text: '#7 conflicted: Hotspot', red: true },
    { text: '#8 was re-synced', red: false },
  ]);
});

test('long lists are cut to three numbers', () => {
  const closed = [1, 2, 3, 4, 5].map(n => ({ number: n, title: '', at: '2026-11-15T22:00:00Z', reason: 'completed' }));
  const m = recapModel([village()], { villages: [{ project: 'shop', checked: true, closed, merged: [] }] }, SINCE);
  assert.equal(recapLines(m.villages[0])[0].text, '5 issues landed (#1, #2, #3, …)');
});

test('what waits on you is in red: the board, blocked agents, work handed back', () => {
  const v = village({
    board: [{ number: 481, title: 'Proposal: islands', label: 'proposal' }],
    manager: { role: 'manager', status: 'blocked' },
    houses: [{ role: 'worker-2', status: 'blocked', issue: 12, issue_title: 'Flaky test' }, { role: 'worker-3', status: 'working' }],
    pipeline: { gates: [{ issue: 30, state: 'failed' }, { issue: 31, state: 'gating' }, { issue: 32, state: 'red-base', landed: true }] },
  });
  assert.deepEqual(waitingItems(v), [
    { issue: 481, text: 'Proposal: islands' },
    { issue: null, text: 'manager is blocked' },
    { issue: 12, text: 'worker-2 is blocked: Flaky test' },
    { issue: 30, text: 'Failed at the gate' },
  ]);
  const m = recapModel([v], { villages: [{ project: 'shop', checked: true, closed: [], merged: [] }] }, SINCE);
  assert.equal(m.quiet, true);
  assert.equal(m.waiting, 4);
  assert.deepEqual(recapLines(m.villages[0]), [{ text: '4 waiting on you (#481, #12, #30)', red: true }]);
  const one = recapModel([village({ board: [{ number: 481, title: 'Proposal: islands' }] })], null, SINCE);
  assert.deepEqual(recapLines(one.villages[0]), [{ text: 'Waiting on you: #481 Proposal: islands', red: true }]);
});

test('a village GitHub was not asked about is named, never called quiet by GitHub', () => {
  const m = recapModel([village(), village({ project: 'mock' })], { villages: [{ project: 'shop', checked: true, closed: [], merged: [] }] }, SINCE);
  assert.deepEqual(m.unchecked, ['mock']);
  assert.deepEqual(recapModel([village()], null, SINCE).unchecked, ['shop']);
});

test('the heading says when, and how long ago', () => {
  const clock = () => '8:00 PM';
  assert.equal(recapHeading(SINCE, SINCE + 45 * 60000, clock), 'Since 8:00 PM (45m ago)');
  assert.equal(recapHeading(SINCE, SINCE + 6 * 3600000, clock), 'Since 8:00 PM (6h ago)');
  assert.equal(recapHeading(SINCE, SINCE + 3 * 86400000, clock), 'Since 8:00 PM (3d ago)');
});
