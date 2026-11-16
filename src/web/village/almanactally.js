/* ---------- The almanac, in numbers ----------
 * What the almanac (#499, almanac.js) draws and says, from the village's
 * `github` summary (#498, src/web/github.rs): the last 14 days oldest first,
 * each { date, landed, opened, bounced, reverted?, deploys }, and as_of.
 * Its size, which marks each day's row gets, and the dialog's words. No DOM,
 * so node tests it (tools/village-tests/almanac.test.mjs). */

/* Days shown: one row each, today's last. */
export const ALMANAC_DAYS = 14;
/* Sizes in detail pixels (two to a scene pixel, detail.js): each day's row,
 * the header and footer bands, and the columns across a row: the revert
 * mark's gutter, the landed bar, and the bounce pips. */
export const ALMANAC_ROW = 6, ALMANAC_HEAD = 6, ALMANAC_FOOT = 6, ALMANAC_DW = 88;
export const ALMANAC_GUTTER_X = 4, ALMANAC_BAR_X = 10, ALMANAC_BAR_W = 56, ALMANAC_PIP_X = 68, ALMANAC_PIPS = 5;
/* Detail pixels per landed issue when they fit: one crop, bar segment or holo bar each. */
export const ALMANAC_UNIT = 4;

/* How tall an almanac of `rows` days is, in detail pixels. */
export const almanacDetailH = (rows = ALMANAC_DAYS) => ALMANAC_HEAD + rows * ALMANAC_ROW + ALMANAC_FOOT;
/* Its footprint on the map, in scene pixels. */
export const ALMANAC_SIZE = { w: ALMANAC_DW / 2, h: almanacDetailH() / 2 };

/* The summary's days, or null when there is none to draw. */
export function almanacDays(gh) {
  return gh && Array.isArray(gh.days) && gh.days.length ? gh.days : null;
}

/* The server's local date in as_of ("2026-11-16T06:00:00-05:00": 2026-11-16). */
const almanacOn = gh => (gh && typeof gh.as_of === 'string' ? gh.as_of.slice(0, 10) : '');

/* How long the landed bar is, in detail pixels: ALMANAC_UNIT per issue while
 * the busiest day fits, else scaled to the busiest day; a day that landed
 * anything shows at least a sliver. */
export function almanacBar(landed, most, room = ALMANAC_BAR_W) {
  if (!landed) return 0;
  if (most * ALMANAC_UNIT <= room) return landed * ALMANAC_UNIT;
  return Math.max(2, Math.round(landed * room / most));
}

/* One row per day, oldest first, today last: what the row shows.
 *   { date, landed, opened, bounced, reverted, deploys, bar, pips, more, revert, today }
 * Today's row grows with the gate's own tally (`pipeline.merge_stats`, #431)
 * between GitHub reads, as crates leave the quay. That tally counts the UTC
 * day and the rows the server's own, so it is used only while the two are
 * the same date: otherwise it would count yesterday's landings again. The
 * larger of the two is shown, so nothing is counted twice (#509); `live` is
 * set when the gate's count is the one shown, and the dialog says so. */
export function almanacRows(gh, ms = null, utcDay = new Date().toISOString().slice(0, 10)) {
  const days = almanacDays(gh);
  if (!days) return [];
  const shown = days.slice(-ALMANAC_DAYS), on = almanacOn(gh);
  const rows = shown.map((d, k) => {
    const today = k === shown.length - 1 && d.date === on;
    const live = today && ms && utcDay === on ? ms : {};
    const gate = (live.landed_today || 0) > (d.landed || 0) || (live.bounced_today || 0) > (d.bounced || 0);
    return {
      date: d.date, today, live: gate,
      landed: Math.max(d.landed || 0, live.landed_today || 0),
      bounced: Math.max(d.bounced || 0, live.bounced_today || 0),
      opened: d.opened || 0, deploys: d.deploys || 0,
      reverted: d.reverted == null ? null : d.reverted,
    };
  });
  const most = Math.max(1, ...rows.map(r => r.landed));
  return rows.map(r => ({ ...r, bar: almanacBar(r.landed, most), pips: Math.min(r.bounced, ALMANAC_PIPS), more: r.bounced > ALMANAC_PIPS, revert: (r.reverted || 0) > 0 }));
}

