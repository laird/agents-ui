// Tools, props, drones and action signs at 2× (src/web/village/people.js,
// #460), under node:
//   node --test tools/village-tests/
// Each is painted onto a pixel grid (a fake context that rasterises fillRect
// through the detail transform) in every theme: nothing new is drawn in scene
// pixels, and each stays on the scene pixels it always covered.
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= {}; // state.js reads window.matchMedia, when there is one
const V = '../../src/web/village/';
const { DETAIL, prepare } = await import(V + 'detail.js');
const { state } = await import(V + 'state.js');
const { person, drawAction, actionSign, ACTION_ICONS } = await import(V + 'people.js');
const { THEMES } = await import(V + 'themes/registry.js');
await import(V + 'themes/medieval.js');
await import(V + 'themes/modern.js');
await import(V + 'themes/future.js');

function grid(w = 120, h = 120) {
  let m = [1, 0, 0, 1, 0, 0];
  const stack = [], px = new Map(), scales = [];
  const c = {
    fillStyle: '#000', globalAlpha: 1, imageSmoothingEnabled: true,
    setTransform(a, b, cc, d, e, f) { m = [a, b, cc, d, e, f]; },
    save() { stack.push([...m]); }, restore() { m = stack.pop(); },
    translate(x, y) { m = [m[0], m[1], m[2], m[3], m[4] + m[0] * x + m[2] * y, m[5] + m[1] * x + m[3] * y]; },
    scale(sx, sy) { m = [m[0] * sx, m[1] * sx, m[2] * sy, m[3] * sy, m[4], m[5]]; },
    beginPath() {}, ellipse() {}, fill() {}, arc() {}, closePath() {}, moveTo() {}, lineTo() {},
    fillRect(x, y, rw, rh) {
      scales.push(m[0]);
      const x0 = m[0] * x + m[4], y0 = m[3] * y + m[5];
      for (let yy = Math.round(y0); yy < Math.round(y0 + m[3] * rh); yy++)
        for (let xx = Math.round(x0); xx < Math.round(x0 + m[0] * rw); xx++)
          if (xx >= 0 && yy >= 0 && xx < w && yy < h) px.set(`${xx},${yy}`, c.fillStyle);
    },
    px, scales,
  };
  prepare(c, DETAIL);
  return c;
}
const sceneRects = c => c.scales.filter(s => s !== 1).length;
/* Pixels painted in `c` and not in `base`: what a prop added. */
const added = (c, base) => [...c.px.entries()].filter(([k, v]) => base.px.get(k) !== v).map(([k]) => k.split(',').map(Number));
function inside(pts, x, y, w, h, what) {
  assert.ok(pts.length > 0, `${what}: drew something`);
  pts.forEach(([px, py]) => assert.ok(px >= 2 * x && px < 2 * (x + w) && py >= 2 * y && py < 2 * (y + h), `${what}: (${px}, ${py}) outside scene ${[x, y, w, h]}`));
}
/* Everyone stands at scene (20, 20), detail (40, 40). */
const X = 20, Y = 20;

for (const [name, T] of Object.entries(THEMES)) {
  const kinds = T.robots ? [['work', {}], ['wave', {}], ['wave2', { status: 'blocked' }], ['stand', { status: 'done' }], ['carry', {}], ['stand', { mayor: true }]]
    : [['work', {}], ['work2', {}], ['wave', {}], ['wave2', {}]];
  test(`${name}: tools and props over the body draw in detail pixels only`, () => {
    state.T = T; state.reduce = true;
    const base = grid(); person(base, X, Y, 'agent-7', 'stand', {});
    for (const [kind, opts] of kinds) {
      const c = grid(); person(c, X, Y, 'agent-7', kind, opts);
      assert.equal(sceneRects(c), sceneRects(base), `${kind} ${JSON.stringify(opts)}: only the shadow is in scene pixels`);
      // A prop reaches at most a few scene pixels beyond the 8 × 12 body, as it always did.
      inside(added(c, base), X - 3, Y - 6, 19, 18, `${kind} ${JSON.stringify(opts)}`);
    }
  });

  test(`${name}: every action's prop draws at 2× beside the person`, () => {
    state.T = T; state.reduce = true;
    for (const action of Object.keys(ACTION_ICONS)) for (const t of [0, 240]) {
      const c = grid(); drawAction(c, X, Y, action, t);
      assert.equal(sceneRects(c), 0, `${action}: no scene pixels`);
      inside(added(c, grid()), X, Y - 8, 15, 20, action);
    }
  });

  test(`${name}: an action sign is 7 × 7, its icon in the theme's ink`, () => {
    state.T = T;
    const [frame, face, ink] = T.sign;
    for (const action of Object.keys(ACTION_ICONS)) {
      const c = grid(); actionSign(c, X, Y, action);
      assert.equal(sceneRects(c), 0);
      inside([...c.px.keys()].map(k => k.split(',').map(Number)), X, Y, 7, 7, action);
      assert.equal(c.px.get(`${2 * X},${2 * Y}`), frame);
      const inked = [...c.px.values()].filter(v => v === ink).length;
      assert.ok(inked >= 12, `${action}: the icon shows (${inked} ink pixels)`);
    }
    assert.ok(face);
  });
}

test('each action icon is drawn for the 2× grid: 10 rows of 10', () => {
  for (const [action, rows] of Object.entries(ACTION_ICONS)) {
    assert.equal(rows.length, 10, action);
    rows.forEach(r => assert.match(r, /^[#+.]{10}$/, action));
  }
});

test("future: a sub-agent's drone draws at 2× in its specialty's colour", () => {
  state.T = THEMES.future; state.reduce = true;
  const c = grid(); person(c, X, Y, 'sub-1', 'stand', { sub: true, accent: '#e070d0' });
  // The drone's own shell is all detail pixels; its shadow is the one scene rect set.
  const base = grid(); person(base, X, Y, 'agent-7', 'stand', {});
  assert.ok(sceneRects(c) <= sceneRects(base));
  assert.ok([...c.px.values()].includes('#e070d0'));
  inside([...c.px.entries()].filter(([, v]) => v === '#e070d0').map(([k]) => k.split(',').map(Number)), X + 1, Y + 3, 8, 6, 'drone');
});
