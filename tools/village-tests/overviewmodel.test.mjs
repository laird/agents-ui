// All swarms at a glance (src/web/village/overviewmodel.js), #481:
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  OVERVIEW_NAME, OV_GAP, OV_TILE, islandAt, nextSaverScene, overviewGrid, overviewIslands, overviewWords, swarmIsland,
} from '../../src/web/village/overviewmodel.js';
import { saverOptions } from '../../src/web/village/saver.js';

const house = status => ({ status });
const village = (project, extra = {}) => ({ project, houses: [], board: [], pipeline: { stops: [], gates: [], quay: [] }, build: null, ...extra });

test('a quiet swarm is green, with its houses counted', () => {
  const i = swarmIsland('shop', 'shop', [village('shop', { houses: [house('working'), house('working'), house('idle')] })]);
  assert.equal(i.health, 'green');
  assert.deepEqual(i.why, []);
  assert.deepEqual([i.working, i.idle, i.blocked, i.storm], [2, 1, 0, false]);
});

test('a blocked or stalled house is amber, and counted with the blocked', () => {
  const blocked = swarmIsland('a', 'a', [village('a', { houses: [house('blocked'), house('idle')] })]);
  assert.equal(blocked.health, 'amber');
  assert.deepEqual(blocked.why, ['1 blocked']);
  const stalled = swarmIsland('b', 'b', [village('b', { houses: [house('stalled'), house('idle'), house('working')] })]);
  assert.equal(stalled.health, 'amber');
  assert.deepEqual(stalled.why, ['1 stalled']);
  assert.deepEqual([stalled.working, stalled.idle, stalled.blocked], [1, 1, 1], 'a stalled house is not idle');
});

test('red: the board, a blocked manager, work the gate handed back, a broken build, a failed deploy', () => {
  const red = v => swarmIsland(v.project, v.project, [v]);
  assert.equal(red(village('a', { board: [{ n: 1 }] })).health, 'red');
  assert.deepEqual(red(village('a', { board: [{ n: 1 }] })).why, ['1 waiting on you']);
  assert.equal(red(village('b', { manager: { status: 'blocked' } })).health, 'red');
  assert.equal(red(village('c', { pipeline: { stops: [], quay: [], gates: [{ state: 'conflict' }] } })).health, 'red');
  const storm = red(village('d', { build: { state: 'red' } }));
  assert.equal(storm.health, 'red');
  assert.equal(storm.storm, true, 'a storm over it');
  assert.deepEqual(storm.why, ['build broken']);
  const deploy = red(village('e', { pipeline: { gates: [], quay: [], stops: [{ name: 'prod', issues: [], health: { state: 'failed' } }] } }));
  assert.deepEqual([deploy.health, deploy.why], ['red', ['deploy to prod failed']]);
  // Red outranks amber, and both reasons are listed, red first.
  const both = red(village('f', { build: { state: 'red' }, houses: [house('blocked')] }));
  assert.deepEqual([both.health, both.why], ['red', ['build broken', '1 blocked']]);
});

test('a running or green build is no storm', () => {
  assert.equal(swarmIsland('a', 'a', [village('a', { build: { state: 'running' } })]).storm, false);
  assert.equal(swarmIsland('a', 'a', [village('a', { build: { state: 'green' } })]).health, 'green');
});

test('a house held by a usage limit is idle on the overview', () => {
  const i = swarmIsland('a', 'a', [village('a', { houses: [{ status: 'idle', limit: { resets: '3pm' } }, house('working')] })]);
  assert.deepEqual([i.working, i.idle, i.blocked, i.health], [1, 1, 0, 'green']);
});

