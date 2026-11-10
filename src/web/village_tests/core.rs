//! The page's core: its files and size, the replay bundle, the registry, the hit test,
//! dialogs, links, keyboard targets, and the DOM-free node tests (tools/village-tests).

use super::*;

/// The village stays in files a reader (or an agent's context) can take in
/// whole: about 400 lines each. Split a module that grows past that.
#[test]
fn village_files_stay_small() {
    for (path, _, body) in VILLAGE_ASSETS {
        let lines = body.lines().count();
        assert!(lines <= 420, "src/web/village/{path} is {lines} lines; split it (CLAUDE.md, Adding to the village)");
    }
}

/// The replay page (tools/mock/replay-build.py) bundles these modules into
/// one script, so a change here that it cannot bundle fails here (#392).
#[test]
fn replay_build_bundles_the_village_modules() {
    let root = env!("CARGO_MANIFEST_DIR");
    let out = std::process::Command::new("python3")
        .args(["-m", "unittest", "discover", "-s", &format!("{root}/tools/mock"), "-p", "test_replay_build.py"])
        .output()
        .expect("run tools/mock/test_replay_build.py");
    assert!(
        out.status.success(),
        "tools/mock/test_replay_build.py failed:\n{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn village_core_functions_know_no_feature() {
    let body = &village_source();
    // paint() walks the layers, placeLabels() the label lists,
    // explain()/act() the hits, and the hit test only the rectangles
    // paint() recorded: none of them names a feature.
    for name in ["paint", "hitAt", "objAt", "explain", "act", "placeLabels", "placeOverlays"] {
        let f = js_fn(body, name);
        for word in [
            "house", "crate", "porter", "ship", "harbour", "quay", "stop", "banner", "barge", "feature", "docks", "board", "hall",
            "manager", "backlog", "pipeline", "lay.", "state.T.", "LW", "incidents", "'world'",
        ] {
            assert!(!f.contains(word), "{name}() knows about `{word}`: {f}");
        }
    }
    assert!(js_fn(body, "paint").contains("for (const layer of LAYERS) layer.draw(c, scene, t);"));
    // Each feature registers itself: its layers, hits and labels.
    for (file, registers) in [
        ("layout.js", &["registerLayer("][..]),
        ("lifecycle.js", &["registerLayer(", "registerHit(", "registerLabels("][..]),
        ("ship.js", &["registerLayer(", "registerHit(", "registerLabels("][..]),
        ("gatehouse.js", &["registerLayer(", "registerHit(", "registerLabels("][..]),
        ("features.js", &["registerLayer(", "registerHit(", "registerOverlay("][..]),
        ("envrow.js", &["registerLayer(", "registerHit(", "registerLabels(", "registerOverlay("][..]),
        ("voyages.js", &["registerLayer(", "registerHit("][..]),
        ("dialogs.js", &["registerOverlay("][..]),
        ("labels.js", &["registerOverlay("][..]),
    ] {
        let src = village_asset(file).unwrap().1;
        for r in registers {
            assert!(src.contains(r), "{file} does not call {r}");
        }
    }
}

#[test]
fn village_html_hit_tests_what_it_draws() {
    let body = &village_source();
    // paint() records a rectangle per sprite; hit() reads them back, topmost first.
    assert!(body.contains("const tap = (x, y, w, h, o) =>"));
    assert!(body.contains("sc.hits = hits; hits = null;"));
    assert!(body.contains("function hitAt(sc, x, y)"));
    assert!(body.contains("for (let k = hs.length - 1; k >= 0; k--)"));
    // No hand-copied rectangles left in objAt(), which clicks and hovers share.
    let hit = &body[body.find("function objAt(e)").unwrap()..];
    let hit = &hit[..hit.find("\n}\n").unwrap()];
    assert!(!hit.contains("lay."), "objAt() should not know the layout: {hit}");
    // Every kind of object on the map is recorded where it is drawn...
    for kind in [
        "'river'", "'docks'", "'canal'", "'quay'", "'hall'", "'board'", "'sub'", "'crate'",
        "'house'", "'person'", "'stop'", "'harbour'", "'porter'", "'barge'", "'banner'",
    ] {
        assert!(body.contains(&format!("{{ kind: {kind}")), "no tap for {kind}");
    }
    // ...and the ones with a dialog of their own say what they are, each
    // registered by the module that draws it.
    for kind in ["river", "docks", "board", "hall", "house", "person", "sub", "porter", "supply", "crate", "quay", "stop", "harbour", "ship", "canal"] {
        let at = body.find(&format!("registerHit({{ kind: '{kind}',")).unwrap_or_else(|| panic!("no hit registered for {kind}"));
        assert!(body[at..at + 80].contains("dialog"), "no dialog for {kind}");
    }
    // A flag only follows (the strip's Details opens the feature); a barge opens its feature.
    assert!(body.contains("registerHit({ kind: 'banner', key: false, act(o, sc) { toggleFollow(sc.v.project, o.f); return true; } });"));
    assert!(body.contains("registerHit({ kind: 'barge', act(o, sc) { const v = sc.v, f = (v.features || []).find(f => f.number === o.f); if (f) showFeature(f, v); return !!f; } });"));
    // explain() and act() only dispatch on the kind.
    assert!(body.contains("return hit && hit.dialog ? hit.dialog(o, sc, theme) : null;"));
    assert!(body.contains("if (hit && hit.act) return hit.act(o, sc, theme);"));
}

#[test]
fn village_html_hints_when_open_ground_is_clicked() {
    let body = &village_source();
    assert!(body.contains(r#"<div class="hint" id="hint" role="status" hidden>Click a building, crate or person for details</div>"#));
    // act() says whether a click did anything; when it did not, the hint shows.
    // (A click that did something also keeps a dialog a hover opened.)
    assert!(body.contains("if (h && act(h.o, h.sc, h.theme)) hovered = false;\n  else groundHint(e);"));
    assert!(body.contains("return !!info;"));
    assert!(body.contains("hintTimer = setTimeout(() => { hintEl.hidden = true; }, 2500);"));
}

#[test]
fn village_html_links_only_what_it_knows() {
    let body = &village_source();
    // The repo's pages only when a real issue URL names the repo.
    assert!(body.contains(r"const ISSUE_URL = /^(https:\/\/github\.com\/[^/]+\/[^/]+)\/issues\/\d+$/;"));
    assert!(body.contains("url: (c && c.url) || (own && own.url) || (repo ? `${repo}/issues/${n}` : null)"));
    assert!(body.contains("const link = (label, url) => (url ? { label, url } : null);"));
    // Houses, people and sub-agents link to the agent session view.
    assert!(body.contains(r"const sessionOf = h => (h.links || []).find(l => /^\/#\/swarm\/[^/]+\/agent\//.test(l.url));"));
    assert!(body.contains("link('Swarm view', swarm)"));
    assert!(body.contains("label: 'Parent agent session'"));
    // Environments link to their deployments or commit; the quay to the integration branch.
    assert!(body.contains("/deployments/${encodeURIComponent(s.name)}"));
    assert!(body.contains("`${repo}/commits/${encodeURIComponent(br)}`"));
}

/// The village's DOM-free decisions under node (tools/village-tests/):
/// the Fit scale (#400), each epic's flag colour (#406), the docks, and a
/// worker at rest (#411), and the "+N" badge and number tags (#436).
#[test]
fn village_node_tests_pass() {
    let root = env!("CARGO_MANIFEST_DIR");
    let dir = format!("{root}/tools/village-tests");
    // Every test file there, so a new one can't be left out of this list.
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .expect("tools/village-tests")
        .filter_map(|e| e.ok().map(|e| e.path().to_string_lossy().into_owned()))
        .filter(|p| p.ends_with(".test.mjs"))
        .collect();
    files.sort();
    assert!(files.len() >= 7, "village tests found: {files:?}");
    match std::process::Command::new("node").arg("--test").args(&files).output() {
        Ok(out) => assert!(out.status.success(), "tools/village-tests failed:\n{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)),
        Err(e) => eprintln!("node is not installed ({e}); skipping tools/village-tests"),
    }
}

/// The dialog's title keeps clear of its close × (#438).
#[test]
fn village_dialog_title_leaves_room_for_the_close_button() {
    let body = &village_source();
    assert!(body.contains(r#"<button id="close" aria-label="Close">×</button>"#));
    assert!(body.contains(".dialog button { position: absolute; top: 8px; right: 10px;"));
    assert!(body.contains(".dialog h2 { margin: 0 0 4px; padding-right: 26px; overflow-wrap: anywhere;"));
}

#[test]
fn village_html_names_the_village_and_links_its_repo() {
    let body = &village_source();
    assert!(body.contains(r#"<span id="vname" class="vname"></span> <a id="repo" hidden target="_blank" rel="noopener"></a>"#));
    // The name shows whenever the picker (which also shows it) is hidden.
    assert!(body.contains("vn.hidden = !pick.hidden;"));
    // A village by its project, a world as `dev ⇄ platform`.
    assert!(body.contains("nameBar(sc.v.project, sc.v);"));
    assert!(body.contains(r"nameBar(`${w.dev} \u21c4 ${w.platform}`, villages.find(x => x.project === w.dev));"));
    // The repo link only when a real issue URL names the repo.
    assert!(body.contains("repo = v ? repoUrl(v) : null;"));
    assert!(body.contains("a.hidden = !repo;"));
}

#[test]
fn village_html_reaches_every_object_by_keyboard() {
    let body = &village_source();
    assert!(body.contains(r#"<div class="hitkeys" id="hitkeys"></div>"#));
    assert!(body.contains("function placeKeys()"));
    // Placed after the banners and the area labels, whenever the map moves or changes.
    assert!(body.contains("registerOverlay(placeBanners, 10);") && body.contains("registerOverlay(placeKeys, 30);"));
    assert!(body.contains("OVERLAYS.forEach(o => o.fn());"));
    assert!(body.contains(".hk:focus-visible"));
    assert!(body.contains("b.setAttribute('aria-label', [info.title, info.line].filter(Boolean).join(': '));"));
}
