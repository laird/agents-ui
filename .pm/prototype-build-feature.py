"""Build the run-2 prototype: a feature from spec to production.

The real village renderer (src/web/village.html) with synthetic snapshots for every
step of the flow, plus a feature layer drawn on top: a banner per feature, a trail to
its subtasks, a barge for whole-feature promotion, fog over an environment nobody can
answer for, and a per-feature strip. Usage:

    python3 .pm/prototype-build-feature.py outputs/prototype/index.html
"""
import json, os, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
src = open(os.path.join(ROOT, 'src/web/village.html')).read()

GH = 'https://github.com/larkspur-labs/shop/'
ISSUE = lambda n: GH + 'issues/%d' % n

# --- Features (D-011: sub-issues else "Sub-task of #N"; not-planned leaves the total) ---
FEATURES = [
    dict(number=460, title='Saved carts', color='#40b8e8', url=ISSUE(460), linked_by='GitHub sub-issues',
         spec=dict(kind='issue', label='Spec: #460 issue body', url=ISSUE(460)),
         subtasks=[dict(n=461, title='Cart table'), dict(n=462, title='Save on logout'),
                   dict(n=463, title='Restore banner'), dict(n=464, title='Tests')]),
    dict(number=470, title='Gift cards', color='#e070d0', url=ISSUE(470), linked_by='"Sub-task of #470" in each body (autocoder)',
         spec=dict(kind='doc', label='Spec: docs/specs/gift-cards.md', url=GH + 'blob/main/docs/specs/gift-cards.md'),
         subtasks=[dict(n=471, title='Gift card ledger'), dict(n=472, title='Gift card PDF', dropped='closed as not planned (folded into #471)'),
                   dict(n=473, title='Redeem at checkout')]),
]
TITLES = {s['n']: s['title'] for f in FEATURES for s in f['subtasks']}
TITLES.update({480: 'Search is slow on mobile', 481: 'Typo in footer', 482: 'Which payment provider?'})

def house(i, status='idle', issue=None, action=None):
    role = 'worker-%d' % i
    links = [dict(label='Agent session', url='/#/swarm/shop/agent/' + role)]
    if issue: links.append(dict(label='GitHub issue #%d' % issue, url=ISSUE(issue)))
    kind = {461: 'feature', 462: 'feature', 463: 'feature', 464: 'tests', 473: 'feature', 480: 'perf'}.get(issue, 'cottage')
    return dict(id='/src/shop-wt-%d' % i, role=role, kind=kind, status=status, issue=issue,
                issue_title=TITLES.get(issue), priority='P1' if issue in (461, 462, 463, 464) else ('P2' if issue else None),
                crew=[role], subagents=[], action=action, links=links)

def crate(n, kind='feature', pr='P1'):
    return dict(number=n, title=TITLES[n], kind=kind, priority=pr, url=ISSUE(n))

def shop(houses, backlog, quay, dev, uat, prod, board=()):
    return dict(project='shop', plane='development', framework='herdr', workflow='Autocoder', repo_path='~/src/shop',
                manager=dict(id='manager', role='manager', kind='cottage', status='working', issue=None, issue_title=None,
                             priority=None, crew=['manager'], subagents=[], action='messaging',
                             links=[dict(label='Agent session', url='/#/swarm/shop/agent/manager')]),
                houses=houses, backlog=backlog, board=[dict(number=482, title=TITLES[482], label='needs-design', url=ISSUE(482))] + list(board),
                pipeline=dict(integration='main', sentinel='green', quay=quay,
                              stops=[dict(name='dev', issues=dev), dict(name='uat', issues=uat), dict(name='prod', issues=prod)],
                              release='3f9c2a1'),
                builds=[])

OTHER = [crate(480, 'perf', 'P2'), crate(481, 'docs', 'P3')]
SUB4 = [crate(n, 'tests' if n == 464 else 'feature') for n in (461, 462, 463, 464)]
DONE_PROD = [440, 441]
ENV = dict(dev='branch `dev` in .autocoder.json', uat='GitHub Deployment to uat · 9b1e440', prod='GitHub Deployment to production · 3f9c2a1')

