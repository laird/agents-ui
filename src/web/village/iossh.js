/* ---------- "Open in herdr" on iOS: an SSH app instead (#591, for #549) ----------
 * iOS has no agents-ui:// handler, so an agent's pane opens in an SSH app:
 * Blink runs the attach command itself (blinkshell://run, with the key set
 * in Blink's Settings > x-callback-url), Termius only opens ssh://<host>, so
 * the page shows the command to paste. The browser view stays the default
 * until an app is chosen; the choice (and Blink's key) is remembered in
 * localStorage. No DOM, so node tests it (tools/village-tests/iossh.test.mjs);
 * dialogs.js wires the click. */

export const IOS_SSH_KEY = 'agents-ui.ios-ssh';
/* Blink's x-callback key: what Blink generates, nothing that could carry more. */
const BLINK_KEY = /^[A-Za-z0-9_-]{1,64}$/;

/* An iPhone, iPod or iPad, including an iPad that reports itself as a Mac
 * (iPadOS 13+ asks for desktop pages: MacIntel with a touch screen). */
export function isIOS(nav) {
  if (!nav) return false;
  return /iPad|iPhone|iPod/.test(nav.userAgent || '')
    || (nav.platform === 'MacIntel' && (nav.maxTouchPoints || 0) > 1);
}

/* The command an SSH app runs to attach to the pane `a` (attachOf's
 * { mux, host, pane }): `herdr agent attach`, never `agent focus`, which
 * moves the pointer every attached client shares (#576). */
export function sshCommand(a) {
  return a.mux === 'herdr'
    ? `ssh -t ${a.host} herdr agent attach ${a.pane}`
    : `ssh -t ${a.host} tmux attach -t ${a.pane}`;
}

/* What the page remembers, checked again on the way in: { app: 'blink', key }
 * or { app: 'termius' }, or null (nothing chosen, or anything else). */
export function parseChoice(raw) {
  let c;
  try { c = JSON.parse(raw || 'null'); } catch (e) { return null; }
  if (!c || typeof c !== 'object') return null;
  if (c.app === 'termius') return { app: 'termius' };
  if (c.app === 'blink' && BLINK_KEY.test(c.key || '')) return { app: 'blink', key: c.key };
  return null;
}
/* The choice to store for app `app` (and Blink's `key`), or null when the key
 * is not one Blink would give. */
export function choiceOf(app, key) {
  const c = parseChoice(JSON.stringify({ app, key: (key || '').trim() }));
  return c ? JSON.stringify(c) : null;
}

/* What a click on "Open in herdr" does on iOS, given the remembered choice:
 * { kind: 'browser' } (none chosen: the web terminal, and an offer to pick an
 * app), or { kind: 'app', app, url, cmd } (open `url`; `cmd` is the command,
 * shown to copy). */
export function iosPlan(choice, a) {
  const cmd = sshCommand(a);
  if (!choice) return { kind: 'browser', cmd };
  if (choice.app === 'blink') {
    return { kind: 'app', app: 'Blink', cmd,
      url: `blinkshell://run?key=${encodeURIComponent(choice.key)}&cmd=${encodeURIComponent(cmd)}` };
  }
  return { kind: 'app', app: 'Termius', cmd, url: `ssh://${a.host}` };
}
