// Marks on the 2× detail grid (src/web/village/glyphs2x.js, #456), under node:
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';

const { CELL, FONT2X, bitmap, ellipseBand, inset, rounded, tail, write } = await import('../../src/web/village/glyphs2x.js');

/* A context that keeps every pixel's last colour. */
function grid() {
  const px = new Map();
  const g = { fillStyle: '', fillRect(x, y, w, h) { for (let i = 0; i < w; i++) for (let j = 0; j < h; j++) px.set(`${x + i},${y + j}`, this.fillStyle); } };
  g.at = (x, y) => px.get(`${x},${y}`);
  g.keys = () => [...px.keys()].map(k => k.split(',').map(Number));
  return g;
}

test('the font has 0-9 and "+", each 6 by 10, and no two alike', () => {
  const keys = Object.keys(FONT2X);
  assert.deepEqual(keys.sort(), ['+', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9'].sort());
  for (const k of keys) {
    assert.equal(FONT2X[k].length, 10, `${k} is 10 rows`);
    FONT2X[k].forEach(row => assert.match(row, /^[.#]{6}$/, `${k}: ${row}`));
  }
  assert.equal(new Set(keys.map(k => FONT2X[k].join())).size, keys.length);
  assert.equal(CELL, 8, 'a cell is the 3×5 font\'s 4 scene pixels');
});

test('write lays each character a cell apart', () => {
  const g = grid(); write(g, 0, 0, '1234567890', 'ink');
  for (let j = 0; j < 10; j++) {
    const d = '1234567890'[j];
    for (let r = 0; r < 10; r++) for (let i = 0; i < 6; i++)
      assert.equal(g.at(j * CELL + i, r) === 'ink', FONT2X[d][r][i] === '#', `${d} at (${i}, ${r})`);
  }
});

test('bitmap draws one run per stretch of ink', () => {
  const calls = [];
  bitmap({ set fillStyle(v) {}, fillRect: (...a) => calls.push(a) }, 0, 0, ['##.###', '......'], 'c');
  assert.deepEqual(calls, [[0, 0, 2, 1], [3, 0, 3, 1]]);
});

test('a rounded box fills its size, outlined, with its corners off', () => {
  const g = grid(); rounded(g, 0, 0, 18, 18, { outline: 'o', fill: 'f' });
  const xs = g.keys().map(([x]) => x), ys = g.keys().map(([, y]) => y);
  assert.deepEqual([Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)], [0, 17, 0, 17]);
  assert.equal(g.at(0, 0), undefined); assert.equal(g.at(17, 17), undefined);
  for (let j = 0; j < 18; j++) {
    const a = inset(j, 18);
    assert.equal(g.at(a, j), 'o', `left outline on row ${j}`);
    assert.equal(g.at(17 - a, j), 'o', `right outline on row ${j}`);
  }
  assert.equal(g.at(9, 9), 'f');
  // Every outline pixel touches the inside or another outline pixel: no gaps.
  for (const [x, y] of g.keys()) if (g.at(x, y) === 'o') {
    const near = [[1, 0], [-1, 0], [0, 1], [0, -1], [1, 1], [-1, -1], [1, -1], [-1, 1]].filter(([dx, dy]) => g.at(x + dx, y + dy) === 'o');
    assert.ok(near.length >= 2, `outline at (${x}, ${y}) is joined up`);
  }
});

test('a tail narrows to a point and keeps its outline', () => {
  const g = grid(); tail(g, 6, 18, 6, 4, { outline: 'o', fill: 'f' });
  assert.equal(g.at(5, 18), 'o'); assert.equal(g.at(12, 18), 'o');
  assert.equal(g.at(8, 21), 'o'); assert.equal(g.at(9, 21), 'o');
  assert.equal(g.at(9, 19), 'f');
  assert.ok(g.keys().every(([, y]) => y >= 18 && y < 22), 'four rows');
});

test('an ellipse band is a ring, symmetric, with a hole', () => {
  const g = grid(); ellipseBand(g, 22, 7.5, 1.6, 'r');
  const keys = g.keys(), xs = keys.map(([x]) => x);
  assert.equal(Math.min(...xs), -22); assert.equal(Math.max(...xs), 21);
  for (const [x, y] of keys) assert.equal(g.at(-1 - x, y), 'r', `mirror of (${x}, ${y})`);
  assert.equal(g.at(0, 0), undefined, 'the middle is open');
});
