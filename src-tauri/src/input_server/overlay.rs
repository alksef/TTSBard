//! Self-contained text-entry page served by the input server at
//! `GET /overlay`.

use axum::http::header;
use axum::response::{Html, IntoResponse, Response};

/// The page served at `GET /overlay`.
///
/// The page is a single HTML document with inline CSS/JS: no CDN, remote
/// font, image, script, stylesheet, analytics or Steam API. It submits to the
/// same-origin relative `/v1/speech` so the loopback `Host` gate keeps
/// protecting the form without any CORS header. Keeping the document in a
/// dedicated HTML file preserves editor/tooling support while `include_str!`
/// still embeds it into the executable at compile time.
const OVERLAY_HTML: &str = include_str!("overlay.html");

/// Token in [`OVERLAY_HTML`] replaced with the chosen language marker.
const OVERLAY_LANGUAGE_PLACEHOLDER: &str = "__OVERLAY_LANGUAGE__";

/// Overlay UI language, decided once when the router is built.
///
/// Only Russian has translated overlay strings; every other effective UI
/// locale serves the English overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayLanguage {
    English,
    Russian,
}

impl OverlayLanguage {
    /// Language marker injected into the overlay script.
    fn marker(self) -> &'static str {
        match self {
            OverlayLanguage::English => "en",
            OverlayLanguage::Russian => "ru",
        }
    }
}

/// Serve the overlay page with the language marker chosen at router build time.
pub async fn overlay(language: OverlayLanguage) -> Response {
    let html = OVERLAY_HTML.replace(OVERLAY_LANGUAGE_PLACEHOLDER, language.marker());
    // `frame-ancestors 'none'` prevents remote pages from framing the form
    // (clickjacking a text submit into the local queue). It is delivered as a
    // header because the `frame-ancestors` directive is ignored in meta tags.
    (
        [(header::CONTENT_SECURITY_POLICY, "frame-ancestors 'none'")],
        Html(html),
    )
        .into_response()
}
