// Researcher, analyst and auditor sub-agents carry their own prop in every
// theme (src/web/village/subprops.js, #553), under node:
//   node --test tools/village-tests/subprops.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= {}; // state.js reads window.matchMedia, when there is one
performance.now = () => 0; // drones bob and rotors spin: pin the clock (#450)
const V = '../../src/web/village/';
const { DETAIL, prepare } = await import(V + 'detail.js');
const { state } = await import(V + 'state.js');
const { person } = await import(V + 'people.js');
const { P, subHat, subRole } = await import(V + 'palette.js');
const { THEMES } = await import(V + 'themes/registry.js');
await import(V + 'themes/medieval.js');
await import(V + 'themes/modern.js');
await import(V + 'themes/future.js');

/* A fake context that rasterises fillRect through the transform: canvas pixel → colour. */
function grid(w = 80, h = 80) {
  let m = [1, 0, 0, 1, 0, 0];
  const stack = [], px = new Map();
  const c = {
    fillStyle: '#000', globalAlpha: 1, imageSmoothingEnabled: true,
    setTransform(a, b, cc, d, e, f) { m = [a, b, cc, d, e, f]; },
    save() { stack.push([...m]); }, restore() { m = stack.pop(); },
    translate(x, y) { m = [m[0], m[1], m[2], m[3], m[4] + m[0] * x + m[2] * y, m[5] + m[1] * x + m[3] * y]; },
    scale(sx, sy) { m = [m[0] * sx, m[1] * sx, m[2] * sy, m[3] * sy, m[4], m[5]]; },
    beginPath() {}, ellipse() {}, fill() {}, arc() {}, closePath() {}, moveTo() {}, lineTo() {},
    fillRect(x, y, rw, rh) {
      const x0 = m[0] * x + m[4], y0 = m[3] * y + m[5];
      for (let yy = Math.round(y0); yy < Math.round(y0 + m[3] * rh); yy++)
        for (let xx = Math.round(x0); xx < Math.round(x0 + m[0] * rw); xx++)
          if (xx >= 0 && yy >= 0 && xx < w && yy < h) px.set(`${xx},${yy}`, c.fillStyle);
    },
    px,
  };
  prepare(c, DETAIL);
  return c;
}
const X = 20, Y = 20;
/* A sub-agent of `type` as the village draws it (lifecycle.js). */
function sub(type, kind = 'stand') {
  const sa = { agent_type: type };
  const c = grid(); person(c, X, Y, 'sub-1', kind, { accent: subHat(sa), prop: subRole(sa), sub: true });
  return c;
}
/* What a drawing has that a plain sub-agent of the same seed lacks. */
const extra = (c, base) => [...c.px.entries()].filter(([k, v]) => base.px.get(k) !== v);
const key = pts => pts.map(([k, v]) => `${k}=${v}`).sort().join(' ');

const KINDS = ['research:researcher', 'research:Analyst', 'RESEARCH:AUDITOR'];

test('the three kinds resolve to their specialty, case-insensitively, after the colon', () => {
  assert.deepEqual(KINDS.map(t => subRole({ agent_type: t })), ['researcher', 'analyst', 'auditor']);
  assert.equal(subRole({ agent_type: 'research:summariser' }), null);
  assert.equal(subHat({ agent_type: 'research:summariser' }), P.white);
});

for (const [name, T] of Object.entries(THEMES)) {
  test(`${name}: each new kind holds its own prop at its left side`, () => {
    state.T = T; state.reduce = true;
    const props = KINDS.map(type => {
      for (const kind of ['stand', 'wave']) {
        const base = grid(); person(base, X, Y, 'sub-1', kind, { accent: subHat({ agent_type: type }), sub: true });
        const pts = extra(sub(type, kind), base);
        assert.ok(pts.length >= 12, `${type} ${kind}: a prop is drawn (${pts.length} pixels)`);
        // Left of the body, by the hand: detail x -6..-1 and y 10..19 from its top left.
        pts.forEach(([k]) => {
          const [px, py] = k.split(',').map(Number);
          assert.ok(px >= 2 * X - 6 && px < 2 * X && py >= 2 * Y + 10 && py < 2 * Y + 20, `${type} ${kind}: (${px}, ${py}) beside the hand`);
        });
      }
      const base = grid(); person(base, X, Y, 'sub-1', 'stand', { accent: subHat({ agent_type: type }), sub: true });
      return key(extra(sub(type), base));
    });
    assert.equal(new Set(props).size, 3, 'researcher, analyst and auditor look different');
  });

  test(`${name}: an unknown kind, or an older specialist, is drawn with no prop`, () => {
    state.T = T; state.reduce = true;
    const plain = grid(); person(plain, X, Y, 'sub-1', 'stand', { accent: P.white, sub: true });
    assert.equal(key(extra(sub('research:summariser'), plain)), '', 'a plain villager');
    for (const type of ['modernize:architect', 'modernize:tester']) {
      const base = grid(); person(base, X, Y, 'sub-1', 'stand', { accent: subHat({ agent_type: type }), sub: true });
      assert.equal(key(extra(sub(type), base)), '', `${type}: its accent only`);
    }
  });
}

test('each kind has a different prop in each theme', () => {
  state.reduce = true;
  for (const type of KINDS) {
    const looks = Object.values(THEMES).map(T => {
      state.T = T;
      const base = grid(); person(base, X, Y, 'sub-1', 'stand', { accent: subHat({ agent_type: type }), sub: true });
      return key(extra(sub(type), base));
    });
    assert.equal(new Set(looks).size, Object.keys(THEMES).length, type);
  }
});
