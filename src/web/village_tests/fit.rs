//! Fitting the map to the window, zoom, and no counter row.

use super::*;

/// Fit (#400): the map fills the window, width and height, with no
/// scrolling. The scale choice itself is tested under node (fitscale.js).
#[test]
fn village_html_fits_the_map_to_the_window() {
    let body = &village_source();
    // The room is the viewport's width and height; a phone gets its own scale (below).
    // It is chosen on the scene's size, not the 2× canvas's (#418), so the
    // finer canvas fills exactly the room the 1× one did.
    assert!(body.contains("const vp = document.getElementById('viewport'), w = cv.width / DETAIL, h = cv.height / DETAIL;"));
    assert!(body.contains("return chooseScale(w, h, vp.clientWidth - 8, vp.clientHeight - 8);"));
    assert!(body.contains("const mapScale = () => zoomedScale() / DETAIL;"), "CSS pixels per canvas pixel");
    // A phone (#437): a readable minimum scale, scrolling sideways, not a sliver.
    assert!(body.contains("if (!tall) return phoneScale(w, h, vp.clientWidth - 8);"));
    assert!(body.contains("keepInside([...abox.children], cv.getBoundingClientRect());"), "labels stay inside the map");
    assert!(body.contains(".legend { position: fixed; inset: 0; max-height: none;"), "the key is a full-screen sheet on a phone");
    assert!(body.contains("document.getElementById('legend-close').addEventListener('click'"));
    assert!(body.contains("html:not(.screensaver) .viewport { min-height: 200px; overflow-x: auto; overflow-y: hidden; }"), "a phone's map scrolls sideways only");
    assert!(body.contains("const NARROW = '(max-width: 640px)';"));
    assert!(body.contains("return bareW > FIT_SLACK && bareH > FIT_SLACK ? most : whole;"));
    // The page is a column the window's height; the viewport takes the rest, at any width.
    assert!(body.contains("body { height: 100vh; height: 100dvh; display: flex; flex-direction: column; overflow: auto;"));
    assert!(body.contains(".stage > .viewport { flex: 1; }"));
    assert!(!body.contains("max-width: 1400px"), "the stage uses the whole window");
    assert!(!body.contains("scrollbar-gutter"), "no room is kept for a page scrollbar");
    // The key floats over the map instead of pushing it down.
    assert!(body.contains(".legend { position: absolute;"));
    // Fit again, debounced, when the window, fullscreen or the chrome around the map changes.
    assert!(body.contains("new ResizeObserver(refitSoon).observe(document.getElementById('viewport'));"));
    assert!(body.contains("document.addEventListener('fullscreenchange', refitSoon);"));
    // A zoomed-in map scrolls from its left edge instead of being cut off.
    assert!(body.contains("justify-content: safe center;"));
}

/// #444: the bar stays one row at 1280 (and down to 1100), so the map keeps
/// its height: pickers are as wide as what they show, and the workflow words,
/// the repo link and the live status ellipsize instead of wrapping. Measured
/// in headless Chromium when it landed: one 31 px row at 1280 and at 1100,
/// wrapping below. Fit needs nothing more: it measures the viewport, which a
/// ResizeObserver watches, so the bar's real height is what it gets.
#[test]
fn village_bar_stays_one_row() {
    let body = &village_source();
    assert!(body.contains(".title select { field-sizing: content; }"), "a picker fits its shown option");
    assert!(body.contains("@media (min-width: 1100px) {\n    .bar { flex-wrap: nowrap; }"));
    assert!(body.contains(".title { flex-wrap: nowrap; flex: 0 1 auto; white-space: nowrap; }"));
    assert!(body.contains(".title > .status, .title > #repo { flex: 0 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; }"));
    assert!(body.contains(".bar > .status { flex: 1 1 0; min-width: 15em; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; text-align: right; }"));
    // The status and the workflow words are still where the page writes them.
    assert!(body.contains(r#"<span id="workflow" class="status"></span>"#));
    assert!(body.contains(r#"<div class="status" id="status" aria-live="polite">"#));
}

/// #405: no row of counters under the map; the map shows the counts.
/// A world keeps what only the row said (its release, the platform's
/// build IDs) in the bar's line.
#[test]
fn village_has_no_counter_row() {
    let body = &village_source();
    for gone in ["id=\"flow\"", ".flow {", "renderFlow", "registerCounter", "COUNTERS"] {
        assert!(!body.contains(gone), "the counter row is back: `{gone}`");
    }
    assert!(body.contains("document.getElementById('workflow').textContent = worldWords(w);"));
    assert!(body.contains("return `· ${w.dev} builds ${w.platform} · release ${short(w.release)} ${release}${builds ? ` · platform builds ${builds}` : ''}`;"));
}

#[test]
fn village_html_zooms_and_remembers_it() {
    let body = &village_source();
    assert!(body.contains(r#"<button id="zoom-out" aria-label="Zoom out">−</button>"#));
    assert!(body.contains(r#"<button id="zoom-in" aria-label="Zoom in">+</button>"#));
    assert!(body.contains(r#"<button id="zoom-fit""#));
    // +/- step the scale from the fitted one, never past the smallest or largest.
    assert!(body.contains("return zoom ? Math.max(Math.min(1, f), Math.min(ZOOM_MAX_SCALE, Math.max(1, Math.floor(f)) + zoom)) : f;"));
    assert!(body.contains("if (d && mapScale() === before) zoom -= d;"));
    // Remembered, and storage that throws (private windows) never breaks the page.
    assert!(body.contains("try { localStorage.setItem('village-zoom', String(zoom)); } catch {}"));
    assert!(body.contains("try { zoom = Math.max(-ZOOM_MAX_SCALE, Math.min(ZOOM_MAX_SCALE, parseInt(localStorage.getItem('village-zoom'), 10) || 0)); } catch {}"));
    assert!(body.contains("addEventListener('click', () => stepZoom(-1))"));
    assert!(body.contains("addEventListener('click', () => stepZoom(1))"));
    assert!(body.contains("addEventListener('click', () => stepZoom(0))"));
}
