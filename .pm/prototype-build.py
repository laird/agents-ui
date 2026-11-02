"""Build the village prototype: the real village renderer, synthetic data, attention markers."""
import json, re, sys
src = open('/home/user/agents-ui/src/web/village.html').read()

GH = 'https://github.com/larkspur-labs/'
def house(id, role, kind, status, issue=None, title=None, pr=None, crew=None, subs=None, action=None, att=None, links=None, project=None):
    if links is None and project:
        links = [dict(label='Agent session', url='/#/swarm/%s/agent/%s' % (project, role))]
        if issue: links.append(dict(label='GitHub issue #%d' % issue, url=GH + project + '/issues/%d' % issue))
    h = dict(id=id, role=role, kind=kind, status=status, issue=issue, issue_title=title, priority=pr,
             crew=crew or [role], subagents=subs or [], action=action, links=links or [])
    if att: h['attention'] = att
    return h
def village(project, framework, plane, manager, houses, backlog=(), board=(), stops=(), workflow=None):
    return dict(project=project, plane=plane, framework=framework, workflow=workflow, repo_path='~/src/' + project,
                manager=manager, houses=houses, backlog=list(backlog), board=list(board),
                pipeline=dict(integration='main', sentinel='green', quay=[], stops=list(stops), release=None), builds=[])
crate = lambda n, t, k, p=None, repo='shop': dict(number=n, title=t, kind=k, priority=p, url=GH + repo + '/issues/%d' % n)
TUI = 'http://localhost:8233/namespaces/support/workflows/'

stop = lambda n, i: dict(name=n, issues=i)

W2 = dict(reported=True, source='herdr', state='blocked', ask='Allow `npm publish`? (y/n)', since='14:02', wait='3 min',
          agree='Screen also shows a prompt', session=['✓ 214 tests passed', 'Bash(npm publish --dry-run)', '  ⎿  package size 48.2 kB, 112 files', '', 'Allow `npm publish`? (y/n)'])
B1 = dict(reported=False, source='screen guess', state='"Allow? (y/n)" in the last 15 lines', ask='Allow? (y/n)', since='about 14:04', wait='40 s',
          agree='No reported state to compare: this swarm runs in plain tmux', hint='Run this swarm under herdr to get reported states.',
          session=['Edit(src/invoices/retry.ts)', '  ⎿  Updated with 12 additions', '', 'Run the migration now? Allow? (y/n)'])
R1 = dict(reported=True, source='Temporal', state='AgentStatus = waiting_for_human', ask='Approve a refund of $240 for order 5531?', since='13:59', wait='6 min',
          agree='Child workflow RefundCheck is paused on the approval signal',
          session=['RefundCheck refund-8812/check', '  policy: amount over $200 needs a person', '  waiting for signal: approve_refund'])

shop = village('shop', 'herdr', 'development',
  house('manager', 'manager', 'cottage', 'working', action='messaging', project='shop'),
  [house('/src/shop-wt-1', 'worker-1', 'feature', 'working', 41, 'Saved carts survive logout', 'P1', action='writing', project='shop'),
   house('/src/shop-wt-2', 'worker-2', 'bug', 'blocked', 43, 'Checkout 500 on empty basket', 'P0', att=W2, project='shop'),
   house('/src/shop-wt-3', 'worker-3', 'cottage', 'idle', project='shop'),
   house('/src/shop-wt-4', 'worker-4', 'security', 'working', 44, 'Bump openssl', 'P1', action='running', project='shop')],
  [crate(45, 'Search is slow on mobile', 'perf', 'P2'), crate(46, 'Typo in footer', 'docs', 'P3')],
  [dict(number=47, title='Which payment provider?', label='needs-design', url=GH + 'shop/issues/47')],
  [stop('main-green', [39]), stop('staging', []), stop('prod', [37])], 'Autocoder')
billing = village('billing', 'tmux', 'development',
  house('manager', 'manager', 'cottage', 'idle', project='billing'),
  [house('/src/billing-wt-1', 'worker-1', 'refactor', 'blocked', 12, 'Retry failed invoices', 'P1', att=B1, project='billing'),
   house('/src/billing-wt-2', 'worker-2', 'tests', 'working', 13, 'Flaky ledger test', 'P2', action='running', project='billing')],
  [crate(14, 'Tax rounding', 'bug', 'P1', 'billing')], [], [stop('main-green', []), stop('prod', [9])], 'Autocoder')
