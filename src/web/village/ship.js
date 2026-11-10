/* ---------- The ship ----------
 * The canal and what moves down it: the quay where landed work waits, a lock
 * per environment, and the harbour with its ship.
 *
 * Work waiting for the next release is loaded onto the ship in the harbour
 * (a rocket on the launch pad): the issues at the stop before the last, or
 * at the quay when there is only one stop. When the last stop's deployment
 * changes, the ship sails off east with what shipped, the fireworks fire,
 * and an empty ship comes in. The change is read from the last stop's sha
 * (or the released commit), never its issues, so a release of the same
 * issues still sails; a page loaded after a release just shows the new ship. */
import { state } from './state.js';
import { P, R, STOPCOL, blink } from './palette.js';
import { puffs } from './sprites/houses.js';
import { badge, badgeSize } from './scenery.js';
import { registerHit, registerLabels, registerLayer, tap } from './registry.js';
import { porterPos, shownScenes } from './layout.js';
import { toast } from './panels.js';
import { canalNames } from './envlabels.js';
import { isSha, issueItems, link, repoUrl } from './dialogs.js';

export const SAIL = 4200, ARRIVE = 3200, GATE_OPEN = 2400, SHIP_CAP = 6;
/* Crates drawn at the quay; the rest are a "+N". */
export const QUAY_CAP = 6;
/* Where the k-th crate at the quay rests: three to a row, stacked back from the edge. */
export const quaySpot = (lay, k) => [lay.roadX + 6 + (k % 3) * 6, lay.quayY - 3 - Math.floor(k / 3) * 6];
export const cargoStop = v => v.pipeline.stops.length - 2; // -1: the quay
export function cargoOf(v) {
  const i = cargoStop(v);
  return i < -1 ? [] : i < 0 ? v.pipeline.quay : v.pipeline.stops[i].issues;
}
const lastStop = v => v.pipeline.stops[v.pipeline.stops.length - 1] || null;
/* What the last stop runs, with what says so ("deployment:<sha>"). A poll
 * where GitHub didn't answer falls back to the branch or to nothing; that
 * is a different source, not a release, so it never sails the ship. */
export function releaseOf(v) {
  const last = lastStop(v), id = last && (last.sha || last.tag || v.pipeline.release);
  return id ? `${last.source || 'branch'}:${id}` : null;
}
/* Where the ship is: moored, sailing out with the last release, or coming in. */
function shipAt(sc, t, lay) {
  const vo = sc.voyage, since = vo ? t - vo.t0 : Infinity, flies = !!state.T.flies, out = lay.W - lay.castleX + 4;
  if (since < SAIL) { const p = (since / SAIL) ** 2; return { leaving: true, ox: flies ? 0 : p * out, oy: flies ? -p * (lay.canalY + 64) : 0 }; }
  if (since < SAIL + ARRIVE) { const p = 1 - (since - SAIL) / ARRIVE; return { leaving: false, ox: out * p * p, oy: 0 }; }
  return { leaving: false, ox: 0, oy: 0 };
}
export const holdPos = (lay, k, ox = 0, oy = 0) => [Math.round(lay.castleX + state.T.hold[k][0] + ox), Math.round(lay.canalY + state.T.hold[k][1] + oy)];
/* A crate's resting point at the quay (-1) or in stop i's basin. */
export const placePoint = (i, lay) => (i < 0 ? [lay.roadX + 9, lay.quayY + 2] : [lay.lockStart + i * 34 + 10, lay.canalY + 22]);
/* Where an issue is on the canal: -1 the quay, i stop i, null elsewhere. */
export function placeOf(n, v) {
  if (v.pipeline.quay.includes(n)) return -1;
  const i = v.pipeline.stops.findIndex(s => s.issues.includes(n));
  return i >= 0 ? i : null;
}
/* A porter's walk from a point along the canal bank to the ship's side. */
export function toShip([x, y], lay) {
  const bank = lay.canalY + 26, sx = lay.castleX + state.T.hold[0][0] + 4;
  return [[x, y], [x, bank], [sx, bank], [sx, lay.canalY + 18]];
}
export function sail(sc, v, t) {
  const old = sc.v, last = lastStop(v), before = new Set((lastStop(old) || { issues: [] }).issues);
  const shipped = last ? last.issues.filter(n => !before.has(n)) : [];
  sc.fireworks = t;
  sc.voyage = state.reduce ? null : { t0: t, cargo: shipped };
  if (shownScenes().some(([x]) => x === sc)) toast(`${v.project}: shipped ${releaseName(v, last) || 'a release'}${shipped.length ? ` · ${shipped.length} ${shipped.length === 1 ? 'issue' : 'issues'}` : ''}`);
}
/* A release's name: its tag, else its commit. */
function releaseName(v, s) {
  if (!s) return null;
  const sha = s.sha || v.pipeline.release;
  return s.tag || (sha ? String(sha).slice(0, 7) : null);
}

