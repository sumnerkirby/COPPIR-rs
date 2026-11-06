//! Saving, loading and clearing a session.
//!
//! Scenario names reach the filesystem. They are reduced to a safe character
//! set, which on its own rules out traversal, and the joined path is checked
//! against the scenarios directory anyway.

use std::path::{Path as FsPath, PathBuf};

use axum::{
    Json,
    extract::{Path, State},
};
use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    error::ApiError,
    models::ScenarioReq,
    state::{AppState, Session, now},
    validate::Valid,
};

const FORMAT_VERSION: u32 = 1;

/// On-disk layout. Field names match what the Python version wrote, so its
/// scenarios load here and these load there.
#[derive(Serialize)]
struct ScenarioOut<'a> {
    #[serde(flatten)]
    session: &'a Session,
    name: &'a str,
    saved_at: String,
    format_version: u32,
}

/// Keep alphanumerics, hyphens, underscores and spaces — safe as a filename on
/// every platform — then trim and cap at 64 characters.
pub fn safe_name(name: &str) -> String {
    let kept: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ' ')).collect();
    kept.trim().chars().take(64).collect::<String>().trim_end().to_owned()
}

/// Names Windows refuses as files, with or without an extension.
fn is_reserved_on_windows(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.len() == 4
            && upper.as_bytes()[3].is_ascii_digit())
}

fn scenario_path(dir: &FsPath, name: &str) -> Result<(PathBuf, String), ApiError> {
    let safe = safe_name(name);
    if safe.is_empty() || is_reserved_on_windows(&safe) {
        return Err(ApiError::BadRequest("Invalid scenario name".into()));
    }
    let path = dir.join(format!("{safe}.json"));
    if path.parent() != Some(dir) {
        return Err(ApiError::BadRequest("Invalid scenario name".into()));
    }
    Ok((path, safe))
}

pub async fn list(State(app): State<AppState>) -> Result<Json<Vec<String>>, ApiError> {
    let mut names = Vec::new();
    let mut dir = tokio::fs::read_dir(&app.config.scenarios_dir).await.map_err(ApiError::internal)?;
    while let Some(entry) = dir.next_entry().await.map_err(ApiError::internal)? {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if !stem.starts_with('.') {
                    names.push(stem.to_owned());
                }
            }
        }
    }
    names.sort();
    Ok(Json(names))
}

pub async fn save(State(app): State<AppState>, Valid(req): Valid<ScenarioReq>) -> Result<Json<Value>, ApiError> {
    let dir = &app.config.scenarios_dir;
    let (path, name) = scenario_path(dir, &req.name)?;
    let bytes = app.hub.read(|s| {
        serde_json::to_vec_pretty(&ScenarioOut {
            session: s,
            name: &name,
            saved_at: now(),
            format_version: FORMAT_VERSION,
        })
    });
    let bytes = bytes.map_err(ApiError::internal)?;

    // Write beside the target and rename over it, so a crash or a full disk
    // mid-write leaves the previous save intact rather than a truncated file.
    let tmp = dir.join(format!(".{name}.json.tmp"));
    tokio::fs::write(&tmp, &bytes).await.map_err(ApiError::internal)?;
    if let Err(e) = tokio::fs::rename(&tmp, &path).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(ApiError::internal(e));
    }
    Ok(Json(json!({ "ok": true, "name": name })))
}

pub async fn load(State(app): State<AppState>, Valid(req): Valid<ScenarioReq>) -> Result<Json<Value>, ApiError> {
    let (path, name) = scenario_path(&app.config.scenarios_dir, &req.name)?;
    let bytes = match tokio::fs::read(&path).await {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(ApiError::NotFound("Scenario not found")),
        Err(e) => return Err(ApiError::internal(e)),
    };
    // Parse fully before touching state: a damaged file must not leave the
    // session half-replaced.
    // Unknown top-level keys (name, saved_at, format_version) are ignored.
    let loaded: Session = serde_json::from_slice(&bytes)
        .map_err(|e| ApiError::BadRequest(format!("Scenario file is damaged and was not loaded: {e}")))?;

    let count = app.hub.with(|s, tx| {
        *s = loaded;
        // Drop edges whose pins are gone, in case the file was edited by hand.
        let pins = &s.pins;
        s.edges.retain(|_, e| pins.contains_key(&e.from_pid) && pins.contains_key(&e.to_pid));
        tx.send(&s.full_state());
        s.pins.len()
    });
    Ok(Json(json!({ "ok": true, "name": name, "count": count })))
}

pub async fn delete(State(app): State<AppState>, Path(name): Path<String>) -> Result<Json<Value>, ApiError> {
    let (path, _) = scenario_path(&app.config.scenarios_dir, &name)?;
    match tokio::fs::remove_file(&path).await {
        Ok(()) => Ok(Json(json!({ "ok": true }))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(ApiError::NotFound("Scenario not found")),
        Err(e) => Err(ApiError::internal(e)),
    }
}

pub async fn clear_state(State(app): State<AppState>) -> Json<Value> {
    app.hub.with(|s, tx| {
        *s = Session::default();
        tx.send(&s.full_state());
    });
    Json(json!({ "ok": true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitising() {
        for (raw, want) in [
            ("Exercise One", "Exercise One"),
            ("with-hyphens_and_underscores", "with-hyphens_and_underscores"),
            ("drop/slashes", "dropslashes"),
            ("dots...everywhere", "dotseverywhere"),
            ("  padded  ", "padded"),
            ("emoji \u{1F600} stripped", "emoji  stripped"),
            ("../../../etc/passwd", "etcpasswd"),
            ("..\\..\\windows\\system32", "windowssystem32"),
            ("...", ""),
        ] {
            assert_eq!(safe_name(raw), want, "{raw:?}");
        }
    }

    #[test]
    fn windows_reserved_names() {
        for n in ["con", "NUL", "com1", "LPT9"] {
            assert!(is_reserved_on_windows(n), "{n}");
        }
        for n in ["console", "com", "COM10", "Exercise"] {
            assert!(!is_reserved_on_windows(n), "{n}");
        }
    }
}
