// A house's label text (src/web/village/houselabel.js), #402:
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { flagLabel, houseLabel, managerSub, shortTitle } from '../../src/web/village/houselabel.js';

test('a house on an issue reads wt-1 · #400', () => {
  const l = houseLabel('wt-1', { issue: 400, status: 'working' });
  assert.deepEqual(l.parts, ['wt-1', '· #400']);
  assert.equal(l.parts.join(' '), 'wt-1 · #400');
  assert.equal(l.sub, null, 'no title known: the number alone');
});

test('a short title is shown whole', () => {
  assert.equal(houseLabel('wt-2', { issue: 7, status: 'working' }, 'Fix the quay').sub, 'Fix the quay');
});

test('a long title is cut to its first words, with an ellipsis', () => {
  const l = houseLabel('wt-1', { issue: 400, status: 'working' }, 'Village: label each worker house with the issue it is working on');
  assert.equal(l.sub, 'Village: label each…');
  assert.ok(l.sub.length <= 25);
});

test('a very long word is cut mid-word', () => {
  const t = shortTitle('Supercalifragilisticexpialidocious-and-more');
  assert.equal(t, 'Supercalifragilisticexp…');
  assert.equal(t.length, 24);
});

test('a house on no issue keeps its name, and says idle when it is', () => {
  assert.deepEqual(houseLabel('wt-3', { issue: null, status: 'idle' }), { parts: ['wt-3'], sub: 'idle' });
  assert.deepEqual(houseLabel('wt-3', { status: 'working' }), { parts: ['wt-3'], sub: null });
});

test('blank titles give no subtitle', () => {
  assert.equal(shortTitle(''), '');
  assert.equal(shortTitle(null), '');
  assert.equal(houseLabel('wt-1', { issue: 4, status: 'working' }, '   ').sub, null);
});

test('the hall names the manager, and its issue when it has one', () => {
  assert.equal(managerSub({ issue: null }), 'manager');
  assert.equal(managerSub({ issue: 12 }), 'manager · #12');
  assert.equal(managerSub(undefined), 'manager');
});

test('a feature flag reads #417 Hi-res · 1/6 (#441)', () => {
  assert.equal(flagLabel(417, 'Hi-res village (#417): finer art', { landed: 1, total: 6 }), '#417 Hi-res… · 1/6');
  assert.equal(flagLabel(394, 'Harden swarm recognition: identify swarms from repo identity', { landed: 0, total: 5 }), '#394 Harden swarm… · 0/5', 'the name before the colon');
  assert.equal(flagLabel(445, 'Village: re-lay to 16:9', { landed: 2, total: 2 }), '#445 re-lay to… · 2/2', 'a one-word area tag is left out');
  assert.equal(flagLabel(7, '', null), '#7 · 0/0', 'no title: the number and the count');
});
