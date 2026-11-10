//! The canal, the ship, and the environment row with its voyages.

use super::*;

#[test]
fn village_html_loads_the_ship_and_sails_it_on_a_release() {
    let body = &village_source();
    // Loading: the stop before the harbour (or the quay) is the ship's cargo, capped with a count.
    assert!(body.contains("const cargoStop = v => v.pipeline.stops.length - 2; // -1: the quay"));
    assert!(body.contains("return i < -1 ? [] : i < 0 ? v.pipeline.quay : v.pipeline.stops[i].issues;"));
    assert!(body.contains("const SAIL = 4200, ARRIVE = 3200, GATE_OPEN = 2400, SHIP_CAP = 6;"));
    assert!(body.contains("if (load.length > SHIP_CAP) badge("));
    assert!(body.contains("tap(x, y, 5, 5, { kind: 'crate', n, at: 'ship' });"));
    // Porters carry newly arrived work to the ship; lock gates open as crates pass.
    assert!(body.contains("if (i === cs) enqueue(sc, t, trip('porter:' + n, n, null, 'ship', [{ pts: toShip(from, lay), carry: true }], { sealed: true, speed: 0.04 }), 300);"));
    assert!(body.contains("for (let g = was + 1; g <= i; g++) sc.gates[g] = t;"));
    assert!(body.contains("const open = !state.reduce && t - (state.S.gates[i] ?? -1e9) < GATE_OPEN;"));
    // Sailing: from the last stop's sha, once per change, never on first load.
    assert!(body.contains("const last = lastStop(v), id = last && (last.sha || last.tag || v.pipeline.release);"));
    assert!(body.contains("return id ? `${last.source || 'branch'}:${id}` : null;"));
    assert!(body.contains("if (rel) { if (sc.v && sameSource && rel !== was) sail(sc, v, t); sc.lastRelease = rel; }"));
    assert!(!body.contains("[stops.length - 1].issues.filter(n => !hid.has(n)).slice(0, 6)"), "shipped crates no longer pile up in the harbour");
    // Reduced motion: no voyage, just the toast.
    assert!(body.contains("sc.voyage = state.reduce ? null : { t0: t, cargo: shipped };"));
    assert!(body.contains("toast(`${v.project}: shipped ${releaseName(v, last) || 'a release'}"));
    assert!(body.contains(r#"<div class="win toast" id="toast" hidden role="status"></div>"#));
    // Both themes have a vessel and a hold; the rocket lifts off, the ship sails east.
    assert!(body.contains("vessel: ship,"));
    assert_eq!(body.matches("hold: [[").count(), 3, "medieval, modern and future each have a hold");
    assert_eq!(body.matches("vessel(c, x, cy, t, ox, oy) {").count(), 2, "the rocket and the shuttle");
    assert_eq!(body.matches(" flies: true,").count(), 2, "the rocket and the shuttle lift off; the ship sails");
    assert!(body.contains("oy: flies ? -p * (lay.canalY + 64) : 0"));
    // Clicking: the ship's cargo and next release, a departed ship, the harbour's last release.
    assert!(body.contains("line: `Next release: ${n} ${n === 1 ? work : work + 's'}${name ? ' since ' + name : ''}`"));
    assert!(body.contains("line: `Shipped ${name || 'a release'} to ${last.name}`"));
    assert!(body.contains("`${repo}/compare/${encodeURIComponent(last.tag)}...${encodeURIComponent(br)}`"));
    assert!(body.contains("const repo = repoUrl(v), url = s.url || (repo ? `${repo}/releases/tag/${encodeURIComponent(s.tag)}` : null);"));
    // Worlds: the release cart sets off as the ship leaves the map.
    assert!(body.contains("const sailed = Ds.voyage ? Ds.voyage.t0 + SAIL : 0;"));
}

/// The environment row (#385, docs/environment-row.md): a village per
/// environment under the dev village on a river of its own (#445),
/// production right-most, with ships between them for each voyage.
#[test]
fn village_html_draws_the_environment_row() {
    let body = &village_source();
    // The row: every environment of a development village, the green gate kept as a lock.
    assert!(body.contains("return v.pipeline.stops.map((s, i) => ({ s, i })).filter(({ s }) => inRow(v, s));"));
    assert!(body.contains("export const inRow = (v, s) => !!v && v.plane !== 'runtime' && !!s && s.kind !== 'gate';"));
    assert!(body.contains("const envs = stops.map(({ s, i }, k, all) => {"));
    assert!(body.contains("rank: k === all.length - 1 ? 'capital' : 'town', platform };"), "production is the right-most village, the capital");
    assert!(body.contains("ex += w + ENV_GAP;"));
    // Under the dev village (#445): the row's own river along the bottom, each village just over it.
    assert!(body.contains("const envCy = rowTop + Math.max(ENV_ABOVE, hosts ? (slot.h || 0) + OVER : 0);"));
    assert!(body.contains("const e = { s, i, k, x: ex, w, top: envCy - ENV_ABOVE, cy: envCy, rank: k === all.length - 1 ? 'capital' : 'town', platform };"));
    assert!(body.contains("const W = Math.max(devW, envs.length ? envs[envs.length - 1].x + envs[envs.length - 1].w + 8 : 0);"));
    assert!(body.contains("const H = envs.length ? envCy + ENV_BELOW : devH;"));
    assert!(body.contains("state.T.channel(c, 0, lay.envCy, lay.W, 14, t, 0.6);"));
    // The map aims at a screen's shape: houses per row chosen so the whole map comes closest to it.
    assert!(body.contains("export const ASPECT = 2;"));
    assert!(body.contains("const L = place(v, slot, per), off = Math.abs(Math.log(L.W / L.H / ASPECT));"));
    // A village's houses stand beside its warehouse, so the row stays low.
    assert!(body.contains("const hx = x + SHED.dx + SHED.w + 4 + k * 18;"));
    assert!(body.contains("state.T.B.cottage(c, hx, cy - 30, 'idle', t);"));
    // Each village: its sign, its warehouse of cargo grouped by feature, its health.
    assert!(body.contains("export const versionOf = s => s.tag || (s.sha ? String(s.sha).slice(0, 7) : null);"));
    assert!(body.contains("const k = fs.findIndex(f => (f.subtasks || []).some(s => s.number === n));"));
    assert!(body.contains("tap(x, y + 1, 5, 5, { kind: 'crate', n: sl.n, at: 'env', i: e.i });"));
    assert!(body.contains("if (o.at === 'env') return `runs in ${stops[o.i] ? stops[o.i].name : 'an environment'}`;"));
    assert!(body.contains("if (health === 'failed') {"));
    assert!(body.contains("} else if (health === 'incident') {"));
    assert!(body.contains("if (e.rank === 'capital' && !e.platform) envKeep("));
    // One label per village, over its warehouse, with its version (#440);
    // the canal no longer names a stop that has a village, and each house is a feature it runs.
    assert!(body.contains("return { x: e.x + SHED.dx + SHED.w / 2, y: e.cy - SHED.up - 3, anchor: 'bottom', text, sub, width: e.w - 8 };"));
    assert!(body.contains("names.locks.forEach((name, i) => { if (name) add(lay.lockStart + i * 34 + 3, lay.canalY - 21, 'bottom', name); });"));
    assert!(body.contains("add(lay.castleX + 10, lay.canalY - 22, 'bottom', theme.labels.castle, names.castle);"));
    assert!(body.contains("const { houses, more } = envHouses(cargoGroups(v, s.cargo || []), e.rank);"));
    assert!(body.contains("registerHit({ kind: 'envhouse', act(o, sc) {"));
    assert!(!body.contains("anchor: 'top', text: e.s.name"), "the old label under the river is gone");
    // The sign's dialog: version and deploy time, the Release and Deployments, and the cargo.
    assert!(body.contains("title: `${s.name} · environment village`, line: ver ? `Runs ${ver}${when ? ', deployed ' + when : ''}` : 'Nothing says what runs here',"));
    assert!(body.contains("deploys = s.deployment_url || (repo && s.source === 'deployment' ? `${repo}/deployments/${encodeURIComponent(s.name)}` : null);"));
    assert!(body.contains("items: issueItems(v, cargo).map(it => ({ ...it, label: label.get(it.number) })), links: envLinks(v, s) };"));
    // Voyages: each state drawn apart, cargo loaded at the source and unloaded at the target.
    for state in ["loading: (v, vo) => `Loading", "sailing: (v, vo) => `Deploying", "docked: (v, vo) => `${shortName(vo)} arrived at", "failed: (v, vo) => `Deploying ${shortName(vo)} to ${vo.to} failed: turned back"] {
        assert!(body.contains(state), "no words for voyage state: {state}");
    }
    assert!(body.contains("if (state_ === 'sailing') { pose.x = r.start + (r.end - r.start) * ease;"));
    assert!(body.contains("if (state_ === 'failed') pose.back = true;"));
    assert!(body.contains("c.save(); c.translate(2 * mid, 0); c.scale(-1, 1); state.T.vessel(c, x, cy, t, 0, oy); c.restore();"), "a failed ship turns back");
    assert!(body.contains("if (state_ === 'loading' && !still) pose.carry = { pts: [[r.a - 14, cy - 4], [r.a + 4, cy - 4]]"));
    assert!(body.contains("if (pose.aboard) pose.carry = { pts: [[r.b - 4, cy - 4], [r.b + SHED.dx + 6, cy - 4], [r.b + SHED.dx + 6, cy - SHED.up + 16]]"));
    assert!(body.contains("tap(left, top, hw, hh, { kind: 'voyage', id, at: pose.state, vo });"));
    // Timed from when first seen, so a reload never replays one; a change nothing reported plays one.
    assert!(body.contains("seen[k] = old ? t : vo.state === 'sailing' ? t - VOYAGE_SAIL / 2 : -1e9;"));
    assert!(body.contains("sc.played.push({ from: k ? envs[k - 1].s.name : 'dev', to: s.name, state: 'sailing'"));
    assert!(body.contains("noteVoyages(sc, sc.v, v, t);"));
    // The mini-map, and `#shop@release` scrolls to that village.
    assert!(body.contains(r#"<nav class="minimap" id="minimap" hidden aria-label="Villages in the environment row"></nav>"#));
    assert!(body.contains("const wide = lay && lay.envs.length && (shown.width > miniVp.clientWidth + 2 || shown.height > miniVp.clientHeight + 2);"));
    assert!(body.contains("if (e && state.scrolledTo !== want) { state.scrolledTo = want; scrollToVillage(e.x, e.top); }"));
    // Worlds (#407): the platform is the village of the environment it runs,
    // production unless the world names another, in that slot of the row.
    assert!(body.contains("return envs.some(({ s }) => s.name === w.environment) ? w.environment : envs[envs.length - 1].s.name;"));
    assert!(body.contains("if (!Ds.L.slot || Ds.L.slot.name !== name || Ds.L.slot.w !== need || Ds.L.slot.h !== tall) Ds.L = layout(Ds.v, { name, w: need, h: tall });"));
    assert!(body.contains("const platform = !!slot && s.name === slot.name, w = platform ? Math.max(ENV_W, slot.w) : ENV_W;"));
    assert!(body.contains("return { row: true, e, W: Ds.L.W, H: Ds.L.H, devTop: 0, dx: 0, px: e.x + ((e.w - Ps.L.W) >> 1), py, srcY: py + Ps.L.top };"));
    assert!(body.contains("return [[Ds, g.dx, g.devTop], [Ps, g.px, g.py]];"));
    // Its workloads are the slot's houses; its sign and warehouse stay under it.
    assert!(body.contains("if (!e.platform) {\n      const { houses, more } = envHouses("));
    assert!(body.contains("signY = cy - 30;"));
    // Incidents float down to the row's river, along it to the left edge and up to the dev docks.
    assert!(body.contains("const pts = g.row ? [[ox, oy], [rx + 7, oy], [rx + 7, rowY], [3, rowY], [3, ry], [g.dx + 30, ry]]"));
    assert!(body.contains(": [[ox, oy], [rx + 7, oy], [rx + 7, ry], [g.dx + 30, ry]];"));
    assert!(body.contains("if (!g.row) roadUp(t, w, g, Ps, Ds);"));
    // A click goes to the scene on top, in its own theme; the mini-map follows the world's dev row.
    assert!(body.contains("const at = shownScenes().reverse().find(([sc, ox, oy]) => x >= ox && y >= oy && x < ox + sc.L.W && y < oy + sc.L.H);"));
    assert!(body.contains("return w && sc.v && sc.v.project === w.platform ? otherTheme() : state.T;"));
    assert!(body.contains("const row = shownScenes().find(([x, ox]) => ox === 0 && x.L.envs.length), sc = row && row[0], lay = sc && sc.L;"));
    // The dev village's trees stop at its own edge.
    assert!(body.contains("x < devW - 14; x += 15)"));
}

#[test]
fn village_html_says_which_source_placed_each_stop() {
    let body = &village_source();
    for line in [
        "if (s.source === 'deployment') return `GitHub Deployment${sha}`;",
        "if (s.source === 'release') return `${s.tag ? s.tag + ' ' : ''}(GitHub Release)${sha}`;",
        "if (s.source === 'tag') return `tag ${s.tag || ''}${sha}`;",
        "if (s.source === 'url') return `its version endpoint${sha}`;",
    ] {
        assert!(body.contains(line), "sourceWords is missing: {line}");
    }
}
