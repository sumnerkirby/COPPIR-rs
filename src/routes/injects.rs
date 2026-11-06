//! Injects: pre-authored events a controller fires during an exercise.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde_json::{Value, json};

use crate::{
    error::ApiError,
    models::{Inject, InjectIn, LogRecord, OpStatus, Pin, StatusChange},
    state::{AppState, Event, new_id, now},
    validate::Valid,
};

pub async fn list(State(app): State<AppState>) -> Json<Vec<Inject>> {
    Json(app.hub.read(|s| s.injects.values().cloned().collect()))
}

pub async fn create(State(app): State<AppState>, Valid(i): Valid<InjectIn>) -> (StatusCode, Json<Inject>) {
    let inject = Inject {
        id: new_id(),
        title: i.title,
        description: i.description,
        severity: i.severity,
        target_pid: i.target_pid,
        target_status: i.target_status,
        new_pin: i.new_pin,
        created_at: now(),
        triggered_at: None,
    };
    app.hub.with(|s, tx| {
        s.injects.insert(inject.id.clone(), inject.clone());
        tx.send(&Event::InjectQueued { inject: &inject });
    });
    (StatusCode::CREATED, Json(inject))
}

pub async fn delete(State(app): State<AppState>, Path(iid): Path<String>) -> Result<Json<Value>, ApiError> {
    app.hub.with(|s, tx| {
        s.injects.shift_remove(&iid).ok_or(ApiError::NotFound("Inject not found"))?;
        tx.send(&Event::InjectDelete { iid: &iid });
        Ok(Json(json!({ "ok": true })))
    })
}

/// Fire an inject. In one step, and one broadcast: move the target pin to its
/// new status, spawn the inject's new pin, and write an automatic log entry.
pub async fn trigger(State(app): State<AppState>, Path(iid): Path<String>) -> Result<Json<Inject>, ApiError> {
    let at = now();
    app.hub.with(|s, tx| {
        let inject = s.injects.get_mut(&iid).ok_or(ApiError::NotFound("Inject not found"))?;
        if inject.triggered_at.is_some() {
            // Firing twice would spawn a second copy of new_pin and write a
            // second log entry. The UI hides the button once fired, but a
            // second window or a double-click can still get here.
            return Err(ApiError::Conflict("Inject has already been triggered"));
        }
        inject.triggered_at = Some(at.clone());
        let inject = inject.clone();

        // A target deleted before the inject fires is tolerated, not an error.
        let target = inject.target_pid.as_deref().and_then(|pid| s.pins.get_mut(pid));
        let asset_name = match (target, inject.target_status) {
            (Some(pin), Some(status)) => {
                pin.set_status(status, &at);
                Some(pin.name.clone())
            }
            (Some(pin), None) => Some(pin.name.clone()),
            (None, _) => None,
        };

        let new_pin = inject.new_pin.as_ref().map(|np| Pin {
            id: new_id(),
            name: np.name.clone(),
            category: np.category.clone(),
            lat: np.lat,
            lon: np.lon,
            status: np.status,
            op_status: OpStatus::Healthy,
            pin_color: None,
            notes: String::new(),
            pinned_at: at.clone(),
            status_history: vec![StatusChange { status: np.status, timestamp: at.clone() }],
        });
        if let Some(pin) = &new_pin {
            s.pins.insert(pin.id.clone(), pin.clone());
        }

        let entry = LogRecord {
            id: new_id(),
            timestamp: at.clone(),
            action: format!("INJECT TRIGGERED: {}", inject.title),
            asset_name,
            asset_pid: inject.target_pid.clone(),
            notes: inject.description.clone(),
            auto: true,
        };
        s.decision_log.push(entry);

        tx.send(&Event::InjectTriggered {
            inject: &inject,
            log_entry: s.decision_log.last().expect("just pushed"),
            pins: &s.pins,
            new_pin: new_pin.as_ref(),
        });
        Ok(Json(inject))
    })
}
