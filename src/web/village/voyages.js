/* ---------- Voyages ----------
 * A ship on the river between two villages of the environment row for each
 * deployment (`pipeline.voyages`, docs/environment-row.md):
 *
 *   loading  at the source's east dock, crates carried aboard;
 *   sailing  under sail on the river between the two, leaving a wake;
 *   docked   at the target's west dock, crates carried off into its warehouse;
 *   failed   turned back to the source, sail torn and a red flag, cargo aboard.
 *
 * Only Deployments say a deployment is under way. When what an environment
 * runs changes between two polls and no voyage says so (a Release, a tag, a
 * version URL or a branch), the page plays one itself: it sails from the
 * village before with what the target's cargo gained, docks and unloads.
 * Each reported voyage is timed from when the page first saw it, so a page
 * loaded later shows it where it is, never replayed from the start. */
import { state } from './state.js';
import { P, R, blink } from './palette.js';
import { badge } from './scenery.js';
import { registerHit, registerLayer, tap } from './registry.js';
import { envStops, porterPos, shownScenes } from './layout.js';
import { issueItems, isSha, link, repoUrl } from './dialogs.js';
import { toast } from './panels.js';
import { agoWords, versionOf, SHED } from './envrow.js';

export const VOYAGE_SAIL = 9000, VOYAGE_UNLOAD = 6000, PLAYED_SAIL = 5000, PLAYED_DOCK = 5000, LOAD_TRIP = 1400, VOYAGE_CAP = 6;
export const voyageKey = vo => `${vo.to}|${vo.sha}|${vo.state}`;
const shortName = vo => vo.tag || (vo.sha ? String(vo.sha).slice(0, 7) : 'a release');
const fromName = (v, vo) => (vo.from === 'dev' ? v.project : vo.from);
/* What a voyage is doing, in words: the dialog's line and the toast. */
export const VOYAGE_WORDS = {
  loading: (v, vo) => `Loading ${shortName(vo)} at ${fromName(v, vo)} for ${vo.to}`,
  sailing: (v, vo) => `Deploying ${shortName(vo)} from ${fromName(v, vo)} to ${vo.to}`,
  docked: (v, vo) => `${shortName(vo)} arrived at ${vo.to}`,
  failed: (v, vo) => `Deploying ${shortName(vo)} to ${vo.to} failed: turned back to ${fromName(v, vo)}`
};

/* Fold a new poll into the scene's voyages: when each was first seen, and a
 * voyage of the page's own for a change of version nothing reported. */
export function noteVoyages(sc, old, v, t) {
  const seen = sc.voyageSeen || (sc.voyageSeen = {}), shown = shownScenes().some(([x]) => x === sc);
  (v.pipeline.voyages || []).forEach(vo => {
    const k = voyageKey(vo);
    if (k in seen) return;
    // On a first load nothing is replayed: a ship under sail is half-way, a docked one unloaded.
    seen[k] = old ? t : vo.state === 'sailing' ? t - VOYAGE_SAIL / 2 : -1e9;
    if (old && shown && VOYAGE_WORDS[vo.state]) toast(`${v.project}: ${VOYAGE_WORDS[vo.state](v, vo)}`);
  });
  sc.played = (sc.played || []).filter(p => t - p.t0 < PLAYED_SAIL + PLAYED_DOCK);
  if (!old || state.reduce) return;
  const envs = envStops(v);
  envs.forEach(({ s }, k) => {
    const was = old.pipeline.stops.find(x => x.name === s.name), id = versionOf(s), wasId = was && versionOf(was);
    if (!id || !wasId || id === wasId || was.source !== s.source) return;
    if ((v.pipeline.voyages || []).some(vo => vo.to === s.name && (vo.sha === s.sha || (vo.tag && vo.tag === s.tag)))) return;
    const before = new Set(was.cargo || []);
    sc.played.push({ from: k ? envs[k - 1].s.name : 'dev', to: s.name, state: 'sailing', sha: s.sha || '', tag: s.tag, played: true,
      issues: (s.cargo || []).filter(n => !before.has(n)), t0: t });
  });
}

