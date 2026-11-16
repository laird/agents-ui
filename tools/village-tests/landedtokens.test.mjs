// Tokens per landed issue (#517): what the crate's dialog and the travel
// log say an issue cost (src/web/village/landedtokens.js).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { landedTokenWords, landedTokens, shortTokens } from '../../src/web/village/landedtokens.js';

const tok = (total, input = 0, output = 0, cache_write = 0, cache_read = 0) => ({ input, output, cache_write, cache_read, total });
const v = { usage: { issues: [
  { issue: 12, total: tok(3_400_000, 120_000, 45_000, 235_000, 3_000_000), week: tok(1) },
  { issue: 13, total: tok(0), week: tok(0) },
] } };

test('a landed issue with usage shows its total; without, nothing', () => {
  assert.deepEqual(landedTokenWords(v, 12), { line: 'Tokens from claim to land: 3.4M', detail: '120k in · 45k out · 3.2M cache' });
  assert.equal(landedTokenWords(v, 99), null, 'no usage on record');
  assert.equal(landedTokenWords(v, 13), null, 'nothing spent is nothing to say');
  assert.equal(landedTokenWords({}, 12), null, 'no usage at all (not served, or not read)');
  assert.equal(landedTokens({ usage: { issues: [{ issue: 12 }] } }, 12), null, 'a malformed entry');
});

test('counts are short', () => {
  assert.equal(shortTokens(950), '950');
  assert.equal(shortTokens(1000), '1k');
  assert.equal(shortTokens(1500), '1.5k');
  assert.equal(shortTokens(12_345), '12k');
  assert.equal(shortTokens(3_400_000), '3.4M');
  assert.equal(shortTokens(12_600_000), '13M');
});
