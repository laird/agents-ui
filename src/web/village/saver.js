/* ---------- Screensaver: the decisions ----------
 * What the screensaver mode (screensaver.js) shows and when, with no DOM
 * (its one import, limits.js, has none either), so node can test it
 * (tools/omarchy-plugin/agents-village-screensaver/tests/saver.test.mjs).
 *
 * The mode is `/village?screensaver=1` or `/village#<project>&screensaver`.
 * Options ride along in the query: `cycle=<seconds>` between swarms (0 stays
 * put), `dim=<from>-<to>` night hours, `dismiss=1` to close on input (only
 * the Omarchy launcher asks for that; a dashboard tab never closes itself),
 * `overview=1` to show every swarm at once (#481): first, between swarms
 * while cycling, or alone with `cycle=0`),
 * `sound=1` for quiet cues on transitions (#489, sounds.js; off by default). */

import { limitedCount, signWords, villageLimit } from './limits.js';

/* The village or world the hash names: `#shop&screensaver` is `shop`. */
export function hashName(hash) {
  const raw = String(hash || '').replace(/^#/, '').split('&')[0];
  try { return decodeURIComponent(raw); } catch { return raw; }
}

const nightHours = s => {
  const m = /^(\d{1,2})-(\d{1,2})$/.exec(String(s || ''));
  return m && +m[1] < 24 && +m[2] < 24 && m[1] !== m[2] ? [+m[1], +m[2]] : null;
};

/* Is the screensaver on, and how it behaves. A project named in the hash is
 * pinned: it is not cycled away from unless `cycle` asks for it. */
export function saverOptions(search, hash) {
  const q = new URLSearchParams(String(search || ''));
  const flags = String(hash || '').replace(/^#/, '').split('&').slice(1);
  const on = ['1', 'true', 'on', ''].includes(q.get('screensaver')) || flags.includes('screensaver');
  const pinned = hashName(hash) || null;
  const asked = q.has('cycle') ? Math.max(0, parseInt(q.get('cycle'), 10) || 0) : null;
  return {
    on,
    pinned,
    cycle: asked !== null ? asked : pinned ? 0 : 30,
    dim: nightHours(q.get('dim')),
    dismiss: q.get('dismiss') === '1',
    overview: ['1', 'true', 'on'].includes(q.get('overview')),
    sound: ['1', 'true', 'on'].includes(q.get('sound')),
  };
}

/* What the status strip says about one village. "Waiting on you" counts what
 * the bar widget counts: blocked houses and manager, the board, and a gate
 * that handed work back. Houses held by a usage limit are counted apart
 * (#484): they wait on the limit, not on you; `limit` is the hall's sign when
 * most of the village is held. */
export function saverSummary(v) {
  const houses = (v && v.houses) || [];
  const p = (v && v.pipeline) || {};
  const gates = p.gates || [];
  const n = st => houses.filter(h => h.status === st).length;
  const handedBack = gates.filter(g => g.state === 'failed' || g.state === 'conflict' || (g.state === 'red-base' && !g.landed)).length;
  const blocked = n('blocked'), held = limitedCount(v);
  return {
    project: v ? v.project : '',
    working: n('working'),
    blocked,
    limited: held,
    limit: signWords(villageLimit(v)),
    idle: houses.length - n('working') - blocked - held,
    waiting: ((v && v.board) || []).length + blocked + (v && v.manager && v.manager.status === 'blocked' ? 1 : 0) + handedBack,
    gate: {
      queued: gates.filter(g => g.state === 'queued').length,
      gating: gates.filter(g => g.state === 'gating' || g.state === 'merging').length,
      failed: handedBack,
    },
    quay: (p.quay || []).length,
    stops: (p.stops || []).map(s => ({ name: s.name, count: (s.issues || []).length })),
    build: (v && v.build) || null,
    landed: landedWords(v && v.github),
  };
}

/* "7 landed today (avg 9)" (#500): issues GitHub closed as completed today,
 * and the mean per day, rounded, as { today, avg }. The mean leaves today out
 * (#508), so a morning doesn't drag it down: it is over the full days before
 * today in #498's summary (14, today last). A repo younger than that (its
 * `first_commit` inside the window) averages over its full days from the
 * first that landed anything; `avg` is null, and the strip shows no average,
 * until it has one. An older repo averages over them all, quiet ones too
 * (avg 0 is an answer). null, and
 * the strip shows nothing, when there is no summary (no gh, or not signed
 * in), when a day's count is not a number, or when the newest day is not
 * `today` (the viewer's date, YYYY-MM-DD): just after midnight the cached
 * summary still ends at yesterday. */
export function landedWords(github, today = localDate(new Date())) {
  const days = github && Array.isArray(github.days) ? github.days : [];
  if (!days.length || !days.every(d => d && Number.isFinite(d.landed))) return null;
  const last = days[days.length - 1];
  if (last.date !== today) return null;
  const before = days.slice(0, -1), born = github.first_commit;
  const young = typeof born === 'string' && before.length > 0 && born >= before[0].date;
  const first = before.findIndex(d => d.landed > 0);
  const full = !young ? before : first < 0 ? [] : before.slice(first);
  return { today: last.landed, avg: full.length ? Math.round(full.reduce((sum, d) => sum + d.landed, 0) / full.length) : null };
}
export const localDate = d => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;

/* The strip's build entry (#413's `build`, #415): a red build is "BUILD
 * BROKEN", readable across the room, with what and since when; a running one
 * says so; green or unknown add nothing. `clock` formats an ISO time. */
export function buildEntry(build, clock = iso => new Date(iso).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })) {
  if (!build || build.state === 'green' || !['red', 'running'].includes(build.state)) return null;
  if (build.state === 'running') return { red: false, text: 'Build running', detail: '' };
  const oses = [...new Set((build.jobs || []).filter(j => j.state === 'red' && j.os).map(j => j.os))];
  const n = (build.tests || []).length, since = build.since && !Number.isNaN(Date.parse(build.since)) ? clock(build.since) : '';
  const detail = [build.partial && oses.length ? `on ${oses.join(', ')}` : '', n ? `${n} ${n === 1 ? 'test' : 'tests'} failing` : '', since ? `since ${since}` : '']
    .filter(Boolean).join(' · ');
  return { red: true, text: 'BUILD BROKEN', detail };
}

