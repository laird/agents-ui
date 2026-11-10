/* ---------- Rest ----------
 * A worker with nothing claimed (#411). Just after it lands or closes its
 * issue it shows a ✓ for a while; after that, or when it was simply idle, it
 * sits on its doorstep and "zzz" drifts up from the house. No DOM: tested
 * under node (tools/village-tests/rest.test.mjs). */

/* How long a finished worker shows its ✓. */
export const FRESH_DONE_MS = 5 * 60 * 1000;

/* 'fresh' (✓), 'asleep' (zzz, sitting) or null (anything else: working,
 * blocked, stalled, ... keep their own look). `nowMs` is wall-clock time;
 * `h.since` is when the server first saw the house in its status. A 'done'
 * with no `since` keeps its ✓: nothing says how long ago it finished. */
export function restOf(h, nowMs) {
  if (h.status === 'idle') return 'asleep';
  if (h.status !== 'done') return null;
  const at = h.since ? Date.parse(h.since) : NaN;
  if (Number.isNaN(at)) return 'fresh';
  return nowMs - at < FRESH_DONE_MS ? 'fresh' : 'asleep';
}
