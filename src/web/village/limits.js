/* ---------- Usage limits ----------
 * A worker whose runtime stopped on a usage or rate limit (#484: `limit` on
 * a house, read by the server from the bottom of its pane). It is not idle
 * and not waiting on you: it waits at a dry well until the limit lifts. When
 * most of the village is limited at once, a sign at the hall says so, so a
 * quiet village is not mistaken for a stalled one. No DOM: tested under node
 * (tools/village-tests/limits.test.mjs). The well itself is well.js. */

/* Whether a house's crew is held by a limit: idle or done with one. A
 * working house has got past it, a blocked one is waiting on you first, and
 * stalled, unknown or starting keep their own look. */
export const limited = h => !!(h && h.limit && (h.status === 'idle' || h.status === 'done'));

/* When it lifts, as the runtime said it: "3pm (Europe/London)", "in 2 hours". */
export const resetsOf = h => (h && h.limit && h.limit.resets) || null;

/* "Rate limited · resets 3pm", for a dialog. */
export function limitWords(h) {
  if (!limited(h)) return null;
  const r = resetsOf(h);
  return r ? `Rate limited · resets ${/^in /.test(r) ? r : 'at ' + r}` : 'Rate limited · waiting for it to lift';
}

/* What a limited house's label says under its name: short, so it fits a lot. */
export function limitSub(h) {
  const r = resetsOf(h);
  return r ? `limited · ${/^in /.test(r) ? r : 'til ' + r}` : 'rate limited';
}

/* The village at its limit: most of its houses (more than half of those not
 * stopped) are limited. `{ n, of, resets }`, or null. `resets` is the first
 * reset a limited house names, in the runtime's words: they are free text
 * ("3pm (Europe/London)", "in 2 hours"), so they are not compared. */
export function villageLimit(v) {
  const houses = ((v && v.houses) || []).filter(h => h.status !== 'stopped');
  const held = houses.filter(limited);
  if (!held.length || held.length * 2 <= houses.length) return null;
  const said = held.map(resetsOf).filter(Boolean);
  return { n: held.length, of: houses.length, resets: said[0] || null };
}

/* The hall's sign: "Usage limit: resumes 3pm (Europe/London)". */
export function signWords(s) {
  if (!s) return null;
  if (!s.resets) return 'Usage limit: resumes when it lifts';
  return `Usage limit: resumes ${/^in /.test(s.resets) ? s.resets : '~' + s.resets}`;
}

/* How many houses are limited, counted apart from blocked and idle. */
export const limitedCount = v => ((v && v.houses) || []).filter(limited).length;
