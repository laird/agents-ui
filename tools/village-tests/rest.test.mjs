// A worker at rest (src/web/village/rest.js, #411), under node:
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { FRESH_DONE_MS, restOf } from '../../src/web/village/rest.js';

const at = Date.parse('2026-11-10T02:00:00Z');
const ago = ms => new Date(at - ms).toISOString();

test('just finished shows its tick for five minutes', () => {
  assert.equal(FRESH_DONE_MS, 5 * 60 * 1000);
  assert.equal(restOf({ status: 'done', since: ago(0) }, at), 'fresh');
  assert.equal(restOf({ status: 'done', since: ago(FRESH_DONE_MS - 1000) }, at), 'fresh');
});

test('after that a finished worker is asleep, not still "done"', () => {
  assert.equal(restOf({ status: 'done', since: ago(FRESH_DONE_MS) }, at), 'asleep');
  assert.equal(restOf({ status: 'done', since: ago(3 * 3600 * 1000) }, at), 'asleep');
});

test('an idle worker is asleep straight away, with or without a since', () => {
  assert.equal(restOf({ status: 'idle', since: ago(0) }, at), 'asleep');
  assert.equal(restOf({ status: 'idle' }, at), 'asleep');
});

test('a done with no since, or an unreadable one, keeps its tick: nothing says how long ago', () => {
  assert.equal(restOf({ status: 'done' }, at), 'fresh');
  assert.equal(restOf({ status: 'done', since: 'yesterday-ish' }, at), 'fresh');
});

test('everyone else keeps their own look (waiting on a person is unchanged)', () => {
  for (const status of ['working', 'blocked', 'stalled', 'unknown', 'building', 'stopped']) {
    assert.equal(restOf({ status, since: ago(3 * 3600 * 1000) }, at), null, status);
  }
});
