// The village's "Needs you" list model (src/web/village/needswords.js), #580:
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';

// alarms.js imports state.js, which reads the reduced-motion preference when it loads.
globalThis.window = { matchMedia: () => ({ matches: false }) };
const { needsGroups, needsListInfo, needsOpenKey, needsProject, needsWanted, villageNeeds } = await import('../../src/web/village/needswords.js');

const blockedHouse = (over) => ({ id: '/src/shop-wt-1', role: 'worker-1', status: 'blocked', since: '2026-11-20T10:00:00Z', ...over });

test('a blocked house, with its issue in the title; the manager is "Manager"', () => {
  const v = { project: 'shop', houses: [blockedHouse({ issue: 4, issue_title: 'Checkout' })] };
  assert.deepEqual(villageNeeds(v), [
    { key: 'shop:house:/src/shop-wt-1', project: 'shop', title: '#4 Checkout', detail: 'Waiting on you. Answer in its session.', since: '2026-11-20T10:00:00Z', target: { kind: 'alarm', i: 0 } },
  ]);
  const withManager = { project: 'shop', manager: { status: 'blocked', since: '2026-11-21T10:00:00Z' }, houses: [] };
  assert.deepEqual(villageNeeds(withManager), [
    { key: 'shop:manager', project: 'shop', title: 'Manager', detail: 'Waiting on you. Answer in its session.', since: '2026-11-21T10:00:00Z', target: { kind: 'alarm', i: -1 } },
  ]);
});

test('a house with no issue falls back to its role', () => {
  const v = { project: 'shop', houses: [blockedHouse({ issue: null })] };
  assert.equal(villageNeeds(v)[0].title, 'worker-1');
});

test('a working or idle house is not waiting on anyone', () => {
  const v = { project: 'shop', houses: [blockedHouse({ status: 'working' }), blockedHouse({ status: 'idle' })] };
  assert.deepEqual(villageNeeds(v), []);
});

test('a gate failure or conflict, but not a bare "gating" or "red-base" entry', () => {
  const v = { project: 'shop', houses: [], pipeline: { gates: [
    { issue: 10, state: 'failed', started_at: '2026-11-20T09:00:00Z' },
    { issue: 11, state: 'conflict', started_at: '2026-11-20T09:30:00Z' },
    { issue: 12, state: 'gating', started_at: '2026-11-20T09:45:00Z' },
    { issue: 13, state: 'red-base', started_at: '2026-11-20T09:50:00Z' },
  ] } };
  assert.deepEqual(villageNeeds(v).map(i => [i.key, i.detail]), [
    ['shop:gate:10', 'Gate failed'],
    ['shop:gate:11', 'Merge conflict'],
  ]);
});

test('a bounced issue opens its bounce; one only gated (never bounced) opens the gatecrate', () => {
  const v = {
    project: 'shop', houses: [{ id: '/x', issue: 10, status: 'working' }],
    pipeline: { gates: [{ issue: 10, state: 'failed', started_at: '2026-11-20T09:00:00Z' }, { issue: 11, state: 'conflict', started_at: '2026-11-20T09:00:00Z' }],
      bounces: { 10: [{ kind: 'conflict' }] } },
  };
  const [a, b] = villageNeeds(v);
  assert.deepEqual(a.target, { kind: 'bounce', n: 10, i: 0 });
  assert.deepEqual(b.target, { kind: 'gatecrate', n: 11 });
});

test('a red build, but not a green or running one', () => {
  assert.deepEqual(villageNeeds({ project: 'shop', houses: [], build: { state: 'red', since: '2026-11-20T08:00:00Z' } })[0],
    { key: 'shop:build', project: 'shop', title: 'Build', detail: 'Red', since: '2026-11-20T08:00:00Z', target: { kind: 'build' } });
  assert.deepEqual(villageNeeds({ project: 'shop', houses: [], build: { state: 'green' } }), []);
  assert.deepEqual(villageNeeds({ project: 'shop', houses: [] }), []);
});

test('a failed deployment, but not a healthy or unknown one', () => {
  const v = { project: 'shop', houses: [], pipeline: { stops: [
    { name: 'staging', health: { state: 'ok' }, deployed_at: '2026-11-20T07:00:00Z' },
    { name: 'prod', health: { state: 'failed' }, deployed_at: '2026-11-20T07:30:00Z' },
  ] } };
  assert.deepEqual(villageNeeds(v).map(i => [i.key, i.target]), [['shop:env:prod', { kind: 'env', i: 1 }]]);
});

test('needsGroups: each village newest first, villages ordered by their own newest item', () => {
  const old = { project: 'old', houses: [blockedHouse({ id: '/old', since: '2026-11-01T00:00:00Z' })] };
  const fresh = { project: 'fresh', houses: [
    blockedHouse({ id: '/fresh-a', since: '2026-11-20T00:00:00Z' }),
    blockedHouse({ id: '/fresh-b', since: '2026-11-25T00:00:00Z' }),
  ] };
  const groups = needsGroups([old, fresh]);
  assert.deepEqual(groups.map(g => g.project), ['fresh', 'old']);
  assert.deepEqual(groups[0].items.map(i => i.key), ['fresh:house:/fresh-b', 'fresh:house:/fresh-a']);
  assert.equal(groups.length, 2);
});

test('needsGroups leaves out a village with nothing waiting, and null/undefined villages', () => {
  const quiet = { project: 'quiet', houses: [] };
  const v = { project: 'shop', houses: [blockedHouse()] };
  assert.deepEqual(needsGroups([quiet, v, null, undefined]).map(g => g.project), ['shop']);
  assert.deepEqual(needsGroups([]), []);
  assert.deepEqual(needsGroups(null), []);
});

test('needsWanted: bare #needs or &needs after a project; never a plain project hash', () => {
  assert.equal(needsWanted('#needs'), true);
  assert.equal(needsWanted('#shop&needs'), true);
  assert.equal(needsWanted('#shop&needs&open=x'), true);
  assert.equal(needsWanted('#shop'), false);
  assert.equal(needsWanted('#shop&house=wt-1'), false);
  assert.equal(needsWanted(''), false);
  assert.equal(needsWanted(null), false);
});

test('needsProject: null for bare #needs, the project for #<project>&needs', () => {
  assert.equal(needsProject('#needs'), null);
  assert.equal(needsProject('#shop&needs'), 'shop');
  assert.equal(needsProject('#my%20project&needs'), 'my project');
  assert.equal(needsProject(''), null);
});

test('needsOpenKey: the open= flag, decoded; null without one', () => {
  assert.equal(needsOpenKey('#needs&open=shop%3Ahouse%3A%2Fx'), 'shop:house:/x');
  assert.equal(needsOpenKey('#shop&needs'), null);
  assert.equal(needsOpenKey('#needs&open='), null);
});

test('needsListInfo: the count and scope in words; "nothing" when empty', () => {
  const groups = needsGroups([{ project: 'shop', houses: [blockedHouse()] }]);
  assert.equal(needsListInfo(groups).line, '1 item needs you');
  assert.equal(needsListInfo(groups, 'shop').line, '1 item needs you in shop');
  assert.equal(needsListInfo([]).line, 'Nothing needs you right now.');
  assert.equal(needsListInfo([]).title, 'Needs you');
});
