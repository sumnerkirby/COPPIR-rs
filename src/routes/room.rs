//! Opening the room, joining it, and managing who is in it.

use std::net::SocketAddr;

use axum::{
    Json, Router,
    extract::{ConnectInfo, Path, Query, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{CACHE_CONTROL, CONTENT_TYPE, SET_COOKIE},
    },
    middleware,
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use serde::{Deserialize, Deserializer, Serialize};
use tokio::{net::TcpListener, sync::watch};
use ts_rs::TS;

use crate::{
    auth::{self, Viewer, session_token},
    error::ApiError,
    exercise::model::Team,
    room::{COOKIE, OpenRoom, Role, RoomStatus, lan_addresses, url_for},
    state::{AppState, now},
    validate::{Checks, Valid, Validate},
};

pub fn router(app: AppState) -> Router<AppState> {
    let facilitator = Router::new()
        .route("/api/v2/room", get(status))
        .route("/api/v2/room/open", post(open))
        .route("/api/v2/room/close", post(close))
        .route("/api/v2/room/codes", post(rotate))
        .route("/api/v2/room/qr.svg", get(qr))
        .route("/api/v2/room/participants/{id}", patch(update).delete(kick))
        .route_layer(middleware::from_fn_with_state(app, auth::facilitator_only));

    Router::new()
        .route("/api/v2/me", get(me))
        .route("/api/v2/join", get(join_info).post(join))
        .route("/api/v2/leave", post(leave))
        .merge(facilitator)
}

// ───────────────────────── facilitator ─────────────────────────

async fn status(State(app): State<AppState>) -> Json<RoomStatus> {
    Json(app.rooms.status())
}

#[derive(Deserialize, TS)]
#[ts(export)]
pub struct OpenRoomReq {
    pub exercise_id: String,
}

impl Validate for OpenRoomReq {
    fn check(&mut self, _: &mut Checks) {}
}

/// Start listening on this machine's network addresses and hand out codes.
async fn open(State(app): State<AppState>, Valid(req): Valid<OpenRoomReq>) -> Result<Json<OpenRoom>, ApiError> {
    app.live.room(&req.exercise_id)?;
    let router = app.router.get().ok_or_else(|| ApiError::internal("the app is not serving yet"))?.clone();
    let port = app.port.get().copied().unwrap_or(0);
    let (stop, stopped) = watch::channel(());
    let mut urls = Vec::new();
    for ip in lan_addresses() {
        // Same port as on this machine when it is free there, so the address
        // is easy to say aloud; any free port otherwise.
        let listener = match TcpListener::bind(SocketAddr::new(ip, port)).await {
            Ok(l) => l,
            Err(_) => match TcpListener::bind(SocketAddr::new(ip, 0)).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::warn!("cannot listen on {ip}: {e}");
                    continue;
                }
            },
        };
        let Ok(addr) = listener.local_addr() else { continue };
        urls.push(url_for(addr));
        let service = router.clone().into_make_service_with_connect_info::<SocketAddr>();
        let mut stopped = stopped.clone();
        tokio::spawn(async move {
            // The sender is dropped when the room closes, which ends the wait.
            let shutdown = async move {
                let _ = stopped.changed().await;
            };
            if let Err(e) = axum::serve(listener, service).with_graceful_shutdown(shutdown).await {
                tracing::warn!("room listener on {addr} stopped: {e}");
            }
        });
        tracing::info!("room open on {}", url_for(addr));
    }
    Ok(Json(app.rooms.open(&req.exercise_id, urls, stop)))
}

async fn close(State(app): State<AppState>) -> StatusCode {
    app.rooms.close();
    tracing::info!("room closed");
    StatusCode::NO_CONTENT
}

async fn rotate(State(app): State<AppState>) -> Result<Json<OpenRoom>, ApiError> {
    Ok(Json(app.rooms.rotate_codes()?))
}

#[derive(Deserialize)]
struct QrQuery {
    #[serde(default)]
    role: Option<String>,
}

/// A QR code for the join link, with the code filled in.
async fn qr(State(app): State<AppState>, Query(q): Query<QrQuery>) -> Result<Response, ApiError> {
    let room = app.rooms.status().open.ok_or(ApiError::Conflict("The room is not open"))?;
    let base = room.urls.first().ok_or(ApiError::Conflict("This machine has no network address to join on"))?;
    let code = if q.role.as_deref() == Some("observer") { &room.observer_code } else { &room.player_code };
    let link = format!("{base}/join?code={code}");
    let svg = qrcode::QrCode::new(link.as_bytes())
        .map_err(ApiError::internal)?
        .render::<qrcode::render::svg::Color<'_>>()
        .min_dimensions(240, 240)
        .quiet_zone(true)
        .build();
    Ok(([(CONTENT_TYPE, "image/svg+xml"), (CACHE_CONTROL, "no-store")], svg).into_response())
}

