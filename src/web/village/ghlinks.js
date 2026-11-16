/* ---------- Links to GitHub, for the dialogs (#491) ----------
 * The pages behind an object that the report doesn't link itself: a branch,
 * its pull request, the integration branch's history, the Actions tab. Each
 * takes `repo`, which the caller gets from repoUrl() (dialogs.js): only a
 * real issue URL says which GitHub repo this is, so with no repo there is no
 * link. No DOM here, so node can test it (tools/village-tests/ghlinks.test.mjs). */

const ghLink = (label, url) => (url ? { label, url } : null);
/* A branch in a URL path: its slashes kept (`tree/feature/issue-7`), the rest escaped. */
const branchPath = b => encodeURIComponent(b).replace(/%2F/g, '/');
const PR_URL = /\/pull\/\d+\/?$/;

/* A pull request the report already links (an adapter may), if any. */
export const prOf = links => (links || []).find(l => l && PR_URL.test(l.url || '')) || null;
/* The search for a branch's pull requests: GitHub finds any, open or merged. */
export const prSearchUrl = (repo, branch) =>
  (repo && branch ? `${repo}/pulls?q=${encodeURIComponent(`is:pr head:${branch}`)}` : null);

/* A worker's branch and its pull request. The PR the report links is shown
 * with the report's own links already, so the search is only offered when
 * there is none. Nothing for the integration branch itself: the hall links
 * its history (commitsLink). */
export function branchLinks(repo, branch, links, integration) {
  if (!repo || !branch || branch === integration) return [];
  return [ghLink(`Branch ${branch}`, `${repo}/tree/${branchPath(branch)}`),
    prOf(links) ? null : ghLink(`Pull request for ${branch}`, prSearchUrl(repo, branch))].filter(Boolean);
}

/* A gate entry's pull request: the branch's, when a house on its issue says
 * which branch it is, else a search for pull requests that name the issue. */
export function gatePrLink(repo, n, branch) {
  if (!repo || !n) return null;
  if (branch) return ghLink(`#${n} pull request`, prSearchUrl(repo, branch));
  // No branch known: a search, which can also match pull requests that only mention the number.
  return ghLink(`Pull requests mentioning #${n}`, `${repo}/pulls?q=${encodeURIComponent(`is:pr ${n}`)}`);
}

/* The integration branch's commit history. */
export const commitsLink = (repo, branch) =>
  (repo && branch ? ghLink(`Commits on ${branch}`, `${repo}/commits/${branchPath(branch)}`) : null);

/* The repo's workflow runs, for a build with no run of its own to open (green). */
export const actionsLink = repo => (repo ? ghLink('Workflow runs', `${repo}/actions`) : null);
