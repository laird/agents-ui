// No sprite draws in scene pixels (#466, the last of #455's sub-tasks), under
// node:
//   node --test tools/village-tests/
// The whole village is painted, every layer main.js loads, in every theme,
// from recorded /api/village frames (tools/mock/recordings/ and the Omarchy
// widget's fixtures). A fake context follows the transform: a fillRect under
// the bare ×DETAIL transform (prepare(), detail.js) is drawn in scene pixels.
// Each is traced to the function that drew it, and none may come from a
// sprite: the buildings, crates, people and their props, and every mark #455
// redrew at 2× inside detail(), and the ground, roads, yards and water (#471).
// Scenery that still paints in scene pixels (trees, the lock, harbour and ship, the environment
// row, banners, barges, Medieval's decoration and hedge, shadows) is listed in
// SCENERY,
// so a new caller has to be named there or drawn at 2×.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import zlib from 'node:zlib';

// A DOM where every lookup answers with something that answers again: enough
// for the modules to load. Nothing here reads it back.
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

const V = new URL('../../src/web/village/', import.meta.url);
const mainJs = fs.readFileSync(new URL('main.js', V), 'utf8');
for (const [, m] of mainJs.matchAll(/^import '\.\/(.+)';$/gm)) await import(new URL(m, V));
const { state } = await import(new URL('state.js', V));
const { THEMES } = await import(new URL('themes/registry.js', V));
const { layout } = await import(new URL('layout.js', V));
const { drawVillage } = await import(new URL('registry.js', V));
const { DETAIL, prepare } = await import(new URL('detail.js', V));
const { arrangeDocks } = await import(new URL('docks.js', V));

/* Files whose drawing is all sprites: whatever they paint themselves must be
 * on the detail grid. */
const SPRITE_FILES = new Set([
  'sprites/houses.js', 'sprites/houseparts.js', 'themes/medievalbuildings.js', 'themes/futurebuildings.js',
  'themes/modernhouses.js', 'crates.js', 'people.js', 'people2x.js', 'cvdmarks.js', 'alarms.js', 'barrels.js',
  'bounces.js', 'bounceart.js', 'weather.js', 'weatherart.js', 'scoreboard.js', 'lifecycle.js', 'gatehouse.js',
]);
/* A theme's building slots, and scenery.js's bubbles, wherever they are defined. */
const SPRITE_FNS = /^(?:Object\.)?(?:hall|board|dock|scaffold|gatehouse|crate|bubble)\b|\[as (?:hall|board|dock|scaffold|gatehouse|crate)\]/;
/* What may still paint in scene pixels: scenery, not sprites (#455 left them
 * out). Each entry is the drawing function and its file. */
const SCENERY = [
  ['shadow', 'palette.js'], ['fogStop', 'features.js'], ['drawBanner', 'bannerart.js'],
  ['envKeep', 'envart.js'], ['envShed', 'envart.js'], ['envSign', 'envart.js'],
  ['lock', 'scenery.js'], ['harbour', 'scenery.js'], ['ship', 'scenery.js'], ['boat', 'scenery.js'],
  ['drawBarge', 'features.js'], ['poleFlag', 'envart.js'], ['draw', 'envrow.js'],
];
const SCENERY_SLOTS = ['gate', 'destination', 'vessel', 'supply'];

/* One stack frame: [function name, file under src/web/village/]. */
function frames(stack) {
  return stack.split('\n').slice(1).map(l => {
    const m = /at (?:(\S+(?: \[as \w+\])?) \()?.*\/web\/village\/([^:]+):\d+:\d+\)?$/.exec(l.trim());
    return m ? [m[1] || '', m[2]] : null;
  }).filter(Boolean);
}
/* Who drew a scene-pixel rect: the first frame past palette.js's R(), and
 * whether that is a sprite. */
