//! Serving the web UI behind a reverse proxy (#552): under a path prefix
//! (`--base-path`), and framed by another application (`--frame-ancestors`).
//! The pages ask for every URL relative to their own address
//! (`village_tests/urls.rs`), so moving the routes is all a prefix needs.

use axum::{
    Router,
    http::{HeaderValue, header::CONTENT_SECURITY_POLICY},
    response::{Redirect, Response},
    routing::get,
};

/// Where the routes are mounted and who may frame the pages.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mount {
    /// `/ops/village`, or empty for the root ([`base_path`]).
    pub base_path: String,
    /// The whole `Content-Security-Policy` value, `frame-ancestors <origins>`
    /// ([`frame_ancestors`]); none sends no policy.
    pub frame_ancestors: Option<HeaderValue>,
}

/// `--base-path`: `ops/village`, `/ops/village` and `/ops/village/` are all
/// `/ops/village`; empty or `/` is the root, `""`.
pub fn base_path(raw: &str) -> Result<String, String> {
    let path = raw.trim().trim_matches('/');
    if path.is_empty() {
        return Ok(String::new());
    }
    let segment_ok = |s: &str| !s.is_empty() && s != "." && s != ".." && s.chars().all(|c| c.is_ascii_alphanumeric() || "-._~".contains(c));
    if !path.split('/').all(segment_ok) {
        return Err(format!("--base-path '{raw}' is not a path prefix like /ops/village (letters, digits, - . _ ~ between slashes)"));
    }
    Ok(format!("/{path}"))
}

/// `--frame-ancestors`: the sources allowed to frame the pages, as CSP writes
/// them (`'self' https://example.test`). One directive only, so no `;` or `,`.
pub fn frame_ancestors(origins: &str) -> Result<HeaderValue, String> {
    let origins = origins.trim();
    let bad = || format!("--frame-ancestors '{origins}' is not a list of sources like \"'self' https://example.test\"");
    if origins.is_empty() || origins.contains([';', ',']) {
        return Err(bad());
    }
    HeaderValue::from_str(&format!("frame-ancestors {origins}")).map_err(|_| bad())
}

impl Mount {
    /// `app` under the base path, with `/healthz` kept at the root for probes
    /// and `<base>` sent to `<base>/` (the pages' relative URLs need the
    /// slash), and the frame policy on every response when one is set.
    pub fn apply(&self, app: Router) -> Router {
        let app = if self.base_path.is_empty() {
            app
        } else {
            let slash = format!("{}/", self.base_path);
            Router::new()
                .route("/healthz", get(|| async { "ok" }))
                .route(&self.base_path, get({
                    let slash = slash.clone();
                    move || async move { Redirect::permanent(&slash) }
                }))
                .nest(&slash, app)
        };
        match self.frame_ancestors.clone() {
            Some(policy) => app.layer(axum::middleware::map_response(move |mut res: Response| {
                res.headers_mut().insert(CONTENT_SECURITY_POLICY, policy.clone());
                async move { res }
            })),
            None => app,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::server::{WebServerState, app};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    fn served(mount: Mount) -> Router {
        let state = WebServerState {
            swarms: crate::web::new_shared_state(),
            agents_dir: "/tmp/test-agents".into(),
            transport: Default::default(),
            village: Default::default(),
            swarm_sources: Default::default(),
            since: Default::default(),
        };
        app(false, &mount, state)
    }

    async fn get_from(app: &Router, uri: &str) -> (StatusCode, axum::http::HeaderMap, String) {
        let resp = app.clone().oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap()).await.unwrap();
        let (status, headers) = (resp.status(), resp.headers().clone());
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        (status, headers, String::from_utf8_lossy(&body).into_owned())
    }

    #[test]
    fn base_paths_are_normalised_and_odd_ones_refused() {
        for raw in ["x/y", "/x/y", "/x/y/", " /x/y// "] {
            assert_eq!(base_path(raw).unwrap(), "/x/y", "{raw}");
        }
        assert_eq!(base_path("").unwrap(), "");
        assert_eq!(base_path("/").unwrap(), "");
        for raw in ["/x//y", "/x/../y", "/x/{id}", "/x/*rest", "/x y", "/x?y", "/x#y", "/x%20y"] {
            assert!(base_path(raw).is_err(), "{raw}");
        }
    }

    #[test]
    fn frame_ancestors_is_one_directive() {
        assert_eq!(frame_ancestors(" 'self' https://example.test ").unwrap(), "frame-ancestors 'self' https://example.test");
        for raw in ["", "  ", "'self'; script-src *", "'self', https://a.test", "a\nb"] {
            assert!(frame_ancestors(raw).is_err(), "{raw:?}");
        }
    }

    #[tokio::test]
    async fn under_a_base_path_everything_moves_but_the_health_check() {
        let app = served(Mount { base_path: "/x/y".into(), frame_ancestors: None });
        let (status, _, page) = get_from(&app, "/x/y/village").await;
        assert_eq!(status, StatusCode::OK);
        assert!(page.contains(r#"<script type="module" src="village/main.js"></script>"#), "the page loads its modules relative to itself");
        let (status, headers, module) = get_from(&app, "/x/y/village/main.js").await;
        assert_eq!(status, StatusCode::OK);
        assert!(headers["content-type"].to_str().unwrap().contains("javascript") && module.contains("import './modules.js';"));
        let (status, _, village) = get_from(&app, "/x/y/api/village").await;
        assert_eq!(status, StatusCode::OK);
        assert!(village.starts_with('{'), "{village}");
        let (status, _, dashboard) = get_from(&app, "/x/y/").await;
        assert_eq!(status, StatusCode::OK);
        assert!(dashboard.contains("Agents UI"));
        let (status, headers, _) = get_from(&app, "/x/y").await;
        assert_eq!((status, headers["location"].to_str().unwrap()), (StatusCode::PERMANENT_REDIRECT, "/x/y/"));
        for health in ["/healthz", "/x/y/healthz"] {
            assert_eq!(get_from(&app, health).await.0, StatusCode::OK, "{health}");
        }
        // Only the prefix serves: a root URL is a page that would break, so it 404s rather than half-working.
        for root in ["/", "/village", "/api/village", "/village/main.js"] {
            assert_eq!(get_from(&app, root).await.0, StatusCode::NOT_FOUND, "{root}");
        }
    }

    #[tokio::test]
    async fn without_flags_the_root_serves_as_before_and_sends_no_policy() {
        let app = served(Mount::default());
        for uri in ["/", "/village", "/village/main.js", "/api/village", "/healthz"] {
            let (status, headers, _) = get_from(&app, uri).await;
            assert_eq!(status, StatusCode::OK, "{uri}");
            assert!(!headers.contains_key("content-security-policy"), "{uri}");
        }
    }

    #[tokio::test]
    async fn frame_ancestors_sets_the_policy_on_the_pages() {
        let policy = frame_ancestors("'self' https://example.test").unwrap();
        for base in ["", "/x/y"] {
            let app = served(Mount { base_path: base.into(), frame_ancestors: Some(policy.clone()) });
            for page in ["/", "/village"] {
                let (status, headers, _) = get_from(&app, &format!("{base}{page}")).await;
                assert_eq!(status, StatusCode::OK);
                assert_eq!(headers["content-security-policy"], "frame-ancestors 'self' https://example.test", "{base}{page}");
            }
        }
    }
}