/* ---------- Drawing ---------- */
const NONE = new Set();
/* The canal, and the quay's landing beside the road. */
registerLayer({ z: 30, draw(c, { v, lay }, t) {
  const { W, roadX, quayY, canalY } = lay;
  state.T.channel(c, roadX - 16, canalY, W - roadX + 16, 14, t, 0.6); tap(roadX - 16, canalY, W - roadX + 16, 14, { kind: 'canal' });
  state.T.dock(c, roadX - 4, quayY + 1, 22); if (v.plane !== 'runtime') tap(roadX - 4, quayY - 16, 22, 24, { kind: 'quay' });
} });
/* Crates at the quay. Work carried, afloat, on a barge or on the ship is not drawn here. */
registerLayer({ z: 80, draw(c, scene) {
  const { v, lay } = scene, { roadX, quayY } = lay, hid = scene.hid || NONE;
  const carried = scene.carried = new Set([...(scene.bound || NONE), ...state.floats.map(f => f.n)]);
  const cargo = scene.cargo = cargoOf(v), onShip = scene.onShip = new Set(cargo);
  const atQuay = v.pipeline.quay.filter(n => !carried.has(n) && !hid.has(n) && !onShip.has(n));
  // Which crates are drawn here, in order, for anything later layers mark on them.
  (scene.atQuay = atQuay.slice(0, QUAY_CAP)).forEach((n, k) => {
    const [x, y] = quaySpot(lay, k);
    state.T.crate(c, x, y, 'workshop', null, true, n); tap(x, y, 5, 5, { kind: 'crate', n, at: 'quay' });
  });
  // Six crates fit; a "+N" says how many more wait, and the quay's dialog lists them all.
  if (atQuay.length > QUAY_CAP) { const more = atQuay.length - QUAY_CAP, { w, h } = badgeSize(more); badge(c, roadX + 25, quayY - 9, more); tap(roadX + 24, quayY - 11, w, h, { kind: 'quay' }); }
} });
/* A lock per stop but the last, with the crates in its basin. */
registerLayer({ z: 90, draw(c, scene, t) {
  const { v, lay } = scene, { devH: H, canalY } = lay, hid = scene.hid || NONE, onShip = scene.onShip, carried = scene.carried;
  // The sentinel's flag is grey while it still has newer work to test. With
  // no sentinel the first lock is an environment like any other.
  const gated = v.pipeline.sentinel !== 'absent';
  const flagOf = i => i === 0 && gated && v.pipeline.sentinel !== 'green' ? P.stoneD : STOPCOL[i] || P.white;
  const stops = v.pipeline.stops;
  stops.slice(0, -1).forEach((s, i) => {
    const lx = lay.lockStart + i * 34;
    const open = !state.reduce && t - (state.S.gates[i] ?? -1e9) < GATE_OPEN;
    state.T.gate(c, lx, canalY, open, flagOf(i)); tap(lx - 7, canalY - 28, 32, H - canalY + 28, { kind: 'stop', i });
    s.issues.filter(n => !hid.has(n) && !onShip.has(n) && !carried.has(n)).slice(0, 4).forEach((n, k) => {
      const x = lx + 8 + (k % 2) * 6, y = canalY + 18 + Math.floor(k / 2) * 6;
      state.T.crate(c, x, y, 'workshop', null, true, n); tap(x, y, 5, 5, { kind: 'crate', n, at: 'stop', i });
    });
  });
} });
/* Crates floating down to a lock's basin while its gate stands open. */
registerLayer({ z: 100, draw(c, scene, t) {
  state.floats = state.floats.filter(f => {
    const pos = porterPos({ t0: f.t0, speed: 0.03, pts: f.pts }, t);
    if (!pos) return false;
    state.T.crate(c, pos[0] - 2, pos[1] - 2, 'workshop', null, true, f.n);
    return true;
  });
} });
/* The harbour, and the ship in it loaded with the next release; or the last
 * one sailing off with what shipped, leaving a wake, then an empty one coming in. */