function blame(stack) {
  const fs_ = frames(stack);
  let i = fs_.findIndex(([fn, file]) => !(file === 'palette.js' && fn === 'R'));
  // An anonymous callback (a forEach) draws for the named function around it, in its file.
  while (fs_[i] && !fs_[i][0] && fs_[i + 1] && fs_[i + 1][1] === fs_[i][1]) i++;
  const [fn, file] = fs_[i] || ['?', '?'];
  const bare = fn.replace(/^Object\./, '').replace(/ \[as .*\]$/, '');
  const slot = (/\[as (\w+)\]/.exec(fn) || [, bare])[1];
  const scenery = SCENERY.some(([n, f]) => n === bare && f === file) || (file.startsWith('themes/') && SCENERY_SLOTS.includes(slot));
  // A sprite slot anywhere up the stack (a building's own helper) still counts.
  const sprite = !scenery && (SPRITE_FILES.has(file) || fs_.slice(i).some(([f]) => SPRITE_FNS.test(f)));
  return { who: `${fn || '<anonymous>'} (${file})`, sprite, scenery };
}

/* A 2D context that keeps nothing but who painted in scene pixels. */
function context() {
  let m = [1, 0, 0, 1, 0, 0];
  const stack = [], found = new Map();
  const c = new Proxy({
    fillStyle: '', globalAlpha: 1, found,
    setTransform(a, b, cc, d, e, f) { m = [a, b, cc, d, e, f]; },
    save() { stack.push([...m]); }, restore() { m = stack.pop() || m; },
    translate(x, y) { m = [m[0], m[1], m[2], m[3], m[4] + m[0] * x, m[5] + m[3] * y]; },
    scale(sx, sy) { m = [m[0] * sx, m[1], m[2], m[3] * sy, m[4], m[5]]; },
    measureText: () => ({ width: 4 }),
    createLinearGradient: () => ({ addColorStop() {} }),
    fillRect() {
      if (m[0] !== DETAIL) return;
      const b = blame(new Error().stack);
      const k = b.who;
      found.set(k, { ...b, n: (found.get(k)?.n || 0) + 1 });
    },
  }, { get: (t, k) => (k in t ? t[k] : () => {}), set: (t, k, v) => { t[k] = v; return true; } });
  prepare(c, DETAIL);
  return c;
}

/* Recorded villages: every 8th frame of each mock recording, and the widget's fixtures. */
const REC = new URL('../mock/recordings/', import.meta.url), FIX = new URL('../omarchy-plugin/agents-village/tests/fixtures/', import.meta.url);
const villages = [];
for (const f of fs.readdirSync(REC).filter(f => f.endsWith('.json.gz'))) {
  const runs = JSON.parse(zlib.gunzipSync(fs.readFileSync(new URL(f, REC))));
  for (const frames of Object.values(runs)) frames.filter((_, i) => i % 8 === 0).forEach(fr => villages.push(...fr.villages));
}
for (const f of ['village.json', 'village-build-running.json']) villages.push(...JSON.parse(fs.readFileSync(new URL(f, FIX))).villages);

function paintAll(T) {
  state.T = T; state.reduce = false;
  const c = context();
  for (const v0 of villages) {
    const v = structuredClone(v0);
    v.backlog = arrangeDocks(v.backlog || []);
    const sc = { v, L: layout(v), porters: [], floats: [], gates: {}, voyage: null, supplies: [], nextAt: 0, fireworks: -1e9, liveSince: {}, fade: {} };
    for (const t of [0, 650]) drawVillage(c, sc, t);
  }
  return c.found;
}

test('the recordings give the scene something to draw', () => {
  assert.ok(villages.length > 20, `${villages.length} villages`);
});

for (const [name, T] of Object.entries(THEMES)) {
  test(`${name}: no sprite draws in scene pixels`, () => {
    const found = [...paintAll(T).values()];
    const sprites = found.filter(f => f.sprite).map(f => `${f.who} ×${f.n}`);
    assert.deepEqual(sprites, [], 'drawn in scene pixels, not in detail()');
    // Everything else that still does is named scenery: a new one is a choice, not an accident.
    const unnamed = found.filter(f => !f.sprite && !f.scenery).map(f => `${f.who} ×${f.n}`);
    assert.deepEqual(unnamed, [], 'not in SCENERY: draw it at 2×, or name it there');
  });
}
