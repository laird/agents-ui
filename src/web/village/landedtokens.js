/* ---------- Tokens per landed issue (#517) ----------
 * What an issue cost, from claim to land: the total of `usage.issues`
 * (src/web/usage.rs, #514), read from Claude Code's transcripts. Shown on a
 * landed crate's dialog (lifecycle.js) and in its travel log (travel.js).
 * An issue with no usage on record says nothing. No DOM, so node tests it
 * (tools/village-tests/landedtokens.test.mjs). */

/* The issue's tokens, { input, output, cache_write, cache_read, total }, or null. */
export function landedTokens(v, n) {
  const hit = ((v && v.usage && v.usage.issues) || []).find(i => i && i.issue === n);
  const t = hit && hit.total;
  return t && Number.isFinite(t.total) && t.total > 0 ? t : null;
}

/* 950, 12k, 3.4M: a count of tokens, short. */
export function shortTokens(n) {
  if (n < 1000) return String(n);
  if (n < 1e6) return `${n < 1e4 ? (n / 1e3).toFixed(1).replace(/\.0$/, '') : Math.round(n / 1e3)}k`;
  return `${n < 1e7 ? (n / 1e6).toFixed(1).replace(/\.0$/, '') : Math.round(n / 1e6)}M`;
}

/* "Tokens from claim to land: 3.4M" and what they were, or null. */
export function landedTokenWords(v, n) {
  const t = landedTokens(v, n);
  if (!t) return null;
  const parts = [`${shortTokens(t.input || 0)} in`, `${shortTokens(t.output || 0)} out`,
    `${shortTokens((t.cache_read || 0) + (t.cache_write || 0))} cache`];
  return { line: `Tokens from claim to land: ${shortTokens(t.total)}`, detail: parts.join(' · ') };
}
