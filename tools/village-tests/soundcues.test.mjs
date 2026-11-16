// The screensaver's sound cues (#489): which transition plays what, and how
// often (src/web/village/soundcues.js).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { CUES, CUE_SOUNDS, cueSnapshot, cueSounds, cuesBetween, pickCue } from '../../src/web/village/soundcues.js';

const village = (o = {}) => ({
  project: 'shop',
  houses: o.blocked ? [{ status: 'blocked' }] : [{ status: 'working' }],
  build: o.build ? { state: o.build } : null,
  pipeline: { stops: [{ name: 'nuc-1' }, { name: 'release' }], voyages: o.voyages || [] },
});
const step = (a, b) => {
  const s1 = cueSnapshot([a]);
  return cuesBetween(s1, cueSnapshot([b], s1));
};

test('the first sight of a village is silent', () => {
  assert.deepEqual(cuesBetween(new Map(), cueSnapshot([village({ blocked: true, build: 'red' })])), []);
});

test('a bell when more starts waiting on you, and not when it clears', () => {
  assert.deepEqual(step(village(), village({ blocked: true })), ['bell']);
  assert.deepEqual(step(village({ blocked: true }), village()), []);
  assert.deepEqual(step(village({ blocked: true }), village({ blocked: true })), []);
});

test('thunder when the build breaks, birdsong when it mends, even through a run', () => {
  assert.deepEqual(step(village({ build: 'green' }), village({ build: 'red' })), ['thunder']);
  assert.deepEqual(step(village({ build: 'red' }), village({ build: 'green' })), ['birdsong']);
  const s1 = cueSnapshot([village({ build: 'red' })]);
  const s2 = cueSnapshot([village({ build: 'running' })], s1);
  assert.equal(s2.get('shop').build, 'red');
  assert.deepEqual(cuesBetween(s1, s2), []);
  assert.deepEqual(cuesBetween(s2, cueSnapshot([village({ build: 'green' })], s2)), ['birdsong']);
  // An unknown build turning red is not a break anyone saw.
  assert.deepEqual(step(village(), village({ build: 'red' })), []);
});

test('a horn when a deploy docks at production, not elsewhere or when it fails', () => {
  const docked = (to, sha, state = 'docked') => village({ voyages: [{ to, sha, state }] });
  assert.deepEqual(step(village(), docked('release', 'abc')), ['horn']);
  assert.deepEqual(step(docked('release', 'abc', 'sailing'), docked('release', 'abc')), ['horn']);
  assert.deepEqual(step(docked('release', 'abc'), docked('release', 'abc')), []);
  assert.deepEqual(step(village(), docked('nuc-1', 'abc')), []);
  assert.deepEqual(step(village(), docked('release', 'abc', 'failed')), []);
});

test('several at once come most important first, each once', () => {
  const a = [village({ build: 'green' }), { ...village(), project: 'b' }];
  const b = [village({ build: 'red', blocked: true }), { ...village({ blocked: true }), project: 'b' }];
  const s1 = cueSnapshot(a);
  assert.deepEqual(cuesBetween(s1, cueSnapshot(b, s1)), ['thunder', 'bell']);
});

test('rate limited: one cue at a time, each kind not again for a while', () => {
  const played = new Map();
  assert.equal(pickCue(['bell'], played, 1000), 'bell');
  played.set('bell', 1000);
  assert.equal(pickCue(['thunder'], played, 5000), null, 'too soon after any cue');
  assert.equal(pickCue(['bell'], played, 60000), null, 'the same cue again too soon');
  assert.equal(pickCue(['bell', 'horn'], played, 60000), 'horn');
  assert.equal(pickCue(['bell'], played, 1000 + 120000), 'bell');
  assert.equal(pickCue([], new Map(), 0), null);
});

test('every theme has a quiet sound for every cue', () => {
  for (const [theme, set] of Object.entries(CUE_SOUNDS)) {
    for (const cue of CUES) {
      assert.ok(set[cue] && set[cue].length, `${theme} ${cue}`);
      for (const x of set[cue]) {
        assert.ok(x.gain > 0 && x.gain <= 0.2, `${theme} ${cue} gain ${x.gain}`);
        assert.ok(x.dur > 0 && x.at >= 0 && x.freq > 0, `${theme} ${cue}`);
        assert.ok(['sine', 'square', 'sawtooth', 'triangle', 'noise'].includes(x.wave));
      }
    }
  }
  assert.equal(cueSounds('nonesuch'), CUE_SOUNDS.medieval);
  assert.notEqual(cueSounds('future').bell, cueSounds('medieval').bell);
});
