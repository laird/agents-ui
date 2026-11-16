// A valley of related repos (src/web/village/valleymodel.js, #510), under node:
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';

const { VALLEY_CELL, VALLEY_GAP, VALLEY_MARGIN, valleyName, valleyLayout, valleyAt, repoWords, valleyWords, valleyMissing } =
  await import('../../src/web/village/valleymodel.js');

const repo = (name, depth, summary = {}, dormant = false) => ({ name, depth, dormant, summary: { working: 0, idle: 0, blocked: 0, backlog: 0, ...summary } });
// lib-core feeds web and api; api's package feeds nobody, web calls api's API.
const SHOP = {
  name: 'shop',
  repos: [repo('lib-core', 0), repo('web', 1), repo('api', 1), repo('admin', 2)],
  edges: [
    { from: 'lib-core', to: 'web', via: 'package' },
    { from: 'lib-core', to: 'api', via: 'package' },
    { from: 'api', to: 'admin', via: 'package' },
    { from: 'api', to: 'web', via: 'api' },
  ],
};

test('the URL names the group', () => {
  assert.equal(valleyName('?valley=shop'), 'shop');
  assert.equal(valleyName('?theme=modern&valley=a%20b'), 'a b');
  assert.equal(valleyName('?valley='), null);
  assert.equal(valleyName(''), null);
});

test('upstream repos sit west, each depth a column, each repo below the one before it', () => {
  const { cells, W, H } = valleyLayout(SHOP);
  const { w, h } = VALLEY_CELL, step = w + VALLEY_GAP.x, m = VALLEY_MARGIN;
  assert.deepEqual(cells.map(c => [c.x, c.y]), [[m, m], [m + step, m], [m + step, m + h + VALLEY_GAP.y], [m + 2 * step, m]]);
  assert.equal(W, 2 * m + 3 * w + 2 * VALLEY_GAP.x);
  // Room below the lowest row for the roads' lane.
  assert.ok(H > m + 2 * h + VALLEY_GAP.y);
  // No two repos overlap.
  cells.forEach((a, i) => cells.forEach((b, j) => {
    if (i < j) assert.ok(a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y, `${i} and ${j} overlap`);
  }));
});

test('a river runs along each package edge, east side to west side; a road along each api edge', () => {
  const { cells, rivers, roads } = valleyLayout(SHOP);
  assert.equal(rivers.length, 3);
  assert.equal(roads.length, 1);
  const [core, web, api] = cells;
  // lib-core to web: level, straight across.
  assert.deepEqual(rivers[0], [[core.x + core.w, core.y + core.h / 2], [web.x, web.y + web.h / 2]]);
  // lib-core to api: out east, down in the gap, into api's west side.
  const r = rivers[1];
  assert.deepEqual(r[0], [core.x + core.w, core.y + core.h / 2]);
  assert.deepEqual(r[r.length - 1], [api.x, api.y + api.h / 2]);
  assert.ok(r[1][0] > core.x + core.w && r[1][0] < api.x, 'it turns in the gap');
  // Every leg is level or plumb.
  [...rivers, ...roads].forEach(p => p.slice(1).forEach(([x, y], k) => assert.ok(x === p[k][0] || y === p[k][1])));
  // The road leaves api's west side and comes into web's.
  const road = roads[0];
  assert.deepEqual([road[0][0], road[road.length - 1][0]], [api.x, web.x]);
  noLegUnderARepo(cells, [...rivers, ...roads]);
});

/* No leg of a river or road runs through the inside of any repo. */
function noLegUnderARepo(cells, paths) {
  paths.forEach(p => p.slice(1).forEach(([x1, y1], k) => {
    const [x0, y0] = p[k], lo = [Math.min(x0, x1), Math.min(y0, y1)], hi = [Math.max(x0, x1), Math.max(y0, y1)];
    cells.forEach((c, i) => assert.ok(hi[0] <= c.x || lo[0] >= c.x + c.w || hi[1] <= c.y || lo[1] >= c.y + c.h,
      `the leg ${JSON.stringify([p[k], [x1, y1]])} runs under repo ${i}`));
  }));
}

