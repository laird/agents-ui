/* ---------- Deciding proposals: the words and choices (#483) ----------
 * DOM-free, so node can test it (tools/village-tests/decide.test.mjs);
 * proposals.js puts it on the board's dialog. */

/* Where a decision on issue `n` of village `project` is posted. */
export const decisionUrl = (project, n) =>
  `api/swarms/${encodeURIComponent(project)}/issues/${encodeURIComponent(n)}/decision`;

export const isProposal = note => !!note && note.label === 'proposal' && note.number > 0;

/* 0 for P0 … 3 for P3; an issue with no priority ranks below P3. */
const proposalRank = p => { const m = /^P(\d)$/.exec(p || ''); return m ? Number(m[1]) : 9; };

/* "Approve all P3 and below": the proposals at P3, or with no priority. */
export const bulkPicks = board => (board || []).filter(n => isProposal(n) && proposalRank(n.priority) >= 3);

/* What the dialog says after the server answers. */
export function answerWords(status, body, action) {
  if (status >= 200 && status < 300) {
    return { approve: 'Approved: on its way to the docks', reject: 'Rejected: closed', changes: 'Asked for changes' }[action] || 'Done';
  }
  if (status === 403) return 'This dashboard is read-only: decide on GitHub.';
  if (status === 409) return 'No longer a proposal: someone decided already.';
  return (body && body.error) || `Could not decide (${status})`;
}

/* The bulk approval's question, over the list of what it would approve. */
export const bulkWords = picks => `Approve ${picks.length === 1 ? 'this proposal' : `these ${picks.length} proposals`}?`;

/* One proposal in that list. */
export const pickWords = n => `#${n.number} ${n.title}${n.priority ? ' · ' + n.priority : ' · no priority'}`;
