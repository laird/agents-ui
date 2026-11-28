// The chat panel's words and requests (src/web/village/chatwords.js, #566):
//   node --test tools/village-tests/chatwords.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { CHAT_KEYS, discussPrefill, inputUrl, keyStep, paneUrl, plainPane, sendSteps, streamUrl, trustPrompt } from '../../src/web/village/chatwords.js';

test("it reads and types through the web terminal's own endpoints", () => {
  assert.equal(paneUrl('agents ui', 'wt-1'), 'api/swarms/agents%20ui/agents/wt-1/pane');
  assert.equal(streamUrl('p', 'manager'), 'api/swarms/p/agents/manager/pane/stream');
  assert.equal(inputUrl('p', 'manager'), 'api/swarms/p/agents/manager/input');
});

test('sending types the line, then presses Enter', () => {
  assert.deepEqual(sendSteps('p', 'manager', 'go ahead'), [
    { url: 'api/swarms/p/agents/manager/input', body: { text: 'go ahead', enter: false } },
    { url: 'api/swarms/p/agents/manager/keys', body: { items: [{ type: 'key', key: 'Enter' }] } },
  ]);
  assert.deepEqual(CHAT_KEYS.map(k => k.key), ['Escape', 'C-c']);
  assert.deepEqual(keyStep('p', 'wt-2', 'C-c').body, { items: [{ type: 'key', key: 'C-c' }] });
});

test('the output is plain text: no colours, no cursor moves, no trailing blank lines', () => {
  assert.equal(plainPane('\x1b[1;32m● done\x1b[0m\r\n\x1b[2K> \x1b]0;title\x07ok\n\n  '), '● done\n> ok');
  assert.equal(plainPane(null), '');
});

test('a trust or consent prompt is spotted, colours and all, and ordinary output is not', () => {
  assert.ok(trustPrompt('…\n\x1b[1mDo you trust the files in this folder?\x1b[0m\n❯ 1. Yes, proceed'));
  assert.ok(trustPrompt('WARNING: Claude Code running in Bypass Permissions mode\n❯ 1. No, exit\n  2. Yes, I accept'));
  assert.ok(!trustPrompt('Running cargo test…\n1209 passed'));
  // Only what is on screen now: a prompt scrolled far up is gone.
  assert.ok(!trustPrompt('Do you trust the files in this folder?\n' + 'line\n'.repeat(40)));
});

test('"Discuss with manager" starts the line with the issue, its blocking label and its question', () => {
  const d = { number: 42, labels: ['P2', 'needs-design'], blocked_reason: '🚫 **Blocked**\n\nSSE or WebSocket?' };
  assert.equal(discussPrefill(d), 'About #42 (needs-design): 🚫 **Blocked** SSE or WebSocket? — ');
  assert.equal(discussPrefill({ number: 7, labels: [] }), 'About #7: — ');
  assert.equal(discussPrefill({ number: 8, labels: ['bug'], blocked_reason: 'why' }), 'About #8 (bug): why — ');
});
