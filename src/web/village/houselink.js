/* ---------- Opening a deep link ----------
 * After a poll draws the village a `#<project>&house=<name>` (or `&issue=`,
 * `&gate=`, `&build`, `&env=`, #579) link names, open what it names
 * (deeplink.js says what), once per link: closing it leaves it closed, and
 * a new link opens again. What is no longer there says so. Not on the
 * screensaver, which has no dialogs. */
import { state } from './state.js';
import { act } from './dialogs.js';
import { toast } from './panels.js';
import { hashName } from './saver.js';
import { linkTarget } from './deeplink.js';
import { showIssueDialog } from './issuedialog.js';

let linkOpened = '';
window.addEventListener('village:polled', e => {
  if (!e.detail || !e.detail.ok || state.screensaver || linkOpened === location.hash) return;
  const project = hashName(location.hash).split('@')[0], sc = state.scenes[project];
  const o = sc && sc.v && linkTarget(sc.v, location.hash);
  if (!o) return;
  linkOpened = location.hash;
  if (o.stale) toast(o.stale);
  else if (o.kind === 'issue') showIssueDialog(sc.v.project, o.n);
  else act(o, sc, state.T);
});
