//! Who is asking.
//!
//! - A request carrying a valid session cookie is that participant.
//! - Otherwise, a request from this machine is the facilitator.
//! - Anything else is nobody, and gets a 401 that sends the client to `/join`.
//!
//! The cookie wins over being local, so a facilitator can open a second
//! browser profile on the same laptop and join as a player to see what
//! players see.

use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{ConnectInfo, FromRequestParts, Request, State},
    http::{HeaderMap, header::COOKIE as COOKIE_HEADER, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{
    error::ApiError,
    exercise::view::Audience,
    room::{COOKIE, Role, Session},
    state::AppState,
};

pub enum Viewer {
    Facilitator,
    Session(Arc<Session>),
}

impl Viewer {
    pub fn role(&self) -> Role {
        match self {
            Viewer::Facilitator => Role::Facilitator,
            Viewer::Session(s) => s.role,
        }
    }

    pub fn audience(&self) -> Audience {
        match self {
            Viewer::Facilitator => Audience::Facilitator,
            Viewer::Session(s) => s.audience(),
        }
    }

    /// Participants may only touch the exercise they joined.
    pub fn check_exercise(&self, id: &str) -> Result<(), ApiError> {
        match self {
            Viewer::Session(s) if s.exercise_id != id => Err(ApiError::Forbidden("Not your exercise")),
            _ => Ok(()),
        }
    }
}

pub fn session_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(COOKIE_HEADER)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(k, _)| *k == COOKIE)
        .map(|(_, v)| v)
}

pub fn identify(headers: &HeaderMap, peer: Option<SocketAddr>, app: &AppState) -> Result<Viewer, ApiError> {
    if let Some(session) = session_token(headers).and_then(|t| app.rooms.session(t)) {
        return Ok(Viewer::Session(session));
    }
    match peer {
        Some(addr) if addr.ip().is_loopback() => Ok(Viewer::Facilitator),
        _ => Err(ApiError::Unauthorized),
    }
}

impl FromRequestParts<AppState> for Viewer {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, app: &AppState) -> Result<Self, Self::Rejection> {
        let peer = parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|c| c.0);
        identify(&parts.headers, peer, app)
    }
}

/// Middleware for routes only the facilitator may use.
pub async fn facilitator_only(State(app): State<AppState>, req: Request, next: Next) -> Response {
    let peer = req.extensions().get::<ConnectInfo<SocketAddr>>().map(|c| c.0);
    match identify(req.headers(), peer, &app) {
        Ok(Viewer::Facilitator) => next.run(req).await,
        Ok(_) => ApiError::Forbidden("Only the facilitator can do that").into_response(),
        Err(e) => e.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn finds_the_session_cookie_among_others() {
        let mut h = HeaderMap::new();
        h.insert(COOKIE_HEADER, HeaderValue::from_static("theme=dark; coppir_session=abc123; other=1"));
        assert_eq!(session_token(&h), Some("abc123"));
        let mut h = HeaderMap::new();
        h.insert(COOKIE_HEADER, HeaderValue::from_static("coppir_session_x=nope"));
        assert_eq!(session_token(&h), None);
        assert_eq!(session_token(&HeaderMap::new()), None);
    }
}