def snapshots(mode):
    whole = mode == 'whole'
    # Feature 470 sits partly promoted in per-subtask mode; held at the quay in whole-feature mode.
    g_uat, g_quay = ([], [471]) if whole else ([471], [])
    S = {}
    S['spec'] = shop([house(1), house(2), house(3), house(4, 'working', 473, 'writing')],
                     SUB4 + OTHER, g_quay, [], g_uat, DONE_PROD)
    S['follow'] = S['spec']
    S['build0'] = shop([house(1, 'working', 461, 'writing'), house(2, 'working', 462, 'running'),
                        house(3, 'working', 463, 'thinking'), house(4, 'working', 473, 'writing')],
                       [SUB4[3]] + OTHER, g_quay, [], g_uat, DONE_PROD)
    if whole:
        S['build4'] = shop([house(1), house(2, 'working', 480, 'reading'), house(3), house(4)],
                           [OTHER[1]], [461, 462, 463, 464, 471, 473], [], [], DONE_PROD)
        S['promote'] = shop([house(1), house(2, 'working', 480, 'running'), house(3), house(4)],
                            [OTHER[1]], [471, 473], [], [461, 462, 463, 464], DONE_PROD)
        S['live'] = shop([house(1), house(2), house(3), house(4)],
                         [OTHER[1]], [], [471, 473], [], DONE_PROD + [461, 462, 463, 464])
        S['stuck'] = shop([house(1), house(2, 'working', 480, 'reading'), house(3, 'blocked', 463), house(4)],
                          [OTHER[1]], [461, 462, 464, 471, 473], [], [], DONE_PROD, board=[dict(number=463, title=TITLES[463], label='needs-design', url=ISSUE(463))])
        S['unknown'] = dict(S['promote'])
    else:
        S['build4'] = shop([house(1), house(2, 'working', 480, 'reading'), house(3), house(4)],
                           [OTHER[1]], [464], [461, 462, 463, 473], [471], DONE_PROD)
        S['promote'] = shop([house(1), house(2, 'working', 480, 'running'), house(3), house(4)],
                            [OTHER[1]], [], [463, 464, 473], [461, 462, 471], DONE_PROD)
        S['live'] = shop([house(1), house(2), house(3), house(4)],
                         [OTHER[1]], [], [473], [471], DONE_PROD + [461, 462, 463, 464])
        S['stuck'] = shop([house(1), house(2, 'working', 480, 'reading'), house(3, 'blocked', 463), house(4)],
                          [OTHER[1]], [464], [461, 462, 473], [471], DONE_PROD, board=[dict(number=463, title=TITLES[463], label='needs-design', url=ISSUE(463))])
        S['unknown'] = dict(S['promote'])
    out = {}
    for k, v in S.items():
        out[k] = dict(villages=[v], worlds=[], unknown=['uat'] if k == 'unknown' else [],
                      env=dict(ENV, uat='no Deployment to uat and no `uat` branch' if k == 'unknown' else ENV['uat']))
    return out

DATA = dict(sub=snapshots('sub'), whole=snapshots('whole'))

STEPS = [
    ('spec', '1 · Spec arrives', 'A feature arrives: "Saved carts" (#460) is split into 4 subtasks, #461 to #464, waiting at the docks. Its banner stands east of the village, with the spec scroll at its foot. A second feature, Gift cards, is already under way.'),
    ('follow', '2 · Follow', 'Following #460: the trail leads from its banner to each subtask, and the strip reads Spec, Build 0/4, Dev, UAT, Prod.'),
    ('build0', '3 · Build', 'Workers claim three subtasks; #464 still waits at the docks. Nothing has landed: Build 0/4.'),
    ('build4', '4 · Landed', 'All 4 subtasks have landed: Build 4/4. In per-subtask projects they are already moving through the locks one by one; in whole-feature projects they wait at the quay as one barge.'),
    ('promote', '5 · Promote', 'Per subtask: Dev 4/4, UAT 2/4. The feature is partly in UAT, and the trail shows which two. Whole feature: the barge has passed the UAT lock together, UAT 4/4.'),
    ('live', '6 · Live', 'Prod runs code containing all 4 subtasks: the banner turns gold and reads "Live in prod". Later it fades.'),
    ('stuck', 'Stuck', '#463 is blocked (needs design). The banner flies a red pennant, the trail leads to worker-3 waving at you, and the strip reads "Build 3/4: #463 needs design".'),
    ('unknown', "Can't tell", 'There is no Deployment record and no branch for UAT. The UAT lock is in fog with a "?", and the strip reads "UAT: unknown", not "not live".'),
]

