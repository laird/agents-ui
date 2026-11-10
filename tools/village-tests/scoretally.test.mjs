// The gate's scoreboard and hotspots sign, in words (src/web/village/scoretally.js, #435):
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { hotspotLines, scoreLine } from '../../src/web/village/scoretally.js';

test("the scoreboard says today's totals", () => {
  assert.equal(scoreLine({ landed_today: 14, bounced_today: 6, hotspots: [] }), 'landed 14 · bounced 6 today');
  assert.equal(scoreLine({ landed_today: 0, bounced_today: 0 }), 'landed 0 · bounced 0 today');
  assert.equal(scoreLine({}), 'landed 0 · bounced 0 today', 'missing counts read as none');
  assert.equal(scoreLine(null), null, 'no tally from the gate: no scoreboard');
});

test('the sign lists the hotspots, most conflicted first', () => {
  const ms = { hotspots: [
    { file: 'src/web/server.rs', count: 3 },
    { file: 'src/web/village/main.js', count: 4 },
    { file: 'CLAUDE.md', count: 3 },
  ] };
  assert.deepEqual(hotspotLines(ms), ['src/web/village/main.js ×4', 'CLAUDE.md ×3', 'src/web/server.rs ×3']);
  assert.deepEqual(ms.hotspots.map(h => h.count), [3, 4, 3], 'the report is left as it was');
  assert.deepEqual(hotspotLines({ hotspots: [] }), []);
  assert.deepEqual(hotspotLines(undefined), []);
});
