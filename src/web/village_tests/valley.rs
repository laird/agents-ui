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
    assert!(body.contains("}).join('') + shipLabels(ships, ox0, oy0, s);"));
    assert!(body.contains("shipKeys(vkeys, ships, ox0, oy0, s, sh => openInfo(shipWords(sh))).forEach(b => keep.add(b));"));
    let main = village_asset("valley.js").unwrap().1;
    assert!(main.contains("import { drawShip, shipKeys, shipLabels } from './valleyships.js';"));
}