registerLayer({ z: 110, draw(c, scene, t) {
  const { v, lay } = scene, { devH: H, canalY } = lay, hid = scene.hid || NONE, carried = scene.carried, cargo = scene.cargo;
  const stops = v.pipeline.stops;
  state.T.destination(c, lay.castleX, canalY, t, state.fireworks); tap(lay.castleX - 4, canalY - 28, 46, H - canalY + 28, { kind: 'harbour' });
  if (stops.length) {
    const at = shipAt(state.S, t, lay), bob = state.T.flies || state.reduce || !blink(t, 900) ? 0 : 1, oy = at.oy + bob;
    if (at.leaving && state.T.flies) puffs(c, lay.castleX + 18, canalY - 12, t, ...state.T.steam);
    else if (at.leaving) {
      for (let x = lay.castleX + 4; x < lay.castleX + at.ox; x += 5) R(c, x, canalY + 12 + ((x >> 2) % 2), 3, 1, P.white);
      tap(lay.castleX + 4, canalY + 8, Math.max(0, at.ox - 4), 8, { kind: 'harbour' });
    }
    const load = at.leaving ? state.S.voyage.cargo : cargo.filter(n => !carried.has(n) && !hid.has(n));
    state.T.vessel(c, lay.castleX, canalY, t, at.ox, oy);
    const [hx, hy, hw, hh] = state.T.hull;
    tap(lay.castleX + hx + at.ox, canalY + hy + at.oy, hw, hh, at.leaving ? { kind: 'ship', at: 'away' } : { kind: 'ship' });
    load.slice(0, SHIP_CAP).forEach((n, k) => {
      const [x, y] = holdPos(lay, k, at.ox, oy);
      state.T.crate(c, x, y, 'workshop', null, true, n);
      if (!at.leaving) tap(x, y, 5, 5, { kind: 'crate', n, at: 'ship' });
    });
    if (load.length > SHIP_CAP) badge(c, Math.round(lay.castleX + state.T.holdBadge[0] + at.ox), Math.round(canalY + state.T.holdBadge[1] + oy), load.length - SHIP_CAP);
  }
} });

