/* ---------- The gate's tally, in words ----------
 * What the gate's scoreboard and its hotspots sign say (#435), from
 * `pipeline.merge_stats` (#431): today's landings and bounces, and the files
 * that conflict most. No DOM, so node tests it
 * (tools/village-tests/scoretally.test.mjs). */

/* "landed 14 · bounced 6 today", or null when the gate reports no tally. */
export function scoreLine(ms) {
  if (!ms) return null;
  return `landed ${ms.landed_today || 0} · bounced ${ms.bounced_today || 0} today`;
}

/* The hotspots, most conflicted first (then by name), as the sign lists them:
 * "src/web/village/main.js ×4". */
export function hotspotLines(ms) {
  return [...((ms && ms.hotspots) || [])]
    .sort((a, b) => b.count - a.count || (a.file < b.file ? -1 : a.file > b.file ? 1 : 0))
    .map(h => `${h.file} ×${h.count}`);
}