/* A span of seconds in words: "under a minute", "45 min", "3 h 20 min", "2 days 4 h". */
export function almanacSpan(secs) {
  if (secs == null || !(secs >= 0)) return null;
  const m = Math.round(secs / 60);
  if (m < 1) return 'under a minute';
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60), mm = m % 60;
  if (h < 24) return mm ? `${h} h ${mm} min` : `${h} h`;
  const d = Math.floor(h / 24), hh = h % 24, days = d === 1 ? '1 day' : `${d} days`;
  return hh ? `${days} ${hh} h` : days;
}

/* "2026-11-16T06:00:00-05:00" as "2026-11-16 06:00": the server's own clock. */
export function almanacAsOf(asOf) {
  if (typeof asOf !== 'string' || asOf.length < 16) return asOf || 'unknown';
  return `${asOf.slice(0, 10)} ${asOf.slice(11, 16)}`;
}

const ALMANAC_WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
const ALMANAC_MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
/* "2026-11-16" as "Mon 16 Nov". */
export function almanacDate(date) {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date || '');
  if (!m) return date || '';
  const d = new Date(Date.UTC(+m[1], +m[2] - 1, +m[3]));
  return `${ALMANAC_WEEKDAYS[d.getUTCDay()]} ${+m[3]} ${ALMANAC_MONTHS[+m[2] - 1]}`;
}

/* Said under the days when today's row shows the merge gate's count (#509). */
export const ALMANAC_LIVE_NOTE = "Today's row may show the merge gate's own count, which is ahead of GitHub's until GitHub is read again";

const almanacSum = (rows, k) => rows.reduce((n, r) => n + (r[k] || 0), 0);
/* The label's line: "7 landed today (avg 9)", the average over the days before today. */
export function almanacToday(gh, ms = null, utcDay) {
  const rows = almanacRows(gh, ms, utcDay);
  if (!rows.length) return null;
  const last = rows[rows.length - 1], before = last.today ? rows.slice(0, -1) : rows;
  const avg = before.length ? Math.round(almanacSum(before, 'landed') / before.length) : 0;
  return last.today ? `${last.landed} landed today (avg ${avg})` : `avg ${avg} landed a day`;
}

/* The dialog: the totals over the days shown, the median claimed→landed, as_of, then each day. */
export function almanacFacts(gh, ms = null, utcDay) {
  const rows = almanacRows(gh, ms, utcDay);
  if (!rows.length) return null;
  const n = rows.length, reverts = rows.some(r => r.reverted != null);
  const median = almanacSpan(gh.median_claim_to_land_secs);
  const meta = [
    `Landed: ${almanacSum(rows, 'landed')} issues in ${n} days`,
    `Opened: ${almanacSum(rows, 'opened')}`,
    `Bounced: ${almanacSum(rows, 'bounced')} times the merge gate handed work back`,
    reverts ? `Reverted: ${almanacSum(rows, 'reverted')} on the integration branch` : 'Reverted: unknown (the integration branch’s log could not be read)',
    `Deploys: ${almanacSum(rows, 'deploys')} commits deployed`,
    median ? `Median from claimed to landed: ${median}` : 'Median from claimed to landed: no claims on record',
    `As of ${almanacAsOf(gh.as_of)}`,
    ...rows.map(r => `${almanacDate(r.date)}${r.today ? ' (today)' : ''}: landed ${r.landed} · bounced ${r.bounced}${r.reverted ? ` · reverted ${r.reverted}` : ''} · opened ${r.opened} · deploys ${r.deploys}`),
    ...(rows.some(r => r.live) ? [ALMANAC_LIVE_NOTE] : []),
  ];
  return { line: almanacToday(gh, ms, utcDay), meta };
}
