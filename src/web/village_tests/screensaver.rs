//! The screensaver mode and its Omarchy installer.

use super::*;

/// `/village?screensaver=1` (#382): the idle display the Omarchy
/// screensaver opens. Its decisions are tested under node (saver.js).
#[test]
fn village_html_has_a_screensaver_mode() {
    let body = &village_source();
    // Fitted to the whole display: the height bounds the scale too, and zoom is ignored.
    assert!(body.contains("const tall = state.screensaver || !matchMedia(NARROW).matches;"));
    assert!(body.contains("if (state.screensaver) return f;"));
    // No bar, picker or buttons; no cursor; the labels stay on.
    assert!(body.contains("html.screensaver .bar, html.screensaver .fstrip, html.screensaver .legend"));
    assert!(body.contains("cursor: none;"));
    assert!(body.contains("document.getElementById('labels-on').checked = true;"));
    // The page says after each poll whether the server answered; the strip and the "can't reach" read it.
    assert!(body.contains("window.dispatchEvent(new CustomEvent('village:polled', { detail: { ok, at: Date.now() } }))"));
    assert!(body.contains("polled(true);") && body.contains("polled(false);"));
    assert!(body.contains("Can't reach agents-tui on ${esc(location.hostname || 'this server')}"));
    assert!(body.contains("html.ss-offline .stage, html.ss-offline .ss-strip { visibility: hidden; }"), "no frozen map");
    // The hash may carry flags after the name (`#shop&screensaver`).
    assert!(!body.contains("decodeURIComponent(location.hash.slice(1))"), "every hash read goes through hashName");
    // Cycling replaces the hash, so the launcher's window keeps one history entry and may close.
    assert!(body.contains("history.replaceState(null, '', location.pathname + location.search + '#' + encodeURIComponent(next) + '&screensaver');"));
    // Input dismisses only when the launcher asked, through the title it watches for.
    assert!(body.contains("if (saverOpts.dismiss) armDismissal();"));
    assert!(body.contains("document.title = 'agents-village-screensaver-exit';"));
    let launcher = include_str!("../../../tools/omarchy-plugin/agents-village-screensaver/village-screensaver");
    assert!(launcher.contains("EXIT_TITLE=agents-village-screensaver-exit"));
    assert!(launcher.contains("screensaver=1&dismiss=1&cycle=${CYCLE}"));
    // Burn-in care: drift, and dim at night; never blank.
    assert!(body.contains("document.body.style.translate = `${d.x}px ${d.y}px`;"));
    assert!(body.contains("html.ss-dim { filter: brightness(.4); }"));
}

/// The installer and launcher (bash, in a throwaway HOME) and the page's
/// decisions (node, when it is installed).
#[test]
fn screensaver_install_and_page_tests_pass() {
    let root = env!("CARGO_MANIFEST_DIR");
    let dir = format!("{root}/tools/omarchy-plugin/agents-village-screensaver/tests");
    let out = std::process::Command::new("bash").arg(format!("{dir}/test_install.sh")).output().expect("run test_install.sh");
    assert!(out.status.success(), "test_install.sh failed:\n{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    match std::process::Command::new("node").args(["--test", &format!("{dir}/saver.test.mjs")]).output() {
        Ok(out) => assert!(out.status.success(), "saver.test.mjs failed:\n{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)),
        Err(e) => eprintln!("node is not installed ({e}); skipping saver.test.mjs"),
    }
}
