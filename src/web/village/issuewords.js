/* ---------- An issue's own dialog, in words (#564) ----------
 * The detail the server reads (issue.rs, on
 * /api/swarms/<project>/issues/<n>): its labels sorted so a blocking one
 * leads, each with the board's own colour and glyph (palette.js, cvdmarks.js,
 * #443) so it reads the same under colour blindness; and why it's blocked,
 * read off the newest comment that says so. No DOM, so node tests it
 * (tools/village-tests/issuewords.test.mjs); issuedialog.js draws it. */
import { NOTECOL } from './palette.js';
import { GLYPH_WORDS } from './cvdmarks.js';
import { APPROVE_LABELS, ANSWER_LABELS } from './decidewords.js';

export const issueUrl = (project, n) => `api/swarms/${encodeURIComponent(project)}/issues/${n}`;

/* A human-judgement gate, not a status the swarm set itself (merging,
 * awaiting-integration, working, subtask, decomposed are the swarm's own
 * bookkeeping, not something a person needs to resolve). The decidable ones
 * (#565) come from decidewords.js, so this list can't drift from which
 * labels the issue dialog's Approve/Answer/Reject buttons actually cover;
 * `proposal` is decided through its own flow (proposals.js) but still gets
 * the chip and the "why it's blocked" section here. */
const BLOCKING = [...APPROVE_LABELS, ...ANSWER_LABELS, 'proposal'];

/* Labels with a blocking one first (stable past that), each with its
 * board colour and glyph when it has one. */
export function labelChips(labels) {
  const ls = (labels || []).slice();
  ls.sort((a, b) => BLOCKING.includes(b) - BLOCKING.includes(a));
  return ls.map(name => ({ name, blocking: BLOCKING.includes(name), color: NOTECOL[name] || null, glyph: GLYPH_WORDS[name] || null }));
}

/* Why it's blocked, in words: the server's reason, or "no reason recorded"
 * when it has none (an issue with no comments at all). */
export function blockedReasonWords(detail) {
  return (detail && detail.blocked_reason) || 'no reason recorded';
}

/* "#7 Saved carts". */
export function issueTitleWords(detail) {
  return `#${detail.number} ${detail.title || ''}`.trim();
}

/* "P1 · open" / "open" with no priority. */
export function issueLineWords(detail) {
  return [detail.priority, detail.state].filter(Boolean).join(' · ');
}

/* The last few comments, newest last, as the thread itself keeps them,
 * capped so a long-lived issue's dialog stays short; the rest are implied
 * by the server's own comment count, not shown. */
export const RECENT_COMMENTS = 5;
export function recentComments(detail) {
  const cs = (detail && detail.comments) || [];
  return cs.slice(Math.max(0, cs.length - RECENT_COMMENTS));
}
