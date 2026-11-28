//! Desktop notifications from the village (#480): the deep link that opens a
//! house's dialog, and the Omarchy notifier's own tests.

use super::*;

/// `/village#<project>&house=<name>` opens that house's dialog once the
/// village is drawn; which house is deeplink.js's (tested under node).
#[test]
fn village_opens_the_house_a_link_names() {
    let link = village_asset("houselink.js").expect("houselink.js is embedded").1;
    assert!(link.contains("window.addEventListener('village:polled'"), "opens after a poll has drawn the village");
    assert!(link.contains("linkOpened === location.hash"), "once per link: a closed dialog stays closed");
    assert!(link.contains("state.screensaver"), "never on the screensaver");
    assert!(link.contains("act(o, sc, state.T);"), "the same dialog a click opens");
    let main = village_asset("modules.js").unwrap().1;
    assert!(main.contains("import './houselink.js';"));
}

/// Every kind a notification is about has a deep link (#579, for #570):
/// `#<project>&issue=<n>` opens #564's issue dialog, `&gate=`, `&build` and
/// `&env=` their dialogs, a waiting house or issue its red "!" view (#571);
/// what is gone says so. What each opens is deeplink.js's (node tests).
#[test]
fn village_opens_the_issue_gate_build_or_environment_a_link_names() {
    let link = village_asset("houselink.js").unwrap().1;
    assert!(link.contains("const o = sc && sc.v && linkTarget(sc.v, location.hash);"));
    assert!(link.contains("else if (o.kind === 'issue') showIssueDialog(sc.v.project, o.n);"), "an issue link opens the issue dialog");
    assert!(link.contains("if (o.stale) toast(o.stale);"), "a stale target says so");
    let deep = village_asset("deeplink.js").unwrap().1;
    for flag in ["f.has('house')", "f.has('issue')", "f.has('gate')", "f.has('build')", "f.has('env')"] {
        assert!(deep.contains(flag), "{flag}");
    }
    assert!(deep.contains("return i >= 0 ? { kind: 'alarm', i } : { kind: 'issue', n };"));
}

/// The notifier's decisions and the bar widget's model share one rule for
/// "needs you"; both run under node, when it is installed.
#[test]
fn notify_and_widget_model_tests_pass() {
    let root = env!("CARGO_MANIFEST_DIR");
    for dir in ["agents-village-notify/tests", "agents-village/tests"] {
        node_tests_in(&format!("{root}/tools/omarchy-plugin/{dir}"));
    }
}
