/* ---------- A click on a red "!" (#571, for #563) ----------
 * Each "!" (alarms.js, drawn by lifecycle.js) is its own click and keyboard
 * target, first in its house's Tab order (keyorder.js). It opens what the
 * agent is waiting on you about, with that first: a blocked house's issue in
 * #564's dialog (issuedialog.js) under its question and answer buttons; a
 * house with no issue, its own dialog; the manager, the hall's dialog, with
 * the issue its question names. Where to is alarmTarget's, tested under
 * node (tools/village-tests/alarms.test.mjs). */
import { registerHit } from './registry.js';
import { explain, openInfo } from './dialogs.js';
import { showIssueDialog } from './issuedialog.js';
import { alarmTarget, waitingWords } from './alarms.js';

/* The dialog of the house or hall under the "!", with what it waits on first. */
function waitingInfo(o, sc, theme) {
  const v = sc.v, t = alarmTarget(v, o.i);
  if (!t) return null;
  const who = o.i < 0 ? v.manager : v.houses[o.i];
  const info = explain(o.i < 0 ? { kind: 'hall' } : { kind: 'house', i: o.i }, sc, theme);
  if (!info) return null;
  return { ...info, meta: [waitingWords(who), ...(info.meta || [])], openIssue: t.kind === 'hall' ? t.issue : null };
}
registerHit({ kind: 'alarm',
  // What the keyboard target and a hover say; a click opens act()'s.
  dialog: (o, sc, theme) => { const info = waitingInfo(o, sc, theme); return info && { ...info, title: `Waiting on you · ${info.title}` }; },
  act(o, sc, theme) {
    const v = sc.v, t = alarmTarget(v, o.i);
    if (!t) return false;
    if (t.kind === 'issue') {
      const h = v.houses[t.i];
      showIssueDialog(v.project, t.n, { house: h, meta: [waitingWords(h)] });
      return true;
    }
    const info = waitingInfo(o, sc, theme);
    if (info) openInfo(info);
    return !!info;
  } });
