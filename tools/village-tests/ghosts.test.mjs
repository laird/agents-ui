// Ghosts for flaky tests at the gatehouse (src/web/village/ghostmarks.js, #502):
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  GHOST_WEEK, failureLines, ghostFade, ghostLabel, ghostRoom, ghostSpot, ghostTags, ghostsOf, landedIssues, noteFlakyIssues, shortTest,
} from '../../src/web/village/ghostmarks.js';

const NOW = Date.parse('2026-11-16T12:00:00Z');
const ago = h => new Date(NOW - h * 3600e3).toISOString();
const flaky = (test, hours, extra = {}) => ({ test, count: 2, last_seen: ago(hours), runs: [], ...extra });
const village = (list, pipeline = {}) => ({ project: 'shop', flaky: list, pipeline: { quay: [], stops: [], ...pipeline } });

test('a ghost fades from solid to a quarter over a clean week, then is gone', () => {
  assert.equal(ghostFade(ago(0), NOW), 1);
  assert.equal(ghostFade(ago(84), NOW), 1 - 0.75 * 0.5);
  assert.ok(Math.abs(ghostFade(new Date(NOW - GHOST_WEEK + 1000).toISOString(), NOW) - 0.25) < 0.001);
  assert.equal(ghostFade(ago(24 * 7), NOW), 0);
  assert.equal(ghostFade('', NOW), 0);
  assert.equal(ghostFade('not a time', NOW), 0);
  // A failure "in the future" (clock skew) is as solid as a fresh one.
  assert.equal(ghostFade(ago(-1), NOW), 1);
});

test('the label is the test\'s short name and how often it flaked this week', () => {
  assert.equal(shortTest('web::village_tests::fit::fits_a_wide_screen'), 'fits_a_wide_screen');
  assert.equal(shortTest('plain_name'), 'plain_name');
  assert.equal(shortTest('a::an_extremely_long_test_name_indeed'), 'an_extremely_long_tes…');
  assert.equal(shortTest('a::an_extremely_long_test_name_indeed').length, 22);
  assert.equal(ghostLabel({ test: 'web::gate::slot_owners', count: 3 }), 'slot_owners ×3');
  const gs = ['a::one', 'a::two', 'a::three', 'a::four'].map(t => ({ f: { test: t, count: 1 } }));
  assert.deepEqual(ghostTags(gs, 3), ['one ×1', 'two ×1', 'three ×1', '+1 more']);
  assert.deepEqual(ghostTags(gs.slice(0, 2), 3), ['one ×1', 'two ×1']);
});

test('no flaky tests, no ghosts', () => {
  assert.deepEqual(ghostsOf(village([]), NOW), []);
  assert.deepEqual(ghostsOf({ project: 'shop', pipeline: {} }, NOW), []);
  assert.deepEqual(ghostsOf(null, NOW), []);
});

test('one ghost per flaky test, as solid as it is recent; a clean week takes it away', () => {
  const v = village([flaky('a::new', 1), flaky('a::older', 100), flaky('a::gone', 24 * 8)]);
  const gs = ghostsOf(v, NOW);
  assert.deepEqual(gs.map(g => g.f.test), ['a::new', 'a::older']);
  assert.ok(gs[0].alpha > gs[1].alpha);
});

test('a ghost leaves once its Flaky: issue lands, at the quay or at a stop', () => {
  const list = [flaky('a::fixed', 1, { issue: 341 }), flaky('a::open', 1, { issue: 342 })];
  assert.deepEqual(landedIssues(village(list, { quay: [341] })), new Set([341]));
  assert.deepEqual(ghostsOf(village(list, { quay: [341] }), NOW).map(g => g.f.test), ['a::open']);
  assert.deepEqual(ghostsOf(village(list, { stops: [{ issues: [], cargo: [341] }] }), NOW).map(g => g.f.test), ['a::open']);
  assert.deepEqual(ghostsOf(village(list, { stops: [{ issues: [341] }] }), NOW).map(g => g.f.test), ['a::open']);
});

