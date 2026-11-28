/* ---------- Deciding a blocked (non-proposal) issue: the words (#565) ----------
 * A proposal keeps its own flow (decide.js, proposals.js); this is for every
 * other blocking label, grouped by what the issue dialog (#564) offers for
 * it (decisions.rs decides the same two groups). No DOM, so node tests it
 * (tools/village-tests/decidewords.test.mjs); decisions.js puts it there. */

/* Gets Approve or Reject. */
const APPROVE_LABELS = ['needs-approval', 'too-complex', 'future', 'blocked'];
/* Gets Answer or Reject. */
const ANSWER_LABELS = ['needs-clarification', 'needs-design', 'needs-feedback'];

/* Where a decision on issue `n` of village `project` is posted. */
export const decideUrl = (project, n) => `api/swarms/${encodeURIComponent(project)}/issues/${n}/decide`;

/* Which group, if any, this issue's labels put it in: 'approve', 'answer',
 * or null (a proposal's label, or none of the above — no buttons here). */
export function decideGroup(labels) {
  const ls = labels || [];
  if (ls.some(l => APPROVE_LABELS.includes(l))) return 'approve';
  if (ls.some(l => ANSWER_LABELS.includes(l))) return 'answer';
  return null;
}

/* What the dialog says after the server answers. */
export function decideAnswerWords(status, body, action) {
  if (status >= 200 && status < 300) {
    return { approve: 'Approved: on its way to the docks', answer: 'Answered: unblocked for the next worker', reject: 'Rejected: closed' }[action] || 'Done';
  }
  if (status === 403) return 'This dashboard is read-only: decide on GitHub.';
  if (status === 409) return 'No longer blocked: someone decided already.';
  return (body && body.error) || `Could not decide (${status})`;
}
