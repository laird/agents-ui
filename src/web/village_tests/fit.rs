//! Fitting the map to the window, zoom, and no counter row.

use super::*;

/// Fit (#400): the map fills the window, width and height, with no
/// scrolling. The scale choice itself is tested under node (fitscale.js).
#[test]
fn village_html_fits_the_map_to_the_window() {
    let body = &village_source();
    // The room is the viewport's width and height; a phone gets its own scale (below).
    assert!(body.contains("return chooseScale(cv.width, cv.height, vp.clientWidth - 8, vp.clientHeight - 8);"));
    // A phone (#437): a readable minimum scale, scrolling sideways, not a sliver.
    assert!(body.contains("if (!tall) return phoneScale(cv.width, cv.height, vp.clientWidth - 8);"));
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
