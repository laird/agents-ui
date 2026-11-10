// The environment row's words and houses (src/web/village/envlabels.js), #440:
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { canalNames, envHouses, envLabel, inRow, versionOf } from '../../src/web/village/envlabels.js';

const NOW = Date.parse('2026-11-09T12:00:00Z');
const dev = stops => ({ plane: 'dev', pipeline: { stops } });

test('one label per environment: its name and version, deployed when under it', () => {
  const l = envLabel({ name: 'release', tag: 'v2026.11.09.5', deployed_at: '2026-11-09T09:00:00Z' }, NOW);
  assert.equal(l.text, 'release · v2026.11.09.5');
  assert.equal(l.sub, '3 h ago');
});

test('without a tag the commit names the version', () => {
  assert.equal(envLabel({ name: 'nuc-1', sha: 'abcdef1234567' }, NOW).text, 'nuc-1 · abcdef1');
  assert.equal(versionOf({ sha: 'abcdef1234567' }), 'abcdef1');
});

test('nothing known says unknown, with no time under it', () => {
  const l = envLabel({ name: 'nuc-1', source: 'unknown', deployed_at: '2026-11-09T09:00:00Z' }, NOW);
  assert.equal(l.text, 'nuc-1 · unknown');
  assert.equal(l.sub, null);
});

test('an environment with a village is not named again at its lock or the castle', () => {
  // nuc-1 then release: both villages in the row, so the canal names neither.
  const n = canalNames(dev([{ name: 'nuc-1', issues: [] }, { name: 'release', issues: [] }]));
  assert.deepEqual(n.locks, [null]);
  assert.equal(n.castle, null);
});

test('a gate keeps its name at its lock', () => {
  const n = canalNames(dev([{ name: 'green', kind: 'gate', issues: [] }, { name: 'nuc-1', issues: [] }, { name: 'release', issues: [] }]));
  assert.deepEqual(n.locks, ['green', null]);
  assert.equal(n.castle, null);
});

test("a runtime's stages are its pipeline: every one is named on the canal", () => {
  const v = { plane: 'runtime', pipeline: { stops: [{ name: 'build' }, { name: 'deploy' }, { name: 'prod' }] } };
  assert.deepEqual(canalNames(v), { locks: ['build', 'deploy'], castle: 'prod' });
  assert.equal(inRow(v, v.pipeline.stops[0]), false);
});

test('an environment village has a house per feature it runs, flying its colour', () => {
  const groups = [{ f: { number: 7 }, col: '#40b8e8', ns: [8, 9] }, { f: { number: 12 }, col: '#e070d0', ns: [13] }, { f: null, col: '#999', ns: [20, 21] }];
  const { houses, more } = envHouses(groups, 'town');
  assert.deepEqual(houses.map(h => [h.f.number, h.col, h.n]), [[7, '#40b8e8', 2], [12, '#e070d0', 1]]);
  assert.equal(more, 0, 'fixes that belong to no feature build no house');
});

test('no feature, no houses; past the room the rest is a +N', () => {
  assert.deepEqual(envHouses([{ f: null, ns: [1] }], 'town'), { houses: [], more: 0 });
  const many = [1, 2, 3, 4, 5].map(n => ({ f: { number: n }, col: '#fff', ns: [n * 10] }));
  assert.equal(envHouses(many, 'capital').houses.length, 3, 'the keep takes the capital\'s fourth plot');
  assert.equal(envHouses(many, 'capital').more, 2);
  assert.equal(envHouses(many, 'town').houses.length, 4);
  assert.equal(envHouses(many, 'town').more, 1);
});