/* The river a voyage takes along the row: from the source's east dock (from
 * the dev village, the row river's west end) to the target's west dock, as
 * the vessel's x at each end (the hull clear of both docks). */
export function reach(lay, vo) {
  const src = vo.from === 'dev' ? null : lay.envs.find(e => e.s.name === vo.from), dst = lay.envs.find(e => e.s.name === vo.to);
  if (!dst || (vo.from !== 'dev' && !src)) return null;
  const [h0, , hw] = state.T.hull, a = src ? src.x + src.w : 0, b = dst.x;
  return { a, b, dst, start: a + 2 - h0, end: Math.max(a + 2, b - hw - 2) - h0 };
}
/* Where a voyage's ship is at t, and what its crew carries. */
export function voyagePose(vo, lay, seen, t) {
  const r = reach(lay, vo);
  if (!r) return null;
  const cy = lay.envCy, still = state.reduce, since = t - (vo.played ? vo.t0 : seen);
  let state_ = vo.state, p = 0;
  if (vo.played) { state_ = since < PLAYED_SAIL ? 'sailing' : 'docked'; p = Math.min(1, since / PLAYED_SAIL); }
  else if (state_ === 'sailing') p = still ? 0.5 : Math.max(0, Math.min(0.85, since / VOYAGE_SAIL));
  const ease = p * p * (3 - 2 * p), flies = !!state.T.flies;
  const pose = { r, state: state_, x: r.start, oy: 0, back: false, aboard: true, carry: null };
  if (state_ === 'sailing') { pose.x = r.start + (r.end - r.start) * ease; pose.oy = flies ? -Math.round(Math.sin(Math.PI * p) * 20) : 0; }
  if (state_ === 'failed') pose.back = true;
  if (state_ === 'docked') {
    pose.x = r.end;
    const unload = vo.played ? since - PLAYED_SAIL : since;
    pose.aboard = !still && unload < (vo.played ? PLAYED_DOCK : VOYAGE_UNLOAD);
    // Crates go from the ship along the dock into the warehouse bay.
    if (pose.aboard) pose.carry = { pts: [[r.b - 4, cy - 4], [r.b + SHED.dx + 6, cy - 4], [r.b + SHED.dx + 6, cy - SHED.up + 16]], p: (unload % LOAD_TRIP) / LOAD_TRIP };
  }
  // Crates go from the source's dock aboard.
  if (state_ === 'loading' && !still) pose.carry = { pts: [[r.a - 14, cy - 4], [r.a + 4, cy - 4]], p: ((t - seen) % LOAD_TRIP) / LOAD_TRIP };
  return pose;
}

