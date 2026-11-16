/* ---------- Deep links to a house ----------
 * `/village#<project>&house=<name>` opens that house's dialog once the
 * village is drawn (#480): the desktop notifier links a "waiting on you"
 * straight to the house it is about. `<name>` is the house's short name
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

/* A house's short name: its worktree's folder less the project prefix. */
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
  let i = houses.findIndex(h => h.id === name || shortHouse(v.project, h) === name);
  if (i < 0) i = houses.findIndex(h => h.role === name);
  return i < 0 ? null : { kind: 'house', i };
}
