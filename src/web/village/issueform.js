/* ---------- Filing an issue: the form's decisions (#490) ----------
 * DOM-free, so node can test it (tools/village-tests/issueform.test.mjs);
 * newissue.js draws the form. The server checks the same limits again
 * (newissue.rs). */

export const ISSUE_TYPES = ['enhancement', 'bug'];
export const ISSUE_PRIORITIES = ['P0', 'P1', 'P2', 'P3'];
export const TITLE_MAX = 256;
export const BODY_MAX = 64 * 1024;

/* Where the form posts, for village `project`. */
export const newIssueUrl = project => `api/swarms/${encodeURIComponent(project)}/issues`;

/* The request for what the form holds, or { error } saying what to fix. */
export function issueRequest({ title, body, type, priority }) {
  const t = String(title || '').trim(), b = String(body || '').trim();
  if (!t) return { error: 'Give it a title.' };
  if ([...t].length > TITLE_MAX || /[\r\n]/.test(t)) return { error: `The title must be one line of at most ${TITLE_MAX} characters.` };
  if (new TextEncoder().encode(b).length > BODY_MAX) return { error: 'The body is longer than 64 KB.' };
  const labels = [ISSUE_TYPES.includes(type) ? type : 'enhancement', ISSUE_PRIORITIES.includes(priority) ? priority : 'P3'];
  return { title: t, ...(b ? { body: b } : {}), labels };
}

/* What the form says after the server answers: { words, url } for a link. */
export function filedWords(status, body, repo) {
  if (status >= 200 && status < 300) {
    return body && body.number ? { words: `Filed #${body.number}: it joins the backlog on the next refresh.`, url: body.url }
      : { words: 'Filed: it joins the backlog on the next refresh.' };
  }
  if (status === 403) return { words: 'This dashboard is read-only: file it on GitHub.', url: repo ? `${repo}/issues/new` : null };
  return { words: (body && body.error) || `Could not file it (${status})` };
}
