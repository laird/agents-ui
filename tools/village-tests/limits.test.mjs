// Usage limits in the village (src/web/village/limits.js and well.js, #484),
// under node:
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= {}; // state.js reads window.matchMedia, when there is one
const V = '../../src/web/village/';
const { limited, limitWords, limitSub, villageLimit, signWords, limitedCount } = await import(V + 'limits.js');
const { restOf } = await import(V + 'rest.js');
const { houseLabel } = await import(V + 'houselabel.js');
const { saverSummary } = await import(V + 'saver.js');

const held = (status = 'idle', resets = '3pm (Europe/London)') => ({ status, limit: resets ? { resets } : {} });

test('a house is limited while it is idle or done with a limit', () => {
  assert.equal(limited(held('idle')), true);
  assert.equal(limited(held('done')), true);
  assert.equal(limited(held('stalled')), false, 'stalled keeps its own look: no second figure at the well');
  assert.equal(limited(held('unknown')), false);
  assert.equal(limited(held('working')), false, 'got past it');
  assert.equal(limited(held('blocked')), false, 'waiting on you comes first');
  assert.equal(limited(held('stopped')), false);
  assert.equal(limited({ status: 'idle' }), false, 'no limit');
  assert.equal(limited(null), false);
});

test('a limited worker is not asleep: rest.js hands it to the well', () => {
  const now = Date.parse('2026-11-15T12:00:00Z');
  assert.equal(restOf(held('idle'), now), 'limited');
  assert.equal(restOf(held('done'), now), 'limited');
  assert.equal(restOf({ status: 'idle' }, now), 'asleep');
  assert.equal(restOf(held('stalled'), now), null, 'stalled keeps its own look');
});

test('the dialog and the label say when it resets, in the runtime\'s words', () => {
  assert.equal(limitWords(held()), 'Rate limited · resets at 3pm (Europe/London)');
  assert.equal(limitWords(held('idle', 'in 2 hours')), 'Rate limited · resets in 2 hours');
  assert.equal(limitWords(held('idle', null)), 'Rate limited · waiting for it to lift');
  assert.equal(limitWords({ status: 'idle' }), null);
  assert.equal(limitSub(held('idle', '7pm')), 'limited · til 7pm');
  assert.equal(limitSub(held('idle', null)), 'rate limited');
  const l = houseLabel('wt-2', { ...held('idle', '7pm'), issue: 484 }, 'Show a rate limit');
  assert.deepEqual(l, { parts: ['wt-2', '· #484'], sub: 'limited · til 7pm' });
  assert.deepEqual(houseLabel('wt-3', held('idle', null), ''), { parts: ['wt-3'], sub: 'rate limited' });
});

test('the hall\'s sign goes up only when most of the village is limited', () => {
  const v = houses => ({ houses });
  assert.equal(villageLimit(v([held(), { status: 'idle' }])), null, 'half is not most');
  assert.deepEqual(villageLimit(v([held(), held('idle', '4pm'), { status: 'working' }])), { n: 2, of: 3, resets: '3pm (Europe/London)' });
  assert.deepEqual(villageLimit(v([held('idle', null), held(), { status: 'stopped' }])), { n: 2, of: 2, resets: '3pm (Europe/London)' }, 'stopped houses are not counted');
  assert.equal(villageLimit(v([])), null);
  assert.equal(signWords({ n: 2, of: 3, resets: '3pm' }), 'Usage limit: resumes ~3pm');
  assert.equal(signWords({ n: 2, of: 3, resets: 'in 40 minutes' }), 'Usage limit: resumes in 40 minutes');
  assert.equal(signWords({ n: 2, of: 3, resets: null }), 'Usage limit: resumes when it lifts');
  assert.equal(signWords(null), null);
});

test('the screensaver strip counts limited apart from blocked and idle', () => {
  const v = { project: 'shop', houses: [held(), held('idle', '4pm'), { status: 'idle' }, { status: 'blocked' }, { status: 'working' }], board: [] };
  const s = saverSummary(v);
  assert.equal(limitedCount(v), 2);
  assert.deepEqual([s.working, s.blocked, s.limited, s.idle], [1, 1, 2, 1]);
  assert.equal(s.waiting, 1, 'a limit is not waiting on you');
  assert.equal(s.limit, null, 'two of five is not most');
  const most = saverSummary({ project: 'shop', houses: [held(), held(), { status: 'working' }] });
  assert.equal(most.limit, 'Usage limit: resumes ~3pm (Europe/London)');
});

test('the well, its bubble and the hall sign are drawn in detail pixels', async () => {
  const { DETAIL } = await import(V + 'detail.js');
  const { state } = await import(V + 'state.js');
  const { THEMES } = await import(V + 'themes/registry.js');
  await import(V + 'themes/medieval.js');
  await import(V + 'themes/modern.js');
  await import(V + 'themes/future.js');
  const { well, limitBubble, limitSign, WELL } = await import(V + 'well.js');
  let scale = 1, x0 = 0, y0 = 0;
  const stack = [], rects = [];
  const c = {
    fillStyle: '#000',
    save() { stack.push([scale, x0, y0]); }, restore() { [scale, x0, y0] = stack.pop(); },
    translate(x, y) { x0 += x * scale; y0 += y * scale; }, scale(s) { scale *= s; },
    fillRect(x, y, w, h) { rects.push({ scale, x: x0 + x * scale, y: y0 + y * scale, w: w * scale, h: h * scale }); },
  };
  for (const theme of Object.values(THEMES)) {
    state.T = theme;
    rects.length = 0;
    well(c, 10, 20, theme);
    assert.ok(rects.length > 4, `${theme.name}: a well is drawn`);
    assert.ok(rects.every(r => r.scale === 1 / DETAIL), `${theme.name}: all at 2×`);
    // Inside its footprint.
    assert.ok(rects.every(r => r.x >= 10 && r.y >= 20 && r.x + r.w <= 10 + WELL.w && r.y + r.h <= 20 + WELL.h), `${theme.name}: within ${WELL.w}×${WELL.h}`);
  }
  rects.length = 0;
  limitBubble(c, 0, 0); limitSign(c, 0, 0);
  assert.ok(rects.length && rects.every(r => r.scale === 1 / DETAIL));
});
