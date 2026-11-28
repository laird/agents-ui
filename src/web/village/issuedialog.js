/* ---------- An issue's own dialog, in the village (#564) ----------
 * The board's rows, and any list naming an issue (the docks, a feature's or
 * the ship's subtasks), get `#N` as a button (dialogs.js); clicking it, or
 * the "Open issue" button a single-subject dialog offers for the issue it
 * is about (a crate's, or a house's claimed one — the same number travel.js
 * already finds), reads the issue in full from the server (issue.rs) and
 * shows it here: its labels (a blocking one first, with the board's own
 * colour and glyph), why it's blocked, its body (markdown, rendered safely:
 * `markdown.js`), and its last few comments, collapsed. The GitHub link it
 * already had stays, now secondary. A platform's workloads (`plane:
 * 'runtime'`) aren't issues, so they get no button. */
import { state } from './state.js';
import { registerDialogPart } from './registry.js';
import { esc } from './panels.js';
import { link, openInfo } from './dialogs.js';
import { renderMarkdown } from './markdown.js';
import { blockedReasonWords, issueLineWords, issueTitleWords, issueUrl, labelChips, recentComments } from './issuewords.js';

/* The dialog this fetch last filled: a slow answer for an earlier click is dropped. */
let issueToken = 0;

/* Issue `n`'s dialog. `top` goes first in it: a "!"'s house (its question,
 * answered with ask()) and what it waits on (alarmhit.js, #571). */
export async function showIssueDialog(project, n, top = {}) {
  const mine = ++issueToken;
  openInfo({ ...top, title: `#${n}`, line: 'Loading…', project, opened: n });
  let d = null, status = 0;
  try {
    const res = await fetch(issueUrl(project, n));
    status = res.status;
    if (res.ok) d = await res.json();
  } catch { /* unreachable: said below */ }
  if (mine !== issueToken) return;
  if (!d) { openInfo({ ...top, title: `#${n}`, line: status ? "Couldn't read this issue from GitHub." : "Can't reach agents-tui.", project, opened: n }); return; }
  openInfo({ ...top, title: issueTitleWords(d), line: issueLineWords(d), links: [link('On GitHub', d.url)], project, issueDetail: d, opened: n });
}

function chipHtml(c) {
  return `<span class="chip" style="${c.color ? `background:${esc(c.color)}` : ''}">${c.glyph ? esc(c.glyph) + ' ' : ''}${esc(c.name)}</span>`;
}
function commentHtml(c) {
  return `<p class="comment"><span class="who">${esc(c.author || 'someone')}:</span> ${esc(c.body)}</p>`;
}
function renderDetail(box, d) {
  const chips = labelChips(d.labels), blocked = chips.some(c => c.blocking), comments = d.comments || [];
  box.innerHTML = (chips.length ? `<div class="chips">${chips.map(chipHtml).join('')}</div>` : '') +
    (blocked ? `<p class="blocked"><strong>Why it's blocked:</strong> ${esc(blockedReasonWords(d))}</p>` : '') +
    (d.body ? `<div class="body">${renderMarkdown(d.body)}</div>` : '') +
    (comments.length ? `<details><summary>${comments.length === 1 ? '1 comment' : `${comments.length} comments`}</summary>${recentComments(d).map(commentHtml).join('')}</details>` : '');
}

registerDialogPart(info => {
  // A fresh box each time, so no listener is left from the last dialog.
  const old = document.getElementById('d-issue');
  const box = old.cloneNode(false);
  old.replaceWith(box);
  if (info.issueDetail) { renderDetail(box, info.issueDetail); return; }
  const project = info.project, v = project && (state.scenes[project] || {}).v;
  if (!v || v.plane === 'runtime') return;
  // Every row of a list naming an issue (dialogs.js's items).
  document.querySelectorAll('#d-meta li[data-n] button.issue-open').forEach(b => {
    b.addEventListener('click', () => showIssueDialog(project, Number(b.dataset.n)));
  });
  // A single-subject dialog's own issue (the same number travel.js uses), or
  // the one a manager's question names (#571); none while it is that issue's.
  const n = info.openIssue || info.travel || (info.house && info.house.issue) || null;
  if (n && n !== info.opened) {
    box.innerHTML = `<div class="opts"><button type="button">${info.openIssue ? `Open #${n}` : 'Open issue'}</button></div>`;
    box.querySelector('button').addEventListener('click', () => showIssueDialog(project, n));
  }
}, 45);