# --- Page changes -----------------------------------------------------------
css = '''
  .proto { position: fixed; top: 8px; right: 12px; font: 11px var(--mono); color: var(--ink-dim); border: 2px solid var(--rule); border-radius: 4px; padding: 2px 8px; background: var(--win-bot); z-index: 20; }
  .viewport { position: relative; }
  .fbanners { position: absolute; inset: 0; pointer-events: none; }
  /* A label under each feature's banner: a real button, so Tab reaches it. */
  .fb { position: absolute; transform: translateX(-50%); pointer-events: auto; max-width: 118px; font: 700 11px/1.25 var(--mono); color: var(--ink); background: rgba(6,8,20,.82); border: 2px solid var(--fc); border-radius: 4px; padding: 2px 6px; cursor: pointer; text-align: center; }
  .fb small { display: block; font-weight: 400; color: var(--ink-dim); }
  .fb[aria-pressed="true"] { background: var(--fc); color: #0c1028; }
  .fb[aria-pressed="true"] small { color: #0c1028; }
  .fb.live { border-color: var(--gold); }
  .fb.stuck small { color: #ff8070; }
  .fb.stuck[aria-pressed="true"] small { color: #600; }
  .fstrip { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 8px; margin-top: 8px; padding: 6px 8px; border: 2px solid var(--fc, var(--rule)); border-radius: 4px; background: rgba(6,8,20,.55); font: 13px var(--mono); }
  .fstrip h3 { margin: 0 6px 0 0; font: 700 13px var(--mono); color: var(--fc); }
  .chip { padding: 2px 8px; border-radius: 4px; border: 2px solid var(--rule); color: var(--ink-dim); white-space: nowrap; }
  .chip.done { border-color: #38c848; color: #b8f0b8; }
  .chip.part { border-color: var(--gold); color: var(--gold); }
  .chip.stuck { border-color: #e02828; color: #ffb0a8; background: rgba(224,40,40,.15); }
  .chip.unknown { border-style: dashed; border-color: var(--ink-dim); color: var(--ink); background: rgba(208,212,232,.12); }
  .chip.live { border-color: var(--gold); background: var(--gold); color: #241400; font-weight: 700; }
  .fstrip .sep { color: var(--ink-dim); }
  .fstrip .mode { margin-left: auto; color: var(--ink-dim); font-size: 12px; }
  .fstrip button, .demo button { background: var(--win-bot); color: var(--ink); border: 2px solid var(--win-inner); border-radius: 4px; font: 700 12px var(--mono); padding: 3px 10px; cursor: pointer; }
  .demo { position: fixed; bottom: 8px; left: 50%; transform: translateX(-50%); width: min(1400px, calc(100vw - 24px)); padding: 8px 10px; z-index: 15; display: grid; gap: 6px; }
  .demo .row { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .demo .lbl { font: 12px var(--mono); color: var(--ink-dim); margin-right: 2px; }
  .demo button[aria-pressed="true"] { background: var(--gold); color: #241400; border-color: var(--gold); }
  .demo .alt { border-style: dashed; }
  .demo .gap { width: 14px; }
  .demo p { margin: 0; font: 13px/1.4 var(--mono); color: var(--ink); min-height: 2.8em; }
  .dialog { bottom: 128px; max-height: calc(100vh - 160px); overflow: auto; }
  .dialog .subs { margin-top: 6px; }
  .dialog .subs li { display: flex; gap: 8px; }
  .dialog .subs .w { color: var(--ink-dim); }
  .dialog .subs .drop { text-decoration: line-through; color: var(--ink-dim); }
  .dialog dl { margin: 8px 0 0; font: 13px/1.4 var(--mono); display: grid; grid-template-columns: max-content 1fr; gap: 2px 10px; }
  .dialog dt { color: var(--ink-dim); }
  .dialog dd { margin: 0; }
  body { padding-bottom: 140px; }
'''
src = src.replace('</style>', css + '</style>', 1)
src = src.replace('<title>Worktree Hollow</title>', '<title>Prototype: agents-ui · A feature from spec to production (village)</title>')
src = src.replace('<body>', '<body>\n  <span class="proto">Prototype · synthetic data</span>', 1)
src = src.replace('<div class="empty" id="empty">Looking for swarms…</div>',
                  '<div class="empty" id="empty">Looking for swarms…</div>\n      <div class="fbanners" id="fbanners"></div>', 1)
