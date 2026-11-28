// Keyboard order on the village map (src/web/village/keyorder.js), #478:
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { keyOrder, nearest, optionFor, owner, waitsOnYou } from '../../src/web/village/keyorder.js';

const v = {
  manager: { status: 'working', subagents: [] },
  houses: [
    { status: 'working', issue: 10 },
    { status: 'blocked', issue: 11 },
    { status: 'idle', issue: 12 },
    { status: 'working', issue: 13, question: { prompt: 'Go?' } }
  ]
};
const at = (o, x, y, scene = 0) => ({ o, v, scene, x, y, w: 10, h: 10 });
const names = list => list.map(e => e.o.kind + (e.o.i ?? e.o.house ?? e.o.n ?? e.o.at ?? ''));

test('waiting on you first, then houses left to right, then the board, gate, quay, docks and environments', () => {
  const entries = [
    at({ kind: 'env', i: 0 }, 10, 200),
    at({ kind: 'docks' }, 5, 20),
    at({ kind: 'quay' }, 300, 60),
    at({ kind: 'gatehouse' }, 250, 60),
    at({ kind: 'board' }, 120, 28),
    at({ kind: 'hall' }, 80, 6),
    at({ kind: 'house', i: 2 }, 200, 80),
    at({ kind: 'house', i: 0 }, 100, 80),
    at({ kind: 'house', i: 3 }, 150, 120),
    at({ kind: 'house', i: 1 }, 50, 120)
  ];
  assert.deepEqual(names(keyOrder(entries)), ['house1', 'house3', 'house0', 'house2', 'hall', 'board', 'gatehouse', 'quay', 'docks', 'env0']);
});

test("a house's people, sub-agents and crate follow it, whatever their position", () => {
  const entries = [
    at({ kind: 'crate', n: 12, at: 'house', i: 2 }, 211, 96),
    at({ kind: 'house', i: 2 }, 200, 76),
    at({ kind: 'person', i: 0, who: 'a' }, 104, 88),
    at({ kind: 'sub', house: 2, id: 's' }, 190, 90),
    at({ kind: 'house', i: 0 }, 100, 76),
    at({ kind: 'gatecrate', n: 10, at: 'house' }, 100, 76)
  ];
  assert.deepEqual(names(keyOrder(entries)), ['house0', 'person0', 'gatecrate10', 'house2', 'sub2', 'crate2']);
});

test('crates go with the place they wait at', () => {
  const entries = [
    at({ kind: 'crate', n: 1, at: 'env', i: 0 }, 0, 0),
    at({ kind: 'crate', n: 2, at: 'backlog' }, 0, 0),
    at({ kind: 'crate', n: 3, at: 'quay' }, 0, 0),
    at({ kind: 'gatecrate', n: 4, at: 'gate' }, 0, 0)
  ];
  assert.deepEqual(keyOrder(entries).map(e => e.o.n), [4, 3, 2, 1]);
});

test('a blocked manager leads, and a sub-agent waiting on you makes its house wait on you', () => {
  const v2 = { manager: { status: 'blocked' }, houses: [{ status: 'working' }, { status: 'working', subagents: [{ status: 'blocked' }] }] };
  const e = (o, x) => ({ o, v: v2, scene: 0, x, y: 0, w: 1, h: 1 });
  const order = keyOrder([e({ kind: 'house', i: 0 }, 10), e({ kind: 'house', i: 1 }, 50), e({ kind: 'hall' }, 30), e({ kind: 'sub', house: -1 }, 31)]);
  assert.deepEqual(order.map(x => x.o.kind + (x.o.i ?? '')), ['hall', 'sub', 'house1', 'house0']);
  assert.equal(waitsOnYou(v2.houses[1]), true);
  assert.equal(waitsOnYou(v2.houses[0]), false);
});

test('in a world, each shown village is finished before the next', () => {
  const order = keyOrder([at({ kind: 'house', i: 1 }, 50, 120, 1), at({ kind: 'docks' }, 900, 0, 0), at({ kind: 'house', i: 0 }, 10, 0, 1)]);
  assert.deepEqual(order.map(e => e.scene), [0, 1, 1]);
});

test('kinds the order does not know come last, and keyOrder leaves its input alone', () => {
  const entries = [at({ kind: 'mystery' }, 0, 0), at({ kind: 'docks' }, 9, 9)];
  assert.deepEqual(names(keyOrder(entries)), ['docks', 'mystery']);
  assert.equal(entries[0].o.kind, 'mystery');
});

test('owner(): which house an object belongs to', () => {
  assert.equal(owner({ kind: 'hall' }, v), -1);
  assert.equal(owner({ kind: 'sub', house: -1 }, v), -1);
  assert.equal(owner({ kind: 'person', i: 3 }, v), 3);
  assert.equal(owner({ kind: 'gatecrate', n: 13, at: 'house' }, v), 3);
  assert.equal(owner({ kind: 'gatecrate', n: 99, at: 'house' }, v), null);
  assert.equal(owner({ kind: 'crate', n: 1, at: 'quay' }, v), null);
});

test('arrow keys: the nearest target that way, preferring a straight line', () => {
  const r = (id, x, y) => ({ id, x, y, w: 10, h: 10 });
  const here = r('here', 100, 100), right = r('right', 140, 100), farDiag = r('diag', 120, 140), up = r('up', 100, 40), left = r('left', 20, 110);
  const all = [here, right, farDiag, up, left];
  assert.equal(nearest(here, all, 'right').id, 'right');
  assert.equal(nearest(here, all, 'up').id, 'up');
  assert.equal(nearest(here, all, 'left').id, 'left');
  assert.equal(nearest(here, all, 'down').id, 'diag');
  assert.equal(nearest(up, all, 'up'), null);
});

test('1-9 in a question dialog: a numbered menu by number, yes/no by place', () => {
  assert.equal(optionFor('2', ['1', '2', '3']), 1);
  assert.equal(optionFor('3', ['1', '2', '4']), -1);
  assert.equal(optionFor('4', ['1', '2', '4']), 2);
  assert.equal(optionFor('1', ['y', 'n']), 0);
  assert.equal(optionFor('2', ['y', 'n']), 1);
  assert.equal(optionFor('3', ['y', 'n']), -1);
  assert.equal(optionFor('y', ['y', 'n']), -1);
  assert.equal(optionFor('0', ['1']), -1);
});

test('a "!" leads its house in Tab order, and its owner is that house or the hall (#571)', () => {
  assert.equal(owner({ kind: 'alarm', i: 1 }, v), 1);
  assert.equal(owner({ kind: 'alarm', i: -1 }, v), -1);
  const order = keyOrder([at({ kind: 'house', i: 0 }, 10, 50), at({ kind: 'house', i: 1 }, 30, 50), at({ kind: 'crate', i: 1, at: 'house', n: 11 }, 31, 55), at({ kind: 'alarm', i: 1 }, 32, 20)]);
  assert.deepEqual(names(order).slice(0, 3), ['alarm1', 'house1', 'crate1']);
});