test('a ghost whose Flaky: issue the page saw is gone from it has had it closed', () => {
  const seen = new Map();
  noteFlakyIssues(village([flaky('a::t', 1, { issue: 341 }), flaky('a::u', 1)]), seen);
  assert.deepEqual([...seen], [['a::t', { issue: 341, last: ago(1) }]]);
  // The next poll: the open issue no longer names it, so it was closed.
  const later = village([flaky('a::t', 1), flaky('a::u', 1)]);
  assert.deepEqual(ghostsOf(later, NOW, seen).map(g => g.f.test), ['a::u']);
  // A test that never had an issue stays.
  assert.deepEqual(ghostsOf(later, NOW, new Map()).map(g => g.f.test), ['a::t', 'a::u']);
  // A test that fails again after its issue closed comes back, with no issue yet.
  const again = village([flaky('a::t', 0.5)]);
  assert.deepEqual(ghostsOf(again, NOW, seen).map(g => g.f.test), ['a::t']);
  // A poll that missed the issue hides it for that poll only: back once it is named again.
  assert.deepEqual(ghostsOf(village([flaky('a::t', 1, { issue: 341 })]), NOW, seen).map(g => g.f.test), ['a::t']);
});

test('ghosts hover over the gatehouse roof from its east end, as many as it has room for', () => {
  const g = { x0: 100, w: 80, y: 200 };
  assert.equal(ghostRoom(g), 3);
  assert.equal(ghostRoom({ ...g, w: 20 }), 1);
  assert.equal(ghostRoom({ ...g, w: 32 }), 3);
  assert.equal(ghostRoom({ ...g, w: 30 }), 2);
  assert.deepEqual(ghostSpot(g, 0), { x: 170, y: 197 });
  assert.deepEqual(ghostSpot(g, 1), { x: 161, y: 197 });
  // Never east of the roof's end (the failure lamp and the scoreboard are there).
  for (let k = 0; k < 3; k++) assert.ok(ghostSpot(g, k).x + 6 <= g.x0 + g.w - 4);
  // Bobbing moves a ghost at most a pixel, and not at all when still.
  for (let t = 0; t < 5000; t += 250) assert.ok(Math.abs(ghostSpot(g, 0, t, false).y - 197) <= 1);
  assert.equal(ghostSpot(g, 0, 1234, true).y, 197);
});

test('the dialog lists each failure: when, whose merge, and the log', () => {
  const f = { runs: [{ issue: 498, when: '2026-11-16T08:10:00Z', log: '/tmp/autocoder-merge-r-498.log' }, { issue: 0, when: '', log: '' }] };
  assert.deepEqual(failureLines(f, () => '16 Nov 08:10'), ['16 Nov 08:10 · gating #498 · /tmp/autocoder-merge-r-498.log']);
  assert.deepEqual(failureLines({}, () => ''), []);
  // #538: a sighting the gate recorded has no log, but the start of its failure.
  assert.deepEqual(failureLines({ runs: [{ when: 'x', issue: 9, excerpt: '\n  thread panicked\nleft: 1' }] }, () => 'T'), ['T · gating #9 · thread panicked']);
});

test('"File flaky issue" (#503): its words, where it posts, what the answer says', async () => {
  const { flakyButtonWords, flakyFileUrl, flakyFileWords } = await import('../../src/web/village/ghostmarks.js');
  assert.equal(flakyButtonWords({ test: 'a::b', issue: null }), 'File flaky issue');
  assert.equal(flakyButtonWords({ test: 'a::b', issue: 358 }), 'Add these failures to #358');
  assert.equal(flakyFileUrl('my shop'), 'api/swarms/my%20shop/flaky');
  assert.equal(flakyFileWords(200, { ok: true, action: 'created', number: 509 }), 'Filed as #509');
  assert.equal(flakyFileWords(200, { ok: true, action: 'commented', number: 358 }), 'Added the new failures to #358');
  assert.equal(flakyFileWords(200, { ok: true, action: 'unchanged', number: 358 }), '#358 already lists every failure');
  assert.equal(flakyFileWords(501, { error: 'filing a flaky test needs the GitHub tracker' }), 'filing a flaky test needs the GitHub tracker');
  assert.equal(flakyFileWords(200, { ok: true, action: 'created', number: null }), 'Filed as the issue');
  assert.equal(flakyFileWords(403, {}), 'This dashboard is read-only: file it on GitHub.');
  assert.equal(flakyFileWords(404, { error: 'that test has not flaked here this week' }), 'that test has not flaked here this week');
  assert.equal(flakyFileWords(502, { error: 'gh: not logged in' }), 'gh: not logged in');
  assert.equal(flakyFileWords(500, {}), 'Could not file it (500)');
});