support = village('temporal:support', 'temporal', 'runtime',
  house('manager', 'workers', 'cottage', 'working', title='3 workers polling 1 task queue', crew=['workers'],
        subs=[dict(agent_type='worker', description='worker@pod-%s' % p, status='working', action=None) for p in 'abc']),
  [house('refund-8812', 'SupportAgent', 'workshop', 'blocked', title='refund-8812', crew=['SupportAgent'],
         subs=[dict(agent_type='RefundCheck', description='refund-8812/check', status='blocked', action=None)], att=R1, links=[dict(label='Temporal UI', url=TUI + 'refund-8812')]),
   house('research-31', 'ResearchAgent', 'research', 'working', title='research-31', crew=['ResearchAgent'], action='searching', links=[dict(label='Temporal UI', url=TUI + 'research-31')]),
   house('triage-77', 'TriageAgent', 'workshop', 'working', title='triage-77', crew=['TriageAgent'], action='thinking', links=[dict(label='Temporal UI', url=TUI + 'triage-77')])],
  [], [], [stop('completed (24h)', [8801, 8802, 8803])], 'Temporal')
down = json.loads(json.dumps(shop))
for h in down['houses'] + [down['manager']]:
    h['last_known'] = h['status'] + ((' · ' + h['attention']['source'] + ', since ' + h['attention']['since']) if h.get('attention') else '')
    h['status'] = 'unknown'; h['action'] = None; h.pop('attention', None)
down['unreachable'] = dict(source='herdr', since='14:05')

data = dict(main=dict(villages=[shop, billing, support], worlds=[]),
            down=dict(villages=[down, billing, support], worlds=[]))

# --- page changes ---------------------------------------------------------
css = '''
  .proto { position: fixed; top: 8px; right: 12px; font: 11px var(--mono); color: var(--ink-dim); border: 2px solid var(--rule); border-radius: 4px; padding: 2px 8px; background: var(--win-bot); z-index: 20; }
  .viewport { position: relative; }
  .signs { position: absolute; inset: 0; pointer-events: none; }
  /* A sign over each house that needs you: who reported it and how long it has waited. Solid = reported, dashed = guessed. */
  .sign { position: absolute; transform: translate(-50%, -100%); pointer-events: auto; font: 700 12px/1.2 var(--mono); color: #241400; background: var(--gold); border: 3px solid #241400; border-radius: 4px; padding: 3px 7px; white-space: nowrap; cursor: pointer; box-shadow: 0 2px 0 #241400; }
  .sign.guess { background: var(--win-bot); color: var(--gold); border: 3px dashed var(--gold); box-shadow: none; }
  .sign.lost { background: #3a3e58; color: var(--ink); border-color: var(--ink-dim); }
  .sign small { font-weight: 400; }
  .banner { max-width: 1400px; margin: 0 auto 10px; padding: 10px 14px; font: 14px var(--mono); border: 3px solid var(--orange); background: rgba(240,112,32,.15); border-radius: 6px; }
  .why dt { font: 12px var(--mono); color: var(--ink-dim); margin-top: 8px; }
  .why dd { margin: 2px 0 0; font: 14px/1.4 var(--mono); }
  .why code, .term { font-family: var(--mono); }
  .tag { display: inline-block; font: 700 12px var(--mono); padding: 1px 8px; border-radius: 4px; background: var(--gold); color: #241400; }
  .tag.guess { background: none; color: var(--gold); border: 2px dashed var(--gold); }
  .acts { display: flex; gap: 10px; margin-top: 12px; }
  .acts button, .demo button { position: static; background: var(--win-bot); color: var(--ink); border: 2px solid var(--win-inner); border-radius: 4px; font: 700 13px var(--mono); padding: 6px 12px; cursor: pointer; }
  .acts .go { background: var(--gold); color: #241400; border-color: var(--gold); }
  .term { background: #050814; border: 2px solid var(--rule); border-radius: 4px; padding: 10px; font-size: 13px; white-space: pre-wrap; margin: 8px 0 0; color: var(--ink); }
  .term .ask { color: var(--gold); font-weight: 700; }
  .dialog { bottom: 76px; max-height: calc(100vh - 100px); overflow: auto; }
  .demo { position: fixed; bottom: 10px; left: 50%; transform: translateX(-50%); display: flex; gap: 8px; padding: 8px 10px; z-index: 15; }
  .demo span { font: 12px var(--mono); color: var(--ink-dim); align-self: center; }
'''
src = src.replace('</style>', css + '</style>', 1)
src = src.replace('<title>Worktree Hollow</title>', '<title>Prototype: agents-ui · Attention and answering (village)</title>')
src = src.replace('<body>', '<body>\n  <span class="proto">Prototype · synthetic data</span>\n  <div class="banner" id="banner" role="alert" hidden></div>', 1)
src = src.replace('<div class="flow" id="flow"></div>', '<div class="flow" id="flow"></div>', 1)
src = src.replace('<div class="empty" id="empty">Looking for swarms…</div>', '<div class="empty" id="empty">Looking for swarms…</div>\n      <div class="signs" id="signs"></div>', 1)
dialog_old = '''    <h2 id="d-title"></h2>
    <p id="d-line"></p>
    <p class="dim" id="d-meta"></p>
    <div class="links" id="d-links"></div>
  </div>'''
