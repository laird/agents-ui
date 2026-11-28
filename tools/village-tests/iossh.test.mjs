// "Open in herdr" on iOS, in an SSH app (src/web/village/iossh.js, #591 for #549):
//   node --test tools/village-tests/iossh.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { attachOf } from '../../src/web/village/herdrlink.js';
import { choiceOf, iosPlan, isIOS, parseChoice, sshCommand } from '../../src/web/village/iossh.js';

const v = { project: 'agents-ui', host: 'nuc-1' };
const herdr = attachOf(v, { role: 'wt-1', mux: 'herdr', pane: 'w1F:p1' });
const tmux = attachOf(v, { role: 'manager', mux: 'tmux', pane: 'claude-shop:0.0' });

test('an iPhone, an iPad, and an iPad asking for the desktop page are iOS; a Mac and Android are not', () => {
  assert.ok(isIOS({ userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)', platform: 'iPhone' }));
  assert.ok(isIOS({ userAgent: 'Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X)', platform: 'iPad' }));
  assert.ok(isIOS({ userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)', platform: 'MacIntel', maxTouchPoints: 5 }));
  assert.ok(!isIOS({ userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)', platform: 'MacIntel', maxTouchPoints: 0 }));
  assert.ok(!isIOS({ userAgent: 'Mozilla/5.0 (Linux; Android 14)', platform: 'Linux armv8l', maxTouchPoints: 5 }));
  assert.ok(!isIOS(undefined));
});

test('the command attaches over ssh -t: herdr agent attach (never focus, #576), or tmux attach', () => {
  assert.equal(sshCommand(herdr), 'ssh -t nuc-1 herdr agent attach w1F:p1');
  assert.equal(sshCommand(tmux), 'ssh -t nuc-1 tmux attach -t claude-shop:0.0');
});

test('nothing chosen: the browser view, with the command to copy', () => {
  assert.deepEqual(iosPlan(null, herdr), { kind: 'browser', cmd: 'ssh -t nuc-1 herdr agent attach w1F:p1' });
});

test('Blink runs the command through its x-callback URL with the key set in Blink', () => {
  const p = iosPlan({ app: 'blink', key: 'abc123' }, herdr);
  assert.equal(p.kind, 'app');
  assert.equal(p.url, 'blinkshell://run?key=abc123&cmd=ssh%20-t%20nuc-1%20herdr%20agent%20attach%20w1F%3Ap1');
  const u = new URL(p.url);
  assert.equal(u.searchParams.get('cmd'), 'ssh -t nuc-1 herdr agent attach w1F:p1', 'decodes back to the command');
  assert.equal(u.searchParams.get('key'), 'abc123');
});

test('Termius can only open the host, so the command is shown to paste', () => {
  assert.deepEqual(iosPlan({ app: 'termius' }, tmux),
    { kind: 'app', app: 'Termius', url: 'ssh://nuc-1', cmd: 'ssh -t nuc-1 tmux attach -t claude-shop:0.0' });
});

test('the remembered choice is checked on the way in: a bad key or an unknown app is no choice', () => {
  assert.deepEqual(parseChoice('{"app":"blink","key":"k-1_X"}'), { app: 'blink', key: 'k-1_X' });
  assert.deepEqual(parseChoice('{"app":"termius","key":"ignored"}'), { app: 'termius' });
  assert.equal(parseChoice('{"app":"blink","key":"a&cmd=rm"}'), null);
  assert.equal(parseChoice('{"app":"blink"}'), null);
  assert.equal(parseChoice('{"app":"prompt"}'), null);
  assert.equal(parseChoice('not json'), null);
  assert.equal(parseChoice(null), null);
});

test('what is stored for a choice, trimmed, and nothing for a key Blink would not give', () => {
  assert.equal(choiceOf('blink', '  abc  '), '{"app":"blink","key":"abc"}');
  assert.equal(choiceOf('termius'), '{"app":"termius"}');
  assert.equal(choiceOf('blink', ''), null);
  assert.equal(choiceOf('blink', 'a b'), null);
});
