/* ---------- The GitHub panel's words and links (#492) ----------
 * What waits on GitHub for this repo, a line each, from the server's cached
 * summary (`github` on /api/village, github.rs, shared with the almanac):
 * open pull requests and the oldest's age, review requests waiting on you,
 * open issues by label (proposals awaiting approval, blocked `needs-*`, the
 * backlog by priority), what landed on the integration branch in the last
 * 24 h, and the issues closed today. Each line links the matching GitHub
 * search, built on `repo` from repoUrl() (dialogs.js): with no repo there is
 * no link, never a guessed one. A count the server couldn't read says so
 * instead of showing a zero. No DOM: node tests it
 * (tools/village-tests/ghpanel.test.mjs); ghpanel.js draws the sign. */
import { commitsLink } from './ghlinks.js';

const ghSearch = (repo, page, query) => (repo ? `${repo}/${page}?q=${encodeURIComponent(query)}` : null);
const howMany = (n, one, many = one + 's') => `${n} ${n === 1 ? one : many}`;
const PRIORITIES = ['P0', 'P1', 'P2', 'P3', 'P4'];

/* How long from `from` to `to` (RFC 3339), in whole units: "40 min", "5 h", "3 d"; null when either is unreadable. */
export function ageWords(from, to) {
  const a = Date.parse(from || ''), b = Date.parse(to || '');
  if (!Number.isFinite(a) || !Number.isFinite(b)) return null;
  const m = Math.max(0, Math.round((b - a) / 60000));
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  return h < 48 ? `${h} h` : `${Math.floor(h / 24)} d`;
}

/* When the summary was read, from `now` (ms): "as of 2 min ago". */
export function asOfWords(g, now) {
  const w = g && ageWords(g.as_of, new Date(now).toISOString());
  if (!w) return '';
  return w === '0 min' ? 'as of just now' : `as of ${w} ago`;
}

/* The open issues' `needs-*` labels: what blocks them, by name. */
export const blockedLabels = by => Object.keys(by || {}).filter(l => /^needs-/.test(l)).sort();

/* The panel: [{ text, url, mine? }], in order. `mine` marks what waits on
 * you (review requests, proposals to approve), when there is any. */
export function panelLines(g, repo, integration) {
  if (!g) return [];
  const out = [], by = g.open_by_label || {};
  const line = (text, url, mine) => out.push({ text, url: url || null, ...(mine ? { mine: true } : {}) });
  if (g.open_prs == null) line("Open pull requests: GitHub couldn't be read");
  else {
    const age = g.open_prs && ageWords(g.oldest_pr_at, g.as_of);
    line(`${howMany(g.open_prs, 'open pull request')}${age ? `, the oldest ${age} old` : ''}`, ghSearch(repo, 'pulls', 'is:pr is:open'));
  }
  if (g.review_requests == null) line("Review requests: GitHub couldn't be read");
  else line(`${howMany(g.review_requests, 'review request')} waiting on you`, ghSearch(repo, 'pulls', 'is:pr is:open review-requested:@me'), g.review_requests > 0);
  if (g.open_issues == null) line("Open issues by label: GitHub couldn't be read");
  else {
    const proposals = by.proposal || 0, blocked = blockedLabels(by);
    line(`${howMany(proposals, 'proposal')} awaiting approval`, ghSearch(repo, 'issues', 'is:issue is:open label:proposal'), proposals > 0);
    if (blocked.length) line(`Blocked: ${blocked.map(l => `${by[l]} ${l}`).join(', ')}`, ghSearch(repo, 'issues', `is:issue is:open label:${blocked.join(',')}`));
    else line('Nothing blocked (needs-*)', ghSearch(repo, 'issues', 'is:issue is:open label:needs-clarification,needs-design,needs-approval'));
    const prios = PRIORITIES.filter(p => by[p]);
    line(prios.length ? `By priority: ${prios.map(p => `${p} ${by[p]}`).join(' · ')}` : 'By priority: none labelled',
      ghSearch(repo, 'issues', `is:issue is:open label:${(prios.length ? prios : PRIORITIES).join(',')}`));
    line(`${howMany(g.open_issues, 'open issue')} in all`, ghSearch(repo, 'issues', 'is:issue is:open'));
  }
  if (g.commits_24h != null) {
    const history = commitsLink(repo, integration);
    line(`${howMany(g.commits_24h, 'commit')} landed on ${integration || 'the integration branch'} in the last 24 h`, history && history.url);
  }
  const today = (g.days || []).length ? g.days[g.days.length - 1].date : null;
  line(`${howMany(g.closed_today || 0, 'issue')} closed today`, today ? ghSearch(repo, 'issues', `is:issue is:closed closed:>=${today}`) : null);
  return out;
}

/* The sign's label, short: what waits on you, then the open pull requests. */
export function panelSub(g) {
  if (!g) return '';
  const by = g.open_by_label || {}, parts = [];
  if (g.review_requests) parts.push(`${g.review_requests} to review`);
  if (by.proposal) parts.push(howMany(by.proposal, 'proposal'));
  if (g.open_prs != null) parts.push(howMany(g.open_prs, 'PR'));
  if (g.open_prs == null && g.open_issues == null) return 'not read';
  return parts.join(' · ');
}

/* The dialog: the lines with a page to open as its links, the rest (none
 * without a repo) as plain facts. */
export function panelDialog(g, repo, integration, now) {
  const lines = panelLines(g, repo, integration);
  return {
    title: 'On GitHub',
    line: ['What waits on GitHub for this repo', asOfWords(g, now)].filter(Boolean).join(', ') + '.',
    meta: lines.filter(l => !l.url).map(l => l.text),
    links: lines.filter(l => l.url).map(l => ({ label: l.text, url: l.url }))
  };
}
