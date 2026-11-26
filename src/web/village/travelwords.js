/* ---------- An issue's travel log, in words (#495) ----------
 * Where an issue's time went, from the server's log (travel.rs, on
 * /api/swarms/<project>/issues/<n>/travel): one row per stage it reached
 * (opened, approved, claimed, first commit, landed, each environment), each
 * with how long it took since the stage before and that stage's median over
 * the last 14 days (`stage_medians` in the GitHub summary, github.rs). The
 * slowest stage is marked. An open issue gets a last row for the wait it is
 * in now. The footer names where the swarm's time goes across issues. No
 * DOM: node tests it (tools/village-tests/travelwords.test.mjs); travel.js
 * draws it in the dialog. */

export const travelUrl = (project, n) => `api/swarms/${encodeURIComponent(project)}/issues/${n}/travel`;

/* Seconds as whole units: "under a minute", "40 min", "5 h", "3 d". */
export function durWords(secs) {
  if (!Number.isFinite(secs) || secs < 0) return null;
  const m = Math.round(secs / 60);
  if (m < 1) return 'under a minute';
  if (m < 60) return `${m} min`;
  const h = Math.round(m / 60);
  return h < 48 ? `${h} h` : `${Math.round(h / 24)} d`;
}

const envOf = key => (key.startsWith('deployed:') ? key.slice('deployed:'.length) : null);

/* What reaching a stage is called: "Claimed", "Deployed to nuc-1", "Released". */
export function stageName(key) {
  const e = envOf(key);
  if (e) return e === 'release' ? 'Released' : `Deployed to ${e}`;
  return { opened: 'Opened', approved: 'Approved', claimed: 'Claimed', first_commit: 'First commit', landed: 'Landed' }[key] || key;
}

/* What the time before a stage was spent on: "waiting for approval". */
export function waitName(key) {
  const e = envOf(key);
  if (e) return e === 'release' ? 'waiting for a release' : `waiting to deploy to ${e}`;
  return { approved: 'waiting for approval', claimed: 'waiting for a worker', first_commit: 'to the first commit', landed: 'coding and the gate' }[key] || key;
}

/* What an open issue waits for after its last stage. */
const NEXT_STAGE = { opened: 'claimed', approved: 'claimed', claimed: 'first_commit', first_commit: 'landed' };

/* The rows of the log: { key, name, at, took, median, slow }; for an open
 * issue a last row { key: 'now', name, took } for the wait it is in at `now`
 * (ms). `who` names the house that claimed it, when the village knows. */
export function travelRows(log, medians = {}, now = Date.now(), who = null) {
  const stages = (log && log.stages) || [];
  const rows = stages.map(s => ({
    key: s.key,
    name: s.key === 'claimed' && who ? `Claimed by ${who}` : stageName(s.key),
    at: s.at,
    took: s.secs == null ? null : durWords(s.secs),
    median: s.secs == null || medians[s.key] == null ? null : durWords(medians[s.key]),
    slow: !!log.slowest && log.slowest === s.key,
  }));
  const last = stages[stages.length - 1];
  if (log && log.state === 'open' && last) {
    const since = (now - Date.parse(last.at)) / 1000;
    const next = NEXT_STAGE[last.key];
    rows.push({ key: 'now', name: `Now ${next ? waitName(next) : 'waiting'}`, at: null, took: durWords(since), median: next && medians[next] != null ? durWords(medians[next]) : null, slow: false });
  }
  return rows;
}

/* One row as a line: "Claimed · 22 h (median 5 h)". */
export function rowWords(r) {
  return [r.name, r.took ? `· ${r.took}` : null, r.median ? `(median ${r.median})` : null, r.slow ? '· slowest' : null].filter(Boolean).join(' ');
}

const GATE_HANDBACKS = { conflict: 'a conflict', 'gate-failed': 'a failed gate', 'push-retry': 'a push rejected and re-synced' };

/* The gate's hand-backs: "Handed back by the gate 2 times: a conflict (src/x.rs), a failed gate". */
export function bounceWords(bounces) {
  const bs = bounces || [];
  if (!bs.length) return null;
  const each = bs.map(b => `${GATE_HANDBACKS[b.kind] || b.kind}${(b.files || []).length ? ` (${b.files.join(', ')})` : ''}`);
  return `Handed back by the gate ${bs.length === 1 ? 'once' : bs.length + ' times'}: ${each.join(', ')}`;
}

const STAGE_ORDER = ['approved', 'claimed', 'first_commit', 'landed'];

/* The footer across issues: each stage's median over the last 14 days, the
 * longest named. Null with no medians. */
export function medianWords(medians) {
  const keys = Object.keys(medians || {}).filter(k => k !== 'opened' && Number.isFinite(medians[k]));
  if (!keys.length) return null;
  const rank = k => (STAGE_ORDER.includes(k) ? STAGE_ORDER.indexOf(k) : STAGE_ORDER.length);
  keys.sort((a, b) => rank(a) - rank(b) || a.localeCompare(b));
  const most = keys.reduce((a, b) => (medians[b] > medians[a] ? b : a));
  return {
    line: `Median over the last 14 days: ${keys.map(k => `${waitName(k)} ${durWords(medians[k])}`).join(' · ')}`,
    most: `Most time goes to ${waitName(most)}`,
  };
}
