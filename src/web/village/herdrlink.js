/* ---------- "Open in herdr": an agent's own pane, in a real client (#575, for #549) ----------
 * The village names where each agent's pane lives (#574: the village's
 * `host`, each agent's `mux` and `pane`), so an agent dialog can open that
 * pane in herdr (or tmux) on this computer instead of the web terminal. The
 * link is agents-ui://attach?…, which `agents-tui open` handles once it is
 * registered; without a handler the page falls back to the web terminal and
 * shows the commands to run by hand. No DOM, so node tests it
 * (tools/village-tests/herdrlink.test.mjs); dialogs.js wires the click. */

/* The web terminal's link, as the server hands it to the page (relative since #552). */
export const SESSION = /^\.?\/#\/swarm\/[^/]+\/agent\//;
/* Only names that can't carry anything else end up in a link or a command:
 * a host name, a herdr pane id (w1F:p1), a tmux target (claude-shop:0.0). */
const HOST = /^[A-Za-z0-9][A-Za-z0-9.-]*$/;
const PANE = { herdr: /^w[0-9A-Za-z]+:p[0-9]+$/, tmux: /^[A-Za-z0-9._-]+:[0-9]+\.[0-9]+$/ };

/* Where house (or manager) `h` of village `v` can be attached to:
 * { mux, url, cmds } — the agents-ui:// link and the commands that do the
 * same by hand — or null when the village doesn't say where its pane is. */
export function attachOf(v, h) {
  const host = v && v.host, mux = h && h.mux, pane = h && h.pane;
  if (!host || !HOST.test(host) || !PANE[mux] || !PANE[mux].test(pane || '')) return null;
  const q = { host, mux, pane, project: v.project || '', role: h.role || '' };
  const url = 'agents-ui://attach?' + Object.entries(q).map(([k, x]) => `${k}=${encodeURIComponent(x)}`).join('&');
  const cmds = mux === 'herdr'
    ? [`herdr --remote ${host}`, `herdr --machine ${host} agent focus ${pane}`]
    : [`ssh -t ${host} tmux attach -t ${pane.split(':')[0]} \\; select-pane -t ${pane}`];
  return { mux, url, cmds };
}

/* An agent dialog's links with "Open in herdr" first and the web terminal
 * kept second as "View in browser". Unchanged when there is nowhere to attach. */
export function withAttach(v, h, list) {
  const a = attachOf(v, h);
  if (!a) return list;
  const s = list.find(l => l && SESSION.test(l.url));
  return [{ label: a.mux === 'herdr' ? 'Open in herdr' : 'Open in tmux', url: a.url, attach: { ...a, web: s ? s.url : null } },
    s ? { label: 'View in browser', url: s.url } : null, ...list.filter(l => l !== s)];
}

/* Whether a handler answered an agents-ui:// link before, kept in
 * localStorage so a repeat visit doesn't wait out the timeout again. */
export const HANDLER_KEY = 'agents-ui.attach-handler', ATTACH_WAIT = 1500;
/* What a click does, given what the page remembers: 'open' (a handler
 * answered before: hand it the link), 'fallback' (none did: the web terminal
 * straight away), or 'try' (unknown: hand it over, and fall back if nothing
 * takes the page's focus within ATTACH_WAIT). */
export const attachPlan = remembered => (remembered === 'yes' ? 'open' : remembered === 'none' ? 'fallback' : 'try');
