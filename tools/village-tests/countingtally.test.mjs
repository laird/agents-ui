// The counting house's numbers and words (src/web/village/countingtally.js), #516:
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { COUNTING_TOP, countingBars, countingDate, countingFacts, countingSub, countingUsage, tokenWords } from '../../src/web/village/countingtally.js';

const t = (total, k = 0.25) => ({ input: total * k, output: total * k, cache_write: total * k, cache_read: total * k, total });
const USAGE = {
  today: t(1_234_000),
  days: ['2026-11-10', '2026-11-11', '2026-11-12', '2026-11-13', '2026-11-14', '2026-11-15', '2026-11-16']
    .map((date, k) => ({ date, tokens: t([400_000, 0, 2_000_000, 800_000, 1_000_000, 500_000, 1_234_000][k]) })),
  issues: [
    { issue: 481, total: t(9_000_000), week: t(3_400_000) },
    { issue: 12, total: t(50_000), week: t(0) },
    { issue: 516, total: t(700_000), week: t(700_000) },
    { issue: 500, total: t(2_100_000), week: t(2_100_000) },
  ],
  catching_up: false,
};

test('token counts in a few characters', () => {
  assert.equal(tokenWords(0), '0');
  assert.equal(tokenWords(950), '950');
  assert.equal(tokenWords(1_000), '1k');
  assert.equal(tokenWords(12_345), '12k');
  assert.equal(tokenWords(340_000), '340k');
  assert.equal(tokenWords(1_234_000), '1.2M');
  assert.equal(tokenWords(12_400_000), '12M');
  assert.equal(tokenWords(3_400_000_000), '3.4B');
  assert.equal(tokenWords(NaN), '?');
  assert.equal(tokenWords(-1), '?');
});

test('no usage, or a broken one, is no counting house', () => {
  assert.equal(countingUsage({}), null);
  assert.equal(countingUsage(null), null);
  assert.equal(countingUsage({ usage: {} }), null);
  assert.equal(countingUsage({ usage: { today: { total: 'x' } } }), null);
  assert.equal(countingUsage({ usage: USAGE }), USAGE);
  assert.equal(countingSub(null), '');
  assert.equal(countingFacts(null), null);
});

test('the label says today’s tokens', () => {
  assert.equal(countingSub(USAGE), '1.2M tokens today');
});

test('the tally board: each of the last 7 days as a share of the busiest', () => {
  const bars = countingBars(USAGE);
  assert.equal(bars.length, 7);
  assert.equal(bars[2].share, 1, 'the busiest day is full height');
  assert.equal(bars[1].share, 0, 'a day with nothing is empty');
  assert.equal(bars[6].date, '2026-11-16', 'today last');
  assert.deepEqual(countingBars({ today: t(0), days: [{ date: '2026-11-16', tokens: t(0) }] }).map(b => b.share), [0], 'all quiet: no division by zero');
  assert.equal(countingBars({ today: t(0), days: Array.from({ length: 9 }, () => ({ date: 'd', tokens: t(1) })) }).length, 7, 'never more than 7');
  assert.deepEqual(countingBars({ today: t(0) }), [], 'no days, no bars');
});

test('the dialog: today in parts, the week, each day, the costliest issues this week', () => {
  const f = countingFacts(USAGE);
  assert.match(f.line, /^1\.2M tokens used today/);
  assert.equal(f.meta[0], 'Today: 1.2M tokens (in 309k · out 309k · cache write 309k · cache read 309k)');
  assert.equal(f.meta[1], 'Last 7 days: 5.9M tokens');
  assert.ok(f.meta.some(m => /^Mon,? 16 Nov: 1\.2M$/.test(m)), f.meta.join('\n'));
  const top = f.meta.slice(f.meta.indexOf('Costliest this week:') + 1).filter(m => m.startsWith('#'));
  assert.deepEqual(top, ['#481: 3.4M this week (9M in all)', '#500: 2.1M this week (2.1M in all)', '#516: 700k this week (700k in all)'],
    'costliest first; an issue with nothing this week is left out');
  assert.ok(f.meta.includes('Tokens, not dollars: what they cost depends on the plan'));
  assert.ok(!f.meta.some(m => /\$|dollar.*\d/.test(m)), 'no dollar figures');
});

test('at most the top few issues; none this week says so', () => {
  const many = { ...USAGE, issues: Array.from({ length: 9 }, (_, k) => ({ issue: k + 1, total: t(10 + k), week: t(10 + k) })) };
  assert.equal(countingFacts(many).meta.filter(m => /^#\d+:/.test(m)).length, COUNTING_TOP);
  assert.ok(countingFacts({ ...USAGE, issues: [] }).meta.includes('No issue has used tokens this week'));
});

test('still reading older transcripts says so', () => {
  assert.ok(countingFacts({ ...USAGE, catching_up: true }).meta.includes('Still reading older transcripts: the totals will grow'));
  assert.ok(!countingFacts(USAGE).meta.some(m => m.startsWith('Still reading')));
});

test('dates as the calendar reads them', () => {
  assert.match(countingDate('2026-11-16'), /^Mon,? 16 Nov$/);
  assert.equal(countingDate('junk'), 'junk');
});
