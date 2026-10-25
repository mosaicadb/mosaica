//! `GET /v1/basemaps/world`: the built-in world outline, for any session on the viewer plane.
//! The body is `assets/world.mbw`, compiled in and the same for every principal; it says nothing
//! about the corpus or the node. `scripts/build-world-basemap.py` writes it from Natural Earth,
//! and that script's docstring is the format.

use std::sync::OnceLock;

use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use sha2::{Digest, Sha256};

use crate::state::ViewerSession;

const WORLD: &[u8] = include_bytes!("../assets/world.mbw");

/// The body never changes within a binary and is no secret, so any cache may keep it for a year.
/// The token only keeps callers without a session from loading the server.
const CACHE_CONTROL: &str = "public, max-age=31536000, immutable";

/// A strong entity tag from the first 16 bytes of the body's SHA-256.
fn etag() -> &'static HeaderValue {
    static ETAG: OnceLock<HeaderValue> = OnceLock::new();
    ETAG.get_or_init(|| {
        let digest: [u8; 16] = Sha256::digest(WORLD)[..16].try_into().unwrap();
        HeaderValue::from_str(&format!("\"{}\"", crate::stream::hex16(&digest)))
            .expect("a quoted hex string is a valid header value")
    })
}

/// Whether `If-None-Match` names this body: `*`, or our tag in its list, weak or strong, as the
/// weak comparison `If-None-Match` uses.
fn not_modified(headers: &HeaderMap, tag: &HeaderValue) -> bool {
    let tag = tag.as_bytes();
    headers
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .any(|t| t == "*" || t.strip_prefix("W/").unwrap_or(t).as_bytes() == tag)
}

/// The session is checked before `If-None-Match` is read, so a `304` needs a token too.
pub async fn world(_: ViewerSession, headers: HeaderMap) -> Response {
    let tag = etag();
    let cache = [
        (header::ETAG, tag.clone()),
        (
            header::CACHE_CONTROL,
            HeaderValue::from_static(CACHE_CONTROL),
        ),
    ];
    if not_modified(&headers, tag) {
        return (StatusCode::NOT_MODIFIED, cache).into_response();
    }
    (
        cache,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        )],
        WORLD,
    )
        .into_response()
}
