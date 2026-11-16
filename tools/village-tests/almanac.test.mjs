// The almanac (src/web/village/almanactally.js and almanac.js, #499), under node:
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`. Its rows, marks and words
// from a `github` summary (#498), then each theme's art painted onto a
// pixel grid and read back: today's row last, landed, bounced and reverted
// where the row says.
import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.window ??= {};
performance.now = () => 0; // nothing here may depend on the frame it lands on (#450)
const V = '../../src/web/village/';
const T = await import(V + 'almanactally.js');
const { DETAIL, prepare } = await import(V + 'detail.js');
const { state } = await import(V + 'state.js');
await import(V + 'themes/medieval.js');
await import(V + 'themes/modern.js');
await import(V + 'themes/future.js');
const { THEMES } = await import(V + 'themes/registry.js');
const { almanacArt, ALMANAC_BOUNCE, ALMANAC_REVERT, almanacLook } = await import(V + 'almanac.js');

/* A summary of 14 days ending 2026-11-16, the server's today. */
function summary(over = {}) {
  const days = Array.from({ length: 14 }, (_, k) => ({
    date: `2026-11-${String(3 + k).padStart(2, '0')}`, landed: k % 5, opened: 2, bounced: k === 4 ? 7 : k % 3 === 0 ? 1 : 0, reverted: k === 9 ? 1 : 0, deploys: k % 2,
  }));
  return { as_of: '2026-11-16T06:00:00-05:00', days, median_claim_to_land_secs: 3 * 3600 + 20 * 60, ...over };
}

test('one row per day, oldest first, today last', () => {
  const rows = T.almanacRows(summary());
  assert.equal(rows.length, 14);
  assert.equal(rows[0].date, '2026-11-03');
  assert.equal(rows[13].date, '2026-11-16');
  assert.deepEqual(rows.map(r => r.today), [...Array(13).fill(false), true]);
  // A summary whose last day is not as_of's date has no today.
  assert.equal(T.almanacRows(summary({ as_of: '2026-11-17T00:01:00-05:00' }))[13].today, false);
});

test('no github summary: no rows, no words', () => {
  for (const gh of [undefined, null, {}, { as_of: 'x', days: [] }]) {
    assert.deepEqual(T.almanacRows(gh), []);
    assert.equal(T.almanacFacts(gh), null);
    assert.equal(T.almanacToday(gh), null);
    assert.equal(T.almanacDays(gh), null);
  }
});

test('the marks per day: a bar per landed issue, pips per bounce, a cross per revert', () => {
  const rows = T.almanacRows(summary());
  assert.equal(rows[3].bar, 3 * T.ALMANAC_UNIT, 'four a day fit: one unit each');
  assert.equal(rows[0].bar, 0);
  assert.equal(rows[3].pips, 1);
  assert.equal(rows[4].pips, T.ALMANAC_PIPS, 'pips stop at five');
  assert.equal(rows[4].more, true);
  assert.equal(rows[9].revert, true);
  assert.equal(rows[8].revert, false);
  // A busy fortnight scales to the busiest day, and a day with any landing shows.
  assert.equal(T.almanacBar(30, 30), T.ALMANAC_BAR_W);
  assert.equal(T.almanacBar(15, 30), T.ALMANAC_BAR_W / 2);
  assert.equal(T.almanacBar(1, 200), 2);
  // Reverts unknown (no git log): no cross, and the dialog says so.
  const unknown = summary();
  unknown.days.forEach(d => delete d.reverted);
  assert.ok(T.almanacRows(unknown).every(r => r.reverted === null && !r.revert));
  assert.ok(T.almanacFacts(unknown).meta.some(m => m.startsWith('Reverted: unknown')));
});

test("today's row grows with the gate's tally between GitHub reads", () => {
  const ms = { landed_today: 9, bounced_today: 0 };
  const rows = T.almanacRows(summary(), ms);
  assert.equal(rows[13].landed, 9);
  assert.equal(rows[13].bar, T.almanacBar(9, 9));
  assert.equal(rows[12].landed, summary().days[12].landed, 'only today grows');
  assert.equal(T.almanacRows(summary(), { landed_today: 1 })[13].landed, 3, 'never shrinks below GitHub');
});

test('row heights and the footprint', () => {
  assert.equal(T.almanacDetailH(14), T.ALMANAC_HEAD + 14 * T.ALMANAC_ROW + T.ALMANAC_FOOT);
  assert.deepEqual(T.ALMANAC_SIZE, { w: T.ALMANAC_DW / DETAIL, h: T.almanacDetailH() / DETAIL });
  assert.ok(T.ALMANAC_BAR_X + T.ALMANAC_BAR_W < T.ALMANAC_PIP_X, 'bars never run into the pips');
  assert.ok(T.ALMANAC_PIP_X + 3 * T.ALMANAC_PIPS <= T.ALMANAC_DW - 2, 'pips stay inside the frame');
});

test('the median in words', () => {
  assert.equal(T.almanacSpan(null), null);
  assert.equal(T.almanacSpan(20), 'under a minute');
  assert.equal(T.almanacSpan(45 * 60), '45 min');
  assert.equal(T.almanacSpan(3 * 3600), '3 h');
  assert.equal(T.almanacSpan(3 * 3600 + 20 * 60), '3 h 20 min');
  assert.equal(T.almanacSpan(24 * 3600), '1 day');
  assert.equal(T.almanacSpan(52 * 3600), '2 days 4 h');
});

test("the dialog's numbers, with as_of", () => {
  const f = T.almanacFacts(summary());
  const landed = summary().days.reduce((n, d) => n + d.landed, 0);
  assert.equal(f.line, '3 landed today (avg 2)');
  assert.ok(f.meta.includes(`Landed: ${landed} issues in 14 days`));
  assert.ok(f.meta.includes('Opened: 28'));
  assert.ok(f.meta.includes('Bounced: 12 times the merge gate handed work back'));
  assert.ok(f.meta.includes('Reverted: 1 on the integration branch'));
  assert.ok(f.meta.includes('Deploys: 7 commits deployed'));
  assert.ok(f.meta.includes('Median from claimed to landed: 3 h 20 min'));
  assert.ok(f.meta.includes('As of 2026-11-16 06:00'));
  assert.ok(f.meta.includes('Mon 16 Nov (today): landed 3 · bounced 0 · opened 2 · deploys 1'));
  assert.ok(T.almanacFacts(summary({ median_claim_to_land_secs: undefined })).meta.includes('Median from claimed to landed: no claims on record'));
  assert.equal(T.almanacDate('2026-11-03'), 'Tue 3 Nov');
});

/* A canvas of detail pixels: fillRect lands on a grid through the transform. */
function grid(w, h) {
  let m = [1, 0, 0, 1, 0, 0];
  const stack = [], px = new Map();
  const c = {
    fillStyle: '#000', imageSmoothingEnabled: true,
    setTransform(a, b, cc, d, e, f) { m = [a, b, cc, d, e, f]; },
    save() { stack.push([...m]); }, restore() { m = stack.pop(); },
    translate(x, y) { m = [m[0], m[1], m[2], m[3], m[4] + m[0] * x, m[5] + m[3] * y]; },
    scale(sx, sy) { m = [m[0] * sx, m[1], m[2], m[3] * sy, m[4], m[5]]; },
    fillRect(x, y, rw, rh) {
      const x0 = m[0] * x + m[4], y0 = m[3] * y + m[5];
      for (let yy = Math.round(y0); yy < Math.round(y0 + m[3] * rh); yy++)
        for (let xx = Math.round(x0); xx < Math.round(x0 + m[0] * rw); xx++)
          if (xx >= 0 && yy >= 0 && xx < w && yy < h) px.set(`${xx},${yy}`, c.fillStyle);
    },
    at: (x, y) => px.get(`${x},${y}`),
  };
  prepare(c, DETAIL);
  return c;
}
/* The colours drawn in one row's band of a column range. */
function band(c, k, x0, x1) {
  const ry = T.ALMANAC_HEAD + k * T.ALMANAC_ROW, out = new Set();
  for (let y = ry; y < ry + T.ALMANAC_ROW; y++) for (let x = x0; x < x1; x++) out.add(c.at(x, y));
  return out;
}

for (const [name, theme] of Object.entries(THEMES)) {
  test(`${name}: 14 rows drawn at 2×, landed, bounced and reverted where the row says`, () => {
    state.reduce = true;
    const rows = T.almanacRows(summary()), c = grid(T.ALMANAC_DW, T.almanacDetailH());
    almanacArt(c, 0, 0, rows, theme, 0);
    const look = almanacLook(theme);
    assert.ok(look.name, 'named in its theme');
    rows.forEach((r, k) => {
      const bar = band(c, k, T.ALMANAC_BAR_X, T.ALMANAC_BAR_X + T.ALMANAC_BAR_W);
      assert.equal(bar.has(look.bar), r.landed > 0, `row ${k}: landed ${r.landed}`);
      assert.equal(band(c, k, T.ALMANAC_PIP_X, T.ALMANAC_DW - 2).has(ALMANAC_BOUNCE), r.bounced > 0, `row ${k}: bounced ${r.bounced}`);
      assert.equal(band(c, k, T.ALMANAC_GUTTER_X, T.ALMANAC_BAR_X).has(ALMANAC_REVERT), r.revert, `row ${k}: reverted`);
    });
    // Today's row, the last, stands out.
    assert.ok(band(c, 13, 2, 4).has(look.now));
    assert.ok(!band(c, 12, 2, 4).has(look.now));
    // The bar is as long as the row says: drawn up to it, nothing past it.
    const r = rows[3], ry = T.ALMANAC_HEAD + 3 * T.ALMANAC_ROW + 2;
    assert.notEqual(c.at(T.ALMANAC_BAR_X + r.bar + 1, ry), look.bar);
  });
}

test("the dialog says when today's row shows the gate's count (#509)", () => {
  const gh = summary(), on = gh.as_of.slice(0, 10);
  const ahead = T.almanacRows(gh, { landed_today: 9, bounced_today: 0 }, on);
  assert.equal(ahead[13].live, true);
  assert.ok(ahead.slice(0, 13).every(r => !r.live), 'only today');
  assert.equal(T.almanacRows(gh, { landed_today: 1, bounced_today: 0 }, on)[13].live, false, "GitHub's count is the one shown");
  assert.equal(T.almanacRows(gh, null, on)[13].live, false);
  assert.ok(T.almanacRows(gh, { landed_today: 0, bounced_today: 7 }, on)[13].live, 'a bounce the gate saw first counts too');
  const tomorrow = new Date(Date.parse(on + 'T00:00:00Z') + 86400000).toISOString().slice(0, 10);
  assert.equal(T.almanacRows(gh, { landed_today: 9, bounced_today: 0 }, tomorrow)[13].live, false, "another day's tally is not shown");
  assert.ok(!T.almanacFacts(gh, null, on).meta.includes(T.ALMANAC_LIVE_NOTE));
  const f = T.almanacFacts(gh, { landed_today: 9, bounced_today: 0 }, on);
  assert.equal(f.meta[f.meta.length - 1], T.ALMANAC_LIVE_NOTE, 'said under the days');
  assert.equal(f.line, '9 landed today (avg 2)');
});

test("the gate's UTC tally joins today's row only on the same date", () => {
  const gh = summary();
  const on = gh.as_of.slice(0, 10), ms = { landed_today: 99, bounced_today: 0 };
  assert.equal(T.almanacRows(gh, ms, on)[13].landed, 99);
  // Just after local midnight west of UTC, the tally's day is not the row's.
  const other = new Date(Date.parse(on + 'T00:00:00Z') + 86400000).toISOString().slice(0, 10);
  assert.notEqual(T.almanacRows(gh, ms, other)[13].landed, 99);
});

test('the almanac says how much landed work came back (#497)', async () => {
  const { almanacFacts } = await import('../../src/web/village/almanactally.js');
  const today = new Date().toISOString().slice(0, 10);
  const days = Array.from({ length: 14 }, (_, k) => ({ date: k === 13 ? today : `2000-01-${String(k + 1).padStart(2, '0')}`, landed: 3, opened: 1, bounced: 0, deploys: 0 }));
  const facts = almanacFacts({ as_of: `${today}T06:00:00Z`, days, returned_of: 42, returned: [{ number: 1, how: 'reopened' }] });
  assert.ok(facts.meta.includes('Came back: 1 of 42 landed came back (2%), reopened, reverted or named in a regression'), facts.meta.join('\n'));
  const none = almanacFacts({ as_of: `${today}T06:00:00Z`, days });
  assert.ok(!none.meta.some(m => m.startsWith('Came back')));
});
