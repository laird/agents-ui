//! An epic reaching production (#482): fireworks, the hall's flag, the
//! screensaver's line. Its decisions are tested under node (shipdecide.js).

use super::*;

#[test]
fn a_shipped_epic_fires_fireworks_flies_on_the_hall_and_is_named_by_the_screensaver() {
    let body = &village_source();
    // Fireworks once per shipping, in its colour, over the harbour and the quay;
    // never at night or with reduced motion.
    assert!(body.contains("const quiet = state.reduce || dimmedAt(new Date().getHours(), SHIP_NIGHT);"));
    assert!(body.contains("newCelebrations(v.shipped, seen, Date.now(), quiet)"));
    assert!(body.contains("wfFireworks(c, lay.castleX, lay.canalY, t, fw,") && body.contains("wfFireworks(c, lay.roadX - 8, lay.quayY, t, fw,"));
    // The flag on the hall, clickable, named on hover.
    assert!(body.contains("const s = hallFlag(v.shipped);"));
    assert!(body.contains("registerHit({ kind: 'shipped', dialog(o, sc) {"));
    assert!(body.contains("hallFlagWords(h.sc.v.shipped)"));
    // The screensaver's strip.
    assert!(body.contains("const shippedNow = shippedLine(v.shipped, Date.now());"));
    assert!(body.contains(".ss-shipped, .ss-shipped b"));
}

#[test]
fn the_village_json_carries_what_shipped_today() {
    let s = crate::web::feature::Shipped { number: 482, title: "Fireworks".into(), url: "u".into(), at: "2026-11-16T10:00:00Z".into() };
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json, serde_json::json!({"number": 482, "title": "Fireworks", "url": "u", "at": "2026-11-16T10:00:00Z"}));
}
