// A worker control's result, shown at once (src/web/village/controlfx.js, #507), under node:
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`. What each action shows, how
// long the local override lasts, the reduced-motion toast, and the dark
// house painted with the clock pinned (#450).
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= {}; // state.js reads window.matchMedia, when there is one
const V = '../../src/web/village/';
const { DETAIL, prepare } = await import(V + 'detail.js');
const { barrelFill } = await import(V + 'barrels.js');
const { CONTROL_FX, CONTROL_FX_MS, DARK, darkHouse, fxContext, fxOn, fxRest, fxSnapshot, fxToast, startFx } = await import(V + 'controlfx.js');
const { restOf } = await import(V + 'rest.js');

const idle = { id: 'wt-1', status: 'idle', issue: null, context_pct: 64 };
const working = { id: 'wt-2', status: 'working', issue: 42, context_pct: 80 };

test('each action has its visible result, in one table', () => {
  assert.deepEqual(Object.keys(CONTROL_FX).sort(), ['nudge', 'release', 'restart', 'stop']);
  assert.equal(CONTROL_FX.nudge.up, true);
  assert.equal(CONTROL_FX.release.carry, true);
  assert.ok(CONTROL_FX.restart.up && CONTROL_FX.restart.empty);
  assert.equal(CONTROL_FX.stop.dark, true);
  assert.equal(startFx('shop', idle, 'dance', 0, new Map()), null);
});

test('a nudged worker gets up: it stands at its door instead of sleeping', () => {
  const live = new Map(), fx = startFx('shop', idle, 'nudge', 1000, live);
  assert.equal(restOf(idle, Date.now()), 'asleep');
  assert.equal(fxRest(fxOn('shop', idle, 1500, live), restOf(idle, Date.now())), 'fresh');
  assert.equal(fxRest(null, 'asleep'), 'asleep');
  assert.equal(fxContext(fx, 64), 64);
});

test('a restart empties the barrel; an unknown context stays unknown', () => {
  const live = new Map(), fx = startFx('shop', working, 'restart', 0, live);
  assert.equal(barrelFill(fxContext(fx, working.context_pct)), 0);
  assert.equal(fxContext(fx, null), null);
});

test('the override is keyed by project and house', () => {
  const live = new Map();
  startFx('shop', idle, 'nudge', 0, live);
  assert.ok(fxOn('shop', idle, 10, live));
  assert.equal(fxOn('other', idle, 10, live), null);
  assert.equal(fxOn('shop', { ...idle, id: 'wt-9' }, 10, live), null);
});

test('it expires after a few seconds', () => {
  const live = new Map();
  startFx('shop', idle, 'nudge', 1000, live);
  assert.ok(CONTROL_FX_MS >= 2000 && CONTROL_FX_MS <= 10000);
  assert.ok(fxOn('shop', idle, 1000 + CONTROL_FX_MS - 1, live));
  assert.equal(fxOn('shop', idle, 1000 + CONTROL_FX_MS, live), null);
  assert.equal(live.size, 0);
});

test('it ends when a poll shows the house changed: the real state says it', () => {
  const live = new Map();
  startFx('shop', idle, 'nudge', 0, live);
  // The same state polled again keeps it.
  assert.ok(fxOn('shop', { ...idle }, 100, live));
  assert.equal(fxOn('shop', { ...idle, status: 'working' }, 200, live), null);
  assert.equal(live.size, 0);
  startFx('shop', working, 'release', 0, live);
  assert.equal(fxOn('shop', { ...working, issue: null }, 100, live), null);
  assert.notEqual(fxSnapshot(working), fxSnapshot({ ...working, context_pct: 3 }));
});

test('with reduced motion a toast says what happened', () => {
  assert.equal(fxToast('nudge', 'wt-1'), 'Nudged wt-1: it gets up and starts its loop.');
  assert.equal(fxToast('release', 'wt-2', 42), 'Released #42 from wt-2: the crate goes back to the docks.');
  assert.equal(fxToast('release', 'wt-2', null), 'Released its issue from wt-2: the crate goes back to the docks.');
  assert.equal(fxToast('restart', 'wt-3'), 'Restarted wt-3: a fresh session, its context emptied.');
  assert.equal(fxToast('stop', 'wt-4'), 'Stopped wt-4: the house goes dark.');
  assert.equal(fxToast('dance', 'wt-4'), null);
});

test('a stopped house goes dark over its lot (clock pinned, #450)', () => {
  const realNow = performance.now;
  performance.now = () => 12345;
  try {
    const W = 60, H = 60, px = new Map();
    let m = [1, 0, 0, 1, 0, 0];
    const stack = [];
    const c = {
      fillStyle: '#000', imageSmoothingEnabled: true,
      setTransform(a, b, cc, d, e, f) { m = [a, b, cc, d, e, f]; },
      save() { stack.push([...m]); }, restore() { m = stack.pop(); },
      translate(x, y) { m = [m[0], m[1], m[2], m[3], m[4] + m[0] * x, m[5] + m[3] * y]; },
      scale(sx, sy) { m = [m[0] * sx, m[1], m[2], m[3] * sy, m[4], m[5]]; },
      fillRect(x, y, rw, rh) {
        const x0 = m[0] * x + m[4], y0 = m[3] * y + m[5];
        for (let yy = Math.round(y0); yy < Math.round(y0 + m[3] * rh); yy++)
          for (let xx = Math.round(x0); xx < Math.round(x0 + m[0] * rw); xx++)
            if (xx >= 0 && yy >= 0 && xx < W && yy < H) px.set(`${xx},${yy}`, c.fillStyle);
      },
    };
    prepare(c);
    darkHouse(c, 4, 8);
    const d = DETAIL;
    // The house's middle is shaded; well outside its lot nothing is.
    assert.equal(px.get(`${(4 + 8) * d},${(8 + 4) * d}`), DARK);
    assert.equal(px.get(`${(4 + 8) * d},${(8 + 22) * d}`), undefined);
  } finally {
    performance.now = realNow;
  }
});
