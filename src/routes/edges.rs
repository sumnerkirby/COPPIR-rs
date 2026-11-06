//! Edges: dependencies and links drawn between two pins.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde_json::{Value, json};

use crate::{
    error::ApiError,
    models::{Edge, EdgeIn},
    state::{AppState, Event, new_id},
    validate::Valid,
};

pub async fn list(State(app): State<AppState>) -> Json<Vec<Edge>> {
    Json(app.hub.read(|s| s.edges.values().cloned().collect()))
}

pub async fn create(
    State(app): State<AppState>,
    Valid(e): Valid<EdgeIn>,
) -> Result<(StatusCode, Json<Edge>), ApiError> {
    if e.from_pid == e.to_pid {
        return Err(ApiError::BadRequest("An edge has to join two different pins".into()));
    }
    app.hub.with(|s, tx| {
        if !s.pins.contains_key(&e.from_pid) || !s.pins.contains_key(&e.to_pid) {
            return Err(ApiError::BadRequest("Both pins must exist".into()));
        }
        let edge =
            Edge { id: new_id(), from_pid: e.from_pid, to_pid: e.to_pid, label: e.label, edge_type: e.edge_type };
        s.edges.insert(edge.id.clone(), edge.clone());
        tx.send(&Event::EdgeAdd { edge: &edge });
        Ok((StatusCode::CREATED, Json(edge)))
    })
}

pub async fn delete(State(app): State<AppState>, Path(eid): Path<String>) -> Result<Json<Value>, ApiError> {
    app.hub.with(|s, tx| {
        s.edges.shift_remove(&eid).ok_or(ApiError::NotFound("Edge not found"))?;
        tx.send(&Event::EdgeDelete { eid: &eid });
        Ok(Json(json!({ "ok": true })))
    })
}