assert dialog_old in src, 'village dialog markup changed: update the prototype build'
src = src.replace(dialog_old, '''    <h2 id="d-title" tabindex="-1"></h2>
    <p id="d-line"></p>
    <p class="dim" id="d-meta"></p>
    <div id="d-why" class="why"></div>
    <div class="links" id="d-links"></div>
  </div>
  <div class="win demo" role="group" aria-label="Demo controls"><span>Demo:</span>
    <button data-demo="main" data-at="shop">Main path</button>
    <button data-demo="down" data-at="shop">Source unreachable</button>
    <button data-demo="main" data-at="billing">Guess only</button>
  </div>''', 1)

# Synthetic data instead of the server, switchable by the demo bar.
src = src.replace('<script>\n(() => {', '<script>\nconst PROTO = ' + json.dumps(data) + ''';
let protoScenario = 'main';
window.fetch = async () => ({ ok: true, json: async () => JSON.parse(JSON.stringify(PROTO[protoScenario])) });
</script>
<script>
(() => {''', 1)

# The dialog explains why a house needs you, with Open agent.
old_show = src[src.index('  function show(house, isManager, v, theme) {'):src.index('  function hit(sc, x, y, theme) {')]
new_show = '''  const escH = s => String(s).replace(/[&<>"]/g, ch => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[ch]));
  function show(house, isManager, v, theme) {
    const a = house.attention, why = document.getElementById('d-why');
    const who = `${v.project} · ${isManager ? theme.labels.hall : house.role}`;
    document.getElementById('d-title').textContent = a ? `Why ${who} needs you` : who;
    document.getElementById('d-line').textContent = house.issue ? `#${house.issue} ${house.issue_title || ''}` : (house.issue_title || 'No issue claimed');
    document.getElementById('d-meta').textContent = house.last_known ? `State unknown: ${v.unreachable.source} unreachable since ${v.unreachable.since} · last known: ${house.last_known}` : `Status: ${house.status}${house.priority ? ' · ' + house.priority : ''}`;
    why.innerHTML = a ? `<p>${a.reported ? '<span class="tag">Reported</span> by the source itself, not guessed' : '<span class="tag guess">Guessed from screen</span> this may be wrong'}</p>
      <dl><dt>Source</dt><dd>${a.reported ? escH(a.source) + ' reports <code>' + escH(a.state) + '</code>' : 'None. Matched ' + escH(a.state)}</dd>
      <dt>What it asked</dt><dd><code>${escH(a.ask)}</code></dd>
      <dt>Waiting since</dt><dd>${escH(a.since)} · ${escH(a.wait)}</dd>
      <dt>Other sources</dt><dd>${a.reported ? '✓ ' : ''}${escH(a.agree)}</dd></dl>
      ${a.hint ? `<p class="dim">${escH(a.hint)}</p>` : ''}
      <div class="acts"><button class="go" id="d-open">Open agent</button><button id="d-close2">Close</button></div>` :
      (house.last_known ? '<div class="acts"><button class="go" id="d-open">Open agent anyway</button></div>' : '');
    const open = document.getElementById('d-open');
    if (open) open.onclick = () => {
      document.getElementById('d-title').textContent = `${who} · live session`;
      why.innerHTML = `<pre class="term">${(a ? a.session : ['(no output since herdr went quiet)']).map((l, i, all) => i === all.length - 1 && a ? '<span class="ask">' + escH(l) + '</span> ▌' : escH(l)).join('\\n')}</pre><p class="dim">Answering is out of this prototype's scope.</p><div class="acts"><button id="d-close2">Back to the village</button></div>`;
      document.getElementById('d-close2').onclick = () => { dialog.hidden = true; };
      document.getElementById('d-title').focus();
    };
    const c2 = document.getElementById('d-close2'); if (c2) c2.onclick = () => { dialog.hidden = true; };
    links(house.links);
    dialog.hidden = false;
    document.getElementById('d-title').focus();
  }
  /* Signs over the houses that need you, placed from the scene's layout. */
  function placeSigns() {
    const box = document.getElementById('signs'); box.innerHTML = '';
    const banner = document.getElementById('banner');
    if (!view || !view.village) { banner.hidden = true; return; }
    const sc = scenes[view.village]; if (!sc || !sc.v) return;
    const v = sc.v, r = cv.getBoundingClientRect(), vr = document.getElementById('viewport').getBoundingClientRect(), s = r.width / cv.width;
    banner.hidden = !v.unreachable;
    if (v.unreachable) banner.innerHTML = `<b>${escH(v.unreachable.source)} can't be reached</b> since ${escH(v.unreachable.since)}. ${escH(v.project)}'s houses are in fog: their states are unknown. Select a house for its last known state.`;
    const put = (h, x, y, isManager) => {
      const a = h.attention; if (!a && !h.last_known) return;
      const b = document.createElement('button');
      b.className = 'sign' + (a ? (a.reported ? '' : ' guess') : ' lost');
      b.innerHTML = a ? `${escH(a.source)} · ${escH(a.wait)}` : '?';
      b.setAttribute('aria-label', a ? `${h.role} needs you: ${a.reported ? 'reported by' : 'guessed from'} ${a.source}, waiting ${a.wait}. Show why.` : `${h.role}: state unknown. Show last known.`);
      b.style.left = (r.left - vr.left + x * s) + 'px'; b.style.top = (r.top - vr.top + y * s) + 'px';
      b.onclick = () => show(h, isManager, v, T);
      box.appendChild(b);
    };
    v.houses.forEach((h, i) => { const lot = sc.L.lots[i]; if (lot) put(h, lot.x + 8, lot.y - 6, false); });
    put(v.manager, sc.L.hallX + 16, 12, true);
  }
'''
src = src.replace(old_show, new_show, 1)
# Guessed waits draw a dashed bubble.
src = src.replace("bubbles.forEach(([x, y, s]) => bubble(c, x, y, s));", "bubbles.forEach(([x, y, s, guess]) => { bubble(c, x, y, s); if (guess) for (let k = 0; k < 9; k += 2) { R(c, x + k, y, 1, 1, P.white); R(c, x + k, y + 8, 1, 1, P.white); R(c, x, y + k, 1, 1, P.white); R(c, x + 8, y + k, 1, 1, P.white); } });", 1)
src = src.replace("if (ICONS[st]) bubbles.push([lot.x + 4, lot.y - (st === 'blocked' ? 1 : 3) + (st === 'blocked' && blink(t, 200) ? -1 : 0), st]);",
                  "if (ICONS[st]) bubbles.push([lot.x + 4, lot.y - (st === 'blocked' ? 1 : 3) + (st === 'blocked' && blink(t, 200) ? -1 : 0), st, h.attention && !h.attention.reported]);", 1)
# Re-place signs whenever the view or size changes, and wire the demo bar.
src = src.replace("    fit();\n    if (reduce) draw(0);\n  }", "    fit();\n    if (reduce) draw(0);\n    requestAnimationFrame(placeSigns);\n  }", 1)
src = src.replace("  window.addEventListener('resize', fit);", "  window.addEventListener('resize', () => { fit(); placeSigns(); });\n  window.addEventListener('hashchange', () => { dialog.hidden = true; const n = decodeURIComponent(location.hash.slice(1)); if ([...pick.options].some(o => o.value === n)) pick.value = n; });\n  document.querySelectorAll('[data-demo]').forEach(b => b.addEventListener('click', () => { protoScenario = b.dataset.demo; dialog.hidden = true; location.hash = b.dataset.at; poll(); }));", 1)
src = src.replace("    setTimeout(poll, 3000);\n", "", 1)
for must in ['placeSigns()', 'protoScenario', 'id="d-why"', 'data-demo="down"']:
    assert must in src, must
open(sys.argv[1], 'w').write(src)
print('built', len(src))
