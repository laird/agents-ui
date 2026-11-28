// The village's deep link to a house (src/web/village/deeplink.js), #480:
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { houseParam, houseTarget, linkFlags, linkTarget } from '../../src/web/village/deeplink.js';

const v = { project: 'shop', houses: [
  { id: '/src/shop-wt-1', role: 'worker-1' },
  { id: '/src/shop-wt-2/', role: 'worker-2' },
  { id: 'remote-lane', role: 'lane' }
] };

test('the house flag, after the project and among other flags', () => {
  assert.equal(houseParam('#shop&house=wt-2'), 'wt-2');
  assert.equal(houseParam('#shop&screensaver&house=wt%202'), 'wt 2');
  assert.equal(houseParam('shop&house=%E0%A4%A'), '%E0%A4%A', 'a bad escape is taken as it is');
  assert.equal(houseParam('#shop'), null);
  assert.equal(houseParam('#shop&house='), null);
  assert.equal(houseParam('#house=wt-2'), null, 'the first part is the project, never a flag');
  assert.equal(houseParam(''), null);
});

test('a house by its short name, its id or its role; the manager is the hall', () => {
  assert.deepEqual(houseTarget(v, 'wt-2'), { kind: 'house', i: 1 }, 'a trailing slash on the id is ignored');
  assert.deepEqual(houseTarget(v, '/src/shop-wt-1'), { kind: 'house', i: 0 });
  assert.deepEqual(houseTarget(v, 'remote-lane'), { kind: 'house', i: 2 });
  assert.deepEqual(houseTarget(v, 'worker-1'), { kind: 'house', i: 0 });
  assert.deepEqual(houseTarget(v, 'manager'), { kind: 'hall' });
});

test('no such house, no village: nothing opens', () => {
  assert.equal(houseTarget(v, 'wt-9'), null);
  assert.equal(houseTarget(null, 'wt-1'), null);
  assert.equal(houseTarget(v, null), null);
  assert.equal(houseTarget({ project: 'x' }, 'wt-1'), null);
});

test('a full id wins over another house whose short name matches it', () => {
  const twins = { project: 'shop', houses: [{ id: '/a/shop-wt-1', role: 'w1' }, { id: '/b/shop-wt-1', role: 'w2' }] };
  assert.deepEqual(houseTarget(twins, '/b/shop-wt-1'), { kind: 'house', i: 1 });
});

/* ---------- Every kind a notification links to (#579, for #570) ---------- */
const w = { project: 'shop', plane: 'dev',
  manager: { status: 'working' },
  houses: [{ id: '/src/shop-wt-1', role: 'worker-1', issue: 7, status: 'blocked' }, { id: '/src/shop-wt-2', role: 'worker-2', issue: 9, status: 'working' }],
  build: { state: 'red' },
  pipeline: { stops: [{ name: 'master-green', kind: 'gate' }, { name: 'deploy' }, { name: 'release' }],
    gates: [{ issue: 12, state: 'failed' }], bounces: { 9: [{ kind: 'conflict' }] } } };

test('the flags after the project, decoded, bare ones with no value', () => {
  assert.deepEqual(linkFlags('#shop&issue=7&build&env=uat%201'), [['issue', '7'], ['build', ''], ['env', 'uat 1']]);
  assert.deepEqual(linkFlags('#shop'), []);
});

test('a house or an issue waiting on you opens its red "!" view; one that is not, its own dialog', () => {
  assert.deepEqual(linkTarget(w, '#shop&house=wt-1'), { kind: 'alarm', i: 0 });
  assert.deepEqual(linkTarget(w, '#shop&house=wt-2'), { kind: 'house', i: 1 });
  assert.deepEqual(linkTarget({ ...w, manager: { status: 'blocked' } }, '#shop&house=manager'), { kind: 'alarm', i: -1 });
  assert.deepEqual(linkTarget(w, '#shop&house=manager'), { kind: 'hall' });
  assert.deepEqual(linkTarget(w, '#shop&issue=7'), { kind: 'alarm', i: 0 });
  assert.deepEqual(linkTarget(w, '#shop&issue=42'), { kind: 'issue', n: 42 }, "any issue: #564's dialog reads it from the server");
});

test("gate=: the house's bounce when it came back, else its crate at the gate", () => {
  assert.deepEqual(linkTarget(w, '#shop&gate=9'), { kind: 'bounce', n: 9, i: 1 });
  assert.deepEqual(linkTarget(w, '#shop&gate=12'), { kind: 'gatecrate', n: 12 });
});

test('build and env=: the build, an environment village, or a gate stop on the canal', () => {
  assert.deepEqual(linkTarget(w, '#shop&build'), { kind: 'build' });
  assert.deepEqual(linkTarget(w, '#shop&env=deploy'), { kind: 'env', i: 1 });
  assert.deepEqual(linkTarget(w, '#shop&env=master-green'), { kind: 'stop', i: 0 });
  assert.deepEqual(linkTarget({ ...w, plane: 'runtime' }, '#shop&env=deploy'), { kind: 'stop', i: 1 }, "a runtime's stages are its pipeline");
});

test('what is no longer there says so; a link naming nothing opens nothing', () => {
  assert.deepEqual(linkTarget(w, '#shop&house=wt-9'), { stale: 'wt-9 is no longer waiting' });
  assert.deepEqual(linkTarget(w, '#shop&gate=99'), { stale: '#99 is no longer at the merge gate' });
  assert.deepEqual(linkTarget(w, '#shop&env=uat'), { stale: 'uat: no such environment' });
  assert.deepEqual(linkTarget({ ...w, build: null }, '#shop&build'), { stale: "Nothing reports this village's build" });
  assert.equal(linkTarget(w, '#shop'), null);
  assert.equal(linkTarget(w, '#shop&issue=x'), null);
  assert.equal(linkTarget(null, '#shop&issue=7'), null);
});
