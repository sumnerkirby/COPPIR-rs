//! Alert thresholds watching sector integrity.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde_json::{Value, json};

use crate::{
    error::ApiError,
    models::{Threshold, ThresholdIn},
    state::{AppState, Event, new_id},
    validate::Valid,
};

pub async fn list(State(app): State<AppState>) -> Json<Vec<Threshold>> {
    Json(app.hub.read(|s| s.thresholds.clone()))
}

pub async fn create(State(app): State<AppState>, Valid(t): Valid<ThresholdIn>) -> (StatusCode, Json<Threshold>) {
    let rec = Threshold {
        id: new_id(),
        name: t.name,
        sector: t.sector,
        below_pct: t.below_pct.try_into().expect("validated to 0..=100"),
        severity: t.severity,
    };
    app.hub.with(|s, tx| {
        s.thresholds.push(rec.clone());
        tx.send(&Event::ThresholdAdd { threshold: &rec });
    });
    (StatusCode::CREATED, Json(rec))
}

pub async fn delete(State(app): State<AppState>, Path(tid): Path<String>) -> Result<Json<Value>, ApiError> {
    app.hub.with(|s, tx| {
        let i = s.thresholds.iter().position(|t| t.id == tid).ok_or(ApiError::NotFound("Threshold not found"))?;
        s.thresholds.remove(i);
        tx.send(&Event::ThresholdDelete { tid: &tid });
        Ok(Json(json!({ "ok": true })))
    })
}
