//! `/api/v2/maps`: offline map packs. The facilitator downloads and deletes
//! them; anyone in the room may read them, so phones get the map from the
//! facilitator's laptop.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get},
};

use crate::{
    auth::{self, Viewer},
    error::ApiError,
    maps::{MapPack, NewMapPack},
    state::AppState,
    validate::{Checks, Valid, Validate},
};

/// Packs do not change once made, and a new download gets a new id.
const CACHE: &str = "public, max-age=604800";

pub fn router(app: AppState) -> Router<AppState> {
    let facilitator = Router::new()
        .route("/api/v2/maps", get(list).post(create))
        .route("/api/v2/maps/{id}", delete(remove))
        .route_layer(middleware::from_fn_with_state(app, auth::facilitator_only));
    Router::new()
        .route("/api/v2/maps/{id}/style/{name}", get(style))
        .route("/api/v2/maps/{id}/tiles/{z}/{x}/{y}", get(tile))
        .route("/api/v2/maps/{id}/sprite/{file}", get(sprite))
        .route("/api/v2/maps/{id}/fonts/{stack}/{file}", get(font))
        .merge(facilitator)
}

impl Validate for NewMapPack {
    // `Maps::start` checks it, since it also needs the tile count.
    fn check(&mut self, _: &mut Checks) {}
}

async fn list(State(app): State<AppState>) -> Json<Vec<MapPack>> {
    let maps = app.maps.clone();
    Json(tokio::task::spawn_blocking(move || maps.list()).await.unwrap_or_default())
}

async fn create(State(app): State<AppState>, Valid(req): Valid<NewMapPack>) -> Result<impl IntoResponse, ApiError> {
    let pack = app.maps.start(app.http.clone(), req)?;
    Ok((StatusCode::ACCEPTED, Json(pack)))
}

async fn remove(State(app): State<AppState>, Path(id): Path<String>) -> Result<StatusCode, ApiError> {
    app.maps.remove(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Run a pack lookup off the async threads: each one opens the file.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, ApiError> + Send + 'static) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(f).await.map_err(ApiError::internal)?
}

async fn style(
    _: Viewer,
    State(app): State<AppState>,
    headers: HeaderMap,
    Path((id, name)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    // The guard has already checked the Host header against this machine's
    // own addresses, so it is safe to build URLs from.
    let host = headers.get(header::HOST).and_then(|h| h.to_str().ok()).ok_or(ApiError::BadRequest("No Host".into()))?;
    let base = format!("http://{host}");
    let name = name.trim_end_matches(".json").to_owned();
    let maps = app.maps.clone();
    let doc = blocking(move || maps.style(&id, &name, &base)).await?;
    Ok(([(header::CACHE_CONTROL, "no-cache")], Json(doc)).into_response())
}

async fn tile(
    _: Viewer,
    State(app): State<AppState>,
    Path((id, z, x, y)): Path<(String, u8, u32, String)>,
) -> Result<Response, ApiError> {
    let y: u32 = y.trim_end_matches(".pbf").parse().map_err(|_| ApiError::NotFound("No such tile"))?;
    let maps = app.maps.clone();
    let found = blocking(move || maps.tile(&id, z, x, y)).await?;
    Ok(match found {
        // Nothing there: MapLibre draws an empty tile for a 204.
        None => (StatusCode::NO_CONTENT, [(header::CACHE_CONTROL, CACHE)]).into_response(),
        Some(t) if t.gzipped => (
            [
                (header::CONTENT_TYPE, "application/x-protobuf"),
                (header::CONTENT_ENCODING, "gzip"),
                (header::CACHE_CONTROL, CACHE),
            ],
            t.data,
        )
            .into_response(),
        Some(t) => {
            ([(header::CONTENT_TYPE, "application/x-protobuf"), (header::CACHE_CONTROL, CACHE)], t.data).into_response()
        }
    })
}

async fn sprite(
    _: Viewer,
    State(app): State<AppState>,
    Path((id, file)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let mime = if file.ends_with(".png") { "image/png" } else { "application/json" };
    let maps = app.maps.clone();
    let data = blocking(move || maps.stored(&id, &format!("sprite/{file}"))).await?;
    let data = data.ok_or(ApiError::NotFound("No such sprite"))?;
    Ok(([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, CACHE)], data).into_response())
}

async fn font(
    _: Viewer,
    State(app): State<AppState>,
    Path((id, stack, file)): Path<(String, String, String)>,
) -> Result<Response, ApiError> {
    let range = file.trim_end_matches(".pbf").to_owned();
    let maps = app.maps.clone();
    // A range the pack lacks (a script it did not save) is sent empty, which
    // MapLibre takes as "no glyphs here" rather than as an error.
    let data = blocking(move || maps.stored(&id, &format!("fonts/{stack}/{range}"))).await?.unwrap_or_default();
    Ok(([(header::CONTENT_TYPE, "application/x-protobuf"), (header::CACHE_CONTROL, CACHE)], data).into_response())
}
