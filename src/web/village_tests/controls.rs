//! Worker controls on the house dialog (#506, #567, for #488): Nudge,
//! Release, Stop and Restart, previewed, confirmed, and hidden on a
//! read-only server. The words and choices are tested under node
//! (controlwords.test.mjs).

use super::*;

#[test]
fn a_workers_house_dialog_offers_its_controls() {
    let lifecycle = village_asset("lifecycle.js").unwrap().1;
    // A development village's house passes who it is; a runtime's has none.
    assert!(lifecycle.contains("controls: runtime ? null : { project: v.project, role: h.role, name: houseName(v, h), issue: h.issue || null, house: h.id } };"));
    assert!(village_asset("index.html").unwrap().1.contains("<div id=\"d-ctl\"></div>"));
    assert!(village_asset("modules.js").unwrap().1.contains("import './controls.js';"));
    let controls = village_asset("controls.js").unwrap().1;
    // A fresh box per dialog; a slow answer for an earlier dialog is dropped.
    assert!(controls.contains("const token = ++ctlToken;"));
    assert!(controls.contains("if (token !== ctlToken) return;"));
    // Read-only: nothing shown (readonly.js, #565, shared with proposals.js and newissue.js).
    assert!(controls.contains("import { canWrite } from './readonly.js';"));
    assert!(controls.contains("if (!(await canWrite()) || token !== ctlToken) return;"));
    let readonly = village_asset("readonly.js").unwrap().1;
    assert!(readonly.contains("fetch('api/config')") && readonly.contains("!c.read_only"));
    // Each action is previewed first (GET), confirmed showing what is sent, then posted.
    assert!(controls.contains("previews[action] = await askServer(previewUrl(c.project, c.role, action));"));
    assert!(controls.contains("<pre class=\"sent\">${esc(ch.sent)}</pre>"));
    assert!(controls.contains("<button data-go=\"${esc(ch.action)}\">Send</button><button data-no=\"1\">Cancel</button>"));
    assert!(controls.contains("{ method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action: d.go }) }"));
    let words = village_asset("controlwords.js").unwrap().1;
    assert!(words.contains("if (status === 501) return \"This server can't do that here.\";"));
}

#[test]
fn a_controls_result_shows_on_the_house_at_once() {
    // #507: a successful control starts a local override, played by controlplay.js.
    let controls = village_asset("controls.js").unwrap().1;
    assert!(controls.contains("import { controlDone } from './controlplay.js';"));
    assert!(controls.contains("if (r && r.status >= 200 && r.status < 300) controlDone(c, d.go);"));
    assert!(village_asset("modules.js").unwrap().1.contains("import './controlplay.js';"));
    let play = village_asset("controlplay.js").unwrap().1;
    // Reduced motion: a toast, no override.
    assert!(play.contains("if (state.reduce) {") && play.contains("toast(`${c.project}: ${words}`);"));
    // A release carries the crate back to the docks as a trip; a stop darkens the house.
    assert!(play.contains("carryBack(sc, sc.L, sc.L.lots[i], h, v.backlog.length, v.backlog.length + 1, t);"));
    assert!(play.contains("if (fx && fx.dark) darkHouse(c, lot.x, lot.y);"));
    assert!(village_asset("trips.js").unwrap().1.contains("export function carryBack("));
    // The house and its barrel read the override; the model is never changed.
    let lifecycle = village_asset("lifecycle.js").unwrap().1;
    assert!(lifecycle.contains("rest = fxRest(fxOn(v.project, h, t), restOf(h, wall));"));
    let barrels = village_asset("barrels.js").unwrap().1;
    assert!(barrels.contains("fxContext(fxOn(v.project, h, t), h.context_pct)"));
    // One table keyed by action: nudge, release, restart and stop (#505, #567).
    let fx = village_asset("controlfx.js").unwrap().1;
    for action in ["nudge: {", "release: {", "restart: {", "stop: {"] {
        assert!(fx.contains(action), "{action}");
    }
}

/// "Open in herdr" (#575, for #549): first in an agent's dialog, the web
/// terminal kept as "View in browser", and a fallback when no handler answers.
/// The link and the commands are tested under node (herdrlink.test.mjs).
#[test]
fn an_agents_dialog_opens_it_in_herdr_with_a_web_terminal_fallback() {
    let lifecycle = village_asset("lifecycle.js").unwrap().1;
    assert!(lifecycle.contains("links: withAttach(v, h, [...(h.links || []), issue,"), "the house");
    assert!(lifecycle.contains("links: withAttach(v, m, [...(m.links || []), link('Swarm view', swarm)"), "the manager's hall");
    assert!(lifecycle.contains("links: withAttach(v, h, [sessionOf(h),"), "the agent walking about");
    let link = village_asset("herdrlink.js").unwrap().1;
    assert!(link.contains("const url = 'agents-ui://attach?' + Object.entries(q)"));
    assert!(link.contains("s ? { label: 'View in browser', url: s.url } : null"));
    let dialogs = village_asset("dialogs.js").unwrap().1;
    // Handed to the handler in the same tab (no new tab), each other link in a new one.
    assert!(dialogs.contains("? `<a href=\"${esc(l.url)}\" data-attach=\"${i}\">${esc(l.label)}</a>`"));
    // No handler: the page keeps its focus, so after the wait it opens the web
    // terminal and shows copyable commands; what happened is remembered.
    assert!(dialogs.contains("window.addEventListener('blur', on); document.addEventListener('visibilitychange', on);"));
    assert!(dialogs.contains("attachMemo(answered ? 'yes' : 'none');"));
    assert!(dialogs.contains("if (a.web) window.open(a.web, '_blank', 'noopener');"));
    assert!(dialogs.contains("<button type=\"button\" data-copy=\"${esc(c)}\">Copy</button>"));
    assert!(dialogs.contains("if (plan === 'fallback') { e.preventDefault(); attachFallback(a, box, true); return; }"));
    assert!(village_asset("village.css").unwrap().1.contains(".dialog .attach-cmds {"));
}

/// "Open in herdr" on iOS (#591): no agents-ui:// handler there, so the click
/// opens the chosen SSH app (or the browser until one is chosen) and never
/// waits on a handler. The links and the choice are tested under node
/// (iossh.test.mjs).
#[test]
fn on_ios_an_agents_pane_opens_in_the_chosen_ssh_app() {
    let dialogs = village_asset("dialogs.js").unwrap().1;
    assert!(dialogs.contains("if (isIOS(navigator)) { e.preventDefault(); iosAttach(a, box, true); return; }"));
    assert!(dialogs.contains("if (p.kind === 'app') window.location.href = p.url; else if (a.web) window.open(a.web, '_blank', 'noopener');"));
    assert!(dialogs.contains("iosMemo(null); iosAttach(a, box, false);"), "the choice can be changed");
    let ios = village_asset("iossh.js").unwrap().1;
    assert!(ios.contains("`blinkshell://run?key=${encodeURIComponent(choice.key)}&cmd=${encodeURIComponent(cmd)}`"));
    assert!(village_asset("village.css").unwrap().1.contains(".dialog .attach-cmds .ios-key {"));
}
