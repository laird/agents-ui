/* ---------- Deep links into a village ----------
 * `/village#<project>&house=<name>` opens that house's dialog once the
 * village is drawn (#480): the desktop notifier links a "waiting on you"
 * straight to the house it is about. `&issue=`, `&gate=`, `&build` and
 * `&env=` open the rest of what a notification can be about (#579, for
 * #570): linkTarget says what. `<name>` is the house's short name
 * (`wt-2`, its worktree less the project prefix), its full id, or its
 * agent's role; `manager` is the hall. No DOM here: houselink.js opens it,
 * and node tests this (tools/village-tests/deeplink.test.mjs). */

/* The `house=` flag of a hash such as `#shop&house=wt-2`, or null. */
export function houseParam(hash) {
  const flag = String(hash || '').replace(/^#/, '').split('&').slice(1).find(f => f.startsWith('house='));
  if (!flag) return null;
  const raw = flag.slice('house='.length);
  let name = raw;
  try { name = decodeURIComponent(raw); } catch {}
  return name || null;
}

/* A house's short name: its worktree's folder less the project prefix, as
 * the notifier names it (Model.js houseName in the Omarchy plugin). */
function shortHouse(project, h) {
  const base = String(h.id || '').replace(/\/+$/, '').split('/').pop() || '';
  return base.startsWith(project + '-') ? base.slice(project.length + 1) : base;
}

/* What `name` opens in village `v`: { kind: 'hall' }, { kind: 'house', i },
 * or null when the village has no such house. */
export function houseTarget(v, name) {
  if (!v || !name) return null;
  if (name === 'manager' || name === 'hall') return { kind: 'hall' };
  const houses = v.houses || [];
  // The full id first: the notifier names a house by it when two share a short name.
  let i = houses.findIndex(h => h.id === name);
  if (i < 0) i = houses.findIndex(h => shortHouse(v.project, h) === name);
  if (i < 0) i = houses.findIndex(h => h.role === name);
  return i < 0 ? null : { kind: 'house', i };
}

/* Each flag of a hash after its project, as [name, value], decoded; a bare
 * flag such as `build` has the value ''. */
export function linkFlags(hash) {
  return String(hash || '').replace(/^#/, '').split('&').slice(1).filter(Boolean).map(f => {
    const k = f.indexOf('='), name = k < 0 ? f : f.slice(0, k), raw = k < 0 ? '' : f.slice(k + 1);
    let value = raw;
    try { value = decodeURIComponent(raw); } catch {}
    return [name, value];
  });
}

/* What a deep link into village `v` opens: an object to act() on as a click
 * does ({ kind, ... }), { kind: 'issue', n } for #564's issue dialog, or
 * { stale } with what to say when what it names is no longer there. null
 * when the link names nothing to open. A house or an issue that is waiting
 * on you opens as a click on its red "!" does (#571): its question first. */
export function linkTarget(v, hash) {
  if (!v) return null;
  const f = new Map(linkFlags(hash)), houses = v.houses || [];
  if (f.has('house')) {
    const name = f.get('house'), o = name ? houseTarget(v, name) : null;
    if (!o) return name ? { stale: `${name} is no longer waiting` } : null;
    const h = o.kind === 'hall' ? v.manager : houses[o.i];
    return h && h.status === 'blocked' ? { kind: 'alarm', i: o.kind === 'hall' ? -1 : o.i } : o;
  }
  if (f.has('issue')) {
    const n = Number(f.get('issue'));
    if (!(n > 0)) return null;
    const i = houses.findIndex(h => h.issue === n && h.status === 'blocked');
    return i >= 0 ? { kind: 'alarm', i } : { kind: 'issue', n };
  }
  if (f.has('gate')) {
    const n = Number(f.get('gate')), p = v.pipeline || {};
    if (!(n > 0)) return null;
    const i = houses.findIndex(h => h.issue === n);
    if (i >= 0 && ((p.bounces || {})[n] || []).length) return { kind: 'bounce', n, i };
    if ((p.gates || []).some(e => e.issue === n)) return { kind: 'gatecrate', n };
    return { stale: `#${n} is no longer at the merge gate` };
  }
  if (f.has('build')) return v.build ? { kind: 'build' } : { stale: 'Nothing reports this village\'s build' };
  if (f.has('env')) {
    const name = f.get('env'), stops = (v.pipeline || {}).stops || [], i = stops.findIndex(s => s.name === name);
    if (i < 0) return name ? { stale: `${name}: no such environment` } : null;
    // An environment with a village of its own in the row, else its stop on the canal (envlabels.js inRow).
    return v.plane !== 'runtime' && stops[i].kind !== 'gate' ? { kind: 'env', i } : { kind: 'stop', i };
  }
  return null;
}
