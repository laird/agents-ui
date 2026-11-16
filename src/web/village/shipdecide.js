/* ---------- An epic reaching production: the decisions (#482) ----------
 * No DOM, so node tests it (tools/village-tests/shipdecide.test.mjs).
 *
 * The server lists the epics that reached the last stop today in
 * `v.shipped`, newest first, each with when its voyage docked (`at`). The
 * village sets off fireworks once for a shipping it sees arrive, flies the
 * newest one's flag on the hall for the rest of the day, and the screensaver
 * says "Shipped: <epic>" for a while. */

/* Fireworks only for a shipping this recent: opening the page hours later
 * shows the flag, not a show. */
export const SHIP_FRESH = 15 * 60e3;
/* How long the screensaver's strip says "Shipped: <epic>". */
export const SHIP_SHOWN = 2 * 60 * 60e3;
/* A little slack for a server clock ahead of this one. */
const SKEW = 60e3;

const shipAge = (s, now) => now - Date.parse((s && s.at) || '');

/* Which of `shipped` set off fireworks now: those not yet in `seen` (a Set
 * of issue numbers, updated here, so each fires once) that shipped within
 * SHIP_FRESH. In the night hours (`quiet`) none do, and they are still
 * marked seen: no fireworks in the morning for a shipping at 3 am. */
export function newCelebrations(shipped, seen, now, quiet) {
  const out = [];
  for (const s of shipped || []) {
    if (seen.has(s.number)) continue;
    seen.add(s.number);
    const age = shipAge(s, now);
    if (!quiet && age >= -SKEW && age < SHIP_FRESH) out.push(s);
  }
  return out;
}

/* The flag on the hall's pole: the newest epic that shipped today. */
export const hallFlag = shipped => (shipped && shipped.length ? shipped[0] : null);

/* What hovering the hall's flag says. */
export function hallFlagWords(shipped) {
  const s = hallFlag(shipped);
  if (!s) return '';
  const more = shipped.length - 1;
  return `Shipped today: #${s.number} ${s.title}` + (more ? ` (and ${more} more)` : '');
}

/* The screensaver's line: the newest epic shipped within SHIP_SHOWN, and how
 * many more did; null when none did. */
export function shippedLine(shipped, now) {
  const recent = (shipped || []).filter(s => { const a = shipAge(s, now); return a >= -SKEW && a < SHIP_SHOWN; });
  if (!recent.length) return null;
  const s = recent[0];
  return `Shipped: #${s.number} ${s.title}` + (recent.length > 1 ? ` (+${recent.length - 1})` : '');
}
