// The screensaver page's decisions (src/web/village/saver.js):
//   node --test tools/omarchy-plugin/agents-village-screensaver/tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import {
  buildEntry, staleWords, cycleOrder, dimmedAt, dismisses, driftAt, gateWords, hashName, nextName, saverOptions, saverSummary,
} from '../../../../src/web/village/saver.js';

const fixture = JSON.parse(readFileSync(new URL('../../agents-village/tests/fixtures/village.json', import.meta.url)));

test('the mode is on from the query or the hash, and off otherwise', () => {
  assert.equal(saverOptions('?screensaver=1', '').on, true);
  assert.equal(saverOptions('?screensaver', '').on, true);
  assert.equal(saverOptions('', '#shop&screensaver').on, true);
  assert.equal(saverOptions('', '#shop').on, false);
  assert.equal(saverOptions('?screensaver=0', '').on, false);
  assert.equal(saverOptions('?theme=modern', '').on, false);
});

test('a project in the hash is pinned; otherwise it cycles every 30 s', () => {
  assert.deepEqual(saverOptions('?screensaver=1', ''), { on: true, pinned: null, cycle: 30, dim: null, dismiss: false });
  const pinned = saverOptions('', '#shop&screensaver');
  assert.equal(pinned.pinned, 'shop');
  assert.equal(pinned.cycle, 0);
  assert.equal(saverOptions('?screensaver=1&cycle=10', '#shop').cycle, 10);
  assert.equal(saverOptions('?screensaver=1&cycle=0', '').cycle, 0);
  assert.equal(saverOptions('?screensaver=1&cycle=junk', '').cycle, 0);
});

test('night hours and dismissal come from the query', () => {
  assert.deepEqual(saverOptions('?screensaver=1&dim=23-7', '').dim, [23, 7]);
  assert.equal(saverOptions('?screensaver=1&dim=25-7', '').dim, null);
  assert.equal(saverOptions('?screensaver=1&dim=7-7', '').dim, null);
  assert.equal(saverOptions('?screensaver=1&dismiss=1', '').dismiss, true);
  assert.equal(saverOptions('?screensaver=1', '').dismiss, false);
});

test('the hash names the village without its flags', () => {
  assert.equal(hashName('#shop&screensaver'), 'shop');
  assert.equal(hashName('#world%3Ashop'), 'world:shop');
  assert.equal(hashName('#agents-ui'), 'agents-ui');
  assert.equal(hashName(''), '');
  assert.equal(hashName('#%E0%A4%A'), '%E0%A4%A');
});

test('the strip counts houses, the gate, the quay, each stop and what waits on you', () => {
  const v = {
    project: 'shop',
    houses: [{ status: 'working' }, { status: 'working' }, { status: 'blocked' }, { status: 'idle' }, { status: 'stalled' }],
    manager: { status: 'blocked' },
    board: [{ number: 1 }, { number: 2 }],
    pipeline: {
      quay: [5, 6, 7],
      stops: [{ name: 'nuc-1', issues: [8] }, { name: 'release', issues: [] }],
      gates: [{ state: 'gating' }, { state: 'merging' }, { state: 'queued' }, { state: 'failed' }, { state: 'red-base', landed: true }, { state: 'conflict' }],
    },
  };
  assert.deepEqual(saverSummary(v), {
    project: 'shop', working: 2, blocked: 1, idle: 2,
    // 2 on the board, 1 blocked house, the manager, a failed gate and a conflict.
    waiting: 6,
    gate: { queued: 1, gating: 2, failed: 2 },
    quay: 3,
    stops: [{ name: 'nuc-1', count: 1 }, { name: 'release', count: 0 }],
    build: null,
  });
});

test('the strip copes with a recorded village and with an empty one', () => {
  const s = saverSummary(fixture.villages[0]);
  assert.equal(s.project, fixture.villages[0].project);
  assert.equal(s.working + s.idle + s.blocked, fixture.villages[0].houses.length);
  assert.deepEqual(saverSummary({ project: 'x', houses: [], pipeline: { quay: [], stops: [] } }).stops, []);
  assert.equal(saverSummary(null).waiting, 0);
});

