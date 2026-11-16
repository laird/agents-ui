// An issue's travel log in words (src/web/village/travelwords.js, #495):
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { bounceWords, durWords, medianWords, rowWords, stageName, travelRows, travelUrl, waitName } from '../../src/web/village/travelwords.js';

// The log as travel.rs serializes it (its test `a_landed_issue_travels_from_opened_to_released`).
const landed = {
  number: 495, title: 'Travel log', state: 'landed', slowest: 'approved',
  stages: [
    { key: 'opened', at: '2026-11-10T10:00:00+00:00' },
    { key: 'approved', at: '2026-11-11T10:00:00+00:00', secs: 86400 },
    { key: 'claimed', at: '2026-11-12T08:00:00+00:00', secs: 79200 },
    { key: 'first_commit', at: '2026-11-12T09:00:00+00:00', secs: 3600 },
    { key: 'landed', at: '2026-11-12T12:00:00+00:00', secs: 10800 },
    { key: 'deployed:nuc-1', at: '2026-11-12T12:10:00+00:00', secs: 600 },
    { key: 'deployed:release', at: '2026-11-12T12:20:00+00:00', secs: 600 },
  ],
  bounces: [{ at: '2026-11-12T11:00:00Z', kind: 'conflict', files: ['src/web/server.rs'] }, { at: '2026-11-12T11:30:00Z', kind: 'push-retry' }],
};
const medians = { approved: 7200, claimed: 1800, first_commit: 900, landed: 5400, 'deployed:nuc-1': 1200, 'deployed:release': 300 };

test('durations in whole units', () => {
  assert.equal(durWords(20), 'under a minute');
  assert.equal(durWords(2400), '40 min');
  assert.equal(durWords(5 * 3600), '5 h');
  assert.equal(durWords(3 * 86400), '3 d');
  assert.equal(durWords(-1), null);
  assert.equal(durWords(undefined), null);
});

test('stages and waits are named, environments by name', () => {
  assert.equal(stageName('first_commit'), 'First commit');
  assert.equal(stageName('deployed:nuc-1'), 'Deployed to nuc-1');
  assert.equal(stageName('deployed:release'), 'Released');
  assert.equal(waitName('approved'), 'waiting for approval');
  assert.equal(waitName('deployed:nuc-1'), 'waiting to deploy to nuc-1');
});

test('a landed issue: one row per stage, against its median, the slowest marked', () => {
  const rows = travelRows(landed, medians, Date.parse('2026-11-16T00:00:00Z'), 'wt-5');
  assert.deepEqual(rows.map(rowWords), [
    'Opened',
    'Approved · 24 h (median 2 h) · slowest',
    'Claimed by wt-5 · 22 h (median 30 min)',
    'First commit · 1 h (median 15 min)',
    'Landed · 3 h (median 2 h)',
    'Deployed to nuc-1 · 10 min (median 20 min)',
    'Released · 10 min (median 5 min)',
  ]);
  assert.deepEqual(rows.filter(r => r.slow).map(r => r.key), ['approved']);
});

test('an open issue ends with the wait it is in now', () => {
  const open = { number: 7, state: 'open', slowest: 'claimed', stages: [
    { key: 'opened', at: '2026-11-16T10:00:00Z' }, { key: 'claimed', at: '2026-11-16T10:05:00Z', secs: 300 }] };
  const rows = travelRows(open, medians, Date.parse('2026-11-16T12:05:00Z'));
  assert.equal(rowWords(rows[1]), 'Claimed · 5 min (median 30 min) · slowest');
  assert.deepEqual(rows[2], { key: 'now', name: 'Now to the first commit', at: null, took: '2 h', median: '15 min', slow: false });
});

test('a closed issue that did not land has no wait row', () => {
  const rows = travelRows({ number: 8, state: 'closed', stages: [{ key: 'opened', at: '2026-11-16T10:00:00Z' }] }, {}, 0);
  assert.equal(rows.length, 1);
  assert.deepEqual(travelRows(null), []);
});

test("the gate's hand-backs, with the files that conflicted", () => {
  assert.equal(bounceWords(landed.bounces), 'Handed back by the gate 2 times: a conflict (src/web/server.rs), a push rejected and re-synced');
  assert.equal(bounceWords([{ kind: 'gate-failed' }]), 'Handed back by the gate once: a failed gate');
  assert.equal(bounceWords([]), null);
});

test('the footer: medians in travel order, the longest named', () => {
  const f = medianWords(medians);
  assert.equal(f.line, 'Median over the last 14 days: waiting for approval 2 h · waiting for a worker 30 min · to the first commit 15 min · coding and the gate 2 h · waiting to deploy to nuc-1 20 min · waiting for a release 5 min');
  assert.equal(f.most, 'Most time goes to waiting for approval');
  assert.equal(medianWords({}), null);
  assert.equal(medianWords(undefined), null);
});

test('the log is asked for by project and number', () => {
  assert.equal(travelUrl('agents ui', 495), '/api/swarms/agents%20ui/issues/495/travel');
});
