//! Keeps the API reachable from this machine's own pages and nothing else.
//!
//! Binding to 127.0.0.1 stops other machines, but not other *websites*: any
//! page open in the user's browser can send requests to localhost. Two checks
//! close that:
//!
//! - **Host** must name a loopback address. A DNS-rebinding page reaches us
//!   with its own hostname in `Host`, so it is refused before it can read
//!   anything.
//! - **Origin**, when the browser sends one, must be this server. That covers
//!   cross-site `fetch`, form posts to body-less routes like
//!   `/api/state/clear`, and websockets, which CORS never protected — the
//!   Python version would hand its full state to any page that opened `/ws`.
//!
//! Requests without an `Origin` (curl, the seed script, same-origin GETs) pass.

use axum::{
    extract::Request,
    http::header::{HOST, ORIGIN},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::error::ApiError;

pub async fn local_only(req: Request, next: Next) -> Response {
    let headers = req.headers();
    let Some(host) = headers.get(HOST).and_then(|h| h.to_str().ok()) else {
        return ApiError::BadRequest("Missing Host header".into()).into_response();
    };
    if !is_loopback_host(host) {
        return ApiError::Forbidden("Host not allowed").into_response();
    }
    if let Some(origin) = headers.get(ORIGIN) {
        let same = origin.to_str().ok().and_then(|o| o.strip_prefix("http://")).is_some_and(|o| o == host);
        if !same {
            return ApiError::Forbidden("Cross-origin request refused").into_response();
        }
    }
    next.run(req).await
}

/// `localhost`, `127.0.0.1` or `[::1]`, with or without a port.
pub fn is_loopback_host(host: &str) -> bool {
    let name = if let Some(rest) = host.strip_prefix('[') {
        // [::1]:8000
        match rest.split_once(']') {
            Some((addr, tail)) if tail.is_empty() || tail.starts_with(':') => return addr == "::1",
            _ => return false,
        }
    } else {
        host.split_once(':').map_or(host, |(name, _)| name)
    };
    name.eq_ignore_ascii_case("localhost") || name == "127.0.0.1"
}

#[cfg(test)]
mod tests {
    use super::is_loopback_host;

    #[test]
    fn loopback_hosts() {
        for ok in ["localhost", "localhost:8000", "LOCALHOST:1", "127.0.0.1", "127.0.0.1:8000", "[::1]", "[::1]:8000"] {
            assert!(is_loopback_host(ok), "{ok}");
        }
        for bad in [
            "evil.com",
            "evil.com:8000",
            "127.0.0.1.evil.com",
            "localhost.evil.com",
            "[::2]:80",
            "[::1]x",
            "10.0.0.1:8000",
            "",
        ] {
            assert!(!is_loopback_host(bad), "{bad}");
        }
    }
}
