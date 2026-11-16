// The valley's ropes, caravans and barges (src/web/village/valleymodel.js, #530):
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
// Where each is drawn and what a click on it says, from /api/valley's
// `ropes`, `caravans` (crossrepo.rs) and `barges` (breakage.rs).
import { test } from 'node:test';
import assert from 'node:assert/strict';

const { BARGE_TRIP_MS, CARAVAN_CRATE, CARAVAN_LEAD, CARAVAN_ROW, VALLEY_MARGIN, bargeWords, caravanWords, ropeWords, valleyBargeAt, valleyBarges,
  valleyCaravanAt, valleyCaravans, valleyLayout, valleyRopeAt, valleyRopes } = await import('../../src/web/village/valleymodel.js');

const repo = (name, depth) => ({ name, depth, dormant: false, summary: {} });
const ref = (repo, number, title, slug = `acme/${repo}`) => ({ repo, number, title, state: 'open', url: `https://github.com/${slug}/issues/${number}` });
// lib-core feeds web and api (a river each); web calls api (a road).
const SHOP = {
  name: 'shop',
  repos: [repo('lib-core', 0), repo('web', 1), repo('api', 1)],
  edges: [{ from: 'lib-core', to: 'web', via: 'package' }, { from: 'lib-core', to: 'api', via: 'package' }, { from: 'api', to: 'web', via: 'api' }],
  ropes: [
    { from: ref('web', 7, 'Show cents'), to: ref('lib-core', 12, 'Money type') },
    { from: ref('web', 8, 'Show refunds'), to: ref('lib-core', 13, 'Refund type') },
    { from: ref('api', 40, 'Charge in cents'), to: ref('web', 7, 'Show cents') },
  ],
  caravans: [
    { parent: ref('lib-core', 1, 'Money everywhere'), arrived: false,
      crates: [{ repo: 'lib-core', slug: 'acme/lib-core', issues: [2], loaded: true }, { repo: 'web', slug: 'acme/web', issues: [9], loaded: false }, { repo: 'api', slug: 'acme/api', issues: [30, 31], loaded: false }] },
    { parent: ref('lib-core', 5, 'Dates'), arrived: true,
      crates: [{ repo: 'lib-core', slug: 'acme/lib-core', issues: [6], loaded: true }, { repo: 'api', slug: 'acme/api', issues: [33], loaded: true }] },
  ],
  barges: [{ from: 'web', to: 'lib-core', why: 'the log of test names lib-core', run: 'https://github.com/acme/web/actions/runs/42' }],
};

test('a rope runs between the facing sides of its two repos; two between the same pair stand apart', () => {
  const lay = valleyLayout(SHOP), ropes = valleyRopes(lay, SHOP), [lib, web, api] = lay.cells;
  assert.equal(ropes.length, 3);
  // web (east) to lib-core (west): from web's west side to lib-core's east side.
  assert.deepEqual([ropes[0].x0, ropes[0].x1], [web.x, lib.x + lib.w]);
  assert.notEqual(ropes[0].y0, ropes[1].y0, 'two ropes between web and lib-core stand apart');
  // api is below web in the same column: from api's top to web's bottom.
  assert.deepEqual([ropes[2].y0, ropes[2].y1], [api.y, web.y + web.h]);
  assert.equal(valleyRopeAt(ropes, Math.round((ropes[0].x0 + ropes[0].x1) / 2), ropes[0].y0).rope, SHOP.ropes[0]);
  assert.equal(valleyRopeAt(ropes, lib.x + 2, lib.y + lib.h - 2), null);
});

test('a rope to a repo the valley does not draw is left out', () => {
  const g = { ...SHOP, ropes: [{ from: ref('web', 1, 'x'), to: ref('elsewhere', 2, 'y') }] };
  assert.deepEqual(valleyRopes(valleyLayout(g), g), []);
});

test("a rope's dialog names both issues and links them", () => {
  const w = ropeWords(SHOP.ropes[0]);
  assert.equal(w.title, 'web#7 waits on lib-core#12');
  assert.deepEqual(w.meta, ['Waiting: web#7 Show cents', 'Blocking: lib-core#12 Money type']);
  assert.deepEqual(w.links, [{ label: 'web#7', url: 'https://github.com/acme/web/issues/7' }, { label: 'lib-core#12', url: 'https://github.com/acme/lib-core/issues/12' }]);
});

