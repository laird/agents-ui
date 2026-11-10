//! The themes: every slot filled, in every style.

use super::*;

#[test]
fn village_html_future_theme_fills_every_slot() {
    let body = &village_source();
    // Registered, offered in the Style picker, and remembered like the others.
    let registered = |name: &str, file: &str| village_asset(file).unwrap().1.contains(&format!("registerTheme('{name}', {});", name.to_uppercase()));
    assert!(registered("medieval", "themes/medieval.js") && registered("modern", "themes/modern.js") && registered("future", "themes/future.js"));
    assert!(body.contains("for (const [key, th] of Object.entries(THEMES)) themePick.add(new Option(th.name, key));"));
    assert!(body.contains("name: 'Future',"));
    assert!(body.contains("localStorage.setItem('village-theme', themeName)"));
    let future = js_block(body, "FUTURE");
    let future_slots = slots(future);
    let medieval = slots(js_block(body, "MEDIEVAL"));
    for slot in ["labels", "ground", "channel", "hall", "B", "gate", "destination", "decor", "border", "people", "sign"] {
        assert!(medieval.iter().any(|s| s == slot), "slot parser missed {slot}: {medieval:?}");
    }
    for theme in ["MEDIEVAL", "MODERN"] {
        for slot in slots(js_block(body, theme)) {
            assert!(future_slots.contains(&slot), "Future has no `{slot}`, which {theme} defines");
        }
    }
    // Every area is named, so labels and dialogs read the same in Future.
    for label in ["docks: 'Cargo bay'", "board: 'Holo-board'", "houses: 'Pods'", "quay: 'Launch rail'", "castle: 'Spaceport'", "hall: 'Command dome'"] {
        assert!(future.contains(label), "missing label {label}");
    }
    // A pod for every kind of building the business district draws.
    let pods = js_block(body, "FB");
    for line in js_block(body, "MB").lines() {
        if let Some(kind) = line.strip_prefix("    ").and_then(|l| l.split_once("(c, x, y, st, t)")).map(|(k, _)| k) {
            assert!(pods.contains(&format!("    {kind}(c, x, y, st, t)")), "no Future pod for {kind}");
        }
    }
    // People are robots, with a look for every status.
    assert!(future.contains("people: 'future', robots: true"));
    assert!(body.contains("if (state.T.robots) return robot(c, x, y, seed, h, kind, opts);"));
    let eyes = &body[body.find("const ROBOT_EYE = {").unwrap()..];
    let eyes = &eyes[..eyes.find('}').unwrap()];
    for status in ["working", "idle", "blocked", "done", "stalled", "unknown"] {
        assert!(eyes.contains(&format!("{status}:")), "no robot look for {status}");
    }
    assert!(body.contains("const slumped = state.T.robots && st === 'stalled';"));
    // In a world the platform takes the theme's pair; Future pairs with Modern.
    assert!(js_block(body, "MEDIEVAL").contains("pair: 'modern',") && js_block(body, "MODERN").contains("pair: 'medieval',"));
    assert!(js_block(body, "FUTURE").contains("pair: 'modern',"));
    assert!(body.contains("const otherTheme = () => THEMES[state.T.pair] || THEMES.medieval;"));
    // Reduced motion: no hovering bob and no sparks.
    assert!(body.contains("const now = () => (state.reduce ? 0 : performance.now());"));
    assert!(body.contains("kind === 'stand' && !state.reduce && blink("));
    assert!(body.contains("if (!state.reduce && blink(t + (h % 300), 120))"));
}
