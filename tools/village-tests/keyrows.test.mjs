// What the Key says (src/web/village/keyrows.js), #442:
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { PRIO_ROWS, SECTIONS, placeRows, stateRows } from '../../src/web/village/keyrows.js';

// The themes' own labels (themes/medieval.js, themes/future.js).
const MEDIEVAL = { docks: 'Docks', board: 'Notice board', houses: 'Houses', quay: 'Quay', castle: 'Harbour', hall: 'Castle', ship: 'Ship', gate: 'Gatehouse' };
const FUTURE = { docks: 'Cargo bay', board: 'Holo-board', houses: 'Pods', quay: 'Launch rail', castle: 'Spaceport', hall: 'Command dome', ship: 'Shuttle', gate: 'Scanner arch' };

test('places lead, then states, then marks', () => {
  assert.deepEqual(SECTIONS, ['Places', 'States', 'Marks']);
});

test('every place the review asked for has a line', () => {
  const ids = placeRows(MEDIEVAL).map(r => r.id);
  for (const id of ['house', 'hall', 'docks', 'board', 'gatehouse', 'quay', 'harbour', 'env', 'ship', 'flag']) assert.ok(ids.includes(id), `no ${id}`);
  assert.equal(new Set(ids).size, ids.length, 'each place once');
  assert.equal(ids[0], 'house', 'the house first: it is what the map is mostly made of');
});

test("places are named in the theme's own words", () => {
  const words = placeRows(FUTURE).map(r => r.words).join('\n');
  for (const name of ['Pod:', 'Command dome', 'Cargo bay', 'Holo-board', 'Scanner arch', 'Launch rail', 'Spaceport', 'Shuttle']) assert.ok(words.includes(name), `no ${name}`);
  assert.ok(!/Castle|Docks|Gatehouse/.test(words), 'no medieval names in the future theme');
  assert.match(placeRows(MEDIEVAL).find(r => r.id === 'hall').words, /^Castle: the manager/);
  assert.match(placeRows(MEDIEVAL)[0].words, /^House: a worker agent/);
});

test('states run from quiet to loudest, waiting on you last', () => {
  const ids = stateRows(10).map(r => r.id);
  assert.deepEqual(ids, ['busy', 'done', 'idle', 'failed', 'waiting']);
  assert.match(stateRows(10)[1].words, /10 minutes/);
});

test('priority rows lead with the P-number, P0 to P3', () => {
  assert.deepEqual(PRIO_ROWS.map(r => r.p), ['P0', 'P1', 'P2', 'P3']);
  PRIO_ROWS.forEach(r => assert.ok(r.words.startsWith(r.p + ' · '), r.words));
});
