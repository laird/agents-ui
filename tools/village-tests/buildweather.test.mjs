// The village's build weather (src/web/village/buildweather.js), #414:
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { buildSign, buildWords, failingJobs, shortSha, redSince, weatherOf } from '../../src/web/village/buildweather.js';

// As /api/village reports this repo's master: red on macOS only.
const macOnly = {
  state: 'red', partial: true, sha: '95bd5fa449623acd', blame: '3f446af4167696d2', since: '2026-11-09T20:40:00',
  source: 'ci', url: 'https://github.com/laird/agents-ui/actions/runs/42739201274',
  jobs: [{ name: 'test (macos-latest)', os: 'macOS', state: 'red', tests: ['a::one', 'a::two'] }],
  tests: ['a::one', 'a::two'],
};
const everywhere = {
  state: 'red',
  jobs: [
    { name: 'test (ubuntu-latest)', os: 'Linux', state: 'red', tests: ['a::one'] },
    { name: 'test (macos-latest)', os: 'macOS', state: 'red', tests: ['a::one', 'a::two', 'b::three'] },
    { name: 'lint', state: 'red' },
    { name: 'test (windows-latest)', os: 'Windows', state: 'running' },
  ],
  tests: ['a::one', 'a::two', 'b::three'],
};

test('a red build is a storm; red on one OS is a squall over the gatehouse', () => {
  assert.equal(weatherOf(everywhere), 'storm');
  assert.equal(weatherOf(macOnly), 'squall');
});

test('a running build drizzles, and a green or unknown one is clear', () => {
  assert.equal(weatherOf({ state: 'running' }), 'drizzle');
  assert.equal(weatherOf({ state: 'green' }), 'clear');
  assert.equal(weatherOf({ state: 'unknown' }), 'clear');
  assert.equal(weatherOf(undefined), 'clear');
});

test('the sign names the failing jobs by OS, with their test counts', () => {
  assert.deepEqual(buildSign(macOnly), { title: 'BUILD BROKEN', detail: 'macOS: 2 tests' });
  // A job with no OS goes by its name; a running job is not on the sign.
  assert.equal(failingJobs(everywhere), 'Linux: 1 test · macOS: 3 tests · lint');
  assert.equal(buildSign({ state: 'red', source: 'sentinel' }).detail, 'per sentinel');
  assert.equal(buildSign({ state: 'green' }), null);
  assert.equal(buildSign({ state: 'running' }), null);
});

test('the status line says how red and since when', () => {
  const now = new Date('2026-11-09T22:00:00');
  assert.equal(buildWords(macOnly, now), 'build red · 2 tests failing · since 20:40');
  assert.equal(buildWords({ state: 'red' }, now), 'build red');
  assert.equal(buildWords({ state: 'running' }, now), 'build running');
  assert.equal(buildWords({ state: 'green' }, now), 'build green');
  assert.equal(buildWords({ state: 'unknown' }, now), '');
  assert.equal(buildWords(null, now), '');
});

test('since reads as a time today and a date before that', () => {
  const now = new Date('2026-11-10T09:00:00');
  assert.equal(redSince('2026-11-10T08:05:00', now), '08:05');
  assert.equal(redSince('2026-11-09T17:49:00', now), 'Nov 9 17:49');
  assert.equal(redSince('', now), '');
  assert.equal(redSince('not a date', now), '');
});

test('short commit ids', () => {
  assert.equal(shortSha('3f446af4167696d2'), '3f446af');
  assert.equal(shortSha(undefined), '');
});
