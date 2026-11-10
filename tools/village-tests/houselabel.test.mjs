// A house's label text (src/web/village/houselabel.js), #402:
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { houseLabel, managerSub, shortTitle } from '../../src/web/village/houselabel.js';

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