/// Absent leaves a field alone; `"team": null` takes the player off their team.
#[derive(Deserialize, TS)]
#[ts(export)]
pub struct ParticipantPatch {
    #[serde(default)]
    #[ts(optional)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "some")]
    #[ts(optional)]
    pub team: Option<Option<String>>,
}

fn some<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

impl Validate for ParticipantPatch {
    fn check(&mut self, c: &mut Checks) {
        if let Some(n) = &mut self.name {
            c.label("name", n, 60);
        }
    }
}

async fn update(
    State(app): State<AppState>,
    Path(id): Path<String>,
    Valid(req): Valid<ParticipantPatch>,
) -> Result<StatusCode, ApiError> {
    if let Some(Some(team)) = &req.team {
        let exercise = app.rooms.status().open.ok_or(ApiError::Conflict("The room is not open"))?.exercise_id;
        let (ex, _) = app.live.snapshot(&exercise)?;
        if ex.teams.iter().all(|t| &t.id != team) {
            return Err(ApiError::BadRequest("Unknown team".into()));
        }
    }
    app.rooms.update(&id, req.name, req.team)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn kick(State(app): State<AppState>, Path(id): Path<String>) -> Result<StatusCode, ApiError> {
    app.rooms.kick(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ───────────────────────── everyone ─────────────────────────

/// Who the caller is, so the app can show the right interface.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct Me {
    pub role: Role,
    pub name: String,
    pub team: Option<String>,
    /// The exercise a participant joined. `None` for the facilitator.
    pub exercise_id: Option<String>,
}

async fn me(viewer: Viewer) -> Json<Me> {
    Json(match viewer {
        Viewer::Facilitator => {
            Me { role: Role::Facilitator, name: "Facilitator".into(), team: None, exercise_id: None }
        }
        Viewer::Session(s) => {
            Me { role: s.role, name: s.name(), team: s.team(), exercise_id: Some(s.exercise_id.clone()) }
        }
    })
}

#[derive(Deserialize)]
struct CodeQuery {
    code: String,
}

/// What a code admits to: shown on the join page before the name is entered.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct JoinInfo {
    pub exercise: String,
    pub role: Role,
    pub teams: Vec<Team>,
}

async fn join_info(
    State(app): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Query(q): Query<CodeQuery>,
) -> Result<Json<JoinInfo>, ApiError> {
    let (exercise_id, role) = app.rooms.check_code(&q.code, peer.ip(), crate::room::CodeUse::Look)?;
    let (ex, _) = app.live.snapshot(&exercise_id)?;
    Ok(Json(JoinInfo { exercise: ex.name, role, teams: ex.teams }))
}

#[derive(Deserialize, TS)]
#[ts(export)]
pub struct JoinReq {
    pub code: String,
    pub name: String,
    #[serde(default)]
    #[ts(optional)]
    pub team: Option<String>,
}

impl Validate for JoinReq {
    fn check(&mut self, c: &mut Checks) {
        c.label("name", &mut self.name, 60);
    }
}

async fn join(
    State(app): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Valid(req): Valid<JoinReq>,
) -> Result<Response, ApiError> {
    let (exercise_id, role) = app.rooms.check_code(&req.code, peer.ip(), crate::room::CodeUse::Join)?;
    let (ex, _) = app.live.snapshot(&exercise_id)?;
    let team = match (role, req.team) {
        (Role::Player, Some(t)) if ex.teams.iter().any(|x| x.id == t) => Some(t),
        (Role::Player, Some(_)) => return Err(ApiError::BadRequest("Unknown team".into())),
        _ => None,
    };
    // Joining again from the same browser replaces the old session.
    if let Some(old) = session_token(&headers) {
        app.rooms.leave(old);
    }
    let session = app.rooms.join(exercise_id.clone(), role, req.name, team.clone(), now())?;
    tracing::info!("{} joined as {:?}", session.name(), role);
    let me = Me { role, name: session.name(), team, exercise_id: Some(exercise_id) };
    // HttpOnly: page scripts never see the token. SameSite=Strict: other
    // sites cannot make this browser act as the participant.
    let cookie = format!("{COOKIE}={}; Path=/; HttpOnly; SameSite=Strict; Max-Age=86400", session.token);
    let mut res = Json(me).into_response();
    res.headers_mut().insert(SET_COOKIE, HeaderValue::from_str(&cookie).map_err(ApiError::internal)?);
    Ok(res)
}

async fn leave(State(app): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = session_token(&headers) {
        app.rooms.leave(token);
    }
    let clear = format!("{COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0");
    let mut res = StatusCode::NO_CONTENT.into_response();
    if let Ok(v) = HeaderValue::from_str(&clear) {
        res.headers_mut().insert(SET_COOKIE, v);
    }
    res
}
