//! `/api/v2`: exercises, their commands, and their live event streams.

use axum::{
    Json, Router,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::error::RecvError;
use ts_rs::TS;

use crate::{
    auth::{self, Viewer},
    error::ApiError,
    exercise::{
        self, Command, Envelope, Event, Exercise,
        command::Responder,
        import_v1, legacy,
        model::{Taxonomy, Visibility},
        taxonomy,
        view::Audience,
    },
    live::Applied,
    room::{Role, Session},
    state::{AppState, now},
    store::ExerciseSummary,
    validate::{Checks, Valid, Validate},
};

/// The actor recorded for changes made at the facilitator's keyboard.
const FACILITATOR: &str = "facilitator";

pub fn router(app: AppState) -> Router<AppState> {
    // Everything that builds or manages exercises is the facilitator's alone.
    let facilitator = Router::new()
        .route("/api/v2/exercises", get(list).post(create))
        .route("/api/v2/exercises/import/v1", post(import))
        .route("/api/v2/exercises/import", post(import_file))
        .route("/api/v2/exercises/{id}/export", get(export))
        .route("/api/v2/exercises/{id}/duplicate", post(duplicate))
        .route("/api/v2/templates", get(templates))
        .route("/api/v2/templates/{template}", post(from_template))
        .route("/api/v2/legacy/scenarios", get(legacy_list))
        .route("/api/v2/legacy/scenarios/import", post(legacy_import))
        .route("/api/v2/geocode", post(super::osm::geocode))
        .route("/api/v2/osm/categories", get(super::osm::categories))
        .route("/api/v2/osm/search", post(super::osm::search))
        .route("/api/v2/exercises/{id}", delete(remove))
        .route("/api/v2/exercises/{id}/events", get(events))
        .route("/api/v2/exercises/{id}/report", get(report))
        .route_layer(middleware::from_fn_with_state(app, auth::facilitator_only));

    // Participants reach these too; each handler shows them only their view.
    Router::new()
        .route("/api/v2/taxonomy/default", get(default_taxonomy))
        .route("/api/v2/exercises/{id}", get(snapshot))
        .route("/api/v2/exercises/{id}/commands", post(command))
        .route("/api/v2/exercises/{id}/ws", get(ws))
        .merge(facilitator)
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

#[derive(Deserialize)]
pub struct ReportQuery {
    format: Option<String>,
    #[serde(default)]
    download: bool,
}

/// The after-action report: `?format=html` (the default) to read or print,
/// `?format=md` for Markdown. `&download=true` saves it as a file.
async fn report(
    State(app): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<ReportQuery>,
) -> Result<Response, ApiError> {
    let (exercise, _) = app.live.snapshot(&id)?;
    let events = app.live.store().events_after(&id, 0)?;
    let name = file_name(&exercise.name);
    let (format, mime, ext) = match q.format.as_deref() {
        None | Some("html") => (crate::report::Format::Html, "text/html; charset=utf-8", "html"),
        Some("md" | "markdown") => (crate::report::Format::Markdown, "text/markdown; charset=utf-8", "md"),
        Some(_) => return Err(ApiError::BadRequest("format must be html or md".into())),
    };
    let body =
        tokio::task::spawn_blocking(move || crate::report::render(&exercise, &events, &now(), format).into_bytes())
            .await
            .map_err(ApiError::internal)?;
    let disposition = if q.download { "attachment" } else { "inline" };
    Ok((
        [
            (header::CONTENT_TYPE, mime.to_owned()),
            (header::CONTENT_DISPOSITION, format!("{disposition}; filename=\"{name} AAR.{ext}\"")),
            // Everything in it is escaped; this is in case something is not.
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; style-src 'unsafe-inline'; img-src data:".to_owned(),
            ),
        ],
        body,
    )
        .into_response())
}

/// A name safe to put in a `Content-Disposition` header.
fn file_name(name: &str) -> String {
    let s: String =
        name.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ' ') { c } else { '_' }).collect();
    s.trim().to_owned()
}

/// Download an exercise as a `.coppir` file: its current state, without the
/// event history.
async fn export(State(app): State<AppState>, Path(id): Path<String>) -> Result<Response, ApiError> {
    let (exercise, _) = app.live.snapshot(&id)?;
    let filename = file_name(&exercise.name);
    let file = ExerciseFile { format: FILE_FORMAT.into(), version: FILE_VERSION, exported_at: now(), exercise };
    let body = serde_json::to_vec_pretty(&file).map_err(ApiError::internal)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/json".to_owned()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}.coppir\"")),
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

async fn templates() -> Json<Vec<crate::templates::TemplateSummary>> {
    Json(crate::templates::list())
}

