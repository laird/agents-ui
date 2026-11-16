/* ---------- The counting house, in numbers ----------
 * What the counting house (#516, countinghouse.js) says, from the village's
 * `usage` (#514, src/web/usage.rs): tokens used today, each of the last 7
 * days (oldest first), and per issue (the costliest this week first), each
 * count { input, output, cache_write, cache_read, total }. Tokens, never
 * dollars: rates depend on the plan (#496). No DOM, so node tests it
 * (tools/village-tests/countingtally.test.mjs). */

/* Its footprint on the map, in scene pixels: the house, and the tally
 * board of the last 7 days beside it. */
export const COUNTING_SIZE = { w: 34, h: 22 };
/* How many of the costliest issues the dialog names. */
export const COUNTING_TOP = 5;

const isCount = t => !!t && Number.isFinite(t.total) && t.total >= 0;

/* The village's usage, or null when there is none to show. */
export function countingUsage(v) {
  const u = v && v.usage;
  return u && isCount(u.today) ? u : null;
}

/* A token count in a few characters: 950, 12k, 340k, 1.2M, 12M, 3.4B. */
export function tokenWords(n) {
  if (!Number.isFinite(n) || n < 0) return '?';
  const short = (x, unit) => `${x < 10 ? Math.round(x * 10) / 10 : Math.round(x)}${unit}`;
  if (n < 1000) return String(Math.round(n));
  if (n < 1e6) return short(n / 1e3, 'k');
  if (n < 1e9) return short(n / 1e6, 'M');
  return short(n / 1e9, 'B');
}

/* Its label's subtitle: today's total. */
export function countingSub(u) {
  return u ? `${tokenWords(u.today.total)} tokens today` : '';
}

/* The last 7 days for the tally board: each day's share of the busiest, 0..1. */
export function countingBars(u) {
  const days = (u && Array.isArray(u.days) ? u.days : []).filter(d => d && isCount(d.tokens)).slice(-7);
  const most = Math.max(0, ...days.map(d => d.tokens.total));
  return days.map(d => ({ date: d.date, total: d.tokens.total, share: most ? d.tokens.total / most : 0 }));
}

/* "Mon 16 Nov" for a YYYY-MM-DD date, as the calendar reads it. */
export function countingDate(iso) {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(iso || ''));
  if (!m) return String(iso || '');
  const d = new Date(Date.UTC(+m[1], +m[2] - 1, +m[3]));
  return d.toLocaleDateString('en-GB', { weekday: 'short', day: 'numeric', month: 'short', timeZone: 'UTC' });
}

/* What one count is made of. */
const parts = t => [
  `in ${tokenWords(t.input)}`, `out ${tokenWords(t.output)}`,
  `cache write ${tokenWords(t.cache_write)}`, `cache read ${tokenWords(t.cache_read)}`,
].join(' · ');

/* The dialog: a line for today, then the facts. */
export function countingFacts(u) {
  if (!u) return null;
  const days = countingBars(u);
  const week = days.reduce((n, d) => n + d.total, 0);
  const issues = (Array.isArray(u.issues) ? u.issues : []).filter(i => i && Number.isFinite(i.issue) && isCount(i.week) && i.week.total > 0)
    .sort((a, b) => b.week.total - a.week.total).slice(0, COUNTING_TOP);
  const meta = [
    `Today: ${tokenWords(u.today.total)} tokens (${parts(u.today)})`,
    ...(days.length ? [`Last ${days.length} days: ${tokenWords(week)} tokens`] : []),
    ...days.map(d => `${countingDate(d.date)}: ${tokenWords(d.total)}`),
    ...(issues.length ? ['Costliest this week:', ...issues.map(i => `#${i.issue}: ${tokenWords(i.week.total)} this week${isCount(i.total) ? ` (${tokenWords(i.total.total)} in all)` : ''}`)]
      : ['No issue has used tokens this week']),
    ...(u.catching_up ? ['Still reading older transcripts: the totals will grow'] : []),
    'Tokens, not dollars: what they cost depends on the plan',
  ];
  return { line: `${tokenWords(u.today.total)} tokens used today, read from Claude Code's transcripts`, meta };
}