/* ---------- Drawing ---------- */
function voyageVessel(c, x, cy, t, oy, back) {
  if (!back) { state.T.vessel(c, x, cy, t, 0, oy); return; }
  const [hx, , hw] = state.T.hull, mid = x + hx + hw / 2;
  c.save(); c.translate(2 * mid, 0); c.scale(-1, 1); state.T.vessel(c, x, cy, t, 0, oy); c.restore();
}
function drawVoyage(c, v, lay, vo, seen, t) {
  const pose = voyagePose(vo, lay, seen, t);
  if (!pose) return;
  const cy = lay.envCy, [hx, hy, hw, hh] = state.T.hull, bob = state.T.flies || state.reduce || !blink(t, 900) ? 0 : 1, oy = pose.oy + bob;
  if (pose.state === 'sailing' && !state.T.flies) for (let x = Math.round(pose.x + hx - 2); x > pose.r.start + hx; x -= 5) R(c, x, cy + 12 + ((x >> 2) % 2), 3, 1, P.white);
  voyageVessel(c, pose.x, cy, t, oy, pose.back);
  const left = Math.round(pose.x + hx), top = Math.round(cy + hy + pose.oy);
  if (pose.state === 'failed') {
    // Sail torn, a red flag up.
    R(c, left + (hw >> 1) - 1, top + 9, 2, 3, P.o); R(c, left + (hw >> 1) + 2, top + 13, 1, 2, P.o);
    R(c, left + 2, top - 4, 1, 7, P.woodD); R(c, left + 3, top - 4 + (blink(t, 500) ? 1 : 0), 5, 3, P.p0);
  }
  const id = `${vo.to}:${vo.sha}`;
  tap(left, top, hw, hh, { kind: 'voyage', id, at: pose.state, vo });
  const load = pose.aboard ? vo.issues || [] : [];
  load.slice(0, VOYAGE_CAP).forEach((n, k) => {
    let x = Math.round(pose.x + state.T.hold[k][0]);
    if (pose.back) x = 2 * (left + hw / 2) - x - 5;
    const y = Math.round(cy + state.T.hold[k][1] + oy);
    state.T.crate(c, x, y, 'workshop', null, true); tap(x, y, 5, 5, { kind: 'crate', n, at: 'voyage', to: vo.to });
  });
  if (load.length > VOYAGE_CAP) badge(c, Math.round(pose.x + state.T.holdBadge[0]), Math.round(cy + state.T.holdBadge[1] + oy), load.length - VOYAGE_CAP);
  if (pose.carry && load.length) {
    const len = pose.carry.pts.slice(1).reduce((n, q, k) => n + Math.hypot(q[0] - pose.carry.pts[k][0], q[1] - pose.carry.pts[k][1]), 0);
    const at = porterPos({ t0: 0, speed: 1, pts: pose.carry.pts }, pose.carry.p * len);
    if (at) state.T.crate(c, at[0] - 2, at[1] - 3, 'workshop', null, true);
  }
}
/* Finished voyages first, so one under way is drawn over a docked one. */
const VOYAGE_ORDER = { docked: 0, failed: 1, loading: 2, sailing: 3 };
registerLayer({ z: 115, draw(c, scene, t) {
  const { v, lay } = scene, sc = state.S;
  if (!lay.envs || !lay.envs.length) return;
  const seen = (sc && sc.voyageSeen) || {};
  const all = [...(v.pipeline.voyages || []), ...((sc && sc.played) || [])].sort((a, b) => (VOYAGE_ORDER[a.state] ?? 0) - (VOYAGE_ORDER[b.state] ?? 0));
  all.forEach(vo => drawVoyage(c, v, lay, vo, seen[voyageKey(vo)] ?? -1e9, t));
} });

/* ---------- Dialog ---------- */
registerHit({ kind: 'voyage', dialog(o, sc, theme) {
  const v = sc.v, vo = o.vo, repo = repoUrl(v), words = VOYAGE_WORDS[o.at] || VOYAGE_WORDS[vo.state];
  const started = agoWords(vo.started_at), finished = agoWords(vo.finished_at);
  return { title: `${theme.labels.ship} · ${fromName(v, vo)} → ${vo.to}`, line: words ? words(v, vo) : `${vo.state} ${shortName(vo)}`,
    meta: [started ? `Started ${started}` : null, finished ? `Finished ${finished}` : null,
      vo.played ? `Seen when what ${vo.to} runs changed; nothing reported the deployment` : null,
      vo.issues_more ? 'It carries more than the history the village reads' : null,
      `New to ${vo.to}: ${(vo.issues || []).length} ${(vo.issues || []).length === 1 ? 'issue' : 'issues'}`],
    items: issueItems(v, vo.issues || []),
    links: [link('Deployment', vo.url), repo && isSha(vo.sha) ? link(`Commit ${String(vo.sha).slice(0, 7)}`, `${repo}/commit/${vo.sha}`) : null] };
} });
