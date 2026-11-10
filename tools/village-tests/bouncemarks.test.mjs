// A crate the gate handed back (src/web/village/bouncemarks.js, #433):
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { bounceLines, bounceMarks, bounceWord, landed, tagLevels } from '../../src/web/village/bouncemarks.js';

const b = (kind, at, files = []) => ({ kind, at, files });
const village = (bounces, pipeline = {}) => ({
  houses: [{ id: '/wt-1', issue: 412 }, { id: '/wt-2', issue: 413 }, { id: '/wt-3' }],
  pipeline: { quay: [], stops: [], ...pipeline, bounces },
});

test('the tag says why, and from the second bounce how often', () => {
  assert.equal(bounceWord('conflict'), '↺ conflict');
  assert.equal(bounceWord('gate-failed'), '↺ gate failed');
  assert.equal(bounceWord('push-retry'), '↺ push retry');
  assert.equal(bounceWord('conflict', 2), '↺2 conflict');
  assert.equal(bounceWord('gate-failed', 3), '↺3 gate failed');
  assert.equal(bounceWord('something-new'), '↺ bounced');
});

test('a house whose issue bounced gets the mark; the others none', () => {
  // Keys come from JSON, so they are strings.
  const v = village({ 412: [b('conflict', '2026-11-09T20:40:00Z', ['src/web/server.rs'])] });
  const marks = bounceMarks(v);
  assert.equal(marks.length, 1);
  assert.deepEqual({ ...marks[0], last: undefined }, { i: 0, n: 412, count: 1, last: undefined, tag: '↺ conflict', debris: false });
  assert.deepEqual(bounceMarks(village({})), []);
  assert.deepEqual(bounceMarks({ houses: [] }), []);
  assert.deepEqual(bounceMarks(null), []);
});

test('repeat bounces stack on the tag and leave debris, named by the last', () => {
  const v = village({ 413: [b('conflict', 't1'), b('gate-failed', 't2'), b('conflict', 't3')] });
  const [m] = bounceMarks(v);
  assert.equal(m.i, 1);
  assert.equal(m.count, 3);
  assert.equal(m.tag, '↺3 conflict');
  assert.equal(m.debris, true);
});

test('the marks and the debris go once the issue lands', () => {
  const bounces = { 412: [b('conflict', 't1'), b('conflict', 't2')] };
  assert.equal(bounceMarks(village(bounces)).length, 1);
  assert.equal(landed(village(bounces, { quay: [412] }), 412), true);
  assert.deepEqual(bounceMarks(village(bounces, { quay: [412] })), [], 'at the quay');
  assert.deepEqual(bounceMarks(village(bounces, { stops: [{ name: 'nuc-1', issues: [412] }] })), [], 'past it');
});

test('the dialog lists each attempt, newest first: why, when, and which files', () => {
  const at = iso => iso.slice(11, 16);
  const lines = bounceLines([b('conflict', '2026-11-09T20:40:00Z', ['src/web/server.rs', 'main.js']), b('gate-failed', '2026-11-09T21:05:00Z')], at);
  assert.deepEqual(lines, ['Attempt 2 · gate failed · 21:05', 'Attempt 1 · conflict · 20:40 · src/web/server.rs, main.js']);
  assert.deepEqual(bounceLines([b('push-retry')], at), ['Attempt 1 · push retry']);
});

test('tags over neighbouring houses stand at different heights; far ones stay level', () => {
  // Lots 20 apart in one row, then one far along it and one in the next row.
  assert.deepEqual(tagLevels([{ x: 0, y: 60 }, { x: 20, y: 60 }, { x: 40, y: 60 }]), [0, 1, 2]);
  assert.deepEqual(tagLevels([{ x: 0, y: 60 }, { x: 100, y: 60 }]), [0, 0], 'far apart');
  assert.deepEqual(tagLevels([{ x: 0, y: 60 }, { x: 0, y: 86 }]), [0, 0], 'different rows');
  assert.deepEqual(tagLevels([{ x: 0, y: 60 }, { x: 60, y: 60 }, { x: 20, y: 60 }]), [0, 0, 1]);
});
