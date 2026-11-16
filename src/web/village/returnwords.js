/* ---------- Returned goods: the words ----------
 * Landed work that came back (#497): reopened, reverted, or named by a
 * regression report, as the server reads it into the GitHub summary
 * (`github.returned`, src/web/returned.rs). No DOM, so node tests it
 * (tools/village-tests/returnwords.test.mjs); returned.js draws it. */

/* At most this many returned crates on the quay; the dialog lists the rest. */
export const RETURNED_CAP = 2;

/* What came back, in the server's order: reopened, reverted, regressions. */
export const returnedGoods = v => ((v && v.github && v.github.returned) || []).filter(r => r && Number.isFinite(r.number));

/* "3 of 41 came back (7%)": the window's quality figure, or null with
 * nothing landed to measure it by. */
export function returnedShare(gh) {
  const of = gh && Number.isFinite(gh.returned_of) ? gh.returned_of : 0;
  if (!of) return null;
  const n = ((gh && gh.returned) || []).length;
  return { n, of, pct: Math.round((100 * n) / of), words: `${n} of ${of} landed came back (${Math.round((100 * n) / of)}%)` };
}

/* Why it came back, in a line. */
export function returnedLine(r) {
  if (r.how === 'reopened') return 'Reopened after it landed';
  if (r.how === 'reverted') return `Reverted on the integration branch${r.revert ? `: ${r.revert}` : ''}`;
  if (r.how === 'regression') return `Named in a regression report${r.report ? `, #${r.report}` : ''}`;
  return 'Came back';
}
