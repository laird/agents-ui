// The village's deep link to a house (src/web/village/deeplink.js), #480:
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { houseParam, houseTarget } from '../../src/web/village/deeplink.js';

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
