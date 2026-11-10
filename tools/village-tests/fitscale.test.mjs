// The village's Fit scale (src/web/village/fitscale.js):
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { chooseScale, FIT_MIN } from '../../src/web/village/fitscale.js';

test('a wide window: the height bounds the map', () => {
  // 600×180 in 1900×400: height allows 2.2×, width 3.2×. 2× leaves 37% of the
  // width but only 10% of the height bare, so whole pixels win.
  assert.equal(chooseScale(600, 180, 1900, 400), 2);
});

test('a tall window: the width bounds the map', () => {
  assert.equal(chooseScale(600, 180, 1250, 2000), 2);
});

test('a whole scale is kept unless it leaves both axes more than 15% bare', () => {
  // 320×180 in 600×340: 1.875× fits; 1× would leave ~47% bare both ways.
  assert.equal(chooseScale(320, 180, 600, 340), 600 / 320);
  // 320×180 in 1250×700: 3.89× fits; 3× leaves 23% bare both ways, so 3.89×.
  assert.equal(chooseScale(320, 180, 1250, 700), 700 / 180);
  // 320×180 in 1250×380: 2.11× fits; 2× leaves 5% of the height: 2×.
  assert.equal(chooseScale(320, 180, 1250, 380), 2);
});

test('an exact fit is a whole scale', () => {
  assert.equal(chooseScale(320, 180, 1280, 720), 4);
  assert.equal(chooseScale(320, 180, 1920, 1080), 6);
});

test('a big map in a small window shrinks to fit, never below the floor', () => {
  assert.equal(chooseScale(1600, 300, 800, 600), 0.5);
  assert.equal(chooseScale(100000, 300, 800, 600), FIT_MIN);
});

test('a tiny map in a huge window grows (whole steps when that fills it)', () => {
  assert.equal(chooseScale(320, 180, 2560, 1440), 8);
  assert.equal(chooseScale(72, 60, 3840, 2160), 36);
});

test('with no height bound (a phone) only the width counts', () => {
  assert.equal(chooseScale(600, 180, 360), 0.6);
  assert.equal(chooseScale(300, 900, 620), 2);
  assert.equal(chooseScale(300, 900, 620, Infinity), 2);
});

test('degenerate sizes never divide by zero', () => {
  assert.equal(chooseScale(0, 0, 800, 600), 1);
  assert.equal(chooseScale(320, 180, 0, 600), FIT_MIN);
});
