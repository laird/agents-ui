/* ---------- Screensaver: the decisions ----------
 * What the screensaver mode (screensaver.js) shows and when, with no DOM and
 * no imports, so node can test it (tools/omarchy-plugin/
 * agents-village-screensaver/tests/saver.test.mjs).
 *
 * The mode is `/village?screensaver=1` or `/village#<project>&screensaver`.
 * Options ride along in the query: `cycle=<seconds>` between swarms (0 stays
 * put), `dim=<from>-<to>` night hours, `dismiss=1` to close on input (only
 * the Omarchy launcher asks for that; a dashboard tab never closes itself). */

/* The village or world the hash names: `#shop&screensaver` is `shop`. */
export function hashName(hash) {
  const raw = String(hash || '').replace(/^#/, '').split('&')[0];
  try { return decodeURIComponent(raw); } catch { return raw; }
}

const nightHours = s => {
  const m = /^(\d{1,2})-(\d{1,2})$/.exec(String(s || ''));
  return m && +m[1] < 24 && +m[2] < 24 && m[1] !== m[2] ? [+m[1], +m[2]] : null;
};

/* Is the screensaver on, and how it behaves. A project named in the hash is
 * pinned: it is not cycled away from unless `cycle` asks for it. */
export function saverOptions(search, hash) {
  const q = new URLSearchParams(String(search || ''));
  const flags = String(hash || '').replace(/^#/, '').split('&').slice(1);
  const on = ['1', 'true', 'on', ''].includes(q.get('screensaver')) || flags.includes('screensaver');
  const pinned = hashName(hash) || null;
  const asked = q.has('cycle') ? Math.max(0, parseInt(q.get('cycle'), 10) || 0) : null;
  return {
    on,
    pinned,
    cycle: asked !== null ? asked : pinned ? 0 : 30,
    dim: nightHours(q.get('dim')),
    dismiss: q.get('dismiss') === '1',
  };
}

/* What the status strip says about one village. "Waiting on you" counts what
 * the bar widget counts: blocked houses and manager, the board, and a gate
 * that handed work back. */
export function saverSummary(v) {
  const houses = (v && v.houses) || [];
  const p = (v && v.pipeline) || {};
  const gates = p.gates || [];
  const n = st => houses.filter(h => h.status === st).length;
  const handedBack = gates.filter(g => g.state === 'failed' || g.state === 'conflict' || (g.state === 'red-base' && !g.landed)).length;
  const blocked = n('blocked');
  return {
    project: v ? v.project : '',
    working: n('working'),
    blocked,
    idle: houses.length - n('working') - blocked,
    waiting: ((v && v.board) || []).length + blocked + (v && v.manager && v.manager.status === 'blocked' ? 1 : 0) + handedBack,
    gate: {
      queued: gates.filter(g => g.state === 'queued').length,
      gating: gates.filter(g => g.state === 'gating' || g.state === 'merging').length,
      failed: handedBack,
    },
    quay: (p.quay || []).length,
    stops: (p.stops || []).map(s => ({ name: s.name, count: (s.issues || []).length })),
  };
}

/* The gate in a few words. */
export function gateWords(g) {
  const parts = [];
  if (g.gating) parts.push(`${g.gating} gating`);
  if (g.queued) parts.push(`${g.queued} queued`);
  if (g.failed) parts.push(`${g.failed} failed`);
  return parts.join(' · ') || 'clear';
}

/* What the screensaver cycles through: each world, then each village that is
 * not already half of a world. */
export function cycleOrder(villages, worlds) {
  const inWorld = new Set((worlds || []).flatMap(w => [w.dev, w.platform]));
  return [...(worlds || []).map(w => w.name), ...(villages || []).map(v => v.project).filter(p => !inWorld.has(p))];
}

/* The one after `current`, round the list. */
export function nextName(names, current) {
  if (!names.length) return null;
  return names[(names.indexOf(current) + 1) % names.length];
}

/* Burn-in care: where the whole scene sits at `ms`, a few pixels off centre,
 * wandering on two slow periods so it never retraces a short loop. */
export function driftAt(ms, amp = 8) {
  const a = (ms / 600000) * 2 * Math.PI, b = (ms / 470000) * 2 * Math.PI;
  return { x: Math.round(amp * Math.sin(a)), y: Math.round(amp * Math.sin(b + 1)) };
}

/* Is `hour` inside the night hours [from, to)? Ranges may wrap midnight. */
export function dimmedAt(hour, range) {
  if (!range) return false;
  const [from, to] = range;
  return from < to ? hour >= from && hour < to : hour >= from || hour < to;
}

/* Does this input dismiss the screensaver? Keys, clicks, wheels and touches
 * do; the pointer must travel some way, so a nudged desk does not. */
export function dismisses(e, start, travel = 24) {
  if (['keydown', 'pointerdown', 'mousedown', 'wheel', 'touchstart'].includes(e.type)) return true;
  if (e.type !== 'pointermove' && e.type !== 'mousemove') return false;
  return !!start && Math.hypot(e.x - start.x, e.y - start.y) > travel;
}

/* "12s ago", "4m ago": how stale the last answer is. */
export function staleWords(ms) {
  const s = Math.max(0, Math.round(ms / 1000));
  return s < 60 ? `${s}s ago` : s < 3600 ? `${Math.round(s / 60)}m ago` : `${Math.round(s / 3600)}h ago`;
}
