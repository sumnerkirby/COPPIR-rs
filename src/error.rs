//! The one error type every handler returns.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::validate::FieldError;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    /// Not signed in to the room: the client should go and join.
    Unauthorized,
    Forbidden(&'static str),
    TooManyRequests(&'static str),
    NotFound(&'static str),
    Conflict(&'static str),
    UnsupportedMediaType,
    Validation(Vec<FieldError>),
    /// A third-party service (Nominatim, Overpass) failed us.
    Upstream(String),
    Internal(String),
}

impl ApiError {
    pub fn internal(e: impl std::fmt::Display) -> Self {
        Self::Internal(e.to_string())
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ApiError {}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, detail) = match self {
            Self::BadRequest(m) => (StatusCode::BAD_REQUEST, json!(m)),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, json!("Join the room first")),
            Self::Forbidden(m) => (StatusCode::FORBIDDEN, json!(m)),
            Self::TooManyRequests(m) => (StatusCode::TOO_MANY_REQUESTS, json!(m)),
            Self::NotFound(m) => (StatusCode::NOT_FOUND, json!(m)),
            Self::Conflict(m) => (StatusCode::CONFLICT, json!(m)),
            Self::UnsupportedMediaType => {
                (StatusCode::UNSUPPORTED_MEDIA_TYPE, json!("Expected Content-Type: application/json"))
            }
            Self::Validation(errors) => (StatusCode::UNPROCESSABLE_ENTITY, json!(errors)),
            Self::Upstream(m) => (StatusCode::BAD_GATEWAY, json!(m)),
            Self::Internal(m) => {
                tracing::error!("internal error: {m}");
                (StatusCode::INTERNAL_SERVER_ERROR, json!("Internal error"))
            }
        };
        (status, Json(json!({ "detail": detail }))).into_response()
    }
}