src = src.replace('<div class="flow" id="flow"></div>',
                  '<div class="fstrip" id="fstrip" hidden aria-live="polite"></div>\n    <div class="flow" id="flow"></div>', 1)
dialog_old = '''    <div class="links" id="d-links"></div>
  </div>'''
assert dialog_old in src, 'village dialog markup changed: update the prototype build'
steps_html = ''.join('<button data-step="%s"%s>%s</button>' % (k, ' class="alt"' if k in ('stuck', 'unknown') else '', l) +
                     ('<span class="gap"></span>' if k == 'live' else '') for k, l, _ in STEPS)
src = src.replace(dialog_old, '''    <div class="links" id="d-links"></div>
  </div>
  <div class="win demo" role="group" aria-label="Prototype controls">
    <div class="row"><span class="lbl">Promotion:</span>
      <button data-mode="sub" aria-pressed="true">Per subtask</button><button data-mode="whole" aria-pressed="false">Whole feature</button>
      <span class="gap"></span><span class="lbl">Step:</span>''' + steps_html + '''</div>
    <p id="cap" aria-live="polite"></p>
  </div>''', 1)

# --- Synthetic data and feature model (global, read by the renderer) ---
src = src.replace('<script>\n(() => {', '<script>\nconst PROTO = ' + json.dumps(DATA) + ';\nconst FEATURES = ' + json.dumps(FEATURES) +
                  ';\nconst STEPS = ' + json.dumps([dict(key=k, label=l, cap=c) for k, l, c in STEPS]) + r''';
const PF = { mode: 'sub', step: 'spec', followed: null };
(() => { const q = new URLSearchParams(location.search);
  if (PROTO[q.get('mode')]) PF.mode = q.get('mode');
  if (STEPS.some(s => s.key === q.get('step'))) PF.step = q.get('step');
  if (q.get('follow')) PF.followed = Number(q.get('follow'));
  else if (!['spec'].includes(PF.step)) PF.followed = 460; })();
PF.snap = () => PROTO[PF.mode][PF.step];
const counted = f => f.subtasks.filter(s => !s.dropped);
const STAGE_NAME = { dev: 'Dev', uat: 'UAT', prod: 'Prod' };
/* Where a subtask is: null = not landed, -1 = landed at the quay, i = reached stop i. */
function posOf(n, v) {
  if (v.pipeline.quay.includes(n)) return -1;
  const i = v.pipeline.stops.findIndex(s => s.issues.includes(n));
  return i >= 0 ? i : null;
}
/* D-011: a feature reaches a stage when every counted subtask has; an environment
 * with no Deployment and no branch is unknown, never "not live". */
function featureStatus(f, v, snap) {
  const subs = counted(f), total = subs.length, unknown = new Set(snap.unknown || []);
  const landed = subs.filter(s => posOf(s.n, v) !== null).length;
  const stages = v.pipeline.stops.map((s, i) => ({ name: s.name, label: STAGE_NAME[s.name] || s.name, unknown: unknown.has(s.name),
    n: subs.filter(x => { const p = posOf(x.n, v); return p !== null && p >= i; }).length, total }));
  const blocked = subs.filter(x => v.board.some(b => b.number === x.n) || v.houses.some(h => h.issue === x.n && h.status === 'blocked'))
    .map(x => ({ ...x, label: (v.board.find(b => b.number === x.n) || {}).label || 'blocked' }));
  const last = stages[stages.length - 1];
  return { total, landed, stages, blocked, stuck: blocked.length > 0, live: !!last && !last.unknown && last.n === total };
}
function featureSummary(st) {
  if (st.live) return 'Live in prod';
  if (st.stuck) return `Stuck · #${st.blocked[0].n} ${st.blocked[0].label}`;
  if (st.landed < st.total) return `Build ${st.landed}/${st.total}`;
  const s = st.stages.find(s => s.unknown || s.n < s.total);
  return s ? (s.unknown ? `${s.label}: unknown` : `${s.label} ${s.n}/${s.total}`) : 'Live in prod';
}
window.fetch = async () => ({ ok: true, json: async () => JSON.parse(JSON.stringify(PF.snap())) });
</script>
<script>
(() => {''', 1)

