//! The decision log.

use axum::{Json, extract::State, http::StatusCode};

use crate::{
    models::{LogEntryIn, LogRecord},
    state::{AppState, Event, new_id, now},
    validate::Valid,
};

pub async fn list(State(app): State<AppState>) -> Json<Vec<LogRecord>> {
    Json(app.hub.read(|s| s.decision_log.clone()))
}

pub async fn create(State(app): State<AppState>, Valid(e): Valid<LogEntryIn>) -> (StatusCode, Json<LogRecord>) {
    let rec = LogRecord {
        id: new_id(),
        timestamp: now(),
        action: e.action,
        asset_name: e.asset_name,
        asset_pid: e.asset_pid,
        notes: e.notes,
        auto: false,
    };
    app.hub.with(|s, tx| {
        s.decision_log.push(rec.clone());
        tx.send(&Event::LogEntry { entry: &rec });
    });
    (StatusCode::CREATED, Json(rec))
}