test('a river that skips a column, and a road within one column, keep to the gaps', () => {
  // core -> web -> api, and core -> api straight: the straight one would run under web.
  const skip = { repos: [repo('core', 0), repo('web', 1), repo('api', 2)],
    edges: [{ from: 'core', to: 'web', via: 'package' }, { from: 'web', to: 'api', via: 'package' }, { from: 'core', to: 'api', via: 'package' }] };
  let lay = valleyLayout(skip);
  noLegUnderARepo(lay.cells, lay.rivers);
  // The demo's shape: billing calls shop's API, larkspur between them in the same column.
  const col = { repos: [repo('lib', 0), repo('shop', 1), repo('larkspur', 1), repo('billing', 1)],
    edges: [{ from: 'billing', to: 'shop', via: 'api' }, { from: 'lib', to: 'shop', via: 'package' }, { from: 'lib', to: 'billing', via: 'package' }] };
  lay = valleyLayout(col);
  noLegUnderARepo(lay.cells, [...lay.rivers, ...lay.roads]);
  const road = lay.roads[0], [, shop, , billing] = lay.cells;
  assert.equal(road[0][1], billing.y + billing.h - 12, 'it leaves billing');
  assert.equal(road[road.length - 1][1], shop.y + shop.h - 12, 'and comes into shop');
});

test('an edge to a repo not in the valley draws nothing; an empty group still has a size', () => {
  const { rivers } = valleyLayout({ repos: [repo('a', 0)], edges: [{ from: 'a', to: 'ghost', via: 'package' }] });
  assert.equal(rivers.length, 0);
  const empty = valleyLayout({ repos: [], edges: [] });
  assert.ok(empty.W > 0 && empty.H > 0);
  assert.deepEqual(valleyLayout(null).cells, []);
});

test('a click finds the repo under it', () => {
  const { cells } = valleyLayout(SHOP);
  assert.equal(valleyAt(cells, cells[2].x + 1, cells[2].y + 1), 2);
  assert.equal(valleyAt(cells, 0, 0), null);
});

test('a repo label says its houses, backlog and release, or that it is dormant', () => {
  assert.deepEqual(repoWords(repo('web', 1, { working: 2, idle: 1, blocked: 1, backlog: 7, release: 'v1.2.0' })),
    { text: 'web', sub: '2 working · 1 idle · 1 blocked · 7 in backlog · v1.2.0' });
  assert.deepEqual(repoWords(repo('api', 0, { backlog: 0 })), { text: 'api', sub: '0 working · 0 idle · 0 in backlog' });
  assert.deepEqual(repoWords(repo('lib', 0, {}, true)), { text: 'lib', sub: 'dormant' });
});

test('the bar says how many repos and links, and what is wrong', () => {
  assert.equal(valleyWords(SHOP), '4 repos · 3 package links · 1 API link');
  assert.equal(valleyWords({ repos: [repo('a', 0)], edges: [], errors: ['g: a depends on ghost, which is not in the group'] }),
    '1 repo · 0 package links · 0 API links · 1 problem: g: a depends on ghost, which is not in the group');
});

test('why nothing is drawn: no manifest, or no such group', () => {
  assert.equal(valleyMissing(null, 'shop'), null);
  assert.match(valleyMissing({ found: false, manifest: '/h/.agents-ui/group.json', groups: [] }, 'shop'), /declare a group of repos in \/h\/\.agents-ui\/group\.json/);
  assert.equal(valleyMissing({ found: true, manifest: 'g.json', groups: [{ name: 'lab' }], errors: [] }, 'shop'), 'No group named "shop" in g.json. Groups: lab.');
  assert.match(valleyMissing({ found: true, manifest: 'g.json', groups: [], errors: ["can't read the manifest: bad"] }, 'shop'), /can't read the manifest: bad/);
  assert.equal(valleyMissing({ found: true, manifest: 'g.json', groups: [{ name: 'shop' }] }, 'shop'), null);
});