# --- Feature layer, inside the renderer so it shares its drawing kit ---
layer = r'''
  /* ---------- Prototype: features ----------
   * Banners stand in a cleared strip east of the village; following one draws a
   * marching trail to each of its subtasks. Whole-feature projects carry a
   * feature's landed subtasks on one barge; an unknown environment is in fog. */
  let curBarges = [];
  function bannerSpots(lay) {
    const fx = Math.max(lay.cx0 + lay.vw + 6, lay.hallX + 64);
    return FEATURES.map((f, k) => ({ f, x: fx + 12 + k * 30, y: 14 }));
  }
  function protoClear(x, y) {
    if (!L || !current || current.project !== 'shop') return false;
    const g = bannerSpots(L);
    return y < 48 && x >= g[0].x - 14 && x <= g[g.length - 1].x + 22;
  }
  function bargesFor(v, lay) {
    if (PF.mode !== 'whole') return [];
    const last = v.pipeline.stops.length - 1;
    return FEATURES.map(f => {
      const ns = counted(f).map(s => s.n), at = ns.map(n => posOf(n, v));
      if (!ns.length || at.some(a => a === null || a !== at[0])) return null;
      const i = at[0];
      const x = i === -1 ? lay.roadX - 26 : i === last ? lay.castleX - 26 : lay.lockStart + i * 34 + 8;
      return { f, issues: ns, stop: i, x, y: lay.canalY + 1 };
    }).filter(Boolean);
  }
  function whereIs(n, v, lay) {
    const hi = v.houses.findIndex(h => h.issue === n);
    if (hi >= 0) { const lot = lay.lots[hi]; return lot && { x: lot.x + 8, y: lot.y + 18 }; }
    const bi = v.backlog.findIndex(c => c.number === n);
    if (bi >= 0) { const s = slotPos(bi); return { x: s.x + 2, y: s.y + 2 }; }
    const b = curBarges.find(b => b.issues.includes(n));
    if (b) { const k = b.issues.indexOf(n); return { x: b.x + 4 + k * 5, y: b.y + 3 }; }
    const hid = new Set(curBarges.flatMap(b => b.issues));
    const qi = v.pipeline.quay.filter(m => !hid.has(m)).indexOf(n);
    if (qi >= 0) return { x: lay.roadX + 8 + (qi % 3) * 6, y: lay.quayY - 1 - Math.floor(qi / 3) * 6 };
    const stops = v.pipeline.stops;
    for (let i = 0; i < stops.length; i++) {
      const k = stops[i].issues.filter(m => !hid.has(m)).indexOf(n);
      if (k < 0) continue;
      return i < stops.length - 1
        ? { x: lay.lockStart + i * 34 + 10 + (k % 2) * 6, y: lay.canalY + 20 + Math.floor(k / 2) * 6 }
        : { x: lay.castleX + 8 + (k % 4) * 6, y: lay.canalY + 20 + Math.floor(k / 4) * 6 };
    }
    return null;
  }
  function drawBarge(c, b, t) {
    const x = b.x, y = b.y + (!reduce && blink(t, 700) ? 1 : 0), col = b.f.color;
    R(c, x, y + 6, 24, 5, P.o); R(c, x + 1, y + 6, 22, 3, P.wood); R(c, x + 1, y + 6, 22, 1, P.woodL); R(c, x + 3, y + 9, 18, 1, P.woodD);
    b.issues.slice(0, 4).forEach((n, k) => T.crate(c, x + 2 + k * 5, y + 1, 'workshop', null, true));
    R(c, x + 22, y - 6, 1, 8, P.o); R(c, x + 23, y - 6, 5, 4, P.o); R(c, x + 23, y - 5, 4, 2, col);
  }
  function fogStop(c, v, i, lay, t) {
    const last = v.pipeline.stops.length - 1, x = i === last ? lay.castleX - 4 : lay.lockStart + i * 34 - 7;
    R(c, x, lay.canalY - 28, 32, lay.H - lay.canalY + 28, 'rgba(206,210,230,.72)');
    if (!reduce) for (let k = 0; k < 5; k++) puffs(c, x + 4 + k * 6, lay.canalY - 18 + (k % 2) * 22, t + k * 330, '#e0e4f0', '#b8c0d8');
    bubble(c, x + 12, lay.canalY - 6, 'unknown');
  }
  function drawBanner(c, g, st, on, t) {
    const { x, y, f } = g, col = st.live ? P.gold : f.color, wave = !reduce && blink(t + f.number * 7, 320) ? 1 : 0;
    const top = st.stuck ? y - 7 : y - 1;
    R(c, x - 2, y + 23, 5, 2, P.o); R(c, x - 1, y + 23, 3, 1, P.stoneD);
    R(c, x, top, 1, y + 23 - top, P.o); R(c, x, top - 1, 1, 1, P.gold);
    // Cloth with a swallowtail, rippling.
    R(c, x + 1, y, 14, 10, P.o); R(c, x + 1, y + 1, 13, 8, col); R(c, x + 1, y + 1, 13, 1, P.white);
    R(c, x + 12, y + 4 + wave, 3, 2, P.o); R(c, x + 4, y + 4, 6, 1, st.live ? P.o : P.white); R(c, x + 4, y + 6, 4, 1, st.live ? P.o : P.white);
    // Stuck: a red pennant above the cloth.
    if (st.stuck) { const w = blink(t, 260) ? 1 : 0; R(c, x + 1, y - 7, 7 + w, 4, P.o); R(c, x + 1, y - 6, 6 + w, 2, P.p0); }
    // The spec scroll at its foot.
    R(c, x + 3, y + 19, 8, 5, P.o); R(c, x + 4, y + 20, 6, 3, P.parch); R(c, x + 5, y + 21, 4, 1, P.pathS);
    if (st.live && !reduce) for (let k = 0; k < 3; k++) if (blink(t + k * 170, 220)) R(c, x + 3 + k * 5, y - 3 - (k % 2) * 2, 1, 1, P.gold);
    if (on) { const o = blink(t, 400) ? col : P.white; R(c, x - 3, y - 3, 20, 1, o); R(c, x - 3, y + 26, 20, 1, o); R(c, x - 3, y - 3, 1, 30, o); R(c, x + 16, y - 3, 1, 30, o); }
  }
  function drawTrail(c, x0, y0, x1, y1, col, t) {
    const off = reduce ? 0 : Math.floor(t / 110) % 4, pts = [];
    x1 = Math.round(x1); y1 = Math.round(y1);
    for (let y = y0; y !== y1; y += Math.sign(y1 - y0)) pts.push([x0, y]);
    for (let x = x0; x !== x1; x += Math.sign(x1 - x0)) pts.push([x, y1]);
    pts.forEach(([px, py], i) => { if ((i + off) % 4 < 2) { R(c, px, py + 1, 1, 1, P.o); R(c, px, py, 1, 1, col); } });
    const o = blink(t, 300) ? col : P.white;
    R(c, x1 - 4, y1 - 4, 9, 1, o); R(c, x1 - 4, y1 + 4, 9, 1, o); R(c, x1 - 4, y1 - 4, 1, 9, o); R(c, x1 + 4, y1 - 4, 1, 9, o);
  }
  function featureLayer(c, t, v, lay) {
    if (v.project !== 'shop') return;
    const snap = PF.snap();
    curBarges.forEach(b => drawBarge(c, b, t));
    (snap.unknown || []).forEach(name => { const i = v.pipeline.stops.findIndex(s => s.name === name); if (i >= 0) fogStop(c, v, i, lay, t); });
    bannerSpots(lay).forEach(g => {
      const st = featureStatus(g.f, v, snap), on = PF.followed === g.f.number;
      if (on) counted(g.f).forEach(s => { const w = whereIs(s.n, v, lay); if (w) drawTrail(c, g.x, g.y + 25, w.x, w.y, g.f.color, t); });
      drawBanner(c, g, st, on, t);
    });
  }
'''
anchor = '  function drawVillage(c, sc, t) {'
assert anchor in src
src = src.replace(anchor, layer + anchor, 1)

