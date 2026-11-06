//! Pins: the assets, responders and points of interest on the map.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use indexmap::IndexMap;
use serde_json::{Value, json};

use crate::{
    error::ApiError,
    models::{BulkCreate, BulkStatusReq, Pin, PinIn, PinUpdate, StatusChange},
    state::{AppState, Event, new_id, now},
    validate::Valid,
};

pub async fn list(State(app): State<AppState>) -> Json<IndexMap<String, Pin>> {
    Json(app.hub.read(|s| s.pins.clone()))
}

pub async fn create(State(app): State<AppState>, Valid(p): Valid<PinIn>) -> (StatusCode, Json<Pin>) {
    let at = now();
    let pin = Pin {
        id: new_id(),
        name: p.name,
        category: p.category,
        lat: p.lat,
        lon: p.lon,
        status: p.status,
        op_status: p.op_status,
        pin_color: p.pin_color,
        notes: p.notes,
        status_history: vec![StatusChange { status: p.status, timestamp: at.clone() }],
        pinned_at: at,
    };
    app.hub.with(|s, tx| {
        s.pins.insert(pin.id.clone(), pin.clone());
        tx.send(&Event::PinAdd { pin: &pin });
    });
    (StatusCode::CREATED, Json(pin))
}

pub async fn bulk_create(State(app): State<AppState>, Valid(body): Valid<BulkCreate>) -> (StatusCode, Json<Vec<Pin>>) {
    let at = now();
    let created: Vec<Pin> = body
        .items
        .into_iter()
        .map(|it| Pin {
            id: new_id(),
            name: it.name,
            category: it.category,
            lat: it.lat,
            lon: it.lon,
            status: body.status,
            op_status: body.op_status,
            pin_color: body.pin_color.clone(),
            notes: String::new(),
            pinned_at: at.clone(),
            status_history: vec![StatusChange { status: body.status, timestamp: at.clone() }],
        })
        .collect();
    app.hub.with(|s, tx| {
        for pin in &created {
            s.pins.insert(pin.id.clone(), pin.clone());
        }
        tx.send(&Event::BulkAdd { pins: created.iter().collect() });
    });
    (StatusCode::CREATED, Json(created))
}

pub async fn update(
    State(app): State<AppState>,
    Path(pid): Path<String>,
    Valid(upd): Valid<PinUpdate>,
) -> Result<Json<Pin>, ApiError> {
    app.hub.with(|s, tx| {
        let pin = s.pins.get_mut(&pid).ok_or(ApiError::NotFound("Pin not found"))?;
        if let Some(v) = upd.name {
            pin.name = v;
        }
        if let Some(v) = upd.category {
            pin.category = v;
        }
        if let Some(v) = upd.status {
            pin.set_status(v, &now());
        }
        if let Some(v) = upd.op_status {
            pin.op_status = v;
        }
        if let Some(v) = upd.pin_color {
            pin.pin_color = Some(v);
        }
        if let Some(v) = upd.notes {
            pin.notes = v;
        }
        tx.send(&Event::PinUpdate { pin });
        Ok(Json(pin.clone()))
    })
}

pub async fn delete(State(app): State<AppState>, Path(pid): Path<String>) -> Result<Json<Value>, ApiError> {
    app.hub.with(|s, tx| {
        s.pins.shift_remove(&pid).ok_or(ApiError::NotFound("Pin not found"))?;
        // An edge cannot outlive either pin it joins. Clients drop these on
        // their own when they see pin_delete, but the server has to as well or
        // they survive into every later full_state and every saved scenario.
        s.edges.retain(|_, e| e.from_pid != pid && e.to_pid != pid);
        tx.send(&Event::PinDelete { pid: &pid });
        Ok(Json(json!({ "ok": true })))
    })
}

pub async fn clear(State(app): State<AppState>) -> Json<Value> {
    app.hub.with(|s, tx| {
        let pids: Vec<String> = s.pins.drain(..).map(|(id, _)| id).collect();
        s.edges.clear();
        let count = pids.len();
        tx.send(&Event::PinsCleared { pids });
        Json(json!({ "ok": true, "count": count }))
    })
}

pub async fn bulk_status(State(app): State<AppState>, Valid(req): Valid<BulkStatusReq>) -> Json<Value> {
    let at = now();
    app.hub.with(|s, tx| {
        let mut updated = Vec::new();
        for pin in s.pins.values_mut() {
            if req.category.as_ref().is_some_and(|c| *c != pin.category) {
                continue;
            }
            if let Some(st) = req.status {
                pin.set_status(st, &at);
            }
            if let Some(op) = req.op_status {
                pin.op_status = op;
            }
            updated.push(&*pin);
        }
        let count = updated.len();
        tx.send(&Event::BulkUpdate { pins: updated });
        Json(json!({ "ok": true, "count": count }))
    })
}
