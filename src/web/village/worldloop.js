/* ---------- A world's loop, without the DOM (#560) ----------
 * No DOM, so node tests it (tools/village-tests/worldloop.test.mjs).
 *
 * What a world's polls set going (crates afloat, a house's sparkles, the
 * platform's fireworks), how a crate shows the part of the platform that
 * raised it, and the world's line in the bar. worlds.js draws the rest. */
import { FEATURE_COLS } from './palette.js';
import { ribbon } from './crates.js';
import { inkOn } from './cvdmarks.js';
import { glyphTag } from './scenery.js';

/* How long a house sparkles: the release's sparkles, and a verified fix's. */
export const SPARKLE_MS = 2400;

/* A channel's colour: a stable hash of its name into the feature colours, so
 * it is the same across polls, reloads and themes. */
export function channelColour(channel) {
  let h = 2166136261;
  for (const ch of String(channel)) h = Math.imul(h ^ ch.codePointAt(0), 16777619) >>> 0;
  return FEATURE_COLS[h % FEATURE_COLS.length];
}
/* A channel's badge: the initials of its words (`log-analyzer` → `LA`), or a
 * one-word name's first two letters, in the capitals the tags write in. */
export function channelBadge(channel) {
  const words = String(channel).toUpperCase().split(/[^A-Z0-9]+/).filter(Boolean);
  return (words.length > 1 ? words.map(w => w[0]).join('') : words[0] || '').slice(0, 2);
}
/* A floating incident's crate at (x, y) in theme T: tied with its channel's
 * colour, its badge on a tag above it. Without a channel, the plain crate. */
export function floatCrate(c, T, x, y, f) {
  T.crate(c, x, y, 'bug', 'P0', false, f.n);
  if (!f.channel) return;
  const col = channelColour(f.channel);
  ribbon(c, x, y, col);
  glyphTag(c, x - 1, y - 9, channelBadge(f.channel), col, inkOn(col));
}

/* A world's page state, kept between polls. */
export const newWorldState = () => ({ seen: null, floaters: [], verified: new Set(), sparkles: [], landed: undefined, upgradedAt: -1e9 });
/* Fold a poll of world `w` at time t into its state `st`: incidents new since
 * the last poll set off downriver, a fix the platform verified stops its
 * crate and sparkles the house that raised it, once. Returns whether the
 * release has just landed on the platform. */
export function foldWorld(st, w, t) {
  if (st.seen) w.incidents.forEach(i => { if (!st.seen.has(i.number)) st.floaters.push({ n: i.number, origin: i.origin, channel: i.channel, t0: t }); });
  st.seen = new Set(w.incidents.map(i => i.number));
  const verified = w.verified || [];
  st.floaters = st.floaters.filter(f => !verified.some(i => i.number === f.n));
  verified.forEach(i => {
    if (st.verified.has(i.number)) return;
    st.verified.add(i.number);
    if (i.origin) st.sparkles.push({ origin: i.origin, t0: t });
  });
  const landed = st.landed === false && w.landed === true;
  if (landed) st.upgradedAt = t;
  st.landed = w.landed;
  return landed;
}
/* The platform houses sparkling at time t for a verified fix. */
export const sparkling = (st, t) => st.sparkles.filter(s => t >= s.t0 && t - s.t0 < SPARKLE_MS).map(s => s.origin);

/* The bar's line for a world. The map draws the incidents, the fixes, the
 * houses and what is shipping; what it can't draw is which release the
 * platform runs, the build IDs its workers report, and how the window's
 * incidents have gone round, so the line says so. */
const short = s => (s ? s.slice(0, 7) : '—');
export function worldWords(w) {
  const release = w.landed === null ? 'platform reports no build IDs' : w.landed ? 'running on the platform' : 'rolling out';
  const builds = (w.platform_builds || []).map(short).join(' ');
  const n = w.counts;
  const loop = n ? ` · raised ${n.raised} · landed ${n.landed} · deployed ${n.deployed} · verified ${n.verified}` : '';
  return `· ${w.dev} builds ${w.platform} · release ${short(w.release)} ${release}${builds ? ` · platform builds ${builds}` : ''}${loop}`;
}
