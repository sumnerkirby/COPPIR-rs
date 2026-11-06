//! The frontend, compiled into the binary.
//!
//! In release builds `static/` is embedded, so the executable is the whole
//! app. In debug builds rust-embed reads from disk instead, so frontend edits
//! show up on reload without recompiling.

use axum::{
    http::{
        HeaderMap, HeaderValue, StatusCode, Uri,
        header::{CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_NONE_MATCH},
    },
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "static/"]
struct Static;

pub async fn serve(uri: Uri, headers: HeaderMap) -> Response {
    let mut path = uri.path().trim_start_matches('/').to_owned();
    if path.is_empty() || path.ends_with('/') {
        path.push_str("index.html");
    }
    let Some(file) = Static::get(&path) else {
        return (StatusCode::NOT_FOUND, "Not found").into_response();
    };

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