/* The gate in a few words. */
export function gateWords(g) {
  const parts = [];
  if (g.gating) parts.push(`${g.gating} gating`);
  if (g.queued) parts.push(`${g.queued} queued`);
  if (g.failed) parts.push(`${g.failed} failed`);
  return parts.join(' · ') || 'clear';
}

/* What the screensaver cycles through: each world, then each village that is
 * not already half of a world. */
export function cycleOrder(villages, worlds) {
  const inWorld = new Set((worlds || []).flatMap(w => [w.dev, w.platform]));
  return [...(worlds || []).map(w => w.name), ...(villages || []).map(v => v.project).filter(p => !inWorld.has(p))];
}

/* The one after `current`, round the list. */
export function nextName(names, current) {
  if (!names.length) return null;
  return names[(names.indexOf(current) + 1) % names.length];
}

/* Burn-in care: where the whole scene sits at `ms`, a few pixels off centre,
 * wandering on two slow periods so it never retraces a short loop. */
export function driftAt(ms, amp = 8) {
  const a = (ms / 600000) * 2 * Math.PI, b = (ms / 470000) * 2 * Math.PI;
  return { x: Math.round(amp * Math.sin(a)), y: Math.round(amp * Math.sin(b + 1)) };
}

/* Is `hour` inside the night hours [from, to)? Ranges may wrap midnight. */
export function dimmedAt(hour, range) {
  if (!range) return false;
  const [from, to] = range;
  return from < to ? hour >= from && hour < to : hour >= from || hour < to;
}

/* Does this input dismiss the screensaver? Keys, clicks, wheels and touches
 * do; the pointer must travel some way, so a nudged desk does not. */
export function dismisses(e, start, travel = 24) {
  if (['keydown', 'pointerdown', 'mousedown', 'wheel', 'touchstart'].includes(e.type)) return true;
  if (e.type !== 'pointermove' && e.type !== 'mousemove') return false;
  return !!start && Math.hypot(e.x - start.x, e.y - start.y) > travel;
}

/* "12s ago", "4m ago": how stale the last answer is. */
export function staleWords(ms) {
  const s = Math.max(0, Math.round(ms / 1000));
  return s < 60 ? `${s}s ago` : s < 3600 ? `${Math.round(s / 60)}m ago` : `${Math.round(s / 3600)}h ago`;
}