/* ---------- Where an environment's answer comes from ---------- */
export function sourceWords(s) {
  const sha = s.sha ? ' · ' + String(s.sha).slice(0, 7) : '';
  if (s.source === 'deployment') return `GitHub Deployment${sha}`;
  if (s.source === 'release') return `${s.tag ? s.tag + ' ' : ''}(GitHub Release)${sha}`;
  if (s.source === 'tag') return `tag ${s.tag || ''}${sha}`;
  if (s.source === 'url') return `its version endpoint${sha}`;
  if (s.source === 'branch') return `${s.branch ? 'branch ' + s.branch : 'its branch ref'}${sha}`;
  if (s.source === 'unknown') return 'nothing says what runs here (no Deployment, Release or branch)';
  return 'not reported';
}
function stopLinks(v, s) {
  const repo = repoUrl(v);
  if (!repo || !s) return [];
  return [
    s.source === 'deployment' ? link(`${s.name} deployments`, `${repo}/deployments/${encodeURIComponent(s.name)}`) : null,
    isSha(s.sha) ? link(`Commit ${String(s.sha).slice(0, 7)}`, `${repo}/commit/${s.sha}`) : null
  ].filter(Boolean);
}
/* The GitHub Release a stop runs: the page its deployment links, else its tag's page. */
function releaseLink(v, s, words = 'Release') {
  if (!s || !s.tag && !s.url) return null;
  const repo = repoUrl(v), url = s.url || (repo ? `${repo}/releases/tag/${encodeURIComponent(s.tag)}` : null);
  return link(`${words}${s.tag ? ' ' + s.tag : ''}`, url);
}
/* Why nothing says what a stop runs, in plain words, and where the docs are. */
function unknownWords(v, i) {
  const st = v.pipeline.stops[i], br = v.pipeline.integration || 'the integration branch';
  if (i === 0 && v.pipeline.sentinel === 'unknown')
    return `No ${st.name} ref yet, so nothing has been verified green. Run autocoder's sentinel (main-sentinel.sh), or, with no sentinel, mark the first environment "requires": "integration" in .autocoder.json.`;
  return `Nothing says what ${st.name} runs: there is no successful GitHub Deployment to it and no env/${st.name} branch. Record a deployment (tools/deploy.sh records nuc-1), or configure the environment in .autocoder.json.`;
}
function docsLink(v) {
  const repo = repoUrl(v), br = v.pipeline.integration;
  return repo && br ? link('How environments work', `${repo}/blob/${encodeURIComponent(br)}/docs/village.md#environments`) : null;
}

