// The "+N" overflow badge and the crate number tag (src/web/village/scenery.js, #436):
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window = {};  // state.js reads window.matchMedia
const { badge, badgeSize, numberTag, numberTagSize } = await import('../../src/web/village/scenery.js');
const { P } = await import('../../src/web/village/palette.js');
const { FONT2X } = await import('../../src/web/village/glyphs2x.js');

/* Draw with a recording context: every detail pixel's last colour (the tags
 * are drawn at 2× since #456, inside detail()). at(x, y) reads the scene
 * pixel at (x, y), by its top-left detail pixel; at2(i, j) a detail pixel. */
function draw(fn) {
  const px = new Map();
  let m = [1, 1, 0, 0];
  const stack = [];
  const c = {
    fillStyle: '',
    save() { stack.push([...m]); }, restore() { m = stack.pop(); },
    translate(x, y) { m = [m[0], m[1], m[2] + m[0] * x, m[3] + m[1] * y]; },
    scale(sx, sy) { m = [m[0] * sx, m[1] * sy, m[2], m[3]]; },
    fillRect(x, y, w, h) {
      // Into detail pixels: twice the scene's.
      const x0 = 2 * (m[0] * x + m[2]), y0 = 2 * (m[1] * y + m[3]), w2 = 2 * m[0] * w, h2 = 2 * m[1] * h;
      for (let i = 0; i < w2; i++) for (let j = 0; j < h2; j++) px.set(`${x0 + i},${y0 + j}`, this.fillStyle);
    },
  };
  fn(c);
  const at2 = (i, j) => px.get(`${i},${j}`);
  return Object.assign((x, y) => at2(2 * x, 2 * y), { at2 });
}

/* Read a tag drawn at (x, y): its glyphs, from the ink inside the outline. */
function read(at, x, y, glyphs) {
  return Array.from({ length: glyphs }, (_, j) => {
    const rows = Array.from({ length: 10 }, (_, r) => Array.from({ length: 6 }, (_, i) => (at.at2(2 * x + 2 + 8 * j + i, 2 * y + r) === P.o ? '#' : '.')).join(''));
    return Object.keys(FONT2X).find(k => FONT2X[k].join() === rows.join()) ?? '?';
  }).join('');
}

for (const n of [3, 12, 123]) {
  test(`badge(${n}) reads "+${n}", ${badgeSize(n).w} pixels wide, with a margin round the ink`, () => {
    const x = 10, y = 10, { w, h } = badgeSize(n), at = draw(c => badge(c, x, y, n));
    assert.equal(read(at, x, y, String(n).length + 1), `+${n}`);
    assert.equal(w, (String(n).length + 1) * 4 + 3);
    assert.equal(h, 9);
    // The outline is exactly the size badgeSize gives, from (x - 1, y - 2):
    // its edges, in detail pixels, with the corners rounded off.
    const X = 2 * (x - 1), Y = 2 * (y - 2);
    assert.equal(at.at2(X, Y + h), P.o, 'left edge');
    assert.equal(at.at2(X + 2 * w - 1, Y + h), P.o, 'right edge');
    assert.equal(at.at2(X + w, Y), P.o, 'top edge');
    assert.equal(at.at2(X + w, Y + 2 * h - 1), P.o, 'bottom edge');
    assert.equal(at.at2(X, Y), undefined, 'a rounded corner');
    assert.equal(at.at2(X + 2 * w, Y + h), undefined, 'nothing past the right edge');
    // The row above and below the ink, and the column left and right of it, are gold:
    // no stroke touches the outline, so "+31" never reads "+=fl".
    for (let i = 0; i < w - 2; i++) {
      assert.equal(at(x + i, y - 1), P.gold, `top padding at ${i}`);
      assert.equal(at(x + i, y + 5), P.gold, `bottom padding at ${i}`);
    }
    for (let r = -1; r <= 5; r++) {
      assert.equal(at(x, y + r), P.gold, `left padding at row ${r}`);
      assert.equal(at(x + w - 3, y + r), P.gold, `right padding at row ${r}`);
    }
  });
}

test('a crate number tag reads its number, padded the same way', () => {
  const x = 4, y = 6, at = draw(c => numberTag(c, x, y, 407));
  assert.equal(read(at, x, y, 3), '407');
  assert.equal(numberTagSize(407).w, 3 * 4 + 3);
  for (let i = 0; i < numberTagSize(407).w - 2; i++) {
    assert.equal(at(x + i, y - 1), P.parch);
    assert.equal(at(x + i, y + 5), P.parch);
  }
});
