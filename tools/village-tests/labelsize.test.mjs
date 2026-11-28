// Map label size (src/web/village/labelsize.js): node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { lfPx, classPx, LABEL_FLOOR, LABEL_CEIL, CLASS_FLOOR } from '../../src/web/village/labelsize.js';
import { FIT_MIN } from '../../src/web/village/fitscale.js';

test('the label floor never drops below 12px, even at the smallest fit scale', () => {
  assert.equal(LABEL_FLOOR, 12);
  assert.equal(lfPx(FIT_MIN), LABEL_FLOOR);
  assert.equal(lfPx(0), LABEL_FLOOR);
});

test('the label size grows with the map, up to the ceiling', () => {
  assert.equal(lfPx(4), 12);
  assert.equal(lfPx(5), LABEL_CEIL);
  assert.equal(lfPx(100), LABEL_CEIL);
});

test('no .al class resolves below the 11px class floor at the smallest fit scale', () => {
  // village.css's `.al.bounce`, `.al.ov-grid`, `.al.ov-ship` and `.al.ghost`
  // subtract 2px from --lf; `.al.flag` and `.al.house` subtract 1px.
  assert.equal(CLASS_FLOOR, 11);
  for (const offset of [1, 2]) {
    assert.ok(classPx(FIT_MIN, offset) >= CLASS_FLOOR, `offset ${offset} dropped below the floor`);
    assert.equal(classPx(FIT_MIN, offset), CLASS_FLOOR);
  }
});

test('a class with no reduction tracks --lf itself', () => {
  assert.equal(classPx(FIT_MIN, 0), lfPx(FIT_MIN));
  assert.equal(classPx(4, 0), lfPx(4));
});