/* ---------- Dialogs ---------- */
export const workOf = v => (v.plane === 'runtime' ? 'workstream' : 'issue');
registerHit({ kind: 'quay', dialog(o, sc, theme) {
  const v = sc.v, stops = v.pipeline.stops;
  const repo = repoUrl(v), br = v.pipeline.integration;
  return { title: theme.labels.quay, line: `Work landed on ${br || 'the integration branch'}, not yet past ${stops[0] ? stops[0].name : 'the gate'}`,
    items: issueItems(v, v.pipeline.quay), links: [repo && br ? link(`${br} commits`, `${repo}/commits/${encodeURIComponent(br)}`) : null] };
} });
registerHit({ kind: 'stop', dialog(o, sc) {
  const v = sc.v, runtime = v.plane === 'runtime', stops = v.pipeline.stops;
  const st = stops[o.i]; if (!st) return null;
  const sentinel = o.i === 0 && !runtime && v.pipeline.sentinel !== 'absent';
  const unknown = !runtime && (st.source === 'unknown' || (sentinel && v.pipeline.sentinel === 'unknown'));
  return { title: `${st.name} · ${runtime ? 'stage' : 'environment'}`, line: sentinel ? `Sentinel: ${v.pipeline.sentinel}` : sourceWords(st),
    meta: [sentinel ? `Runs: ${sourceWords(st)}` : null, o.i === 0 && v.pipeline.sentinel === 'absent' ? `No sentinel: takes work straight from ${v.pipeline.integration || 'the integration branch'}` : null,
      unknown ? unknownWords(v, o.i) : null],
    items: issueItems(v, st.issues), links: [...stopLinks(v, st), unknown ? docsLink(v) : null] };
} });
registerHit({ kind: 'harbour', dialog(o, sc, theme) {
  const v = sc.v, runtime = v.plane === 'runtime', stops = v.pipeline.stops, last = stops[stops.length - 1];
  const rel = v.pipeline.release, repo = repoUrl(v);
  const relLink = rel && repo ? link(`Release ${isSha(rel) ? rel.slice(0, 7) : rel}`, isSha(rel) ? `${repo}/commit/${rel}` : `${repo}/releases/tag/${encodeURIComponent(rel)}`) : null;
  return { title: `${theme.labels.castle}${last ? ' · ' + last.name : ''}`, line: last ? (runtime ? 'Completed in the last 24 hours' : `What shipped to ${last.name}`) : 'Nothing ships yet',
    meta: [last && last.tag ? `Last release: ${last.tag}` : rel ? `Release: ${rel}` : null, last && !runtime ? `Runs: ${sourceWords(last)}` : null,
      last && !runtime && last.source === 'unknown' ? unknownWords(v, stops.length - 1) : null],
    items: last ? issueItems(v, last.issues) : [], links: [releaseLink(v, last), relLink, ...stopLinks(v, last), last && !runtime && last.source === 'unknown' ? docsLink(v) : null] };
} });
registerHit({ kind: 'ship', dialog(o, sc, theme) {
  const v = sc.v, work = workOf(v), stops = v.pipeline.stops, last = stops[stops.length - 1];
  if (!last) return null;
  const name = releaseName(v, last);
  if (o.at === 'away') {
    const gone = (sc.voyage || {}).cargo || [];
    return { title: `${theme.labels.ship} · ${theme.labels.sails}`, line: `Shipped ${name || 'a release'} to ${last.name}`, items: issueItems(v, gone), links: [releaseLink(v, last)] };
  }
  const cargo = cargoOf(v), n = cargo.length, repo = repoUrl(v), br = v.pipeline.integration;
  return { title: `${theme.labels.ship} · next release`, line: `Next release: ${n} ${n === 1 ? work : work + 's'}${name ? ' since ' + name : ''}`,
    meta: [`${theme.labels.sails.charAt(0).toUpperCase() + theme.labels.sails.slice(1)} when ${last.name} is next deployed`],
    items: issueItems(v, cargo),
    links: [releaseLink(v, last, 'Last release'), repo && last.tag && br ? link(`Changes since ${last.tag}`, `${repo}/compare/${encodeURIComponent(last.tag)}...${encodeURIComponent(br)}`) : null] };
} });
registerHit({ kind: 'canal', key: false, dialog(o, sc) {
  const v = sc.v, runtime = v.plane === 'runtime', work = workOf(v), stops = v.pipeline.stops;
  return { title: runtime ? 'Pipeline' : 'Canal', line: `From ${v.pipeline.integration || 'integration'} through ${stops.map(s => s.name).join(', ') || 'no stages'}`,
    meta: stops.map(s => `${s.name}: ${s.issues.length} ${s.issues.length === 1 ? work : work + 's'}`), links: [] };
} });

/* ---------- Labels ----------
 * The quay, each stop by its own name (master-green, uat, prod) as the
 * pipeline reports it, and the harbour; kept clear of the destination's
 * towers (the lighthouse, the masts downtown and in the future), which each
 * theme lists, and of the ship (or rocket, or shuttle) where it waits. */
registerLabels((sc, theme) => {
  const v = sc.v, lay = sc.L, runtime = v.plane === 'runtime', out = [];
  const at = ([dx, dy, w, h]) => ({ avoid: { x: lay.castleX + dx, y: lay.canalY + dy, w, h } });
  out.push(...theme.towers.map(at), at(theme.hull));
  const add = (x, y, anchor, text, sub) => out.push({ x, y, anchor, text, sub });
  if (!runtime) add(lay.roadX - 7, lay.quayY + 4, 'right', theme.labels.quay, `→ ${v.pipeline.integration || 'integration'}`);
  // A lock whose stop is a village in the environment row is named there, once (#440).
  const names = canalNames(v);
  names.locks.forEach((name, i) => { if (name) add(lay.lockStart + i * 34 + 3, lay.canalY - 21, 'bottom', name); });
  // Over the harbour wall, left of the lighthouse.
  add(lay.castleX + 10, lay.canalY - 22, 'bottom', theme.labels.castle, names.castle);
  return out;
}, 20);

