/* ---------- Each issue's life ----------
 * The docks, the town hall and its board, the houses, and the people walking
 * between them: what each draws, what a click on it says, its labels and its
 * counters. The walks themselves, what each change between two polls sets
 * off, are the trips (trips.js). */
import { state } from './state.js';
import { P, R, blink, shadow, subHat, waveFrame } from './palette.js';
import { cobwebs, pennant, puffs } from './sprites/houses.js';
import { ICONS, badge, bubble, roped } from './scenery.js';
import { QUIET_ACTIONS, actionSign, drawAction, person, wander } from './people.js';
import { registerCounter, registerHit, registerLabels, registerLayer, tap } from './registry.js';
import { LW, slotPos } from './layout.js';
import { ARRIVE_IN, SAIL_OUT, UNLOAD, pickup, tripAt } from './trips.js';
import { workOf } from './ship.js';
import { houseName, issueItems, issueLabel, issueOf, link, repoUrl, sessionOf } from './dialogs.js';

/* Where a house's sub-agents stand: two beside the door, then a row on the
 * lane in front. Past that, a "+N" badge counts the rest, so every running
 * sub-agent is either drawn or counted, never dropped. */
const SUB_SLOTS = [[-1, 12], [9, 12], [-4, 16], [4, 16], [12, 16]];
const workFrame = t => (!state.reduce && blink(t, 180) ? 'work2' : 'work');

/* ---------- Drawing ---------- */
/* Before anything is drawn: where each trip is this frame. A crate on a trip
 * is not at rest anywhere until the trip has delivered it (scene.bound). */
registerLayer({ z: -10, draw(c, scene, t) {
  const at = scene.at = new Map(state.porters.map(p => [p, tripAt(p, t)]));
  scene.bound = new Set(state.porters.filter(p => p.issue && at.get(p) && !(at.get(p).fetched && !at.get(p).carry)).map(p => p.issue));
} });
/* The docks on the river. */
registerLayer({ z: 5, draw(c, { lay }) {
  tap(0, 0, lay.W, 11, { kind: 'river' });
  state.T.dock(c, 14, 10, 40);
  state.T.yard(c, 6, 18, 56, 34); tap(6, 8, 56, 44, { kind: 'docks' });
} });
/* The supply boat: in from the west, moored while its crew unloads, then away. */
registerLayer({ z: 10, draw(c, scene, t) {
  state.S.supplies = state.S.supplies.filter(j => {
    if (t > j.leave + SAIL_OUT) return false;
    if (t < j.t0) return true;
    const x = t < j.t0 + ARRIVE_IN ? -32 + 46 * ((t - j.t0) / ARRIVE_IN) : t < j.leave ? 14 : 14 - 46 * ((t - j.leave) / SAIL_OUT);
    const load = j.crates.filter((_, k) => t < j.t0 + ARRIVE_IN + k * UNLOAD).length;
    state.T.supply(c, x, t, load);
    tap(x, 0, 30, 11, { kind: 'supply', n: j.crates.length, ns: j.crates });
    return true;
  });
} });
/* The town hall, its board, the manager and the manager's sub-agents. */
registerLayer({ z: 50, draw(c, { v, lay, bound }, t) {
  const { hallX } = lay;
  state.T.hall(c, hallX, 18, v.manager.status); tap(hallX - 4, 6, 40, 36, { kind: 'hall' });
  state.T.board(c, hallX + 42, 28, v.board.filter(b => !bound.has(b.number))); tap(hallX + 42, 28, 18, 16, { kind: 'board' });
  if (v.manager.status !== 'stopped') {
    const busy = v.manager.status === 'working';
    person(c, hallX + 26, 30, 'manager:' + v.project, v.manager.status === 'blocked' ? waveFrame(t) : busy ? workFrame(t) : 'stand', { mayor: true, status: v.manager.status });
    if (busy) drawAction(c, hallX + 26, 30, v.manager.action, t);
    tap(hallX + 26, 30, 8, 12, { kind: 'hall' });
  }
  // The manager's sub-agents (a modernize coordinator's specialists) work in the plaza.
  (v.manager.subagents || []).slice(0, 4).forEach((sa, k) => {
    const bob = !state.reduce && blink(t + k * 170, 220) ? -1 : 0;
    const seed = `manager:${v.project}/${sa.description || k}`, px = hallX - 2 + k * 10 + (state.reduce ? 0 : wander(sa.action, t, seed));
    person(c, px, 39 + bob, seed, 'stand', { accent: subHat(sa), sub: true, status: sa.status });
    tap(px, 38, 8, 13, { kind: 'sub', house: -1, sa, id: sa.description || k });
    drawAction(c, px, 39 + bob, sa.action, t + k * 131);
  });
} });
/* The backlog's crates at the docks, roped when they wait on another issue. */
registerLayer({ z: 60, draw(c, { v, bound }) {
  v.backlog.slice(0, 40).forEach((cr, i) => {
    if (bound.has(cr.number)) return;
    const s = slotPos(i); state.T.crate(c, s.x, s.y, cr.kind, cr.priority); if ((cr.waits_on || []).length) roped(c, s.x, s.y); tap(s.x, s.y, 5, 5, { kind: 'crate', n: cr.number, at: 'backlog' });
  });
} });
/* A house per worktree: its agent at the door, its crew and sub-agents, the
 * crate being worked on. Status bubbles are drawn last, over everything. */
