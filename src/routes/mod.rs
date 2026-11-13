//! HTTP and websocket routes.

pub mod osm;
pub mod room;
pub mod v2;

use axum::{Json, Router, extract::DefaultBodyLimit, middleware, routing::get};
use serde_json::{Value, json};
use tower_http::compression::CompressionLayer;

use crate::{assets, guard, state::AppState};

/// Large enough for a bulk import of a few thousand assets, or an exported
/// exercise with a long history.
const BODY_LIMIT: usize = 16 * 1024 * 1024;

pub fn router(app: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .merge(v2::router(app.clone()))
        .merge(room::router(app.clone()))
        .fallback(assets::serve)
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        // The bundle and large snapshots shrink about fourfold, which matters
        // once phones join over venue Wi-Fi.
        .layer(CompressionLayer::new())
        .layer(middleware::from_fn_with_state(app.clone(), guard::local_only))
        .with_state(app)
}

async fn health() -> Json<Value> {
    Json(json!({ "ok": true, "version": env!("CARGO_PKG_VERSION") }))
}
