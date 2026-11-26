// A world's loop (src/web/village/worldloop.js, #560): channel crates, a
// verified fix's sparkles, and the bar's counts, under node:
//   node --test tools/village-tests/*.test.mjs
// Times are passed in: nothing here reads the clock.
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= {}; // state.js reads window.matchMedia, when there is one
const V = '../../src/web/village/';
const { SPARKLE_MS, channelBadge, channelColour, floatCrate, foldWorld, newWorldState, sparkling, worldWords } = await import(V + 'worldloop.js');
const { crate } = await import(V + 'crates.js');
const { FEATURE_COLS } = await import(V + 'palette.js');

/* A context that records every detail pixel's last colour (scale and
 * translate only, as detail() uses). */
function grid() {
  let m = [1, 1, 0, 0];
  const stack = [], px = new Map();
  const c = {
    fillStyle: '', imageSmoothingEnabled: false,
    save() { stack.push([...m]); }, restore() { m = stack.pop(); },
    translate(x, y) { m = [m[0], m[1], m[2] + m[0] * x, m[3] + m[1] * y]; },
    scale(sx, sy) { m = [m[0] * sx, m[1] * sy, m[2], m[3]]; },
    fillRect(x, y, w, h) {
      const x0 = Math.round(2 * (m[0] * x + m[2])), y0 = Math.round(2 * (m[1] * y + m[3]));
      for (let i = 0; i < Math.round(2 * m[0] * w); i++) for (let j = 0; j < Math.round(2 * m[1] * h); j++) px.set(`${x0 + i},${y0 + j}`, c.fillStyle);
    },
  };
  return { c, colours: () => new Set(px.values()), px };
}
const T = { crate };
const world = (incidents, more = {}) => ({ name: 'world:shop', dev: 'shop', platform: 'agentex:shop', incidents, verified: [], landed: null, ...more });

test('two channels float two crates with different badges and colours', () => {
  const st = newWorldState();
  foldWorld(st, world([]), 0);
  foldWorld(st, world([
    { number: 60, origin: 'task-8815', channel: 'log-analyzer' },
    { number: 61, origin: null, channel: 'user-feedback' },
  ]), 100);
  assert.deepEqual(st.floaters.map(f => [f.n, f.channel]), [[60, 'log-analyzer'], [61, 'user-feedback']]);
  assert.deepEqual(st.floaters.map(f => channelBadge(f.channel)), ['LA', 'UF']);
  const [a, b] = st.floaters.map(f => { const g = grid(); floatCrate(g.c, T, 10, 20, f); return g; });
  const [ca, cb] = st.floaters.map(f => channelColour(f.channel));
  assert.notEqual(ca, cb);
  assert.ok(FEATURE_COLS.includes(ca) && FEATURE_COLS.includes(cb));
  assert.ok(a.colours().has(ca) && !a.colours().has(cb), 'each crate wears its own channel colour');
  assert.ok(b.colours().has(cb) && !b.colours().has(ca));
  assert.notDeepEqual([...a.px.entries()], [...b.px.entries()]);
});

test('a channel keeps its badge and colour, whatever else is afloat', () => {
  assert.equal(channelColour('log-analyzer'), channelColour('log-analyzer'));
  assert.equal(channelBadge('billing'), 'BI');
  assert.equal(channelBadge('user_feedback'), 'UF');
});

test('an incident without a channel floats the crate it always did', () => {
  const plain = grid(), crateOnly = grid();
  floatCrate(plain.c, T, 10, 20, { n: 501 });
  crate(crateOnly.c, 10, 20, 'bug', 'P0', false, 501);
  assert.deepEqual([...plain.px.entries()], [...crateOnly.px.entries()]);
});

test('a verified fix stops its crate and sparkles the house that raised it, once', () => {
  const st = newWorldState();
  const asking = { number: 60, origin: 'task-8815', channel: 'log-analyzer' };
  foldWorld(st, world([]), 0);
  foldWorld(st, world([asking]), 100);
  assert.equal(st.floaters.length, 1);
  foldWorld(st, world([], { verified: [{ ...asking, verified_at: '2026-11-20T10:00:00Z' }] }), 1000);
  assert.deepEqual(st.floaters, [], 'it no longer floats');
  assert.deepEqual(sparkling(st, 1000 + SPARKLE_MS / 2), ['task-8815']);
  assert.deepEqual(sparkling(st, 1000 + SPARKLE_MS), [], 'then stops');
  foldWorld(st, world([], { verified: [{ ...asking }] }), 9000);
  assert.deepEqual(sparkling(st, 9001), [], 'a later poll does not sparkle it again');
  // A verified incident never seen floating still sparkles; one from the gate has no house to.
  foldWorld(st, world([], { verified: [{ number: 65, origin: 'task-8812' }, { number: 66, origin: null }] }), 20000);
  assert.deepEqual(sparkling(st, 20001), ['task-8812']);
});

test('the release still lands as it did', () => {
  const st = newWorldState();
  assert.equal(foldWorld(st, world([], { landed: false }), 0), false);
  assert.equal(foldWorld(st, world([], { landed: true }), 50), true);
  assert.equal(st.upgradedAt, 50);
});

test("the bar counts the window's loop after the release", () => {
  const w = world([], { release: '1f2e3d4c5b6a', landed: true, platform_builds: ['1f2e3d4c5b6a'], counts: { raised: 5, landed: 4, deployed: 2, verified: 1 } });
  assert.equal(worldWords(w), '· shop builds agentex:shop · release 1f2e3d4 running on the platform · platform builds 1f2e3d4 · raised 5 · landed 4 · deployed 2 · verified 1');
  const { counts, ...old } = w;
  assert.equal(worldWords(old), '· shop builds agentex:shop · release 1f2e3d4 running on the platform · platform builds 1f2e3d4');
});
