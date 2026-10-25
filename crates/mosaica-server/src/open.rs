//! An open server (`[serve] open`) serves every request as the operator, so each of its planes
//! refuses, with 403, the requests a page on another site could make.
//!
//! - A request whose `Host`, or whose absolute-form request line, names anything but this machine
//!   ([`crate::cors::is_loopback_authority`]), or that carries no host or two. A site that rebinds
//!   its own name to 127.0.0.1 then reads and writes nothing, since the browser sends that name.
//! - A request that may change state, any method but `GET`, `HEAD`, `OPTIONS` and `TRACE`, sent
//!   with no content type, or as `text/plain`, `application/x-www-form-urlencoded` or
//!   `multipart/form-data`: what a browser sends from another site without asking the server
//!   first. Any other type makes the browser ask, and only the viewer plane admits a page, and only
//!   one from this machine.
//! - A request whose `Origin` is not a page on this machine, or which carries no such `Origin` and
//!   whose `Sec-Fetch-Site` says another site sent it. A client that is not a browser sends
//!   neither. A page on `localhost` calling `127.0.0.1` is cross-site to the browser, so a loopback
//!   `Origin` is admitted whatever `Sec-Fetch-Site` says.

use axum::Router;
use axum::extract::Request;
use axum::http::HeaderMap;
use axum::http::header::{CONTENT_TYPE, HOST, ORIGIN};
use axum::middleware::Next;
use axum::response::Response;

use crate::error::ApiError;
use crate::state::AppState;

/// The content types a browser sends from another site without a preflight.
const SENT_WITHOUT_PREFLIGHT: [&str; 3] = [
    "text/plain",
    "application/x-www-form-urlencoded",
    "multipart/form-data",
];

/// The mark [`guard`] leaves on a request it admitted. An open server serves a request as the
/// operator only with it, so a router mounted without the guard serves no one on `open` alone.
#[derive(Clone, Copy)]
struct Guarded;

/// Whether [`guard`] admitted the request these are the extensions of.
pub(crate) fn guarded_request(extensions: &axum::http::Extensions) -> bool {
    extensions.get::<Guarded>().is_some()
}

/// Mounts [`guard`] on `router` when `state`'s server is open.
pub(crate) fn guarded(router: Router, state: &AppState) -> Router {
    if state.open.is_some() {
        router.layer(axum::middleware::from_fn(guard))
    } else {
        router
    }
}

async fn guard(mut request: Request, next: Next) -> Result<Response, ApiError> {
    if !names_this_machine(&request) {
        return Err(ApiError::Forbidden(
            "this server is open, so it answers only a request whose Host is localhost, \
             127.0.0.1 or [::1]; address it by one of those"
                .into(),
        ));
    }
    if !request.method().is_safe() && sent_without_preflight(request.headers()) {
        return Err(ApiError::Forbidden(
            "this server is open, so it refuses a write sent with no content type, or as \
             text/plain, form data or multipart; send the content type the route names, such as \
             application/json, with an empty body where the route takes none"
                .into(),
        ));
    }
    if from_another_site(request.headers()) {
        return Err(ApiError::Forbidden(
            "this server is open, so it answers a browser only for a page served from \
             localhost, 127.0.0.1 or [::1]; serve the page from this machine"
                .into(),
        ));
    }
    request.extensions_mut().insert(Guarded);
    Ok(next.run(request).await)
}

/// Whether the one `Host` header and the request line's authority, of those present, both name
/// this machine. A request with neither, or with two `Host` headers, does not.
fn names_this_machine(request: &Request) -> bool {
    let mut hosts = request.headers().get_all(HOST).iter();
    let header = match (hosts.next(), hosts.next()) {
        (Some(one), None) => match one.to_str() {
            Ok(host) => Some(host),
            Err(_) => return false,
        },
        (None, _) => None,
        (Some(_), Some(_)) => return false,
    };
    let line = request.uri().authority().map(|a| a.as_str());
    (header.is_some() || line.is_some())
        && header
            .into_iter()
            .chain(line)
            .all(crate::cors::is_loopback_authority)
}

/// Whether there is no `Content-Type`, or one names, ignoring case and parameters, a type in
/// [`SENT_WITHOUT_PREFLIGHT`], or is not text at all.
fn sent_without_preflight(headers: &HeaderMap) -> bool {
    let mut types = headers.get_all(CONTENT_TYPE).iter().peekable();
    types.peek().is_none()
        || types.any(|value| {
            let Ok(value) = value.to_str() else {
                return true;
            };
            let essence = value
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            SENT_WITHOUT_PREFLIGHT.contains(&essence.as_str())
        })
}

/// Whether a browser sent this from a page not on this machine: an `Origin` that is not a loopback
/// origin, or, with no `Origin`, `Sec-Fetch-Site: cross-site`.
fn from_another_site(headers: &HeaderMap) -> bool {
    let mut origins = headers.get_all(ORIGIN).iter().peekable();
    if origins.peek().is_some() {
        return !origins.all(crate::cors::is_loopback_origin);
    }
    headers
        .get_all("sec-fetch-site")
        .iter()
        .any(|site| site.as_bytes().eq_ignore_ascii_case(b"cross-site"))
}
