//! Self-contained text-entry page served by the input server at
//! `GET /overlay`.

use axum::http::header;
use axum::response::{Html, IntoResponse, Response};

/// The page served at `GET /overlay`.
///
/// The page is a single HTML document with inline CSS/JS: no CDN, remote
/// font, image, script, stylesheet, analytics or Steam API. It submits to the
/// same-origin relative `/v1/speech`. When the page was requested with a valid
/// `?token=`, the server embeds that token into the script as a JSON literal
/// and the form carries it back as an `Authorization: Bearer` header; without
/// a token (loopback entry) the form posts without a header and relies on the
/// loopback exemption in the auth gate. No cookie is involved. Keeping the
/// document in a dedicated HTML file preserves editor/tooling support while
/// `include_str!` still embeds it into the executable at compile time.
const OVERLAY_HTML: &str = include_str!("overlay.html");

/// Token in [`OVERLAY_HTML`] replaced with the chosen language marker.
const OVERLAY_LANGUAGE_PLACEHOLDER: &str = "__OVERLAY_LANGUAGE__";

/// Token in [`OVERLAY_HTML`] replaced with a JSON literal holding the access
/// token (`"..."`) or `null` for the loopback, tokenless entry.
const OVERLAY_TOKEN_PLACEHOLDER: &str = "__ACCESS_TOKEN__";

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

/// Serve the overlay page with the language marker chosen at router build time
/// and, when provided, the access token embedded as a JSON literal so the form
/// can carry it back as `Authorization: Bearer`. A tokenless request (loopback)
/// embeds `null`.
pub async fn overlay(language: OverlayLanguage, access_token: Option<&str>) -> Response {
    let token_literal = match access_token {
        Some(token) => {
            // Serialize through serde_json so quotes, backslashes and control
            // characters are escaped, then neutralize `<` so an arbitrary token
            // can never close the enclosing `<script>` tag.
            serde_json::to_string(token)
                .expect("serializing a &str cannot fail")
                .replace('<', "\\u003c")
        }
        None => "null".to_string(),
    };
    let html = OVERLAY_HTML
        .replace(OVERLAY_LANGUAGE_PLACEHOLDER, language.marker())
        .replace(OVERLAY_TOKEN_PLACEHOLDER, &token_literal);
    // `frame-ancestors 'none'` prevents remote pages from framing the form
    // (clickjacking a text submit into the local queue). It is delivered as a
    // header because the `frame-ancestors` directive is ignored in meta tags.
    // The page embeds the caller's access token, so it must never be cached or
    // shared across origins.
    (
        [
            (header::CONTENT_SECURITY_POLICY, "frame-ancestors 'none'"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Html(html),
    )
        .into_response()
}
