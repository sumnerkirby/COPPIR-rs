//! The frontend, compiled into the binary.
//!
//! In release builds `web/dist/` is embedded, so the executable is the whole
//! app. In debug builds rust-embed reads from disk instead, so a fresh
//! `npm run build` shows up on reload without recompiling.

use axum::{
    http::{
        HeaderMap, HeaderValue, StatusCode, Uri,
        header::{CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_NONE_MATCH},
    },
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

/// The frontend, built by `npm run build` in `web/`. Until it has been built
/// the app answers with instructions instead.
#[derive(RustEmbed)]
#[folder = "web/dist/"]
#[allow_missing = true]
struct Web;

/// The single-page app. Unknown paths get `index.html` so the client-side
/// router can handle them; unknown *files* are a 404.
pub async fn serve(uri: Uri, headers: HeaderMap) -> Response {
    let path = uri.path().trim_start_matches('/');
    if let Some(file) = Web::get(path).filter(|_| !path.is_empty()) {
        return respond(file, &headers);
    }
    let looks_like_file = path.rsplit('/').next().is_some_and(|last| last.contains('.'));
    if looks_like_file || path.starts_with("api/") {
        return (StatusCode::NOT_FOUND, "Not found").into_response();
    }
    match Web::get("index.html") {
        Some(index) => respond(index, &headers),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            "The interface has not been built. Run `npm --prefix web ci && npm --prefix web run build`, then rebuild.",
        )
            .into_response(),
    }
}

fn respond(file: rust_embed::EmbeddedFile, headers: &HeaderMap) -> Response {
    let etag = format!("\"{}\"", hex(&file.metadata.sha256_hash()[..16]));
    // `no-cache` still lets the webview keep a copy; it just has to revalidate,
    // which costs a 304 and means an upgraded binary is never served stale.
    let mut res = if headers.get(IF_NONE_MATCH).is_some_and(|v| v.as_bytes() == etag.as_bytes()) {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        let mime = file.metadata.mimetype().to_owned();
        let mut r = file.data.into_owned().into_response();
        r.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_str(&mime).unwrap_or(HeaderValue::from_static("application/octet-stream")),
        );
        r
    };
    let h = res.headers_mut();
    h.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    if let Ok(v) = HeaderValue::from_str(&etag) {
        h.insert(ETAG, v);
    }
    res
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