test('one island per world and per village outside a world, in the picker’s order', () => {
  const villages = [village('shop', { houses: [house('working')] }), village('shop-prod', { houses: [house('blocked')] }), village('docs')];
  const worlds = [{ name: 'world:shop', dev: 'shop', platform: 'shop-prod' }];
  const islands = overviewIslands(villages, worlds);
  assert.deepEqual(islands.map(i => [i.name, i.title]), [['world:shop', 'shop ⇄ shop-prod'], ['docs', 'docs']]);
  assert.deepEqual([islands[0].working, islands[0].blocked, islands[0].health], [1, 1, 'amber'], 'a world sums both halves');
  assert.deepEqual(overviewIslands(undefined, undefined), []);
});

test('the grid comes nearest 16:9, and centres a short last row', () => {
  const one = overviewGrid(1);
  assert.deepEqual([one.cols, one.rows, one.W, one.H], [1, 1, OV_TILE.w + 2 * OV_GAP, OV_TILE.h + 2 * OV_GAP]);
  const five = overviewGrid(5);
  assert.deepEqual([five.cols, five.rows], [3, 2]);
  const [a, b, c, d, e] = five.cells;
  assert.equal(a.y, b.y); assert.equal(b.y, c.y);
  assert.ok(d.y > a.y);
  // The two on the last row sit centred under the three above.
  assert.equal(d.x - OV_GAP, five.W - (e.x + e.w) - OV_GAP);
  for (const n of [2, 3, 4, 6, 9, 12, 20]) {
    const g = overviewGrid(n);
    assert.equal(g.cells.length, n);
    assert.ok(g.cols * g.rows >= n && (g.cols - 1) * g.rows < n || g.rows === 1, `${n}: no empty column`);
    g.cells.forEach(c => assert.ok(c.x >= 0 && c.y >= 0 && c.x + c.w <= g.W && c.y + c.h <= g.H, `${n}: inside the map`));
  }
  assert.deepEqual(overviewGrid(0).cells, []);
});

test('a point finds its island, and the sea finds none', () => {
  const g = overviewGrid(4);
  const c = g.cells[3];
  assert.equal(islandAt(g.cells, c.x + 1, c.y + 1), 3);
  assert.equal(islandAt(g.cells, c.x + c.w - 1, c.y + c.h - 1), 3);
  assert.equal(islandAt(g.cells, 1, 1), null);
});

test('the overview in words', () => {
  const isl = h => ({ health: h });
  assert.equal(overviewWords([isl('red'), isl('green'), isl('green'), isl('amber')]), '1 needs you · 1 blocked or stalled · 2 fine');
  assert.equal(overviewWords([isl('red'), isl('red')]), '2 need you');
  assert.equal(overviewWords([]), 'No swarms');
});

test('the screensaver shows the overview between swarms with overview=1', () => {
  const names = ['a', 'b', 'c'];
  assert.equal(nextSaverScene(names, 'a', 'a', false), 'b', 'without it, swarm to swarm');
  assert.equal(nextSaverScene(names, 'a', 'a', true), OVERVIEW_NAME);
  assert.equal(nextSaverScene(names, OVERVIEW_NAME, 'a', true), 'b');
  assert.equal(nextSaverScene(names, OVERVIEW_NAME, 'c', true), 'a', 'round the list');
  assert.equal(nextSaverScene(names, OVERVIEW_NAME, null, true), 'a', 'starting on the overview');
  assert.equal(nextSaverScene([], OVERVIEW_NAME, null, true), OVERVIEW_NAME);
});

test('overview=1 is a screensaver option; the overview may be pinned in the hash', () => {
  assert.equal(saverOptions('?screensaver=1&overview=1', '').overview, true);
  assert.equal(saverOptions('?screensaver=1', '').overview, false);
  assert.equal(saverOptions('?screensaver=1&overview=0', '').overview, false);
  const pinned = saverOptions('', `#${OVERVIEW_NAME}&screensaver`);
  assert.deepEqual([pinned.on, pinned.pinned, pinned.cycle], [true, OVERVIEW_NAME, 0], 'named in the hash, it stays put');
  assert.equal(saverOptions('?screensaver=1&overview=1&cycle=0', '').cycle, 0, 'a wall display');
});
