//! Worker controls on the house dialog (#506, for #488): Nudge and Release,
//! previewed, confirmed, and hidden on a read-only server. The words and
//! choices are tested under node (controlwords.test.mjs).

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
    // Read-only: nothing shown.
    assert!(controls.contains("fetch('api/config')") && controls.contains("!c.read_only"));
    assert!(controls.contains("if (!(await canWrite()) || token !== ctlToken) return;"));
    // Each action is previewed first (GET), confirmed showing what is sent, then posted.
    assert!(controls.contains("previews[action] = await askServer(previewUrl(c.project, c.role, action));"));
    assert!(controls.contains("<pre class=\"sent\">${esc(ch.sent)}</pre>"));
    assert!(controls.contains("<button data-go=\"${esc(ch.action)}\">Send</button><button data-no=\"1\">Cancel</button>"));
    assert!(controls.contains("{ method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action: d.go }) }"));
    let words = village_asset("controlwords.js").unwrap().1;
    assert!(words.contains("if (status === 501) return \"This server can't do that to a worker.\";"));
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
    // One table keyed by action, restart and stop ready for #505.
    let fx = village_asset("controlfx.js").unwrap().1;
    for action in ["nudge: {", "release: {", "restart: {", "stop: {"] {
        assert!(fx.contains(action), "{action}");
    }
}
