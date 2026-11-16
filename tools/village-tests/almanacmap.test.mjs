// The almanac on the map (#499), under node:
//   node --test tools/village-tests/
// Every village of the mock recordings is laid out and painted with a
// `github` summary (#498) and without one, in every theme: with one, the
// almanac has a clearing at the east edge, clear of the river, the quay and
// the environment row, it is clickable, and it draws in detail pixels; with
// none, there is no almanac and nothing is drawn for it.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { loadPage } from './modules.mjs';
import zlib from 'node:zlib';

const stub = () => new Proxy(function () {}, {
  get(t, k) {
    if (k === Symbol.toPrimitive) return () => '';
    if (k === Symbol.iterator) return function* () {};
    if (k === 'length') return 0;
    if (k === 'then') return undefined;
    return stub();
  },
  apply: () => stub(), construct: () => stub(), set: () => true,
});
globalThis.document = stub();
globalThis.window = stub();
globalThis.localStorage = stub();
globalThis.location = { hash: '', search: '' };
globalThis.requestAnimationFrame = () => 0;
performance.now = () => 0; // pinned: the drawing must not depend on the frame (#450)

const V = new URL('../../src/web/village/', import.meta.url);
await loadPage(); // every module main.js loads (modules.mjs)
const { state } = await import(new URL('state.js', V));
const { THEMES } = await import(new URL('themes/registry.js', V));
const { layout } = await import(new URL('layout.js', V));
const { drawVillage } = await import(new URL('registry.js', V));
const { DETAIL, prepare } = await import(new URL('detail.js', V));
const { arrangeDocks } = await import(new URL('docks.js', V));

const REC = new URL('../mock/recordings/', import.meta.url);
const villages = [];
for (const f of fs.readdirSync(REC).filter(f => f.endsWith('.json.gz'))) {
  const runs = JSON.parse(zlib.gunzipSync(fs.readFileSync(new URL(f, REC))));
  for (const frames of Object.values(runs)) frames.filter((_, i) => i % 16 === 0).forEach(fr => villages.push(...fr.villages));
}
const GITHUB = {
  as_of: '2026-11-16T06:00:00-05:00', median_claim_to_land_secs: 5400,
  days: Array.from({ length: 14 }, (_, k) => ({ date: `2026-11-${String(3 + k).padStart(2, '0')}`, landed: k % 6, opened: 1, bounced: k % 4 === 0 ? 2 : 0, reverted: k === 7 ? 1 : 0, deploys: 1 })),
};

/* A context that counts fillRects drawn in scene pixels, and how many in all. */
function context() {
  let m = [1, 0, 0, 1, 0, 0];
  const stack = [], c = new Proxy({
    fillStyle: '', globalAlpha: 1, scenePx: 0, all: 0,
    setTransform(a, b, cc, d, e, f) { m = [a, b, cc, d, e, f]; },
    save() { stack.push([...m]); }, restore() { m = stack.pop() || m; },
    translate(x, y) { m = [m[0], m[1], m[2], m[3], m[4] + m[0] * x, m[5] + m[3] * y]; },
    scale(sx, sy) { m = [m[0] * sx, m[1], m[2], m[3] * sy, m[4], m[5]]; },
    measureText: () => ({ width: 4 }),
    createLinearGradient: () => ({ addColorStop() {} }),
    fillRect() { c.all++; if (m[0] === DETAIL && new Error().stack.includes('almanac')) c.scenePx++; },
  }, { get: (t, k) => (k in t ? t[k] : () => {}), set: (t, k, v) => { t[k] = v; return true; } });
  prepare(c, DETAIL);
  return c;
}
function paint(v0, T, github) {
  state.T = T; state.reduce = false;
  const v = structuredClone(v0);
  if (github) v.github = github; else delete v.github;
  v.backlog = arrangeDocks(v.backlog || []);
  const sc = { v, L: layout(v), porters: [], floats: [], gates: {}, voyage: null, supplies: [], nextAt: 0, fireworks: -1e9, liveSince: {}, fade: {} };
  const c = context();
  for (const t of [0, 500]) drawVillage(c, sc, t);
  return { sc, c };
}

test('the recordings give the almanac villages to stand in', () => {
  assert.ok(villages.length > 10, `${villages.length} villages`);
});

for (const [name, T] of Object.entries(THEMES)) {
  test(`${name}: the almanac stands at the east edge, clickable, drawn at 2×`, () => {
    for (const v of villages) {
      const { sc, c } = paint(v, T, GITHUB), a = sc.L.almanac;
      assert.ok(a, `${v.project}: an almanac`);
      assert.ok(a.x + a.w <= sc.L.devW && a.y >= 14, `${v.project}: inside the dev village, under the river`);
      assert.ok(a.y + a.h + 8 <= sc.L.quayY - 12, `${v.project}: clear of the quay, with room for its label`);
      assert.ok(sc.L.lots.every(l => l.x + 20 <= a.x), `${v.project}: east of every house`);
      assert.ok(sc.hits.some(h => h.o.kind === 'almanac'), `${v.project}: clickable`);
      assert.equal(c.scenePx, 0, 'drawn in detail(), never in scene pixels');
    }
  });
  test(`${name}: no github summary, no almanac and no error`, () => {
    for (const v of villages) {
      const { sc } = paint(v, T, null);
      assert.equal(sc.L.almanac, null);
      assert.ok(!sc.hits.some(h => h.o.kind === 'almanac'));
    }
  });
}
