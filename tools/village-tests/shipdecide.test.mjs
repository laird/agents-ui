// An epic reaching production (src/web/village/shipdecide.js, #482), under node:
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';

const { SHIP_FRESH, SHIP_SHOWN, newCelebrations, hallFlag, hallFlagWords, shippedLine } = await import('../../src/web/village/shipdecide.js');

const NOW = Date.parse('2026-11-16T15:00:00Z');
const ago = ms => new Date(NOW - ms).toISOString();
const epic = (number, ms, title = `Epic ${number}`) => ({ number, title, url: `https://github.com/acme/app/issues/${number}`, at: ago(ms) });

test('a shipping fires once, when it is fresh', () => {
  const seen = new Set(), list = [epic(482, 60e3)];
  assert.deepEqual(newCelebrations(list, seen, NOW, false).map(s => s.number), [482]);
  assert.deepEqual(newCelebrations(list, seen, NOW, false), [], 'the next frame does not fire it again');
});

test('a shipping seen long after it happened shows no fireworks', () => {
  const seen = new Set();
  assert.deepEqual(newCelebrations([epic(482, SHIP_FRESH + 1)], seen, NOW, false), []);
  assert.ok(seen.has(482));
});

test('in the night hours nothing fires, and the morning does not catch up', () => {
  const seen = new Set(), list = [epic(482, 60e3)];
  assert.deepEqual(newCelebrations(list, seen, NOW, true), []);
  assert.deepEqual(newCelebrations(list, seen, NOW + 60e3, false), []);
});

test('a new epic fires while one already seen does not', () => {
  const seen = new Set([470]);
  assert.deepEqual(newCelebrations([epic(482, 0), epic(470, 0)], seen, NOW, false).map(s => s.number), [482]);
});

test('a server clock a little ahead still fires; garbage never does', () => {
  assert.equal(newCelebrations([epic(482, -30e3)], new Set(), NOW, false).length, 1);
  assert.equal(newCelebrations([{ number: 1, title: 'x', at: 'soon' }], new Set(), NOW, false).length, 0);
  assert.deepEqual(newCelebrations(undefined, new Set(), NOW, false), []);
});

test('the hall flies the newest epic, and says so on hover', () => {
  assert.equal(hallFlag([]), null);
  assert.equal(hallFlag(undefined), null);
  const list = [epic(482, 0, 'Fireworks'), epic(470, 3600e3)];
  assert.equal(hallFlag(list).number, 482);
  assert.equal(hallFlagWords(list), 'Shipped today: #482 Fireworks (and 1 more)');
  assert.equal(hallFlagWords([list[0]]), 'Shipped today: #482 Fireworks');
  assert.equal(hallFlagWords([]), '');
});

test('the screensaver says what shipped for a while, then stops', () => {
  assert.equal(shippedLine([epic(482, 10 * 60e3, 'Fireworks')], NOW), 'Shipped: #482 Fireworks');
  assert.equal(shippedLine([epic(482, 0, 'Fireworks'), epic(470, 60e3)], NOW), 'Shipped: #482 Fireworks (+1)');
  assert.equal(shippedLine([epic(482, SHIP_SHOWN + 1)], NOW), null);
  assert.equal(shippedLine([], NOW), null);
  assert.equal(shippedLine(undefined, NOW), null);
});
