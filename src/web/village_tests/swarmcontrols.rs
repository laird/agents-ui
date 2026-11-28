//! Swarm controls on the castle (hall) dialog (#567): Stop swarm, Resume,
//! Add worker and Switch agent, confirmed before Stop/Resume and hidden on
//! a read-only server. The words are tested under node
//! (swarmwords.test.mjs); the manager's own Nudge/Restart ride the existing
//! house-dialog machinery (controls.test.mjs, controlwords.test.mjs).

use super::*;

#[test]
fn the_hall_dialog_offers_manager_and_swarm_controls() {
    let lifecycle = village_asset("lifecycle.js").unwrap().1;
    assert!(lifecycle.contains("managerControls: runtime ? null : { project: v.project, role: m.role, name: 'the manager', issue: null },"));
    assert!(lifecycle.contains("swarmControls: runtime ? null : { project: v.project },"));
    assert!(village_asset("index.html").unwrap().1.contains("<div id=\"d-swarmctl\"></div>"));
    assert!(village_asset("modules.js").unwrap().1.contains("import './swarmcontrols.js';"));
}

#[test]
fn the_manager_gets_only_nudge_and_restart() {
    let words = village_asset("controlwords.js").unwrap().1;
    // Derived from CONTROLS (not hand-copied), so a label change can't drift (#567 review).
    assert!(words.contains("export const MANAGER_CONTROLS = CONTROLS.filter(({ action }) => action === 'nudge' || action === 'restart');"));
    let controls = village_asset("controls.js").unwrap().1;
    // Read-only check shared with swarmcontrols.js (readonly.js, #565, so the two boxes agree).
    assert!(controls.contains("import { canWrite } from './readonly.js';"));
    assert!(controls.contains("export async function askServer("));
    assert!(controls.contains("if (info.controls) fillControls(box, info.controls, token, CONTROLS);"));
    assert!(controls.contains("else if (info.managerControls) fillControls(box, info.managerControls, token, MANAGER_CONTROLS);"));
}

#[test]
fn swarm_controls_hit_the_dashboards_own_routes() {
    let words = village_asset("swarmwords.js").unwrap().1;
    assert!(words.contains("api/swarms/${encodeURIComponent(project)}"));
    assert!(words.contains("${swarmUrl(project)}/resume"));
    assert!(words.contains("${swarmUrl(project)}/workers"));
    let controls = village_asset("swarmcontrols.js").unwrap().1;
    assert!(controls.contains("import { askServer } from './controls.js';"));
    assert!(controls.contains("import { canWrite } from './readonly.js';"));
    // Stop and Resume ask first, showing the question before sending.
    assert!(controls.contains("swarmConfirmWords(action, project)"));
    assert!(controls.contains("method: action === 'stop' ? 'DELETE' : 'POST'"));
    // Switch agent (PATCH) and Add worker (POST) send at once, as the dashboard does.
    assert!(controls.contains("method: 'PATCH'"));
    assert!(controls.contains("agent_type: mine.type"));
    // Read-only: nothing shown.
    assert!(controls.contains("if (!(await canWrite()) || t !== token) return;"));
}