test('the gate in words', () => {
  assert.equal(gateWords({ queued: 0, gating: 0, failed: 0 }), 'clear');
  assert.equal(gateWords({ queued: 2, gating: 1, failed: 1 }), '1 gating · 2 queued · 1 failed');
});

test('it cycles through worlds, then villages that are not half of one', () => {
  const villages = [{ project: 'dev' }, { project: 'plat' }, { project: 'other' }];
  const worlds = [{ name: 'world:dev', dev: 'dev', platform: 'plat' }];
  const names = cycleOrder(villages, worlds);
  assert.deepEqual(names, ['world:dev', 'other']);
  assert.equal(nextName(names, 'world:dev'), 'other');
  assert.equal(nextName(names, 'other'), 'world:dev');
  assert.equal(nextName(names, 'dev'), 'world:dev');
  assert.equal(nextName([], 'x'), null);
  assert.deepEqual(cycleOrder([{ project: 'a' }], undefined), ['a']);
});

test('drift stays within a few pixels and moves', () => {
  const seen = new Set();
  for (let ms = 0; ms < 3600000; ms += 20000) {
    const d = driftAt(ms);
    assert.ok(Math.abs(d.x) <= 8 && Math.abs(d.y) <= 8, JSON.stringify(d));
    seen.add(`${d.x},${d.y}`);
  }
  assert.ok(seen.size > 20, `only ${seen.size} positions in an hour`);
});

test('night hours, including across midnight', () => {
  assert.equal(dimmedAt(23, [23, 7]), true);
  assert.equal(dimmedAt(3, [23, 7]), true);
  assert.equal(dimmedAt(7, [23, 7]), false);
  assert.equal(dimmedAt(12, [23, 7]), false);
  assert.equal(dimmedAt(13, [12, 14]), true);
  assert.equal(dimmedAt(14, [12, 14]), false);
  assert.equal(dimmedAt(3, null), false);
});

test('keys and clicks dismiss; a nudged mouse does not', () => {
  for (const type of ['keydown', 'pointerdown', 'wheel', 'touchstart']) assert.equal(dismisses({ type }, null), true, type);
  const start = { x: 100, y: 100 };
  assert.equal(dismisses({ type: 'pointermove', x: 110, y: 105 }, start), false);
  assert.equal(dismisses({ type: 'pointermove', x: 140, y: 100 }, start), true);
  assert.equal(dismisses({ type: 'pointermove', x: 140, y: 100 }, null), false);
  assert.equal(dismisses({ type: 'focus' }, start), false);
});

test('how long ago, in a word', () => {
  assert.equal(staleWords(4000), '4s ago');
  assert.equal(staleWords(150000), '3m ago');
  assert.equal(staleWords(7200000), '2h ago');
  assert.equal(staleWords(-5), '0s ago');
});

// The build (#413's `build`, #415): a red one is BUILD BROKEN on the strip.
const running = JSON.parse(readFileSync(new URL('../../agents-village/tests/fixtures/village-build-running.json', import.meta.url)));
const at = iso => iso.slice(11, 16);

test('a red build is BUILD BROKEN, with what and since when', () => {
  const red = { state: 'red', since: '2026-11-09T20:40:00Z', tests: ['a', 'b', 'c'], jobs: [{ name: 'test (macos-latest)', os: 'macOS', state: 'red' }] };
  assert.deepEqual(buildEntry(red, at), { red: true, text: 'BUILD BROKEN', detail: '3 tests failing · since 20:40' });
  assert.deepEqual(buildEntry({ ...red, partial: true, tests: ['a'] }, at), { red: true, text: 'BUILD BROKEN', detail: 'on macOS · 1 test failing · since 20:40' });
  assert.deepEqual(buildEntry({ state: 'red' }, at), { red: true, text: 'BUILD BROKEN', detail: '' });
});

test('a running build says so; green and unknown add nothing', () => {
  const v = running.villages[0];
  assert.equal(v.build.state, 'running', 'recorded while CI ran on master');
  assert.deepEqual(buildEntry(saverSummary(v).build), { red: false, text: 'Build running', detail: '' });
  assert.equal(buildEntry({ state: 'green' }), null);
  assert.equal(buildEntry({ state: 'unknown' }), null);
  assert.equal(buildEntry(null), null);
  assert.equal(saverSummary(fixture.villages[0]).build, null, 'a village from before #413 has none');
});
