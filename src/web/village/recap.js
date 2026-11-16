/* ---------- Screensaver: "since you last looked" (#479) ----------
 * What the summary card says when the screensaver comes up after a quiet
 * spell, with no DOM and no imports, so node can test it (tools/omarchy-plugin/
 * agents-village-screensaver/tests/recap.test.mjs). screensaver.js shows it.
 *
 * The launcher passes `since=<unix seconds>`. What closed and merged comes
 * from `/api/village/since` (GitHub, server side); deploys, gate bounces and
 * what waits on you come from `/api/village`, which the page already has. */

/* `since=<unix seconds>` from the query, in ms, or null. A time in the
 * future or before 2000 is a mistake, not a quiet spell. */
export function recapSince(search, now = Date.now()) {
  const raw = new URLSearchParams(String(search || '')).get('since');
  if (!raw || !/^\d+$/.test(raw)) return null;
  const ms = Number(raw) * 1000;
  return ms >= Date.UTC(2000, 0, 1) && ms <= now ? ms : null;
}

const atOrAfter = (iso, since) => {
  const t = Date.parse(iso || '');
  return !Number.isNaN(t) && t >= since;
};

const recapSha = sha => String(sha || '').slice(0, 7);

/* What waits on you in one village now: the board, blocked houses and
 * manager, and work the gate handed back (as saverSummary counts it). */
export function waitingItems(v) {
  const items = [];
  for (const n of (v && v.board) || []) items.push({ issue: n.number ?? null, text: n.title || n.label || 'On the board' });
  for (const h of [(v && v.manager) || null, ...((v && v.houses) || [])]) {
    if (h && h.status === 'blocked') items.push({ issue: h.issue ?? null, text: `${h.role || h.id || 'An agent'} is blocked${h.issue_title ? ': ' + h.issue_title : ''}` });
  }
  for (const g of ((v && v.pipeline) || {}).gates || []) {
    if (g.state === 'failed' || g.state === 'conflict' || (g.state === 'red-base' && !g.landed)) {
      items.push({ issue: g.issue ?? null, text: g.state === 'conflict' ? 'Merge conflict at the gate' : 'Failed at the gate' });
    }
  }
  return items;
}

/* The card's model. `villages` is `/api/village`'s list, `changes` is
 * `/api/village/since`'s (or null when it did not answer). Per village:
 * landed (closed as completed), closed otherwise, merged pull requests,
 * deploys whose time is after `since` (with the release tag or short sha),
 * bounces at the gate, and what waits on you. `quiet` when nothing at all
 * happened; `unchecked` names villages GitHub couldn't be asked about. */
export function recapModel(villages, changes, since) {
  const byProject = new Map((((changes && changes.villages) || [])).map(c => [c.project, c]));
  const rows = (villages || []).map(v => {
    const c = byProject.get(v.project);
    const closed = (c && c.closed) || [];
    const p = v.pipeline || {};
    const titles = p.titles || {};
    const bounces = Object.entries(p.bounces || {}).flatMap(([issue, list]) =>
      (list || []).filter(b => atOrAfter(b.at, since)).map(b => ({ issue: +issue, kind: b.kind, title: titles[issue] || '' })));
    return {
      project: v.project,
      checked: !!(c && c.checked),
      landed: closed.filter(i => (i.reason || 'completed') === 'completed'),
      closed: closed.filter(i => i.reason && i.reason !== 'completed'),
      merged: (c && c.merged) || [],
      deploys: (p.stops || []).filter(s => s.kind === 'environment' && atOrAfter(s.deployed_at, since))
        .map(s => ({ name: s.name, version: s.tag || recapSha(s.sha), at: s.deployed_at })),
      bounces,
      waiting: waitingItems(v),
    };
  });
  const happened = r => r.landed.length + r.closed.length + r.merged.length + r.deploys.length + r.bounces.length;
  return {
    since,
    villages: rows,
    quiet: rows.every(r => !happened(r)),
    waiting: rows.reduce((n, r) => n + r.waiting.length, 0),
    unchecked: rows.filter(r => !r.checked).map(r => r.project),
  };
}

const recapPlural = (n, one, many = one + 's') => `${n} ${n === 1 ? one : many}`;

const BOUNCE_WORDS = { conflict: 'conflicted', 'gate-failed': 'failed the gate', 'push-retry': 'was re-synced' };

/* One village's card lines: `{ text, red }`, most important first. */
export function recapLines(r) {
  const lines = [];
  const list = xs => xs.slice(0, 3).map(i => `#${i.number}`).join(', ') + (xs.length > 3 ? ', …' : '');
  if (r.landed.length) lines.push({ text: `${recapPlural(r.landed.length, 'issue')} landed (${list(r.landed)})`, red: false });
  if (r.merged.length) lines.push({ text: `${recapPlural(r.merged.length, 'pull request')} merged (${list(r.merged)})`, red: false });
  if (r.closed.length) lines.push({ text: `${recapPlural(r.closed.length, 'issue')} closed unfinished (${list(r.closed)})`, red: false });
  for (const d of r.deploys) lines.push({ text: `Deployed to ${d.name}${d.version ? ' · ' + d.version : ''}`, red: false });
  for (const b of r.bounces) lines.push({ text: `#${b.issue} ${BOUNCE_WORDS[b.kind] || 'bounced'}${b.title ? ': ' + b.title : ''}`, red: b.kind !== 'push-retry' });
  // What waits on you: one line, the item itself when it is the only one.
  const w = r.waiting;
  if (w.length === 1) lines.push({ text: `Waiting on you: ${w[0].issue ? '#' + w[0].issue + ' ' : ''}${w[0].text}`, red: true });
  else if (w.length) {
    const nums = w.filter(x => x.issue).map(x => `#${x.issue}`);
    lines.push({ text: `${w.length} waiting on you${nums.length ? ` (${nums.slice(0, 4).join(', ')}${nums.length > 4 ? ', …' : ''})` : ''}`, red: true });
  }
  return lines;
}

/* The card's heading: "Since 11:40 pm (6h ago)". `clock` formats a time. */
export function recapHeading(since, now = Date.now(), clock = ms => new Date(ms).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })) {
  const m = Math.max(0, Math.round((now - since) / 60000));
  const ago = m < 60 ? `${m}m ago` : m < 48 * 60 ? `${Math.round(m / 60)}h ago` : `${Math.round(m / 1440)}d ago`;
  return `Since ${clock(since)} (${ago})`;
}

/* How long the card stays up before the live village, in ms. */
export const RECAP_MS = 20000;
