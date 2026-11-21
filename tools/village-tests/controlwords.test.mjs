// Worker controls on the house dialog (src/web/village/controlwords.js, #506):
//   node --test tools/village-tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { CONTROLS, confirmWords, controlAnswer, controlChoices, controlUrl, previewUrl } from '../../src/web/village/controlwords.js';

test('the controls live at the agent\'s control path, its parts escaped', () => {
  assert.equal(controlUrl('shop', 'worker-2'), '/api/swarms/shop/agents/worker-2/control');
  assert.equal(controlUrl('my shop', 'w/1'), '/api/swarms/my%20shop/agents/w%2F1/control');
  assert.equal(previewUrl('shop', 'worker-2', 'nudge'), '/api/swarms/shop/agents/worker-2/control?action=nudge');
  assert.deepEqual(CONTROLS.map(c => c.action), ['nudge', 'release']);
});

test('a ready action is offered with what it sends; one that can\'t run now is greyed with why', () => {
  const choices = controlChoices({
    nudge: { status: 200, body: { ok: true, sent: '/autocoder:fix-loop' } },
    release: { status: 409, body: { error: 'that worker holds no issue' } },
  });
  assert.deepEqual(choices, [
    { action: 'nudge', label: 'Nudge', ready: true, sent: '/autocoder:fix-loop' },
    { action: 'release', label: 'Release issue', ready: false, why: 'that worker holds no issue' },
  ]);
});

test('an action the server has no worker for, can\'t do, or couldn\'t be asked about is left out', () => {
  assert.deepEqual(controlChoices({ nudge: { status: 404, body: {} }, release: { status: 404, body: {} } }), []);
  assert.deepEqual(controlChoices({ nudge: { status: 501, body: {} }, release: null }), []);
  assert.deepEqual(controlChoices({}), []);
  assert.deepEqual(controlChoices(null), []);
  assert.equal(controlChoices({ release: { status: 409, body: {} } })[0].why, 'Not now');
});

test('the confirmation says what will happen before showing what is sent', () => {
  assert.equal(confirmWords('nudge', 'wt-2'), 'Nudge wt-2? This is typed into its session:');
  assert.equal(confirmWords('release', 'wt-2', 42), 'Release #42 from wt-2? This is posted on the issue:');
  assert.equal(confirmWords('release', 'wt-2', null), 'Release its issue from wt-2? This is posted on the issue:');
});

test('what the dialog says after the server answers', () => {
  assert.equal(controlAnswer(200, { ok: true }, 'nudge'), 'Nudged: its loop is starting.');
  assert.equal(controlAnswer(200, { ok: true, number: 42 }, 'release', null), 'Released #42: the crate goes back to the docks.');
  assert.equal(controlAnswer(200, null, 'release', 7), 'Released #7: the crate goes back to the docks.');
  assert.equal(controlAnswer(200, null, 'release', null), 'Released its issue: the crate goes back to the docks.');
  assert.equal(controlAnswer(403, {}, 'nudge'), 'This dashboard is read-only: use the TUI.');
  assert.equal(controlAnswer(404, {}, 'nudge'), 'That worker is gone.');
  assert.equal(controlAnswer(501, {}, 'restart'), "This server can't do that to a worker.");
  assert.equal(controlAnswer(409, { error: 'that worker is busy: wait until it is idle' }, 'nudge'), 'that worker is busy: wait until it is idle');
  assert.equal(controlAnswer(502, {}, 'release'), 'Could not release it (502)');
});

test('#540: autocoder-ctl does it: the call is confirmed and its answer shown', () => {
  const [ch] = controlChoices({ release: { status: 200, body: { ok: true, via: 'autocoder-ctl', sent: 'autocoder-ctl release 42' } } });
  assert.deepEqual(ch, { action: 'release', label: 'Release issue', ready: true, sent: 'autocoder-ctl release 42', via: 'autocoder-ctl' });
  assert.equal(confirmWords('release', 'wt-2', 42, ch.via), 'Release #42 from wt-2? autocoder runs:');
  assert.equal(confirmWords('nudge', 'wt-2', null, 'autocoder-ctl'), 'Nudge wt-2? autocoder runs:');
  assert.equal(controlAnswer(200, { ok: true, via: 'autocoder-ctl', said: 'released #42', number: 42 }, 'release', 42),
    'Released #42: the crate goes back to the docks. autocoder-ctl: released #42');
  assert.equal(controlAnswer(200, { ok: true, via: 'autocoder-ctl', said: '' }, 'nudge'), 'Nudged: its loop is starting.');
  assert.equal(controlAnswer(409, { error: 'autocoder-ctl: the pane is in /x, not /w', via: 'autocoder-ctl' }, 'nudge'), 'autocoder-ctl: the pane is in /x, not /w');
});
