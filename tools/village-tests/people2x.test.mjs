// People at 2× (src/web/village/people2x.js, #419), under node:
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { lift, lighten, runs } from '../../src/web/village/people2x.js';

// people.js's medieval man in a belted tunic, and the colours it is given.
const MAN = ['..oooo..', '.ohhhho.', 'ohhhhhho', 'ohssssho', 'osossoso', '.oSssSo.', '.occcco.', 'oCccccCo', 'sCttttCs', '.occcco.', '.opoopo.', '.oo..oo.'];
// The modern woman with a bob in a trouser suit: a blouse (w) down the front.
const SUIT = ['..oooo..', '.ohhhho.', 'ohhhhhho', 'hhssssho', 'hsossosh', '.hSssSh.', '.ocwwco.', 'oCcwwcCo', 'sCccccCs', '.occcco.', '.opoopo.', '.oo..oo.'];
// The future round bot.
const BOT = ['....a...', '....o...', '.oooooo.', 'ohhhhhho', 'ohehheho', 'ohhhhhho', '.oooooo.', 'oCcttcCo', 'sCccccCs', '.oCccCo.', '.opoopo.', '.oo..oo.'];
const map = { o: '#202030', s: '#f8d0a8', S: '#d8a880', w: '#f8f8f8', h: '#704830', c: '#4878a0', C: '#305878', t: '#583820', p: '#585060', e: '#40e0f0', a: '#f048c0' };
const count = (g, col) => g.flat().filter(x => x === col).length;

test('a body lifts onto the 2× grid in the same footprint: 16×24 detail pixels', () => {
  const g = lift(MAN, map);
  assert.equal(g.length, 24);
  assert.ok(g.every(row => row.length === 16));
  // Empty where the template is empty: its top-left corner.
  assert.equal(g[0][0], null);
});

test('a face: narrow eyes set towards the nose, a nose and a mouth', () => {
  const g = lift(MAN, map);
  // Row 4 is the eye row (detail rows 8–9); the eyes are cells 2 and 5.
  assert.deepEqual([g[8][4], g[8][5], g[9][4], g[9][5]], [map.s, '#202030', map.s, '#202030'], 'left eye: outline on its inner half');
  assert.deepEqual([g[8][10], g[8][11]], ['#202030', map.s], 'right eye: outline on its inner half');
  assert.equal(g[9][8], map.S, 'a nose between them');
  assert.deepEqual([g[11][7], g[11][8]], ['#8a4a3a', '#8a4a3a'], 'a mouth under it');
});

test('hair catches the light; a belt has a buckle; shoes have toe caps', () => {
  const g = lift(MAN, map);
  assert.ok(count(g, lighten(map.h, 0.3)) >= 3, 'a highlight along the top of the hair');
  assert.equal(g[16][7], '#f8d040', 'a gold buckle on the belt (row 8)');
  assert.equal(count(g, '#4a3a30'), 4, 'four shoe cells, a toe cap each');
});

test('a jacket over a blouse has buttons down the front', () => {
  const g = lift(SUIT, map);
  assert.deepEqual([g[13][8], g[15][8]], ['#202030', '#202030']);
  assert.equal(g[12][8], map.w, 'between them, the blouse');
});

test("roles: the manager's sash, a sub-agent's armband and badge", () => {
  const plain = lift(MAN, map), mayor = lift(MAN, map, { mayor: true, sash: '#f8d040' });
  assert.ok(count(mayor, '#f8d040') > count(plain, '#f8d040') + 4, 'a gold sash across the torso');
  const sub = lift(MAN, map, { sub: true, accent: '#e070d0' });
  assert.equal(count(sub, '#e070d0'), 3, 'an armband (2) and a badge (1) in the specialty colour');
  assert.equal(count(plain, '#e070d0'), 0);
});

test('robots: LED eyes with a glint, rivets, a lit chest light, a thinner antenna', () => {
  const g = lift(BOT, map, { robot: true });
  assert.ok(count(g, '#ffffff') >= 3, 'a glint in each eye and the chest light');
  assert.ok(count(g, lighten(map.C, 0.45)) >= 1, 'rivets on the shell');
  assert.equal(g[0][8], null, 'half the antenna tip is gone');
  assert.equal(g[0][9], map.a);
  // No face is drawn on a robot.
  assert.equal(count(g, '#8a4a3a'), 0);
});

test('a pose still poses at 2×: a sitting body is two rows lower', () => {
  const sit = ['........', '........', ...MAN.slice(0, 9), MAN[11]];
  const g = lift(sit, map);
  assert.equal(g.length, 24);
  assert.ok(g.slice(0, 4).every(row => row.every(x => x === null)));
  assert.equal(g[13][8], map.S, 'the face (and its nose) moved down with it');
});

test('runs: one rectangle per run of a colour along a row', () => {
  assert.deepEqual(runs([['a', 'a', null, 'b'], [null, 'b', 'b', 'b']]), [[0, 0, 2, 'a'], [3, 0, 1, 'b'], [1, 1, 3, 'b']]);
});

test('lighten mixes towards white, and leaves anything else alone', () => {
  assert.equal(lighten('#000000', 0.5), '#808080');
  assert.equal(lighten('#ffffff', 0.5), '#ffffff');
  assert.equal(lighten('rgba(1,2,3,.5)'), 'rgba(1,2,3,.5)');
});
