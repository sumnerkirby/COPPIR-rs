//! `/api/v2`: exercises, their commands, and their live event streams.

use axum::{
    Json, Router,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::error::RecvError;
use ts_rs::TS;

use crate::{
    error::ApiError,
    exercise::{self, Command, Envelope, Exercise, import_v1, legacy, model::Taxonomy, taxonomy},
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
        .route("/api/v2/exercises/import", post(import_file))
        .route("/api/v2/exercises/{id}/export", get(export))
        .route("/api/v2/exercises/{id}/duplicate", post(duplicate))
        .route("/api/v2/legacy/scenarios", get(legacy_list))
        .route("/api/v2/legacy/scenarios/import", post(legacy_import))
        .route("/api/v2/geocode", post(super::osm::geocode))
        .route("/api/v2/osm/categories", get(super::osm::categories))
        .route("/api/v2/osm/search", post(super::osm::search))
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

/// The `.coppir` file format: one exercise, as exported.
#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ExerciseFile {
    /// Always `coppir.exercise`.
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub exercise: Exercise,
}

pub const FILE_FORMAT: &str = "coppir.exercise";
pub const FILE_VERSION: u32 = 1;

#[derive(Deserialize, TS)]
#[ts(export)]
pub struct ImportFile {
    #[ts(type = "unknown")]
    pub file: serde_json::Value,
    /// Overrides the name in the file.
    #[serde(default)]
    #[ts(optional)]
    pub name: Option<String>,
}

impl Validate for ImportFile {
    fn check(&mut self, _: &mut Checks) {}
}

#[derive(Deserialize, TS)]
#[ts(export)]
pub struct Named {
    #[serde(default)]
    #[ts(optional)]
    pub name: Option<String>,
}

impl Validate for Named {
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

async fn import_file(
    State(app): State<AppState>,
    Valid(req): Valid<ImportFile>,
) -> Result<(StatusCode, Json<Snapshot>), ApiError> {
    if req.file.get("format").and_then(|f| f.as_str()) != Some(FILE_FORMAT) {
        return Err(ApiError::BadRequest("Not a COPPIR exercise file (.coppir)".into()));
    }
    let file: ExerciseFile = serde_json::from_value(req.file)
        .map_err(|e| ApiError::BadRequest(format!("The exercise file is damaged: {e}")))?;
    if file.version > FILE_VERSION {
        return Err(ApiError::BadRequest("This file is from a newer COPPIR; upgrade to open it".into()));
    }
    let ex = exercise::adopt(file.exercise, req.name.as_deref(), &now())?;
    let (exercise, seq) = app.live.create(ex, FACILITATOR)?;
    Ok((StatusCode::CREATED, Json(Snapshot { seq, exercise })))
}

/// Download an exercise as a `.coppir` file: its current state, without the
/// event history.
async fn export(State(app): State<AppState>, Path(id): Path<String>) -> Result<Response, ApiError> {
    let (exercise, _) = app.live.snapshot(&id)?;
    let filename: String = exercise
        .name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ' ') { c } else { '_' })
        .collect();
    let file = ExerciseFile { format: FILE_FORMAT.into(), version: FILE_VERSION, exported_at: now(), exercise };
    let body = serde_json::to_vec_pretty(&file).map_err(ApiError::internal)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/json".to_owned()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}.coppir\"", filename.trim())),
        ],
        body,
    )
        .into_response())
}

/// A copy of an exercise as it stands now, with a fresh history. Use it to
/// run the same exercise again, or to start from a template.
async fn duplicate(
    State(app): State<AppState>,
    Path(id): Path<String>,
    Valid(req): Valid<Named>,
) -> Result<(StatusCode, Json<Snapshot>), ApiError> {
    let (source, _) = app.live.snapshot(&id)?;
    let name = req.name.unwrap_or_else(|| format!("{} (copy)", source.name));
    let ex = exercise::adopt(source, Some(&name), &now())?;
    let (exercise, seq) = app.live.create(ex, FACILITATOR)?;
    Ok((StatusCode::CREATED, Json(Snapshot { seq, exercise })))
}

/// Scenario files the v1 app saved, offered for import.
async fn legacy_list(State(app): State<AppState>) -> Json<Vec<String>> {
    let mut names: Vec<String> = std::fs::read_dir(&app.config.scenarios_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            (p.extension()? == "json").then(|| p.file_stem()?.to_str().map(str::to_owned)).flatten()
        })
        .filter(|n| !n.starts_with('.') && legacy::safe_name(n) == *n)
        .collect();
    names.sort();
    Json(names)
}

#[derive(Deserialize, TS)]
#[ts(export)]
pub struct LegacyImport {
    pub name: String,
}

impl Validate for LegacyImport {
    fn check(&mut self, _: &mut Checks) {}
}

async fn legacy_import(
    State(app): State<AppState>,
    Valid(req): Valid<LegacyImport>,
) -> Result<(StatusCode, Json<Snapshot>), ApiError> {
    // Only names the v1 app could have written, so the path stays inside the folder.
    let safe = legacy::safe_name(&req.name);
    if safe.is_empty() || safe != req.name {
        return Err(ApiError::BadRequest("Invalid scenario name".into()));
    }
    let path = app.config.scenarios_dir.join(format!("{safe}.json"));
    let bytes = match tokio::fs::read(&path).await {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(ApiError::NotFound("Scenario not found")),
        Err(e) => return Err(ApiError::internal(e)),
    };
    let json: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| ApiError::BadRequest(format!("Scenario file is damaged: {e}")))?;
    let ex = import_v1::import(&json, &safe, &now())?;
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
