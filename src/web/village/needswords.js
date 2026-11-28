/* ---------- The "Needs you" list's model ----------
 * Every item waiting on the user, across every village the client knows
 * about (#580, for #570): blocked houses and managers (alarms.js), merge-
 * gate failures and conflicts (gatehouse.js), a red build (buildweather.js)
 * and a failed deployment (envrow.js). Mirrors each kind's own alarm so the
 * list and the map always agree. Issues on the board with a blocking label
 * are #581's, not here yet. Newest first; grouped by village, each group
 * itself newest first by its own newest item. No DOM, so node tests this
 * (tools/village-tests/needswords.test.mjs); needs.js opens it and renders
 * it (dialogs.js's openInfo/explain/act open each item's own dialog). */
import { waitingWords } from './alarms.js';

// Mirrors gatehouse.js's gateEntries and envrow.js's healthOf (renamed:
// the replay page bundles every module into one script, so a top-level
// name must be its own). Importing either file here would also pull in
// their DOM-touching imports (dialogs.js, layout.js), breaking this
// module's own "no DOM" guarantee.
const needsGateEntries = v => (v.pipeline && v.pipeline.gates) || [];
const needsHealthOf = s => (s.health && s.health.state) || (s.source === 'unknown' ? 'unknown' : 'ok');

const NEEDS_GATE_WORDS = { failed: 'Gate failed', conflict: 'Merge conflict' };

/* Village `v`'s items waiting on the user: { key, project, title, detail,
 * since, target }. `key` is stable across polls (for `&needs&open=`);
 * `target` is what dialogs.js's explain()/openInfo open, the same shape
 * deeplink.js's linkTarget uses for a blocked house (`alarm`) or a gate
 * (`bounce`/`gatecrate`). */
export function villageNeeds(v) {
  if (!v) return [];
  const out = [];
  if (v.manager && v.manager.status === 'blocked') {
    out.push({
      key: `${v.project}:manager`, project: v.project, title: 'Manager',
      detail: waitingWords(v.manager), since: v.manager.since || null, target: { kind: 'alarm', i: -1 },
    });
  }
  (v.houses || []).forEach((h, i) => {
    if (h.status !== 'blocked') return;
    const title = h.issue ? `#${h.issue} ${h.issue_title || ''}`.trim() : (h.role || `house ${i}`);
    out.push({
      key: `${v.project}:house:${h.id || i}`, project: v.project, title,
      detail: waitingWords(h), since: h.since || null, target: { kind: 'alarm', i },
    });
  });
  needsGateEntries(v).forEach(e => {
    if (!NEEDS_GATE_WORDS[e.state]) return;
    const i = (v.houses || []).findIndex(h => h.issue === e.issue);
    const bounced = ((v.pipeline && v.pipeline.bounces) || {})[e.issue];
    const target = i >= 0 && (bounced || []).length ? { kind: 'bounce', n: e.issue, i } : { kind: 'gatecrate', n: e.issue };
    out.push({
      key: `${v.project}:gate:${e.issue}`, project: v.project, title: `#${e.issue}`,
      detail: NEEDS_GATE_WORDS[e.state], since: e.started_at || null, target,
    });
  });
  if (v.build && v.build.state === 'red') {
    out.push({ key: `${v.project}:build`, project: v.project, title: 'Build', detail: 'Red', since: v.build.since || null, target: { kind: 'build' } });
  }
  ((v.pipeline && v.pipeline.stops) || []).forEach((s, i) => {
    if (needsHealthOf(s) !== 'failed') return;
    out.push({
      key: `${v.project}:env:${s.name}`, project: v.project, title: s.name,
      detail: 'Deployment failed', since: s.deployed_at || null, target: { kind: 'env', i },
    });
  });
  return out;
}

/* Newest first; an item with no timestamp sinks below those that have one,
 * rather than winning or losing ties arbitrarily. */
const byNewest = (a, b) => {
  if (!a.since && !b.since) return 0;
  if (!a.since) return 1;
  if (!b.since) return -1;
  return a.since < b.since ? 1 : a.since > b.since ? -1 : 0;
};

/* Every given village's items, grouped (`{ project, items }`), the group
 * with the newest item first, each group's own items newest first. A
 * village with nothing waiting is left out. */
export function needsGroups(villages) {
  const groups = (villages || []).filter(Boolean)
    .map(v => ({ project: v.project, items: villageNeeds(v).sort(byNewest) }))
    .filter(g => g.items.length);
  groups.sort((a, b) => byNewest(a.items[0], b.items[0]));
  return groups;
}

/* Whether a hash asks for the "needs you" list (#580): `#needs` (every
 * village) or `#<project>&needs` (one). The first segment is the project
 * name unless it is the bare `needs` flag itself. */
export function needsWanted(hash) {
  const segs = String(hash || '').replace(/^#/, '').split('&').filter(Boolean);
  return segs[0] === 'needs' || segs.slice(1).includes('needs');
}
/* The project a `&needs` link is scoped to, or null for every village. */
export function needsProject(hash) {
  const segs = String(hash || '').replace(/^#/, '').split('&').filter(Boolean);
  if (!segs.length || segs[0] === 'needs') return null;
  try { return decodeURIComponent(segs[0]) || null; } catch { return segs[0] || null; }
}
/* The `open=` flag's value on a `&needs` link: which item's dialog to show
 * already open over the list, or null. */
export function needsOpenKey(hash) {
  const segs = String(hash || '').replace(/^#/, '').split('&').filter(Boolean);
  const f = segs.slice(1).find(x => x.startsWith('open='));
  if (!f) return null;
  const raw = f.slice('open='.length);
  try { return decodeURIComponent(raw) || null; } catch { return raw || null; }
}

/* The dialog info for the whole list (dialogs.js's openInfo): `needsGroups`
 * is rendered by needs.js's dialog part, replacing the usual issue-items
 * list, since these items are not all issues. */
export function needsListInfo(groups, scopeProject) {
  const n = groups.reduce((sum, g) => sum + g.items.length, 0);
  return {
    title: 'Needs you',
    line: n ? `${n} ${n === 1 ? 'item needs' : 'items need'} you${scopeProject ? ` in ${scopeProject}` : ''}` : 'Nothing needs you right now.',
    needsGroups: groups,
  };
}
