/* ---------- House labels ----------
 * What the label under a house says without a click (#402): its name, the
 * issue it is on, and the first words of that issue's title. No DOM, so its
 * tests run under node (tests/houselabel.test.mjs). */

/* A title cut to its first few words, ending in an ellipsis when cut. */
export const TITLE_WORDS = 4, TITLE_CHARS = 24;
export function shortTitle(title, words = TITLE_WORDS, chars = TITLE_CHARS) {
  const all = String(title || '').trim().split(/\s+/).filter(Boolean);
  if (!all.length) return '';
  let out = all.slice(0, words).join(' '), cut = all.length > words;
  if (out.length > chars) {
    // Cut at a word if one fits, else mid-word (one very long word).
    out = out.slice(0, chars - 1).replace(/\s+\S*$/, '') || out.slice(0, chars - 1);
    cut = true;
  }
  return cut ? out.replace(/[\s,.;:·-]+$/, '') + '…' : out;
}

/* A house's label: `parts` read as one line (`wt-1 · #400`) and wrap between
 * them when the lot is narrow; `sub` is the short title, shown when there is
 * room. A house on no issue keeps its name, and says `idle` when it is. */
export function houseLabel(name, house, title) {
  const n = house && house.issue;
  if (!n) return { parts: [name], sub: house && house.status === 'idle' ? 'idle' : null };
  return { parts: [name, `· #${n}`], sub: shortTitle(title) || null };
}

/* A feature flag's label (#441): its number, a name of a word or two, and
 * its subtasks landed of all: `#417 Hi-res · 1/6`. The name is what comes
 * before a colon ("Harden swarm recognition: …"), unless that is a one-word
 * area tag ("Village: …", "Proposal: …"); a "(#…)" is left out. The full
 * title is in the dialog. */
export function flagLabel(n, title, build) {
  let t = String(title || '').trim();
  const colon = t.match(/^([^:]{1,40}):\s*(.*)$/);
  if (colon) t = /\s/.test(colon[1].trim()) ? colon[1] : colon[2];
  t = t.replace(/\s*[([].*$/, '');
  const name = shortTitle(t, 2, 12);
  const b = build || { landed: 0, total: 0 };
  return `#${n}${name ? ' ' + name : ''} · ${b.landed}/${b.total}`;
}

/* The hall's subtitle: the manager, and its issue when it has one. */
export const managerSub = m => (m && m.issue ? `manager · #${m.issue}` : 'manager');
