// "Open in herdr" (src/web/village/herdrlink.js, #575 for #549):
//   node --test tools/village-tests/herdrlink.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { SESSION, attachOf, attachPlan, withAttach } from '../../src/web/village/herdrlink.js';

const v = { project: 'agents-ui', host: 'nuc-1' };
const herdr = { role: 'wt-1', mux: 'herdr', pane: 'w1F:p1' };
const web = { label: 'Agent session', url: './#/swarm/agents-ui/agent/wt-1' };
const issue = { label: 'GitHub issue #7', url: 'https://github.com/acme/shop/issues/7' };

test('a herdr agent links to agents-ui://attach with its host, mux, pane, project and role', () => {
  const a = attachOf(v, herdr);
  assert.equal(a.url, 'agents-ui://attach?host=nuc-1&mux=herdr&pane=w1F%3Ap1&project=agents-ui&role=wt-1');
  assert.deepEqual(a.cmds, ['herdr --remote nuc-1', 'herdr --machine nuc-1 agent focus w1F:p1']);
  assert.equal(new URL(a.url).searchParams.get('pane'), 'w1F:p1', 'decodes back to the pane');
});

test('a tmux agent attaches over ssh to its session and selects its pane', () => {
  const a = attachOf(v, { role: 'manager', mux: 'tmux', pane: 'claude-shop:0.0' });
  assert.deepEqual(a.cmds, ['ssh -t nuc-1 tmux attach -t claude-shop \\; select-pane -t claude-shop:0.0']);
  assert.match(a.url, /mux=tmux&pane=claude-shop%3A0\.0/);
});

test('nowhere to attach: no host, no pane, or a pane or host that could carry anything else', () => {
  assert.equal(attachOf({ project: 'p' }, herdr), null);
  assert.equal(attachOf(v, { role: 'wt-1' }), null);
  assert.equal(attachOf(v, { ...herdr, pane: 'w1F:p1; rm -rf ~' }), null);
  assert.equal(attachOf({ ...v, host: 'nuc-1 && curl x' }, herdr), null);
  assert.equal(attachOf(v, { ...herdr, mux: 'screen' }), null);
});

test('"Open in herdr" comes first; the web terminal stays second as "View in browser"', () => {
  const l = withAttach(v, herdr, [web, issue]);
  assert.deepEqual(l.map(x => x && x.label), ['Open in herdr', 'View in browser', 'GitHub issue #7']);
  assert.equal(l[1].url, web.url);
  assert.equal(l[0].attach.web, web.url, 'the fallback opens the web terminal');
  assert.equal(withAttach(v, { ...herdr, mux: 'tmux', pane: 'claude-x:0.1' }, [web])[0].label, 'Open in tmux');
});

test('without a place to attach the links are left as they were', () => {
  const list = [web, issue];
  assert.equal(withAttach({ project: 'p' }, herdr, list), list);
});

test('the web terminal link is found in the relative form the server hands out (#552)', () => {
  assert.ok(SESSION.test('./#/swarm/shop/agent/worker-2'));
  assert.ok(SESSION.test('/#/swarm/shop/agent/worker-2'));
  assert.ok(!SESSION.test('https://github.com/acme/shop/issues/7'));
});

test('a remembered answer decides the click: hand over, fall back at once, or try and wait', () => {
  assert.equal(attachPlan('yes'), 'open');
  assert.equal(attachPlan('none'), 'fallback');
  assert.equal(attachPlan(null), 'try');
});
