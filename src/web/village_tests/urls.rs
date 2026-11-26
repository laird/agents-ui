//! Every URL the pages ask this server for is relative (#552), so the
//! village and the dashboard work unchanged under `--base-path` or behind a
//! reverse proxy that serves them at a path prefix: `api/village` resolves
//! against the page's own address, `/api/village` would leave the prefix.

use super::*;

/// The lines of `body`, comments aside, that name one of the server's routes
/// from the root.
fn root_urls(body: &str) -> Vec<String> {
    let routes = ["/api/", "/village", "/#/", "/healthz", "/\""];
    body.lines()
        .map(str::trim)
        .filter(|l| !(l.starts_with("*") || l.starts_with("/*") || l.starts_with("//")))
        .filter(|l| routes.iter().any(|r| ['\'', '"', '`'].iter().any(|q| l.contains(&format!("{q}{r}")))))
        .map(str::to_string)
        .collect()
}

#[test]
fn the_pages_ask_for_nothing_from_the_root() {
    let pages = VILLAGE_ASSETS.iter().map(|(p, _, body)| (*p, *body)).chain([("ui.html", include_str!("../ui.html"))]);
    let found: Vec<String> = pages.flat_map(|(p, body)| root_urls(body).into_iter().map(move |l| format!("{p}: {l}"))).collect();
    assert!(found.is_empty(), "root-absolute URLs break under a path prefix; drop the leading slash:\n{}", found.join("\n"));
    let framework = include_str!("../framework.rs");
    assert!(framework.contains(r#"format!("./#/swarm/"#) && framework.contains(r#"format!("api/swarms/{}/agents/{}/answer""#), "links the server hands the page are relative too");
}

#[test]
fn a_root_url_is_caught() {
    assert_eq!(root_urls("const res = await fetch('/api/village');\n * `/api/village` in a comment"), ["const res = await fetch('/api/village');"]);
    assert_eq!(root_urls(r#"<script type="module" src="/village/main.js"></script>"#).len(), 1);
    assert!(root_urls("fetch('api/village'); link('./#/swarm/x')").is_empty());
}
