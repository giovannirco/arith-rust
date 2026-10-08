//! The page at `/`, compiled into the binary from `web/`.
//!
//! Three files, no build step: edit `web/index.html`, `web/style.css` or
//! `web/app.js` and rebuild.

use axum::http::header;
use axum::response::IntoResponse;

const INDEX: &str = include_str!("../web/index.html");
const STYLE: &str = include_str!("../web/style.css");
const SCRIPT: &str = include_str!("../web/app.js");

/// The page only loads its own stylesheet and script and only talks to this
/// origin. The header says so, which turns an injected script into a blocked
/// one.
const CONTENT_SECURITY_POLICY: &str = "default-src 'none'; style-src 'self'; script-src 'self'; connect-src 'self'; \
     img-src 'self' data:; form-action 'none'; base-uri 'none'; frame-ancestors 'none'";

pub async fn index() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CONTENT_SECURITY_POLICY, CONTENT_SECURITY_POLICY),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::REFERRER_POLICY, "no-referrer"),
        ],
        INDEX,
    )
}

pub async fn style() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], STYLE)
}

pub async fn script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        SCRIPT,
    )
}