registerLayer({ z: 70, draw(c, scene, t) {
  const { v, lay, at, bound } = scene, { hallX, lots } = lay;
  const bubbles = scene.bubbles = [];
  if (['blocked', 'done', 'unknown', 'stalled'].includes(v.manager.status)) bubbles.push([hallX + 26, 18, v.manager.status]);
  v.houses.forEach((h, i) => {
    const lot = lots[i]; if (!lot) return;
    const st = h.status, tt = t + i * 311;
    shadow(c, lot.x + 2, lot.y + 16, 16, 2); shadow(c, lot.x + 16, lot.y + 4, 2, 12);
    tap(lot.x, lot.y - 4, 16, 25, { kind: 'house', i });
    if (st === 'building') state.T.scaffold(c, lot.x, lot.y, tt);
    else (state.T.B[h.kind] || state.T.B.workshop)(c, lot.x, lot.y, st, state.reduce ? 0 : tt);
    pennant(c, lot.x, lot.y, h.priority, tt);
    if (st === 'stalled') cobwebs(c, lot.x, lot.y);
    if (st === 'unknown') R(c, lot.x - 1, lot.y - 6, 18, 23, 'rgba(32,34,60,.6)');
    const away = state.porters.some(p => p.seed === h.id && t >= p.t0 && at.get(p));
    if (!away) {
      if (st === 'idle') person(c, lot.x + 4, lot.y + 9, h.id, 'stand', { status: st });
      if (st === 'done') person(c, lot.x + 4, lot.y + 8, h.id, 'stand', { status: st });
      if (st === 'blocked') person(c, lot.x + 4, lot.y + 10, h.id, waveFrame(t + i * 90), { status: st });
      // A working agent is one busy figure at its door, acting out what it is doing.
      if (st === 'working') { person(c, lot.x + 4, lot.y + 8, h.id, workFrame(tt), { status: st }); drawAction(c, lot.x + 4, lot.y + 8, h.action, tt); }
      // Robots show every status, so a stalled one sits slumped at its door.
      const slumped = state.T.robots && st === 'stalled';
      if (slumped) person(c, lot.x + 4, lot.y + 9, h.id, 'stand', { status: st });
      if (['idle', 'done', 'blocked', 'working'].includes(st) || slumped) tap(lot.x + 4, lot.y + 8, 8, 12, { kind: 'person', i, who: h.crew[0] });
    }
    if (h.crew.length > 1 && st !== 'working') { person(c, lot.x + 12, lot.y + 11, h.id + '#crew'); tap(lot.x + 12, lot.y + 11, 8, 12, { kind: 'person', i, who: h.crew[1] }); }
    if (h.crew.length > 1 && st === 'working' && !state.reduce) puffs(c, lot.x + 3, lot.y - 3, tt + 700, ...state.T.steam);
    // Sub-agents keep the house busy: one figure each out front (SUB_SLOTS),
    // and a "+N" badge for any that don't fit.
    const subs = h.subagents || [];
    if (subs.length && !state.reduce) puffs(c, lot.x + 3, lot.y - 3, tt + 400, ...state.T.steam);
    // Anyone waiting on a person is shown first, waving.
    [...subs].sort((a, b) => (b.status === 'blocked') - (a.status === 'blocked')).slice(0, SUB_SLOTS.length).forEach((sa, k) => {
      const bob = !state.reduce && blink(t + k * 190, 210) ? -1 : 0;
      const waiting = sa.status === 'blocked', seed = `${h.id}/${sa.description || k}`, [sx, sy] = SUB_SLOTS[k];
      const px = lot.x + sx + (state.reduce ? 0 : wander(sa.action, t, seed)), py = lot.y + sy + bob;
      person(c, px, waiting ? lot.y + sy : py, seed, waiting ? waveFrame(t + k * 110) : 'stand', { accent: subHat(sa), sub: true, status: sa.status });
      tap(px, lot.y + sy - 1, 8, 13, { kind: 'sub', house: i, sa, id: sa.description || k });
      if (!waiting) drawAction(c, px, py, sa.action, t + k * 97);
    });
    // The issue's crate at the door while it is worked on: hammered, sawn, sparking.
    if (st === 'working' && h.issue && !away && !bound.has(h.issue)) {
      state.T.crate(c, lot.x + 11, lot.y + 16, h.kind, h.priority);
      tap(lot.x + 11, lot.y + 16, 5, 5, { kind: 'crate', n: h.issue, at: 'house', i });
      if (!state.reduce && !QUIET_ACTIONS.has(h.action) && blink(tt, 150)) {
        const k = Math.floor(tt / 150) % 3;
        R(c, lot.x + 12 + k, lot.y + 14 - (k % 2), 1, 1, P.gold); R(c, lot.x + 15 - k, lot.y + 15, 1, 1, P.white);
      }
    }
    if (st === 'working' && h.action) actionSign(c, lot.x + 4, lot.y - 4, h.action);
    if (subs.length > SUB_SLOTS.length) { badge(c, lot.x + 12, lot.y + 4, subs.length - SUB_SLOTS.length); tap(lot.x + 11, lot.y + 3, 12, 7, { kind: 'house', i }); }
    if (ICONS[st]) bubbles.push([lot.x + 4, lot.y - (st === 'blocked' ? 1 : 3) + (st === 'blocked' && blink(t, 200) ? -1 : 0), st]);
  });
} });
/* Trips: whoever is walking, with the crate carried overhead, or still
 * waiting where it will be picked up. */
