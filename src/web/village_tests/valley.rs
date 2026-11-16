//! A valley of related repos (#510): the page draws `/api/valley`'s group.
//! Its layout is tested under node (valleymodel.js).

use super::*;

#[test]
fn the_valley_page_draws_the_group_and_opens_a_repo_on_click() {
    let body = &village_source();
    // `?valley=<group>` wins over the hash, and fetches its own answer.
    assert!(body.contains("if (valleyWanted()) { nameBar(`Valley \\u00b7 ${valleyWanted()}`, null); selectValley(valleyWanted()); return; }"));
    assert!(body.contains("const res = await fetch('/api/valley');"));
    // draw() hands it its frame, like the overview's.
    assert!(body.contains("if (view.valley) { drawValley(t); return; }"));
    // Rivers along package edges, roads along api ones, from the layout.
    assert!(body.contains("lay.rivers.forEach(") && body.contains("lay.roads.forEach("));
    // A click, or Enter on a repo's target, opens its village; a repo without one has nothing to open.
    assert!(body.contains("if (!r || !r.project) return;"));
    assert!(body.contains("q.delete('valley');"));
    // Clicks on the valley are its own: no village hint under them.
    assert!(body.contains("if (!state.view || state.view.overview || state.view.valley) return;"));
    // No one village's build speaks for the valley in the status line.
    assert!(body.contains("const shown = valleyWanted() ? null :"));
    assert!(body.contains("b.disabled = !repo.project;"));
    // Nothing to draw says why.
    assert!(body.contains("const missing = valleyMissing(valleyResp, name);"));
}

#[test]
fn the_demo_declares_a_group() {
    let demo = include_str!("../../../tools/mock/demo.sh");
    let start = demo.find("<<'GROUP'\n").unwrap() + "<<'GROUP'\n".len();
    let json = &demo[start..demo[start..].find("\nGROUP\n").unwrap() + start];
    let (groups, errors) = crate::web::valley::valley(json, &[]);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(groups[0].name, "demo");
    assert!(groups[0].errors.is_empty(), "{:?}", groups[0].errors);
    assert_eq!(groups[0].repos[0].name, "lib-core");
    assert!(demo.contains("/village?valley=demo"));
}

/// Releases flow downstream (#511): ships on the rivers, from the edges'
/// versions (decided in valleymodel.js, tested under node).
#[test]
fn the_valley_sails_releases_down_its_rivers() {
    let body = &village_source();
    // Drawn over the rivers, still when motion is reduced.
    assert!(body.contains("valleyShips(lay, state.reduce ? 0 : t).forEach(sh => drawShip(ctx, sh));"));
    // A click on a ship says which versions, before a repo takes the click.
    assert!(body.contains("if (ship) { openInfo(shipWords(ship)); return; }"));
    // A waiting ship is labelled with its version and has a keyboard target.
    assert!(body.contains("}).join('') + shipLabels(ships, ox0, oy0, s)"));
    assert!(body.contains("shipKeys(vkeys, ships, ox0, oy0, s, sh => openInfo(shipWords(sh))).forEach(b => keep.add(b));"));
    let main = village_asset("valley.js").unwrap().1;
    assert!(main.contains("import { drawShip, shipKeys, shipLabels } from './valleyships.js';"));
}

/// The environment grid (#512): the api-joined repos by environment below
/// the valley, prod a city, marked cells with dialogs (decided in
/// valleymodel.js, tested under node).
#[test]
fn the_valley_draws_its_environment_grid() {
    let body = &village_source();
    assert!(body.contains("drawGrid(ctx, lay, state);"));
    assert!(body.contains("if (cell) { openInfo(gridWords(cell.cell)); return; }"));
    assert!(body.contains("+ gridLabels(lay, ox0, oy0, s)"));
    assert!(body.contains("gridKeys(vkeys, lay, ox0, oy0, s, c => openInfo(gridWords(c))).forEach(b => keep.add(b));"));
    let grid = village_asset("valleygrid.js").unwrap().1;
    assert!(grid.contains("if (g.city) {"), "the prod column is a city");
}

/// #530: ropes, caravans and barges are drawn, clickable and keyboard targets.
#[test]
fn the_valley_draws_ropes_caravans_and_barges() {
    assert!(village_asset("modules.js").unwrap().1.contains("import './valleylinks.js';"));
    let valley = village_asset("valley.js").unwrap().1;
    // Drawn: barges on the rivers, ropes over the repos, caravans in their band.
    assert!(valley.contains("valleyBarges(lay, g, state.reduce ? 0 : t).forEach(b => drawValleyBarge(ctx, b));"));
    assert!(valley.contains("valleyRopes(lay, g).forEach(rp => drawValleyRope(ctx, rp));"));
    assert!(valley.contains("valleyCaravans(lay, g).forEach(tr => drawValleyCaravan(ctx, tr, lay.W, VALLEY_MARGIN));"));
    // A click on each opens its dialog.
    assert!(valley.contains("if (barge) { openInfo(bargeWords(barge)); return; }"));
    assert!(valley.contains("if (rope) { openInfo(ropeWords(rope.rope)); return; }"));
    assert!(valley.contains("if (train) { openInfo(caravanWords(train)); return; }"));
    // And each has a keyboard target, opening what a click would.
    assert!(valley.contains("linkKeys(vkeys, links, ox0, oy0, s, openInfo).forEach(b => keep.add(b));"));
    let links = village_asset("valleylinks.js").unwrap().1;
    assert!(links.contains("b.onclick = () => open(words);"));
}

/// #530: `VALLEY=1 tools/mock/demo.sh` serves one rope, one caravan and a barge
/// through the real code path (the gh shim answers for the valley's repos).
#[test]
fn the_valley_demo_has_a_rope_a_caravan_and_a_barge() {
    let demo = include_str!("../../../tools/mock/demo.sh");
    assert!(demo.contains("VALLEY=1 tools/mock/demo.sh"));
    assert!(demo.contains(r#""Depends on: demo/lib-core#12""#), "shop#7 waits on lib-core#12: a rope");
    assert!(demo.contains(r#"- [ ] demo/larkspur#4"#), "lib-core#1's subtasks span three repos: a caravan");
    assert!(demo.contains(r#"remote add origin "https://github.com/demo/$r""#));
    let shim = include_str!("../../../tools/mock/bin/gh");
    assert!(shim.contains("use lib-core::money::Cents;"), "shop's red log names lib-core: a barge");
    assert!(shim.contains(r#""conclusion": "failure""#));
    // The fixtures parse as the issues crossrepo.rs reads.
    for name in ["lib-core.json", "shop.json", "larkspur.json"] {
        let start = demo.find(&format!("state/valley/{name}\" <<'ISSUES'\n")).unwrap();
        let body = &demo[start..];
        let body = &body[body.find('\n').unwrap() + 1..body.find("\nISSUES\n").unwrap()];
        let issues: Vec<crate::web::crossrepo::CrossIssue> = serde_json::from_str(body).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!issues.is_empty(), "{name}");
    }
}