/// Start a new exercise from a built-in template.
async fn from_template(
    State(app): State<AppState>,
    Path(template): Path<String>,
    Valid(req): Valid<Named>,
) -> Result<(StatusCode, Json<Snapshot>), ApiError> {
    let source = crate::templates::get(&template).ok_or(ApiError::NotFound("Template not found"))?;
    let ex = exercise::adopt(source, req.name.as_deref(), &now())?;
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

async fn snapshot(
    viewer: Viewer,
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Snapshot>, ApiError> {
    viewer.check_exercise(&id)?;
    let (exercise, seq) = app.live.snapshot_for(&id, &viewer.audience())?;
    Ok(Json(Snapshot { seq, exercise }))
}

async fn remove(State(app): State<AppState>, Path(id): Path<String>) -> Result<StatusCode, ApiError> {
    app.live.delete(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn command(
    viewer: Viewer,
    State(app): State<AppState>,
    Path(id): Path<String>,
    Valid(cmd): Valid<Command>,
) -> Result<Json<Applied>, ApiError> {
    viewer.check_exercise(&id)?;
    match viewer {
        Viewer::Facilitator => Ok(Json(app.live.execute(&id, FACILITATOR, cmd)?)),
        Viewer::Session(s) if s.role == Role::Player => player_command(&app, &id, &s, cmd).map(Json),
        Viewer::Session(_) => Err(ApiError::Forbidden("Observers cannot make changes")),
    }
}

/// Players can add to the log, and nothing else yet. The entry is stamped
/// with who wrote it and is visible to their team and the facilitator.
fn player_command(app: &AppState, id: &str, s: &Session, cmd: Command) -> Result<Applied, ApiError> {
    if let Command::Respond { poll, option, text, .. } = cmd {
        let by = Responder { participant: s.id.clone(), name: s.name(), team: s.team() };
        let cmd = Command::Respond { poll, option, text, by: Some(by) };
        let applied = app.live.execute_for_player(id, &format!("player:{}", s.id), cmd, |_| {})?;
        return Ok(Applied { seq: applied.seq, events: vec![] });
    }
    let Command::Log { asset, .. } = &cmd else {
        return Err(ApiError::Forbidden("Players can only add log entries and answer polls"));
    };
    let audience = s.audience();
    // Naming an asset they cannot see would confirm that it exists.
    if let Some(asset) = asset {
        let (view, _) = app.live.snapshot_for(id, &audience)?;
        if !view.assets.contains_key(asset) {
            return Err(ApiError::Forbidden("You cannot see that asset"));
        }
    }
    let (exercise, _) = app.live.snapshot_for(id, &Audience::Observer)?;
    let team = s.team();
    let team_name = team.as_ref().and_then(|t| exercise.teams.iter().find(|x| &x.id == t)).map(|t| t.name.clone());
    let author = match &team_name {
        Some(t) => format!("{} ({t})", s.name()),
        None => s.name(),
    };
    let visibility = match team {
        Some(t) => Visibility::Teams { teams: vec![t] },
        None => Visibility::All,
    };
    let actor = format!("player:{}", s.id);
    let applied = app.live.execute_for_player(id, &actor, cmd, |events| {
        for e in events {
            if let Event::Logged { entry } = e {
                entry.author.clone_from(&author);
                entry.visibility = visibility.clone();
            }
        }
    })?;
    // The player gets a receipt, not the facilitator's copy of the events.
    Ok(Applied { seq: applied.seq, events: vec![] })
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
/// after it, or a fresh snapshot if that is too far back. Participants always
/// start from a snapshot of their own view.
async fn ws(
    viewer: Viewer,
    upgrade: WebSocketUpgrade,
    State(app): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<After>,
) -> Result<Response, ApiError> {
    viewer.check_exercise(&id)?;
    let session = match viewer {
        Viewer::Facilitator => None,
        Viewer::Session(s) => Some(s),
    };
    let (initial, rx) = subscribe(&app, &id, session.as_deref(), q.after)?;
    Ok(upgrade.on_upgrade(move |socket| client(socket, app, id, session, initial, rx)))
}

type Messages = Vec<axum::extract::ws::Utf8Bytes>;
type Receiver = tokio::sync::broadcast::Receiver<axum::extract::ws::Utf8Bytes>;

fn subscribe(
    app: &AppState,
    id: &str,
    session: Option<&Session>,
    after: Option<u64>,
) -> Result<(Messages, Receiver), ApiError> {
    match session {
        None => app.live.subscribe(id, after),
        Some(s) => app.live.subscribe_view(id, &s.audience()).map(|(snapshot, rx)| (vec![snapshot], rx)),
    }
}

/// Counts a participant's open sockets for as long as it lives.
struct Connected(Option<std::sync::Arc<Session>>);

impl Connected {
    fn new(session: Option<std::sync::Arc<Session>>) -> Self {
        if let Some(s) = &session {
            s.connections.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        Self(session)
    }
}

impl Drop for Connected {
    fn drop(&mut self) {
        if let Some(s) = &self.0 {
            s.connections.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

async fn client(
    mut socket: WebSocket,
    app: AppState,
    id: String,
    session: Option<std::sync::Arc<Session>>,
    initial: Messages,
    mut rx: Receiver,
) {
    let connected = Connected::new(session);
    let session = connected.0.as_deref();
    for msg in initial {
        if socket.send(Message::Text(msg)).await.is_err() {
            return;
        }
    }
    // Kicked, moved to another team, or the room closed: hang up. A moved
    // client reconnects and lands in its new view. The future is created once,
    // before the loop, so a notification between iterations is not missed.
    let changed = async {
        match session {
            Some(s) => s.changed.notified().await,
            None => std::future::pending().await,
        }
    };
    tokio::pin!(changed);
    loop {
        tokio::select! {
            msg = rx.recv() => {
                let text = match msg {
                    Ok(text) => text,
                    Err(RecvError::Lagged(_)) => {
                        // Too far behind to replay from the channel: resubscribe
                        // and send a fresh snapshot.
                        let Ok((initial, fresh)) = subscribe(&app, &id, session, None) else { break };
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
            () = &mut changed => break,
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
        }
    }
}
