// Marks that don't rely on colour (src/web/village/cvdmarks.js), #443:
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { NOTECOL, PRIOCOL } from '../../src/web/village/palette.js';
import { GLYPH_WORDS, NOTE_GLYPHS, inkOn, note, prioCells, prioRun } from '../../src/web/village/cvdmarks.js';

/* A context that records each fillRect in canvas pixels through a scale and translate. */
function ctx(d = 2) {
  let m = [d, 0, 0, d, 0, 0]; const stack = [], rects = [];
  const c = { rects, fillStyle: '',
    save() { stack.push([...m]); }, restore() { m = stack.pop(); },
    translate(x, y) { m = [m[0], 0, 0, m[3], m[4] + m[0] * x, m[5] + m[3] * y]; },
    scale(sx, sy) { m = [m[0] * sx, 0, 0, m[3] * sy, m[4], m[5]]; },
    fillRect(x, y, w, h) { rects.push([m[0] * x + m[4], m[3] * y + m[5], m[0] * w, m[3] * h, c.fillStyle]); } };
  return c;
}

test('every note type has its own glyph and a word for the Key', () => {
  for (const label of Object.keys(NOTECOL)) {
    const g = NOTE_GLYPHS[label];
    assert.ok(g, `no glyph for ${label}`);
    assert.equal(g.length, 4); g.forEach(r => assert.match(r, /^[01]{4}$/));
    assert.ok(g.join('').includes('1'), `${label}'s glyph is blank`);
    assert.ok(GLYPH_WORDS[label], `no word for ${label}`);
  }
  const shapes = Object.values(NOTE_GLYPHS).map(g => g.join(''));
  assert.equal(new Set(shapes).size, shapes.length, 'no two note types share a glyph');
});

test('failed and conflict, alike under deuteranopia, have different glyphs', () => {
  // A deuteranopia simulation (Viénot 1999): the two colours land close together.
  const sim = hex => { const n = parseInt(hex.slice(1), 16), r = (n >> 16) / 255, g = ((n >> 8) & 255) / 255, b = (n & 255) / 255;
    return [0.625 * r + 0.375 * g, 0.7 * r + 0.3 * g, 0.3 * g + 0.7 * b]; };
  const dist = (x, y) => { const [a, b] = [sim(x), sim(y)]; return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]); };
  // Half as far apart as a yellow note from a lilac one: too close to rely on.
  assert.ok(dist(NOTECOL.failed, NOTECOL.conflict) < dist(NOTECOL['needs-design'], NOTECOL['needs-clarification']) / 2, 'the colours alone are hard to tell apart');
  assert.notDeepEqual(NOTE_GLYPHS.failed, NOTE_GLYPHS.conflict);
});

test('glyph ink is dark on a light note and white on a dark one', () => {
  assert.equal(inkOn(NOTECOL['needs-design']), '#202030');
  assert.equal(inkOn(NOTECOL['too-complex']), '#f8f8f8');
  assert.equal(inkOn('not a colour'), '#202030');
});

test('a note keeps its 3×3 footprint, its glyph inside it at detail pixels', () => {
  const c = ctx(); note(c, 10, 20, 'failed', '#fff');
  const [bg, ...ink] = c.rects;
  assert.deepEqual(bg, [20, 40, 6, 6, NOTECOL.failed]);
  assert.equal(ink.length, 8, 'the ✗ is eight detail pixels');
  ink.forEach(([x, y, w, h]) => { assert.ok(x >= 21 && x + w <= 25 && y >= 41 && y + h <= 45); assert.equal(w, 1); assert.equal(h, 1); });
  const plain = ctx(); note(plain, 0, 0, 'mystery', '#abcdef');
  assert.deepEqual(plain.rects, [[0, 0, 6, 6, '#abcdef']], 'an unknown label is a plain note');
});

test('each priority level has its own pattern, at every band length', () => {
  for (const len of [4, 6, 9]) {
    const pats = ['P0', 'P1', 'P2', 'P3'].map(p => prioCells(p, len).map(Number).join(''));
    assert.equal(new Set(pats).size, 4, `four distinct patterns at length ${len}: ${pats}`);
    assert.equal(pats[0], '1'.repeat(len), 'P0 is solid');
  }
  assert.deepEqual(['P0', 'P1', 'P2', 'P3'].map(p => prioCells(p, 6).map(Number).join('')), ['111111', '110011', '101010', '001100']);
  assert.equal(prioCells(null, 6), null);
});

test("a 2× crate's band (prioRun) is patterned in its own detail pixels", () => {
  const rects = []; const d = { fillStyle: '', fillRect(x, y, w, h) { rects.push([x, y, w, h, d.fillStyle]); } };
  prioRun(d, 1, 8, 9, 'P2', '#f8d040');
  assert.deepEqual(rects.map(r => r[0]), [1, 3, 5, 7, 9], 'dashed across a 9-pixel face');
  rects.length = 0; prioRun(d, 1, 8, 9, 'P3', '#9898a8');
  assert.deepEqual(rects.map(r => r[0]), [5], 'a dot in the middle');
  rects.length = 0; prioRun(d, 1, 8, 9, null, '#000');
  assert.equal(rects.length, 0);
});