# paint(): work out the barges first, keep their crates off the quay and locks, clear the banner strip, draw the layer last.
old = '  function paint(c, t) {\n    const v = current, lay = L;\n'
assert old in src
src = src.replace(old, old + '    curBarges = v.project === \'shop\' ? bargesFor(v, lay) : [];\n    const hid = new Set(curBarges.flatMap(b => b.issues));\n', 1)
old = 'for (let y = 16; y < canalY - 38; y += 16) if (hash(x, y) < 0.55) T.tree('
assert old in src
src = src.replace(old, 'for (let y = 16; y < canalY - 38; y += 16) if (hash(x, y) < 0.55 && !protoClear(x, y)) T.tree(', 1)
for old, new in [
    ('v.pipeline.quay.filter(n => !carried.has(n))', 'v.pipeline.quay.filter(n => !carried.has(n) && !hid.has(n))'),
    ('      s.issues.slice(0, 4).forEach(', '      s.issues.filter(n => !hid.has(n)).slice(0, 4).forEach('),
    ('if (stops.length) stops[stops.length - 1].issues.slice(0, 6)', 'if (stops.length) stops[stops.length - 1].issues.filter(n => !hid.has(n)).slice(0, 6)'),
    ('    bubbles.forEach(([x, y, s]) => bubble(c, x, y, s));\n  }', '    bubbles.forEach(([x, y, s]) => bubble(c, x, y, s));\n    featureLayer(c, t, v, lay);\n  }'),
]:
    assert old in src, old
    src = src.replace(old, new, 1)

