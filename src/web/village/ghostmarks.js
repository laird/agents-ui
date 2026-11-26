/* ---------- Ghost marks ----------
 * Which flaky tests (`flaky` in /api/village, #501) haunt the gatehouse, how
 * faint each ghost is, what its label says and where it hovers (#502). No
 * DOM, so its tests run under node (tools/village-tests/ghosts.test.mjs).
 * ghosts.js draws them. */

/* A flaky test is kept for a week after its last failure (flaky.rs). */
export const GHOST_WEEK = 7 * 24 * 3600 * 1000;

/* How solid a ghost is, from 1 (it just failed) down to a quarter after a
 * week clean; 0 (gone) from then on, or when the time is unknown. */
export function ghostFade(lastSeen, now) {
  const t = Date.parse(lastSeen || '');
  if (Number.isNaN(t)) return 0;
  const clean = Math.max(0, now - t);
  return clean >= GHOST_WEEK ? 0 : 1 - 0.75 * (clean / GHOST_WEEK);
}

/* A test's short name: its last path segment, cut to `max` characters. */
export function shortTest(name, max = 22) {
  const s = String(name || '').split('::').pop().trim() || String(name || '');
  return s.length > max ? s.slice(0, max - 1) + '…' : s;
}

/* What a ghost's label says: the test and how often it flaked this week. */
export const ghostLabel = f => `${shortTest(f.test)} ×${f.count}`;
/* The label's parts for the first `shown` ghosts, and "+N more" past them. */
export function ghostTags(ghosts, shown) {
  const tags = ghosts.slice(0, shown).map(({ f }) => ghostLabel(f));
  return ghosts.length > shown ? [...tags, `+${ghosts.length - shown} more`] : tags;
}

/* Issues that have landed: at the quay, at a stop, or in a stop's cargo. A
 * `Flaky:` issue among them was fixed. */
export function landedIssues(v) {
  const p = (v && v.pipeline) || {};
  return new Set([...(p.quay || []), ...(p.stops || []).flatMap(s => [...(s.issues || []), ...(s.cargo || [])])]);
}

/* Each `Flaky:` issue this page has seen a test carry, by test, with the
 * test's last failure then: { issue, last }. A test that carried one and no
 * longer does had it closed. Call once per poll. */
export function noteFlakyIssues(v, seen) {
  for (const f of (v && v.flaky) || []) if (f.issue) seen.set(f.test, { issue: f.issue, last: f.last_seen });
  return seen;
}

/* Did this test's `Flaky:` issue close: it landed, or the test had one on an
 * earlier poll, has none now, and has not failed since (a failure after the
 * close brings the ghost back until a new issue is filed and closed). */
function issueClosed(f, landed, seen) {
  if (f.issue) return landed.has(f.issue);
  const was = seen.get(f.test);
  return !!was && !(Date.parse(f.last_seen || '') > Date.parse(was.last || ''));
}

/* The ghosts at the gatehouse, newest first: { f, alpha }. A test leaves
 * when it has been clean a week, or once its `Flaky:` issue has closed
 * (issueClosed). */
export function ghostsOf(v, now, seen = new Map()) {
  const landed = landedIssues(v);
  return ((v && v.flaky) || []).flatMap(f => {
    const alpha = ghostFade(f.last_seen, now);
    return alpha > 0 && !issueClosed(f, landed, seen) ? [{ f, alpha }] : [];
  });
}

/* How many ghosts hover at once over a gatehouse `g`: one per 9 scene
 * pixels of its roof, at least one and at most three. Past that, the last
 * says how many more. */
export const ghostRoom = g => Math.max(1, Math.min(3, Math.floor((g.w - 4) / 9)));
/* Where ghost k hovers: over the gatehouse's roof, 6×7 scene pixels each,
 * from its east end westward (clear of the failure lamp at its corner and
 * of the scoreboard east of it), bobbing a pixel unless `still`. */
export function ghostSpot(g, k, t = 0, still = true) {
  const bob = still ? 0 : Math.round(Math.sin(t / 700 + k * 2.1));
  return { x: g.x0 + g.w - 10 - k * 9, y: g.y - 3 + bob };
}

/* The failures in a ghost's dialog, newest first:
 * "16 Nov 08:10 · gating #498 · /tmp/autocoder-merge-…-498.log", or, for a
 * sighting the gate recorded (#538), the first line of its failure. */
export function failureLines(f, clock) {
  const said = r => r.log || (r.excerpt || '').split('\n').map(l => l.trim()).find(Boolean) || '';
  return ((f && f.runs) || []).map(r => [r.when ? clock(r.when) : '', r.issue ? `gating #${r.issue}` : '', said(r)].filter(Boolean).join(' · ')).filter(Boolean);
}

/* "File flaky issue" in the ghost's dialog (#503): where it posts, and what
 * the answer says. A read-only server answers 403 before anything is filed. */
export const flakyFileUrl = project => `api/swarms/${encodeURIComponent(project)}/flaky`;
export function flakyFileWords(status, body) {
  if (status >= 200 && status < 300) {
    const n = body && body.number ? `#${body.number}` : 'the issue';
    if (body && body.action === 'unchanged') return `${n} already lists every failure`;
    return body && body.action === 'commented' ? `Added the new failures to ${n}` : `Filed as ${n}`;
  }
  if (status === 403) return 'This dashboard is read-only: file it on GitHub.';
  if (status === 501) return (body && body.error) || 'Filing needs the GitHub tracker.';
  if (status === 404) return (body && body.error) || 'Nothing to file: this test has not flaked here this week.';
  return (body && body.error) || `Could not file it (${status})`;
}
/* The button's words: comment on the open issue, or file a new one. */
export const flakyButtonWords = f => (f.issue ? `Add these failures to #${f.issue}` : 'File flaky issue');
