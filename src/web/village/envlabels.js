/* ---------- What the environment row says ----------
 * The words on each environment village and what the canal still names
 * (#440). No DOM, so its tests run under node
 * (tools/village-tests/envlabels.test.mjs); envrow.js and ship.js draw them. */

/* Does this stop stand as a village of its own in the environment row: every
 * environment of a development village. The sentinel's green ref stays a
 * lock (a gate), and a runtime's stages stay its pipeline. */
export const inRow = (v, s) => !!v && v.plane !== 'runtime' && !!s && s.kind !== 'gate';

/* What an environment runs, in a word: its release tag, else its commit. */
export const versionOf = s => s.tag || (s.sha ? String(s.sha).slice(0, 7) : null);

/* How long ago an RFC 3339 time was, in a few words; null when it isn't one. */
export function agoWords(iso, now = Date.now()) {
  const at = Date.parse(iso || '');
  if (Number.isNaN(at)) return null;
  const s = Math.max(0, (now - at) / 1000);
  if (s < 90) return 'just now';
  if (s < 5400) return `${Math.round(s / 60)} min ago`;
  if (s < 129600) return `${Math.round(s / 3600)} h ago`;
  return `${Math.round(s / 86400)} days ago`;
}

/* The one label an environment village has, at its warehouse: its name and
 * the version it runs ("release · v2026.11.09.5"), and under it when that
 * was deployed. Nothing known says "unknown". */
export function envLabel(s, now = Date.now()) {
  const ver = versionOf(s), when = agoWords(s.deployed_at, now);
  return { text: `${s.name} · ${ver || 'unknown'}`, sub: ver && when ? when : null };
}

/* The canal's labels: a lock is named only when its stop has no village of
 * its own (a gate); the others are the dev village's outgoing docks, and
 * their names stand once, at their villages. The castle names the last stop
 * on the same terms. Returns { locks: [name or null per lock], castle }. */
export function canalNames(v) {
  const stops = (v && v.pipeline && v.pipeline.stops) || [], last = stops[stops.length - 1];
  return { locks: stops.slice(0, -1).map(s => (inRow(v, s) ? null : s.name)), castle: last && !inRow(v, last) ? last.name : null };
}

/* The houses of an environment village: one per feature it runs (its cargo
 * grouped as cargoGroups does), up to `room`, each flying its feature's
 * colour; how many features more besides. A village running no feature has
 * no houses. */
export const ENV_HOUSES = { town: 4, capital: 3 };
export function envHouses(groups, rank) {
  const fs = (groups || []).filter(g => g.f), room = ENV_HOUSES[rank] || ENV_HOUSES.town;
  return { houses: fs.slice(0, room).map(g => ({ f: g.f, col: g.col, n: g.ns.length })), more: Math.max(0, fs.length - room) };
}