# Panels: banner labels, the feature strip, the feature dialog, and the demo controls.
panels = r'''
  /* ---------- Prototype: banner labels, strip, dialog, controls ---------- */
  const fbox = document.getElementById('fbanners'), fstrip = document.getElementById('fstrip');
  function placeBanners() {
    fbox.innerHTML = '';
    const sc = view && view.village && scenes[view.village];
    if (!sc || !sc.v || sc.v.project !== 'shop') { fstrip.hidden = true; return; }
    const v = sc.v, snap = PF.snap(), r = cv.getBoundingClientRect(), vr = document.getElementById('viewport').getBoundingClientRect(), s = r.width / cv.width;
    bannerSpots(sc.L).forEach(g => {
      const st = featureStatus(g.f, v, snap), b = document.createElement('button');
      b.className = 'fb' + (st.live ? ' live' : '') + (st.stuck ? ' stuck' : '');
      b.style.setProperty('--fc', st.live ? 'var(--gold)' : g.f.color);
      b.setAttribute('aria-pressed', String(PF.followed === g.f.number));
      b.innerHTML = `#${g.f.number} ${esc(g.f.title)}<small>${esc(featureSummary(st))}</small>`;
      b.setAttribute('aria-label', `Feature #${g.f.number} ${g.f.title}: ${featureSummary(st)}. ${PF.followed === g.f.number ? 'Stop following' : 'Follow'}.`);
      b.style.left = (r.left - vr.left + (g.x + 7) * s) + 'px';
      b.style.top = (r.top - vr.top + (g.y + 27) * s) + 'px';
      b.onclick = () => { PF.followed = PF.followed === g.f.number ? null : g.f.number; placeBanners(); };
      fbox.appendChild(b);
    });
    renderStrip(v, snap);
  }
  function renderStrip(v, snap) {
    const f = FEATURES.find(f => f.number === PF.followed);
    fstrip.hidden = !f;
    if (!f) return;
    const st = featureStatus(f, v, snap);
    fstrip.style.setProperty('--fc', f.color);
    const chip = (cls, text) => `<span class="chip ${cls}">${esc(text)}</span>`;
    const build = st.stuck ? chip('stuck', `Build ${st.landed}/${st.total}: #${st.blocked[0].n} ${st.blocked[0].label}`)
      : chip(st.landed === st.total ? 'done' : st.landed ? 'part' : '', `Build ${st.landed}/${st.total}`);
    const stages = st.stages.map(s => s.unknown ? chip('unknown', `${s.label}: unknown`)
      : chip(s.n === s.total ? 'done' : s.n ? 'part' : '', `${s.label} ${s.n}/${s.total}`));
    fstrip.innerHTML = `<h3>Following #${f.number} ${esc(f.title)}</h3>` +
      [chip('done', 'Spec ✓'), build, ...stages].join('<span class="sep">›</span>') +
      (st.live ? ' ' + chip('live', '★ Live in prod') : '') +
      `<span class="mode">${PF.mode === 'whole' ? 'Promotes as a whole feature' : 'Promotes per subtask'}</span>` +
      `<button id="f-details">Details</button><button id="f-stop">Stop following</button>`;
    document.getElementById('f-details').onclick = () => showFeature(f, v, snap);
    document.getElementById('f-stop').onclick = () => { PF.followed = null; placeBanners(); };
  }
  function whereWords(n, v, snap) {
    const h = v.houses.find(h => h.issue === n);
    if (h) return h.status === 'blocked' ? `blocked · ${h.role} is waiting on you` : `being built by ${h.role}`;
    if (v.backlog.some(c => c.number === n)) return 'waiting at the docks';
    const p = posOf(n, v), stops = v.pipeline.stops;
    if (p === -1) return PF.mode === 'whole' ? 'landed · waiting at the quay for the rest' : 'landed · waiting for dev';
    if (p === null) return 'not started';
    const s = stops[p];
    if ((snap.unknown || []).includes(s.name)) return `past ${STAGE_NAME[stops[p - 1].name]} · ${STAGE_NAME[s.name]} unknown`;
    return p === stops.length - 1 ? 'in prod' : `in ${STAGE_NAME[s.name]}`;
  }
  function showFeature(f, v, snap) {
    const st = featureStatus(f, v, snap);
    document.getElementById('d-title').textContent = `#${f.number} ${f.title} · feature`;
    document.getElementById('d-line').textContent = featureSummary(st) + ` · ${st.total} subtasks counted`;
    document.getElementById('d-meta').innerHTML =
      `<ul class="subs">${f.subtasks.map(s => s.dropped
        ? `<li><a href="https://github.com/larkspur-labs/shop/issues/${s.n}" target="_blank" rel="noopener" class="drop">#${s.n}</a><span class="drop">${esc(s.title)}</span><span class="w">${esc(s.dropped)} · not counted</span></li>`
        : `<li><a href="https://github.com/larkspur-labs/shop/issues/${s.n}" target="_blank" rel="noopener">#${s.n}</a><span>${esc(s.title)}</span><span class="w">${esc(whereWords(s.n, v, snap))}</span></li>`).join('')}</ul>
      <dl><dt>Subtasks from</dt><dd>${esc(f.linked_by)}</dd>
      ${v.pipeline.stops.map(s => `<dt>${STAGE_NAME[s.name]}</dt><dd>${esc(snap.env[s.name])}</dd>`).join('')}
      <dt>Live means</dt><dd>all counted subtasks' code runs in prod</dd></dl>`;
    document.getElementById('d-q').innerHTML = '';
    links([f.spec, { label: `GitHub issue #${f.number}`, url: f.url }]);
    dialog.hidden = false;
  }
  function protoGo() {
    document.querySelectorAll('[data-mode]').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.mode === PF.mode)));
    document.querySelectorAll('[data-step]').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.step === PF.step)));
    document.getElementById('cap').textContent = STEPS.find(s => s.key === PF.step).cap;
    dialog.hidden = true;
    poll();
  }
  document.querySelectorAll('[data-mode]').forEach(b => b.addEventListener('click', () => { PF.mode = b.dataset.mode; protoGo(); }));
  document.querySelectorAll('[data-step]').forEach(b => b.addEventListener('click', () => {
    PF.step = b.dataset.step;
    if (PF.step === 'spec') PF.followed = null; else if (PF.step === 'follow' || PF.followed === null) PF.followed = 460;
    protoGo();
  }));
'''
anchor = '  /* ---------- Data ---------- */'
assert anchor in src
src = src.replace(anchor, panels + anchor, 1)

for old, new in [
    ("    fit();\n    if (reduce) draw(0);\n  }", "    fit();\n    if (reduce) draw(0);\n    requestAnimationFrame(placeBanners);\n  }"),
    ("  window.addEventListener('resize', fit);", "  window.addEventListener('resize', () => { fit(); placeBanners(); });"),
    ("    setTimeout(poll, 3000);\n", ""),
    ("Math.floor((window.innerHeight - 190) / H)", "Math.floor((window.innerHeight - 340) / H)"),
    ("  poll();\n\n  if (!reduce) {", "  protoGo();\n\n  if (!reduce) {"),
]:
    assert old in src, old
    src = src.replace(old, new, 1)

for must in ['featureLayer(c, t, v, lay)', 'protoClear(x, y)', 'id="fstrip"', 'data-step="unknown"', 'protoGo();']:
    assert must in src, must
open(sys.argv[1], 'w').write(src)
print('built', len(src))
