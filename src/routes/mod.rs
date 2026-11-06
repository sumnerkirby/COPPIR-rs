//! HTTP and websocket routes, grouped by the resource they act on.

pub mod edges;
pub mod injects;
pub mod log;
pub mod osm;
pub mod pins;
pub mod scenarios;
pub mod thresholds;
pub mod ws;

use axum::{
    Json, Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{delete, get, post, put},
};
use serde_json::{Value, json};

use crate::{assets, guard, state::AppState};

/// Large enough for a bulk import of a few thousand pins.
const BODY_LIMIT: usize = 4 * 1024 * 1024;

pub fn router(app: AppState) -> Router {
    let api = Router::new()
        .route("/api/health", get(health))
        .route("/api/pins", get(pins::list).post(pins::create).delete(pins::clear))
        .route("/api/pins/bulk", post(pins::bulk_create))
        .route("/api/pins/bulk-status", post(pins::bulk_status))
        .route("/api/pins/{pid}", put(pins::update).delete(pins::delete))
        .route("/api/edges", get(edges::list).post(edges::create))
        .route("/api/edges/{eid}", delete(edges::delete))
        .route("/api/injects", get(injects::list).post(injects::create))
        .route("/api/injects/{iid}", delete(injects::delete))
        .route("/api/injects/{iid}/trigger", post(injects::trigger))
        .route("/api/log", get(log::list).post(log::create))
        .route("/api/scenarios", get(scenarios::list))
        .route("/api/scenarios/save", post(scenarios::save))
        .route("/api/scenarios/load", post(scenarios::load))
        .route("/api/scenarios/{name}", delete(scenarios::delete))
        .route("/api/state/clear", post(scenarios::clear_state))
        .route("/api/thresholds", get(thresholds::list).post(thresholds::create))
        .route("/api/thresholds/{tid}", delete(thresholds::delete))
        .route("/api/geocode", post(osm::geocode))
        .route("/api/search/osm", post(osm::search))
        .route("/ws", get(ws::upgrade));

    api.fallback(assets::serve)
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(middleware::from_fn(guard::local_only))
        .with_state(app)
}

async fn health() -> Json<Value> {
    Json(json!({ "ok": true, "version": env!("CARGO_PKG_VERSION") }))
}
