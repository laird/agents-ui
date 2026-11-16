/* ---------- Opening a linked house ----------
 * After a poll draws the village a `#<project>&house=<name>` link names, open
 * that house's dialog (deeplink.js says which), once per link: closing it
 * leaves it closed, and a new link opens again. Not on the screensaver,
 * which has no dialogs. */
import { state } from './state.js';
import { act } from './dialogs.js';
import { hashName } from './saver.js';
import { houseParam, houseTarget } from './deeplink.js';

let linkOpened = '';
window.addEventListener('village:polled', e => {
  if (!e.detail || !e.detail.ok || state.screensaver || linkOpened === location.hash) return;
  const name = houseParam(location.hash);
  if (!name) return;
  const sc = state.scenes[hashName(location.hash).split('@')[0]];
  const o = sc && houseTarget(sc.v, name);
  if (!o) return;
  linkOpened = location.hash;
  act(o, sc, state.T);
});