test('caravans travel a band below the lane, as far along as their crates are loaded', () => {
  const lay = valleyLayout(SHOP), plain = valleyLayout({ ...SHOP, caravans: [] }), trains = valleyCaravans(lay, SHOP);
  assert.equal(lay.H - plain.H, 10 + 2 * CARAVAN_ROW, 'the band takes room only when there are caravans');
  assert.equal(trains[1].y - trains[0].y, CARAVAN_ROW);
  assert.equal(trains[0].w, CARAVAN_LEAD + 3 * CARAVAN_CRATE);
  assert.deepEqual([trains[0].loaded, trains[0].total], [1, 3]);
  // A third loaded: a third of the way; arrived: at the end of its track.
  const track = lay.W - 2 * VALLEY_MARGIN - trains[0].w - 60;
  assert.equal(trains[0].x, VALLEY_MARGIN + Math.round(track / 3));
  assert.equal(trains[1].x, VALLEY_MARGIN + (lay.W - 2 * VALLEY_MARGIN - trains[1].w - 60));
  assert.ok(trains.every(t => t.y > Math.max(...lay.cells.map(c => c.y + c.h))), 'below every repo');
  assert.equal(valleyCaravanAt(trains, trains[0].x + 3, trains[0].y + 5), trains[0]);
  assert.equal(valleyCaravanAt(trains, trains[0].x + 3, trains[1].y + 5), null);
});

test("a caravan's dialog says each repo's part and links every issue", () => {
  const lay = valleyLayout(SHOP), [t, done] = valleyCaravans(lay, SHOP), w = caravanWords(t);
  assert.equal(w.title, 'Caravan: lib-core#1 Money everywhere');
  assert.equal(w.line, "1 of 3 repos' parts landed.");
  assert.deepEqual(w.meta, ['lib-core: #2 · landed', 'web: #9 · not yet', 'api: #30, #31 · not yet']);
  assert.deepEqual(w.links.map(l => l.label), ['lib-core#1', 'lib-core#2', 'web#9', 'api#30', 'api#31']);
  assert.equal(w.links[4].url, 'https://github.com/acme/api/issues/31');
  assert.equal(caravanWords(done).line, "Arrived: every repo's part has landed.");
});

test('a barge sails up the river from the consumer to the library, and waits mid-river without motion', () => {
  const lay = valleyLayout(SHOP), river = lay.rivers[lay.riverEdges.findIndex(e => e.from === 'lib-core' && e.to === 'web')];
  const [still] = valleyBarges(lay, SHOP, 0);
  // Mid-river: between the river's two ends.
  assert.ok(still.x > river[0][0] && still.x < river[river.length - 1][0]);
  // Upstream: later in its trip it is further west.
  const [early] = valleyBarges(lay, SHOP, 1), [late] = valleyBarges(lay, SHOP, BARGE_TRIP_MS / 2);
  assert.ok(late.x < early.x || late.y !== early.y, 'it moves');
  assert.ok(early.x >= late.x, 'from the consumer toward the library');
  assert.equal(valleyBargeAt([still], still.x + 2, still.y), still);
  // No river between them: it waits beside the consumer.
  const g = { ...SHOP, barges: [{ from: 'api', to: 'web', why: 'issue #3 names acme/web' }] };
  const [b] = valleyBarges(valleyLayout(g), g, 0), api = valleyLayout(g).cells[2];
  assert.deepEqual([b.x, b.y], [api.x - 10, api.y + (api.h >> 1)]);
});

test("a barge's dialog says why, and links the failing run", () => {
  const w = bargeWords({ barge: SHOP.barges[0] });
  assert.equal(w.title, "web's build fails, back to lib-core");
  assert.equal(w.line, 'web is red, and the failure names lib-core: the log of test names lib-core.');
  assert.deepEqual(w.links, [{ label: 'The failing run', url: 'https://github.com/acme/web/actions/runs/42' }]);
  assert.deepEqual(bargeWords({ barge: { from: 'a', to: 'b', why: 'x' } }).links, []);
});