registerLayer({ z: 120, draw(c, { at }, t) {
  state.porters = state.porters.filter(p => {
    const a = at.get(p);
    if (!a) return false;
    if (!a.fetched && p.issue && !p.crew) {
      const [px, py] = pickup(p);
      state.T.crate(c, px - 2, py - 6, p.kind, p.pr); tap(px - 2, py - 6, 5, 5, { kind: 'crate', n: p.issue, at: 'transit', to: p.to });
    }
    if (a.waiting) return true;
    person(c, a.x - 4, a.y - 11, p.seed, a.carry ? (blink(t, 140) ? 'carrystep' : 'carry') : 'stand');
    tap(a.x - 4, a.y - 11, 8, 12, { kind: 'porter', seed: p.seed, n: p.issue, to: p.to, carrying: a.carry });
    if (a.carry) { state.T.crate(c, a.x - 2, a.y - 16, p.kind, p.pr, p.sealed); tap(a.x - 2, a.y - 16, 5, 5, { kind: 'crate', n: p.issue, at: 'transit', to: p.to }); }
    return true;
  });
} });
registerLayer({ z: 130, draw(c, { bubbles }) {
  bubbles.forEach(([x, y, s]) => bubble(c, x, y, s));
} });

/* ---------- Dialogs ---------- */
/* "12:05 · 14m ago" from a house's `since`. */
function sinceWords(since) {
  const t = Date.parse(since || '');
  if (!isFinite(t)) return '';
  const m = Math.max(0, Math.round((Date.now() - t) / 60000));
  const ago = m < 1 ? 'just now' : m < 60 ? `${m}m` : `${Math.floor(m / 60)}h ${m % 60}m`;
  return `${new Date(t).toTimeString().slice(0, 5)} (${ago})`;
}
/* " · waits on #361, #362" for a backlog issue with open dependencies. */
function waitsWords(v, n) {
  const w = (v.backlog.find(c => c.number === n) || {}).waits_on || [];
  return w.length ? ` · waits on ${w.map(d => '#' + d).join(', ')}` : '';
}
function crateWhere(o, v, theme) {
  const stops = v.pipeline.stops;
  if (o.at === 'backlog') return `waiting at the ${theme.labels.docks.toLowerCase()}${waitsWords(v, o.n)}`;
  if (o.at === 'quay') return `landed on ${v.pipeline.integration || 'the integration branch'}, at the ${theme.labels.quay.toLowerCase()}`;
  if (o.at === 'ship') return `on the ${theme.labels.ship.toLowerCase()}, waiting for the next release`;
  if (o.at === 'env') return `runs in ${stops[o.i] ? stops[o.i].name : 'an environment'}`;
  if (o.at === 'voyage') return `on its way to ${o.to}`;
  if (o.at === 'stop') return o.i === stops.length - 1 ? `shipped to ${stops[o.i].name}` : `at ${stops[o.i].name}`;
  if (o.at === 'house') { const h = v.houses[o.i]; return `being worked on at ${h ? houseName(v, h) : 'its house'}`; }
  if (o.at === 'transit') return o.to === 'away' ? 'closed without landing: carried away' : `on its way to the ${tripPlace(o.to, theme)}`;
  return '';
}
/* Where a trip takes its crate, in the theme's words. */
function tripPlace(to, theme) {
  return { house: theme.labels.houses.toLowerCase().replace(/s$/, ''), backlog: theme.labels.docks.toLowerCase(), board: theme.labels.board.toLowerCase(),
    quay: theme.labels.quay.toLowerCase(), ship: theme.labels.ship.toLowerCase() }[to] || 'next place';
}
function subWords(sa, v) {
  return `${sa.agent_type}${sa.description ? ' · ' + sa.description : ''}` +
    (sa.status === 'blocked' ? ' · waiting on you' : sa.action ? ` · ${sa.action}` : '');
}
const isRuntime = v => v.plane === 'runtime';
const docks = (o, sc, theme) => ({ title: theme.labels.docks, line: isRuntime(sc.v) ? 'Waiting work' : 'The backlog, highest priority first', items: sc.v.backlog });
registerHit({ kind: 'river', key: false, dialog: docks });
registerHit({ kind: 'docks', dialog: docks });
registerHit({ kind: 'board', dialog: (o, sc, theme) =>
  ({ title: theme.labels.board, line: isRuntime(sc.v) ? 'Failed in the last 24 hours' : 'Blocked issues and proposals waiting on you', items: sc.v.board }) });
