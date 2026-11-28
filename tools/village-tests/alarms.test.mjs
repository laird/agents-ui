// Trouble is the loudest thing on the map (src/web/village/alarms.js, #439),
// measured by drawing on a canvas that records every pixel rectangle:
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';

// state.js reads the reduced-motion preference when it loads.
globalThis.window = { matchMedia: () => ({ matches: false }) };
const { state } = await import('../../src/web/village/state.js');
const { ALARM, ALARM_TAP, BUBBLE, RED, RED_D, alarm, alarmTap, alarmTarget, boardAlert, boardFrame, failed, lamp, ring, waitingWords } = await import('../../src/web/village/alarms.js');
const { PHONE_MIN } = await import('../../src/web/village/fitscale.js');
const { bubble } = await import('../../src/web/village/scenery.js');

/* A canvas that records the box every drawing covers, in scene pixels, and
 * its colours. It follows save/translate/scale, as the 2× marks draw inside
 * detail() (#456). */
function recorder() {
  let m = [1, 1, 0, 0];
  const stack = [];
  const c = {
    rects: [], fillStyle: null,
    save() { stack.push([...m]); }, restore() { m = stack.pop(); },
    translate(x, y) { m = [m[0], m[1], m[2] + m[0] * x, m[3] + m[1] * y]; },
    scale(sx, sy) { m = [m[0] * sx, m[1] * sy, m[2], m[3]]; },
    fillRect(x, y, w, h) { this.rects.push({ x: m[0] * x + m[2], y: m[1] * y + m[3], w: m[0] * w, h: m[1] * h, col: this.fillStyle }); },
  };
  c.box = () => {
    const xs = c.rects.flatMap(r => [r.x, r.x + r.w]), ys = c.rects.flatMap(r => [r.y, r.y + r.h]);
    return { w: Math.max(...xs) - Math.min(...xs), h: Math.max(...ys) - Math.min(...ys) };
  };
  c.colours = () => new Set(c.rects.map(r => r.col));
  c.red = () => c.colours().has(RED) || c.colours().has(RED_D); // either phase of a pulse
  return c;
}
const draw = (fn, ...args) => { const c = recorder(); fn(c, 0, 0, ...args); return c; };

test('waiting on you is drawn larger than the done bubble', () => {
  const done = draw(bubble, 'done').box(), wait = draw(alarm, 0).box();
  assert.deepEqual(done, BUBBLE, 'BUBBLE names the quiet bubble\'s real size');
  assert.ok(wait.w > done.w && wait.h > done.h, `the "!" (${wait.w}x${wait.h}) outsizes the ✓ (${done.w}x${done.h})`);
  assert.ok(wait.h >= ALARM.h, 'the sign is at least ALARM tall');
  assert.ok(draw(alarm, 0).red() && draw(alarm, 380).red());
});

test('with reduced motion the sign holds still but keeps its size', () => {
  state.reduce = true;
  try {
    const a = draw(alarm, 0), b = draw(alarm, 190);
    assert.deepEqual(a.rects, b.rects, 'no pulse');
    assert.ok(a.box().h >= ALARM.h && a.colours().has(RED), 'still large and lit');
  } finally {
    state.reduce = false;
  }
});

test('a failed agent is red and crossed, a waiting one gets a ring', () => {
  const f = draw(failed);
  assert.deepEqual(f.box(), BUBBLE);
  assert.ok(f.colours().has(RED));
  assert.ok(!draw(bubble, 'stalled').colours().has(RED), 'the quiet stalled bubble it replaces was not red');
  const r = recorder(); ring(r, 50, 50, 0);
  assert.ok(r.box().w >= 20, 'the ring spans the door');
});

test("the board's alert follows its note count", () => {
  assert.equal(boardAlert([]), null, 'an empty board stays plain');
  assert.equal(boardAlert(undefined), null);
  assert.deepEqual(boardAlert([{ number: 1 }, { number: 2 }, { number: 3 }]), { n: 3 });
  const framed = draw(boardFrame, 3, 0);
  assert.ok(framed.red());
  assert.ok(framed.box().w > 18, 'the frame sits outside the 18-wide board face');
});

test('the gatehouse lamp is red', () => {
  assert.ok(draw(lamp, 0).red() && draw(lamp, 300).red());
});

test('every mark is drawn on the 2× grid, in half pixels', () => {
  const marks = [draw(bubble, 'done'), draw(alarm, 0), draw(failed), draw(boardFrame, 3, 0), draw(lamp, 0)];
  const r = recorder(); ring(r, 50, 50, 0); marks.push(r);
  for (const c of marks) {
    assert.ok(c.rects.length > 0);
    assert.ok(c.rects.some(q => [q.x, q.y, q.w, q.h].some(v => v % 1 !== 0)), 'something is finer than a scene pixel');
  }
});

/* ---------- A click on a "!" (#571) ---------- */
test('each "!" is a click of at least 24 CSS px, centred on its sign, down to a map shrunk to 3/4', () => {
  for (const scale of [0.75, 1, 1.5, PHONE_MIN, 3, 4]) {
    const r = alarmTap(100, 40, scale);
    assert.ok(r.w * scale >= ALARM_TAP && r.h * scale >= ALARM_TAP, `${r.w}x${r.h} at ${scale}x`);
    // It covers the whole sign and its tail.
    assert.ok(r.x <= 100 && r.y <= 40 && r.x + r.w >= 100 + ALARM.w && r.y + r.h >= 40 + ALARM.h + 2, JSON.stringify(r));
    // Centred: as much on each side, to a scene pixel.
    assert.ok(Math.abs((100 - r.x) - (r.x + r.w - 100 - ALARM.w)) <= 1);
  }
  // Shrunk further, it stops growing rather than cover the next house.
  const tiny = alarmTap(100, 40, 0.1);
  assert.ok(tiny.w <= 32 && tiny.h <= 32);
});

test("a blocked house's \"!\" opens its issue; a house with none, the house; the manager's, the hall and the issue it asks about", () => {
  const v = { manager: { status: 'blocked', question: { prompt: 'Merge #42 now?' } }, houses: [{ status: 'blocked', issue: 7 }, { status: 'blocked' }] };
  assert.deepEqual(alarmTarget(v, 0), { kind: 'issue', i: 0, n: 7 });
  assert.deepEqual(alarmTarget(v, 1), { kind: 'house', i: 1 });
  assert.deepEqual(alarmTarget(v, -1), { kind: 'hall', issue: 42 });
  assert.deepEqual(alarmTarget({ ...v, manager: { status: 'blocked' } }, -1), { kind: 'hall', issue: null });
  // A platform's workloads aren't issues: the house's own dialog.
  assert.deepEqual(alarmTarget({ ...v, plane: 'runtime' }, 0), { kind: 'house', i: 0 });
  assert.equal(alarmTarget(v, 5), null);
});

test('what it waits on comes first, unless its question is already there to answer', () => {
  assert.equal(waitingWords({ question: { prompt: 'Allow Bash?' }, answer_url: '/a' }), null);
  assert.equal(waitingWords({ question: { prompt: 'Allow Bash?' } }), 'Waiting on you: Allow Bash?');
  assert.equal(waitingWords({ action: 'needs permission' }), 'Waiting on you: needs permission. Answer in its session.');
  assert.equal(waitingWords({ action: 'blocked' }), 'Waiting on you. Answer in its session.');
});
