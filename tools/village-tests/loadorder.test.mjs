// The village's modules load in an order build.rs generates (#520): every
// module, sorted by path, after the themes and layout.js (main.js). So no
// list a module registers into may depend on that order: each `order` given
// to registerKey, registerLabels and registerOverlay is distinct, and the
// themes register in the order the Style picker lists them.
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { VILLAGE, generatedModules, loadPage, pageModules, villageScripts } from './modules.mjs';

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
performance.now = () => 0;

await loadPage();
const { KEYS, LABELS, OVERLAYS } = await import(new URL('registry.js', VILLAGE));
const { THEMES } = await import(new URL('themes/registry.js', VILLAGE));

const ties = list => {
  const seen = new Map(), dup = [];
  list.forEach((e, i) => { if (seen.has(e.order)) dup.push(`${e.order} (#${seen.get(e.order)} and #${i}${e.group ? ': ' + e.group : ''})`); else seen.set(e.order, i); });
  return dup;
};

test('every key group, label set and overlay has an order of its own', () => {
  assert.deepEqual(ties(KEYS), [], 'two registerKey calls share an order: give the new one its own (`order`)');
  assert.deepEqual(ties(LABELS), [], 'two registerLabels calls share an order: give the new one its own');
  assert.deepEqual(ties(OVERLAYS), [], 'two registerOverlay calls share an order: give the new one its own');
  assert.ok(KEYS.length >= 10 && LABELS.length >= 10 && OVERLAYS.length >= 6, 'the whole page loaded');
});

test("the key's groups keep the order they had when main.js listed every module", () => {
  const groups = KEYS.map(k => k.group);
  assert.deepEqual(groups.slice(0, 2), ["Beside a house: its agent's context", 'Weather: the build']);
  assert.equal(groups[groups.length - 1], 'Keyboard');
});

test('the themes register in the Style picker order', () => {
  assert.deepEqual(Object.keys(THEMES).slice(0, 3), ['medieval', 'modern', 'future']);
});

test('main.js loads the themes and layout.js, then every module', () => {
  const page = pageModules();
  assert.deepEqual(page.slice(0, 4), ['themes/medieval.js', 'themes/modern.js', 'themes/future.js', 'layout.js']);
  const main = fs.readFileSync(new URL('main.js', VILLAGE), 'utf8');
  assert.match(main, /^import '\.\/modules\.js';$/m);
  // Every script but main.js, so a new module needs no edit to load.
  assert.deepEqual(generatedModules(), villageScripts().filter(f => f !== 'main.js'));
  assert.ok(generatedModules().includes('themes/medieval.js') && generatedModules().includes('sprites/houses.js'));
  for (const f of villageScripts().filter(f => f !== 'main.js')) assert.ok(page.includes(f), `${f} is never loaded`);
});