registerHit({ kind: 'hall', dialog(o, sc, theme) {
  const v = sc.v, runtime = isRuntime(v);
  const m = v.manager, s = sessionOf(m), swarm = s && s.url.replace(/\/agent\/[^/]+$/, '');
  return { title: `${theme.labels.hall} · manager`, line: m.issue ? `#${m.issue} ${m.issue_title || ''}` : (m.issue_title || 'The manager of this swarm'),
    meta: [`Status: ${m.status}`, `Crew: ${m.crew.join(', ') || '—'}`, ...(m.subagents || []).map(sa => `${runtime ? 'Worker' : 'Sub-agent'}: ${subWords(sa, v)}`), m.action ? `Now: ${m.action}` : null],
    links: [...(m.links || []), link('Swarm view', swarm)], house: m };
} });
registerHit({ kind: 'house', dialog(o, sc) {
  const v = sc.v, runtime = isRuntime(v), work = workOf(v);
  const h = v.houses[o.i]; if (!h) return null;
  const hasIssue = h.issue && (h.links || []).some(l => /\/issues\//.test(l.url));
  const issue = h.issue && !hasIssue ? link(issueLabel(h.issue), issueOf(v, h.issue).url) : null;
  // Named by worktree and branch ("wt-1 · feature/issue-7"), not by building.
  const repo = repoUrl(v), br = v.pipeline.integration, since = sinceWords(h.since);
  const compare = !runtime && repo && br && h.branch && h.branch !== br ? link(`Compare ${h.branch} with ${br}`, `${repo}/compare/${encodeURIComponent(br)}...${encodeURIComponent(h.branch)}`) : null;
  return { title: `${houseName(v, h)} · ${h.branch || h.kind}`, line: h.issue ? `#${h.issue} ${h.issue_title || ''}` : (h.issue_title || `No ${work} claimed`),
    meta: [`${runtime ? 'Workstream' : 'Worktree'}: ${h.id}`, h.branch ? `Branch: ${h.branch}` : null, `Agent: ${h.role}`,
      `Status: ${h.status}${h.action && h.action !== h.status ? ' · ' + h.action : ''}${h.priority ? ' · ' + h.priority : ''}`,
      since ? `${h.status.charAt(0).toUpperCase() + h.status.slice(1)} since ${since}` : null,
      `Crew: ${h.crew.join(', ') || '—'}`, ...(h.subagents || []).map(sa => `${runtime ? 'Agent' : 'Sub-agent'}: ${subWords(sa, v)}`)],
    links: [...(h.links || []), issue, compare], house: h };
} });
registerHit({ kind: 'person', dialog(o, sc) {
  const v = sc.v, h = v.houses[o.i]; if (!h) return null;
  return { title: `${o.who || h.role} · agent`, line: h.issue ? `Working on #${h.issue} ${h.issue_title || ''}` : `Status: ${h.status}`,
    meta: [`Lives in ${houseName(v, h)}`, `Status: ${h.status}`, h.action ? `Now: ${h.action}` : null],
    links: [sessionOf(h), h.issue ? link(issueLabel(h.issue), issueOf(v, h.issue).url) : null] };
} });
registerHit({ kind: 'sub', dialog(o, sc) {
  const v = sc.v, runtime = isRuntime(v);
  const h = o.house < 0 ? v.manager : v.houses[o.house], sa = o.sa; if (!h) return null;
  return { title: `${sa.agent_type} · ${runtime ? (o.house < 0 ? 'worker' : 'agent') : 'sub-agent'}`, line: sa.description || '',
    meta: [`Working for ${o.house < 0 ? 'the manager' : houseName(v, h)}`, sa.status ? `Status: ${sa.status}` : null, sa.action ? `Now: ${sa.action}` : null],
    links: [sessionOf(h) && { label: 'Parent agent session', url: sessionOf(h).url }] };
} });
registerHit({ kind: 'porter', key: false, dialog(o, sc, theme) {
  const v = sc.v;
  const h = v.houses.find(h => h.id === o.seed), n = o.n || (h && h.issue);
  const it = n ? issueOf(v, n) : {};
  return { title: `${h ? h.role : 'Porter'} · carrying`, line: n ? `#${n} ${it.title}` : '',
    meta: [!o.carrying && o.to === 'house' ? `Going to the ${theme.labels.docks.toLowerCase()} to fetch its issue`
      : !o.carrying ? 'Walking home'
      : o.to === 'ship' ? `Loading the finished work onto the ${theme.labels.ship.toLowerCase()}`
      : o.to === 'quay' ? `Taking the finished work to the ${theme.labels.quay.toLowerCase()}`
      : o.to === 'house' ? 'Carrying a claimed issue from the backlog to its house'
      : o.to === 'board' ? `Pinning a blocked issue to the ${theme.labels.board.toLowerCase()}`
      : o.to === 'backlog' ? `Bringing an issue to the ${theme.labels.docks.toLowerCase()}`
      : 'Carrying off an issue closed without landing'],
    links: [h && sessionOf(h), n ? link(issueLabel(n), it.url) : null] };
} });
registerHit({ kind: 'supply', key: false, dialog(o, sc, theme) {
  const v = sc.v, work = workOf(v);
  return { title: `Supply ${{ Ship: 'boat', Rocket: 'truck', Shuttle: 'shuttle' }[theme.labels.ship] || 'boat'}`, line: `Bringing ${o.n} new ${o.n === 1 ? work : work + 's'} to the ${theme.labels.docks.toLowerCase()}`,
    items: issueItems(v, o.ns || []), links: [] };
} });
registerHit({ kind: 'crate', dialog(o, sc, theme) {
  const v = sc.v, it = issueOf(v, o.n);
  return { title: `#${o.n} ${it.title}`.trim(), line: crateWhere(o, v, theme),
    meta: [it.priority ? `Priority: ${it.priority}` : null], links: [link(issueLabel(o.n), it.url)] };
} });

/* ---------- Labels ----------
 * The docks, the hall, the board (under it, not over it or the trees beside
 * it, and kept clear of it) and each house's worktree or workstream. */
registerLabels((sc, theme) => {
  const v = sc.v, lay = sc.L, runtime = isRuntime(v), out = [];
  (sc.hits || []).filter(h => h.o.kind === 'board').forEach(h => out.push({ avoid: h }));
  const add = (x, y, anchor, text, sub, cls, width) => out.push({ x, y, anchor, text, sub, cls, width });
  add(34, 51, 'top', theme.labels.docks, runtime ? 'waiting' : `backlog · ${v.backlog.length}`);
  add(lay.hallX + 16, 43, 'top', theme.labels.hall, 'manager');
  add(lay.hallX + 51, 45, 'top', theme.labels.board, runtime ? 'failed' : `blocked · ${v.board.length}`);
  // A house label may be no wider than its lot, so neighbours never overlap.
  v.houses.forEach((h, i) => {
    const lot = lay.lots[i]; if (!lot) return;
    add(lot.x + 8, lot.y + 17, 'top', houseName(v, h), null, 'house ' + h.status, LW - 1);
  });
  return out;
}, 10);

/* ---------- Counters ---------- */
registerCounter({ order: 10, render: v => [state.T.labels.docks, 'backlog', v.backlog.length] });
registerCounter({ order: 20, render: v => [state.T.labels.board, 'blocked · proposals', v.board.length] });
registerCounter({ order: 30, render: v => [state.T.labels.houses, `${v.houses.filter(h => h.status === 'working').length} of ${v.houses.length} working`, v.houses.filter(h => h.issue).length] });
registerCounter({ order: 40, render: v => ['Sub-agents', 'running in sessions', [v.manager, ...v.houses].reduce((n, h) => n + (h.subagents || []).length, 0)] });
/* Runtime villages (Temporal, AgentEx, ...) count workstreams and agents, not issues and pushes. */
const count = (v, st) => v.houses.filter(h => h.status === st).length;
registerCounter({ plane: 'runtime', order: 10, render: v => [state.T.labels.houses, 'top-level workstreams', v.houses.length] });
registerCounter({ plane: 'runtime', order: 20, render: v => ['Working', 'workstreams busy', count(v, 'working')] });
registerCounter({ plane: 'runtime', order: 30, render: v => ['Waiting on you', 'an agent asked a person', count(v, 'blocked')] });
registerCounter({ plane: 'runtime', order: 40, render: v => ['Retrying', 'activity failing', count(v, 'stalled')] });
registerCounter({ plane: 'runtime', order: 50, render: v => ['Agents', 'working for workstreams', v.houses.reduce((n, h) => n + (h.subagents || []).length, 0)] });
registerCounter({ plane: 'runtime', order: 60, render: v => ['Workers', v.manager.status === 'stalled' ? 'a task queue has none' : 'polling task queues', (v.manager.subagents || []).length] });
registerCounter({ plane: 'runtime', order: 70, render: v => [state.T.labels.board, 'failed in 24h', v.board.filter(n => n.label === 'failed').length] });
