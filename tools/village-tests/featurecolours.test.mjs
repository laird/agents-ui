// Each epic's flag colour (src/web/village/palette.js, #406):
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { FEATURE_COLS, P, F, featureColour, featureColours } from '../../src/web/village/palette.js';
// modern.js reads state.js, which asks the window about reduced motion.
globalThis.window ??= { matchMedia: () => ({ matches: false }) };
const { M } = await import('../../src/web/village/themes/modern.js');

const fs = (...ns) => ns.map(number => ({ number }));
const N = FEATURE_COLS.length;

test('twelve colours', () => assert.equal(N, 12));

test('an epic keeps its colour as others come and go, and by number, not position', () => {
  const alone = featureColour({ number: 367 }, fs(367));
  assert.equal(alone, FEATURE_COLS[367 % N]);
  assert.equal(featureColour({ number: 367 }, fs(100, 367, 405)), alone);
  assert.equal(featureColour({ number: 367 }, fs(405, 367)), alone, 'order shown does not matter');
  assert.equal(featureColour({ number: 367 }, fs(401, 402, 403, 367)), alone);
});

test('the two yellow flags of the bug report now differ', () => {
  const c = featureColours(fs(367, 376));
  assert.notEqual(c.get(367), c.get(376));
});

test('no two visible epics share a colour', () => {
  const shown = fs(5, 17, 29, 41, 8, 20, 300, 12);
  const c = featureColours(shown);
  assert.equal(new Set(shown.map(f => c.get(f.number))).size, shown.length);
});

test('a collision is settled the same way every time: the oldest keeps its colour', () => {
  // 5 and 17 share home colour 5; 29 too.
  const a = featureColours(fs(29, 17, 5)), b = featureColours(fs(5, 17, 29));
  assert.deepEqual([...a].sort(), [...b].sort());
  assert.equal(a.get(5), FEATURE_COLS[5]);
  assert.equal(a.get(17), FEATURE_COLS[6]);
  assert.equal(a.get(29), FEATURE_COLS[7]);
  // When the oldest goes, the next one takes the home colour back.
  assert.equal(featureColours(fs(17, 29)).get(17), FEATURE_COLS[5]);
});

test('more than twelve epics: neighbours never repeat', () => {
  const shown = fs(...Array.from({ length: 30 }, (_, k) => 1 + k * 12));  // all share home colour 1
  const c = featureColours(shown);
  for (let k = 1; k < shown.length; k++) assert.notEqual(c.get(shown[k].number), c.get(shown[k - 1].number), `#${shown[k].number}`);
  assert.equal(new Set(shown.slice(0, 12).map(f => c.get(f.number))).size, 12, 'the first twelve are all different');
});

test('an epic not in the visible list still gets its home colour', () => {
  assert.equal(featureColour({ number: 13 }, []), FEATURE_COLS[1]);
});

// redmean distance: a cheap perceptual measure, good enough to keep flags apart.
const rgb = h => [1, 3, 5].map(i => parseInt(h.slice(i, i + 2), 16));
const dist = (a, b) => {
  const [x, y] = [rgb(a), rgb(b)], r = (x[0] + y[0]) / 2, [dr, dg, db] = [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
  return Math.sqrt((2 + r / 256) * dr * dr + 4 * dg * dg + (2 + (255 - r) / 256) * db * db);
};

test('the colours are distinct from each other', () => {
  for (let i = 0; i < N; i++) for (let j = i + 1; j < N; j++)
    assert.ok(dist(FEATURE_COLS[i], FEATURE_COLS[j]) >= 80, `${FEATURE_COLS[i]} vs ${FEATURE_COLS[j]}`);
});

test('the colours stand out from every theme\'s ground, water, paths, live gold and the stuck pennant', () => {
  const ground = {
    grass: P.grass, grassD: P.grassD, grassL: P.grassL, path: P.path, pathS: P.pathS, pathL: P.pathL,
    water: P.water, waterD: P.waterD, waterL: P.waterL, stone: P.stone, gold: P.gold, stuck: P.p0, white: P.white,
    lawn: M.lawn, lawnD: M.lawnD, asphalt: M.asphalt, deck: F.deck, deckD: F.deckD,
  };
  for (const c of FEATURE_COLS) for (const [name, g] of Object.entries(ground))
    assert.ok(dist(c, g) >= 85, `${c} is too close to ${name} ${g}`);
});
