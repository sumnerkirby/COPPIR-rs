//! HTTP and websocket routes.

pub mod osm;
pub mod v2;

use axum::{Json, Router, extract::DefaultBodyLimit, middleware, routing::get};
use serde_json::{Value, json};

use crate::{assets, guard, state::AppState};

/// Large enough for a bulk import of a few thousand assets, or an exported
/// exercise with a long history.
const BODY_LIMIT: usize = 16 * 1024 * 1024;

pub fn router(app: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .merge(v2::router())
        .fallback(assets::serve)
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(middleware::from_fn(guard::local_only))
        .with_state(app)
}

async fn health() -> Json<Value> {
    Json(json!({ "ok": true, "version": env!("CARGO_PKG_VERSION") }))
}
