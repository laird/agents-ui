// The dock arrangement (src/web/village/docks.js), under node:
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { arrangeDocks, crateState, whereWaitedOn } from '../../src/web/village/docks.js';

const c = (number, priority, waits_on = [], parent) => ({ number, priority, waits_on, parent });
const order = backlog => arrangeDocks(backlog).map(x => x.number);

test('with no dependencies the docks keep backlog order', () => {
  const b = [c(1, 'P0'), c(5, 'P1'), c(3, 'P2'), c(9)];
  assert.deepEqual(order(b), [1, 5, 3, 9]);
  assert.deepEqual(order([]), []);
  assert.deepEqual(order(undefined), []);
});

test('a chain reads in order, prerequisite first, as one run', () => {
  // The live backlog: #396 waits on #395; #397 on #395; #398 and #399 on #396.
  const b = [c(400, 'P1'), c(397, 'P2', [395]), c(398, 'P2', [396]), c(395, 'P2'), c(399, 'P2', [396]), c(396, 'P2', [395]), c(401, 'P3')];
  const got = order(b);
  assert.deepEqual(got, [400, 395, 397, 396, 398, 399, 401]);
  // Each crate stands after everything it waits on.
  b.forEach(x => x.waits_on.forEach(d => assert.ok(got.indexOf(d) < got.indexOf(x.number), `#${x.number} after #${d}`)));
});

test('crates waiting on the same issue stand together, behind it', () => {
  const b = [c(10, 'P1'), c(11, 'P1', [20]), c(12, 'P1', [10]), c(13, 'P1', [20]), c(14, 'P1', [10]), c(20, 'P1')];
  // Two groups: #10 with the two waiting on it, then #20 with its two.
  assert.deepEqual(order(b), [10, 12, 14, 20, 11, 13]);
});

test('a group stands where its best crate would', () => {
  // A P3 prerequisite of a P0 crate comes to the front with it.
  const b = [c(1, 'P0', [9]), c(2, 'P1'), c(9, 'P3')];
  assert.deepEqual(order(b), [9, 1, 2]);
});

test('a dependency off the docks does not tie a crate to anything', () => {
  const b = [c(1, 'P1', [77]), c(2, 'P1')];
  assert.deepEqual(order(b), [1, 2], 'already in backlog order');
});

test('a cycle does not hang: its crates keep backlog order', () => {
  const b = [c(4, 'P0'), c(1, 'P1', [3]), c(2, 'P1', [1]), c(3, 'P1', [2]), c(5, 'P2', [5])];
  assert.deepEqual(order(b), [4, 1, 2, 3, 5]);
});

test('the same input gives the same order, and 40 crates all stay', () => {
  const b = Array.from({ length: 60 }, (_, i) => c(100 + i, ['P0', 'P1', 'P2', 'P3', undefined][i % 5], i % 3 ? [100 + Math.floor(i / 2)] : []));
  const one = order(b), two = order(b.map(x => ({ ...x })));
  assert.deepEqual(one, two);
  assert.equal(new Set(one).size, 60);
});

test('a crate is ready or waiting', () => {
  assert.equal(crateState(c(1)), 'ready');
  assert.equal(crateState(c(1, 'P1', [2])), 'waiting');
});

test('where the issue a crate waits on is', () => {
  const v = { backlog: [c(1)], houses: [{ issue: 2, id: 'shop-wt-3' }], board: [{ number: 3, label: 'needs-design' }] };
  const name = (_, h) => h.id;
  assert.equal(whereWaitedOn(v, 1, name), 'at the docks');
  assert.equal(whereWaitedOn(v, 2, name), 'being worked on at shop-wt-3');
  assert.equal(whereWaitedOn(v, 3, name), 'on the board: needs-design');
  assert.equal(whereWaitedOn(v, 4, name), '');
});
