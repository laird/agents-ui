/* ---------- Bounce marks ----------
 * What the gate's bounces (`pipeline.bounces` in /api/village, #431) mean
 * for each house (#433). No DOM, so its tests run under node
 * (tools/village-tests/bouncemarks.test.mjs). bounces.js draws them. */

/* What a bounce's tag says: "↺ conflict", and from the second bounce on
 * how many it has been, "↺2 conflict" (#412's retry markers too). */
const WORDS = { conflict: 'conflict', 'gate-failed': 'gate failed', 'push-retry': 'push retry' };
export const bounceWord = (kind, count = 1) => `↺${count > 1 ? count : ''} ${WORDS[kind] || 'bounced'}`;

/* An issue's bounces, oldest first. */
export const bouncesOf = (v, n) => ((v && v.pipeline && v.pipeline.bounces) || {})[n] || [];

/* Has this issue landed: at the quay or at any stop past it. */
export function landed(v, n) {
  const p = (v && v.pipeline) || {};
  return (p.quay || []).includes(n) || (p.stops || []).some(s => (s.issues || []).includes(n));
}

/* Each house whose issue the gate handed back and that hasn't landed:
 * { i, n, count, last, tag, debris }. `tag` names the last bounce and the
 * count ("↺ conflict", "↺2 conflict"); `debris` is set from the second
 * bounce on, and clears when the issue lands. */
export function bounceMarks(v) {
  return ((v && v.houses) || []).flatMap((h, i) => {
    const bs = h.issue ? bouncesOf(v, h.issue) : [];
    if (!bs.length || landed(v, h.issue)) return [];
    const last = bs[bs.length - 1];
    return [{ i, n: h.issue, count: bs.length, last, tag: bounceWord(last.kind, bs.length), debris: bs.length >= 2 }];
  });
}

/* "2 Nov 21:40": when, in this machine's time. "" when unknown. */
export function when(iso) {
  const t = Date.parse(iso || '');
  if (Number.isNaN(t)) return '';
  const d = new Date(t), pad = n => String(n).padStart(2, '0');
  return `${d.getDate()} ${['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'][d.getMonth()]} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/* The bounces in the crate's dialog, newest first, numbered by attempt:
 * "Attempt 2 · conflict · 9 Nov 21:40 · src/web/server.rs, main.js". */
export function bounceLines(bs, clock = when) {
  return bs.map((b, k) => [`Attempt ${k + 1}`, WORDS[b.kind] || b.kind, b.at ? clock(b.at) : '', (b.files || []).join(', ')].filter(Boolean).join(' · ')).reverse();
}

/* How many steps up each tag stands, for tags over houses at `lots`
 * ({ x, y } each): a tag that would overlap one already placed in the same
 * row (closer than `span` scene pixels) goes a step higher, and so on. */
export function tagLevels(lots, span = 48) {
  const placed = [];
  return lots.map(lot => {
    let level = 0;
    while (placed.some(p => p.y === lot.y && p.level === level && Math.abs(p.x - lot.x) < span)) level += 1;
    placed.push({ x: lot.x, y: lot.y, level });
    return level;
  });
}
