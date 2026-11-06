//! `/api/v2`: exercises, their commands, and their live event streams.

use axum::{
    Json, Router,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::StatusCode,
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::error::RecvError;
use ts_rs::TS;

use crate::{
    error::ApiError,
    exercise::{self, Command, Envelope, Exercise, import_v1, model::Taxonomy, taxonomy},
    live::Applied,
    state::{AppState, now},
    store::ExerciseSummary,
    validate::{Checks, Valid, Validate},
};

/// Every change is attributed. Until player sessions arrive, the only actor is
/// whoever is at the facilitator's keyboard.
const FACILITATOR: &str = "facilitator";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/taxonomy/default", get(default_taxonomy))
        .route("/api/v2/exercises", get(list).post(create))
        .route("/api/v2/exercises/import/v1", post(import))
        .route("/api/v2/exercises/{id}", get(snapshot).delete(remove))
        .route("/api/v2/exercises/{id}/commands", post(command))
        .route("/api/v2/exercises/{id}/events", get(events))
        .route("/api/v2/exercises/{id}/ws", get(ws))
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct Snapshot {
    #[ts(type = "number")]
    pub seq: u64,
    pub exercise: Exercise,
}

#[derive(Deserialize, TS)]
#[ts(export)]
pub struct CreateExercise {
    pub name: String,
    #[serde(default)]
    #[ts(optional)]
    pub description: Option<String>,
}

impl Validate for CreateExercise {
    fn check(&mut self, _: &mut Checks) {}
}

#[derive(Deserialize, TS)]
#[ts(export)]
pub struct ImportV1 {
    pub name: String,
    /// The scenario file's contents, as saved by v1 or the Python app.
    #[ts(type = "unknown")]
    pub scenario: serde_json::Value,
}

impl Validate for ImportV1 {
    fn check(&mut self, _: &mut Checks) {}
}

#[derive(Deserialize)]
pub struct After {
    after: Option<u64>,
}

async fn default_taxonomy() -> Json<Taxonomy> {
    Json(taxonomy::default_taxonomy())
}

async fn list(State(app): State<AppState>) -> Result<Json<Vec<ExerciseSummary>>, ApiError> {
    Ok(Json(app.live.store().list()?))
}

async fn create(
    State(app): State<AppState>,
    Valid(req): Valid<CreateExercise>,
) -> Result<(StatusCode, Json<Snapshot>), ApiError> {
    let ex = exercise::blank(&req.name, req.description.as_deref().unwrap_or(""), &now())?;
    let (exercise, seq) = app.live.create(ex, FACILITATOR)?;
    Ok((StatusCode::CREATED, Json(Snapshot { seq, exercise })))
}

async fn import(
    State(app): State<AppState>,
    Valid(req): Valid<ImportV1>,
) -> Result<(StatusCode, Json<Snapshot>), ApiError> {
    let ex = import_v1::import(&req.scenario, &req.name, &now())?;
    let (exercise, seq) = app.live.create(ex, FACILITATOR)?;
    Ok((StatusCode::CREATED, Json(Snapshot { seq, exercise })))
}

async fn snapshot(State(app): State<AppState>, Path(id): Path<String>) -> Result<Json<Snapshot>, ApiError> {
    let (exercise, seq) = app.live.snapshot(&id)?;
    Ok(Json(Snapshot { seq, exercise }))
}

async fn remove(State(app): State<AppState>, Path(id): Path<String>) -> Result<StatusCode, ApiError> {
    app.live.delete(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn command(
    State(app): State<AppState>,
    Path(id): Path<String>,
    Valid(cmd): Valid<Command>,
) -> Result<Json<Applied>, ApiError> {
    Ok(Json(app.live.execute(&id, FACILITATOR, cmd)?))
}

async fn events(
    State(app): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<After>,
) -> Result<Json<Vec<Envelope>>, ApiError> {
    app.live.room(&id)?; // 404 for an unknown exercise rather than an empty list
    Ok(Json(app.live.store().events_after(&id, q.after.unwrap_or(0))?))
}

/// Connect with `?after=<seq>` to resume: the server sends only the events
/// after it, or a fresh snapshot if that is too far back.
async fn ws(
    upgrade: WebSocketUpgrade,
    State(app): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<After>,
) -> Result<Response, ApiError> {
    let (initial, rx) = app.live.subscribe(&id, q.after)?;
    Ok(upgrade.on_upgrade(move |socket| client(socket, app, id, initial, rx)))
}

async fn client(
    mut socket: WebSocket,
    app: AppState,
    id: String,
    initial: Vec<axum::extract::ws::Utf8Bytes>,
    mut rx: tokio::sync::broadcast::Receiver<axum::extract::ws::Utf8Bytes>,
) {
    for msg in initial {
        if socket.send(Message::Text(msg)).await.is_err() {
            return;
        }
    }
    loop {
        tokio::select! {
            msg = rx.recv() => {
                let text = match msg {
                    Ok(text) => text,
                    Err(RecvError::Lagged(_)) => {
                        // Too far behind to replay from the channel: resubscribe
                        // and send a fresh snapshot.
                        let Ok((initial, fresh)) = app.live.subscribe(&id, None) else { break };
                        rx = fresh;
                        let Some(snap) = initial.into_iter().next() else { break };
                        snap
                    }
                    // The exercise was deleted.
                    Err(RecvError::Closed) => break,
                };
                if socket.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
        }
    }
}
