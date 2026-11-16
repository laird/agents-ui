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
    let main = village_asset("main.js").unwrap().1;
    assert!(main.contains("import './houselink.js';"));
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
