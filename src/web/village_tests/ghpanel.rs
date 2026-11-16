//! The GitHub panel (#492): a sign by the hall over the server's cached
//! GitHub summary, its dialog a line per count, each linking a GitHub search
//! (the lines themselves are tested under node, `ghpanel.test.mjs`).

use super::*;

#[test]
fn the_github_sign_is_drawn_from_the_shared_summary_only() {
    let main = village_asset("main.js").unwrap().1;
    assert!(main.contains("import './ghpanel.js';"));
    let panel = village_asset("ghpanel.js").unwrap().1;
    // Nothing drawn, and no label, without the summary: the village draws as before.
    assert!(panel.contains("if (!v.github) return;"));
    assert!(panel.contains("if (!g) return [];"));
    // It reads `github` from /api/village and nothing else: no fetch of its own.
    assert!(!panel.contains("fetch("));
    let lines = village_asset("ghpanellines.js").unwrap().1;
    assert!(!lines.contains("document") && !lines.contains("fetch("), "DOM-free, so node can test it");
    // Links are built on the repo the issues' URLs name, never guessed.
    assert!(panel.contains("panelDialog(v.github, repoUrl(v), v.pipeline.integration, Date.now())"));
}

#[test]
fn the_github_sign_is_clickable_labelled_and_in_the_key() {
    let panel = village_asset("ghpanel.js").unwrap().1;
    assert!(panel.contains("tap(x, y, GH_SIGN.w, GH_SIGN.h, { kind: 'github' });"));
    assert!(panel.contains("registerHit({ kind: 'github'"));
    assert!(panel.contains("registerKey({ group: 'other'"));
    // The theme's own signpost (envart.js), so it fits all three themes.
    assert!(panel.contains("envSign(c, x, y, null, 'GITHUB');"));
    // Decorations keep off its ground.
    let layout = village_asset("layout.js").unwrap().1;
    assert!(layout.contains("...(v.github ? [[ghSignAt(hallX).x - 1, 18, GH_SIGN.w + 2, 30]] : [])"));
}
