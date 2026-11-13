//! Offline map packs: a region of the base map saved to disk, for venues
//! with poor or no internet.
//!
//! A pack is one MBTiles file (SQLite) in the maps folder. Besides the
//! standard `metadata` and `tiles` tables it holds a `files` table with what
//! a style needs beyond tiles: the style documents themselves, the sprite and
//! the glyph ranges. So one file is the whole map, and copying it to another
//! machine's maps folder brings the map along.
//!
//! Packs are downloaded from the same keyless service the online map uses
//! (OpenFreeMap). The server does the downloading, so players on the LAN get
//! the map from the facilitator's laptop and need no internet at all.

use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ts_rs::TS;

use crate::{error::ApiError, state::new_id};

/// The styles a pack carries, matching the app's themes.
pub const STYLES: &[&str] = &["dark", "positron"];
/// Vector tiles stop at 14; MapLibre over-zooms past it.
pub const MAX_ZOOM: u8 = 14;
pub const MAX_TILES: u64 = 40_000;
/// Glyph ranges to save: Latin, Greek, Cyrillic and common punctuation.
const GLYPH_RANGES: &[u32] = &[0, 256, 512, 768, 1024, 1280, 1536, 1792, 8192];
const PARALLEL: usize = 6;

#[derive(Clone, Debug, Serialize, TS)]
#[ts(export)]
pub struct MapPack {
    pub id: String,
    pub name: String,
    /// West, south, east, north.
    pub bounds: [f64; 4],
    pub max_zoom: u8,
    #[ts(type = "number")]
    pub tiles: u64,
    #[ts(type = "number")]
    pub bytes: u64,
    /// Set while it downloads.
    pub progress: Option<Progress>,
    /// Why the download failed; the pack is not usable.
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, TS)]
#[ts(export)]
pub struct Progress {
    #[ts(type = "number")]
    pub done: u64,
    #[ts(type = "number")]
    pub total: u64,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct NewMapPack {
    pub name: String,
    /// West, south, east, north.
    pub bounds: [f64; 4],
    pub max_zoom: u8,
}

struct Job {
    pack: Mutex<MapPack>,
    done: AtomicU64,
    bytes: AtomicU64,
    cancel: AtomicBool,
}

pub struct Maps {
    dir: PathBuf,
    upstream: String,
    jobs: Mutex<HashMap<String, Arc<Job>>>,
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

// ───────────────────────────── tile maths ─────────────────────────────

fn tile_x(lon: f64, z: u8) -> u32 {
    let n = f64::from(1u32 << z);
    (((lon + 180.0) / 360.0 * n).floor().clamp(0.0, n - 1.0)) as u32
}

fn tile_y(lat: f64, z: u8) -> u32 {
    let n = f64::from(1u32 << z);
    let lat = lat.clamp(-85.0511, 85.0511).to_radians();
    (((1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / std::f64::consts::PI) / 2.0 * n).floor().clamp(0.0, n - 1.0)) as u32
}

/// Every tile covering `bounds`, zoom 0 to `max_zoom`.
fn tiles(bounds: [f64; 4], max_zoom: u8) -> impl Iterator<Item = (u8, u32, u32)> {
    let [w, s, e, n] = bounds;
    (0..=max_zoom).flat_map(move |z| {
        let (x0, x1, y0, y1) = (tile_x(w, z), tile_x(e, z), tile_y(n, z), tile_y(s, z));
        (x0..=x1).flat_map(move |x| (y0..=y1).map(move |y| (z, x, y)))
    })
}

pub fn tile_count(bounds: [f64; 4], max_zoom: u8) -> u64 {
    let [w, s, e, n] = bounds;
    (0..=max_zoom)
        .map(|z| u64::from(tile_x(e, z) - tile_x(w, z) + 1) * u64::from(tile_y(s, z) - tile_y(n, z) + 1))
        .sum()
}

// ───────────────────────────── the file ─────────────────────────────

fn create(path: &Path, pack: &MapPack) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF;
         CREATE TABLE metadata (name TEXT PRIMARY KEY, value TEXT);
         CREATE TABLE tiles (zoom_level INTEGER, tile_column INTEGER, tile_row INTEGER, tile_data BLOB,
                             PRIMARY KEY (zoom_level, tile_column, tile_row));
         CREATE TABLE files (path TEXT PRIMARY KEY, data BLOB);",
    )?;
    let [w, s, e, n] = pack.bounds;
    for (k, v) in [
        ("name", pack.name.clone()),
        ("format", "pbf".into()),
        ("type", "baselayer".into()),
        ("bounds", format!("{w},{s},{e},{n}")),
        ("minzoom", "0".into()),
        ("maxzoom", pack.max_zoom.to_string()),
        ("attribution", "© OpenMapTiles © OpenStreetMap contributors".into()),
    ] {
        conn.execute("INSERT INTO metadata VALUES (?1, ?2)", params![k, v])?;
    }
    Ok(conn)
}

fn open_ro(path: &Path) -> rusqlite::Result<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
}

fn read_pack(id: &str, path: &Path) -> rusqlite::Result<MapPack> {
    let conn = open_ro(path)?;
    let meta = |k: &str| -> rusqlite::Result<Option<String>> {
        conn.query_row("SELECT value FROM metadata WHERE name = ?1", [k], |r| r.get(0)).optional()
    };
    let bounds: Vec<f64> =
        meta("bounds")?.unwrap_or_default().split(',').filter_map(|v| v.trim().parse().ok()).collect();
    let tiles: i64 = conn.query_row("SELECT count(*) FROM tiles", [], |r| r.get(0))?;
    Ok(MapPack {
        id: id.to_owned(),
        name: meta("name")?.unwrap_or_else(|| id.to_owned()),
        bounds: <[f64; 4]>::try_from(bounds).unwrap_or([-180.0, -85.0, 180.0, 85.0]),
        max_zoom: meta("maxzoom")?.and_then(|z| z.parse().ok()).unwrap_or(MAX_ZOOM),
        tiles: tiles.max(0) as u64,
        bytes: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        progress: None,
        error: None,
    })
}

/// What a request for a pack's file gets back.
pub struct Found {
    pub data: Vec<u8>,
    pub gzipped: bool,
}

impl Maps {
    pub fn new(dir: PathBuf, upstream: String) -> Self {
        Self { dir, upstream: upstream.trim_end_matches('/').to_owned(), jobs: Default::default() }
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.mbtiles"))
    }

    /// Every pack on disk, and every download under way or failed.
    pub fn list(&self) -> Vec<MapPack> {
        let mut out: Vec<MapPack> = self.jobs.lock().unwrap().values().map(|j| j.snapshot()).collect();
        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let Some(id) = path.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".mbtiles"))
                else {
                    continue;
                };
                if !valid_id(id) || out.iter().any(|p| p.id == id) {
                    continue;
                }
                match read_pack(id, &path) {
                    Ok(p) => out.push(p),
                    Err(e) => tracing::warn!("map pack {}: {e}", path.display()),
                }
            }
        }
        out.sort_by_key(|a| a.name.to_lowercase());
        out
    }

    /// Check a request and start downloading it in the background.
    pub fn start(self: &Arc<Self>, http: reqwest::Client, req: NewMapPack) -> Result<MapPack, ApiError> {
        let mut c = crate::validate::Checks::default();
        let mut name = req.name;
        c.label("name", &mut name, 100);
        let [w, s, e, n] = req.bounds;
        if !(req.bounds.iter().all(|v| v.is_finite())
            && (-180.0..=180.0).contains(&w)
            && (-180.0..=180.0).contains(&e)
            && (-85.06..=85.06).contains(&s)
            && (-85.06..=85.06).contains(&n)
            && w < e
            && s < n)
        {
            c.fail("bounds", "must be west, south, east, north, in degrees, with west < east and south < north");
        }
        if req.max_zoom > MAX_ZOOM {
            c.fail("max_zoom", format!("at most {MAX_ZOOM}"));
        }
        c.finish()?;
        let total = tile_count(req.bounds, req.max_zoom);
        if total > MAX_TILES {
            c.fail(
                "bounds",
                format!("{total} tiles is too many; pick a smaller area or a lower zoom (at most {MAX_TILES})"),
            );
        }
        c.finish()?;
        std::fs::create_dir_all(&self.dir).map_err(ApiError::internal)?;
        let id = new_id().split('-').next().unwrap_or_default().to_owned();
        let pack = MapPack {
            id: id.clone(),
            name,
            bounds: req.bounds,
            max_zoom: req.max_zoom,
            tiles: 0,
            bytes: 0,
            progress: Some(Progress { done: 0, total }),
            error: None,
        };
        let job = Arc::new(Job {
            pack: Mutex::new(pack.clone()),
            done: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            cancel: AtomicBool::new(false),
        });
        self.jobs.lock().unwrap().insert(id.clone(), job.clone());
        let maps = self.clone();
        tokio::spawn(async move {
            let part = maps.dir.join(format!("{id}.part"));
            let result = maps.download(&http, &job, &part).await;
            if result.is_err() {
                let _ = std::fs::remove_file(&part);
            }
            match result {
                Ok(()) => {
                    if let Err(e) = std::fs::rename(&part, maps.path(&id)) {
                        job.fail(format!("could not save the pack: {e}"));
                        return;
                    }
                    maps.jobs.lock().unwrap().remove(&id);
                    tracing::info!("map pack {id} ready");
                }
                Err(_) if job.cancel.load(Ordering::Relaxed) => {
                    maps.jobs.lock().unwrap().remove(&id);
                }
                Err(e) => {
                    tracing::warn!("map pack {id}: {e}");
                    job.fail(e);
                }
            }
        });
        Ok(pack)
    }

    async fn download(&self, http: &reqwest::Client, job: &Job, part: &Path) -> Result<(), String> {
        let pack = job.pack.lock().unwrap().clone();
        let conn = Arc::new(Mutex::new(create(part, &pack).map_err(|e| e.to_string())?));
        let put = |path: String, data: Vec<u8>| {
            let conn = conn.clone();
            async move {
                tokio::task::spawn_blocking(move || {
                    conn.lock().unwrap().execute("INSERT OR REPLACE INTO files VALUES (?1, ?2)", params![path, data])
                })
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())
            }
        };

        // The styles, and from them the tile, sprite and glyph addresses.
        let mut tile_url = None;
        let mut sprites = BTreeSet::new();
        let mut glyphs = None;
        let mut fonts = BTreeSet::new();
        for style in STYLES {
            let doc: Value = self.fetch_json(http, &format!("{}/styles/{style}", self.upstream)).await?;
            if tile_url.is_none() {
                let source = doc
                    .pointer("/sources/openmaptiles/url")
                    .and_then(Value::as_str)
                    .ok_or("style has no tile source")?;
                let tilejson: Value = self.fetch_json(http, source).await?;
                tile_url = tilejson.pointer("/tiles/0").and_then(Value::as_str).map(str::to_owned);
            }
            if let Some(s) = doc["sprite"].as_str() {
                sprites.insert(s.to_owned());
            }
            if let Some(g) = doc["glyphs"].as_str() {
                glyphs = Some(g.to_owned());
            }
            for layer in doc["layers"].as_array().into_iter().flatten() {
                if let Some(stack) = layer.pointer("/layout/text-font").and_then(Value::as_array) {
                    let names: Vec<&str> = stack.iter().filter_map(Value::as_str).collect();
                    if !names.is_empty() {
                        fonts.insert(names.join(","));
                    }
                }
            }
            put(format!("style/{style}"), serde_json::to_vec(&doc).map_err(|e| e.to_string())?).await?;
        }
        // The app's own labels.
        fonts.insert("Noto Sans Regular".into());
        let tile_url = tile_url.ok_or("tile source has no tile address")?;
        self.same_host(&tile_url)?;

        for sprite in &sprites {
            self.same_host(sprite)?;
            let name = sprite.rsplit('/').next().unwrap_or("sprite");
            for suffix in [".json", ".png", "@2x.json", "@2x.png"] {
                let data = self.fetch(http, &format!("{sprite}{suffix}")).await?.ok_or("sprite is missing")?;
                put(format!("sprite/{name}{suffix}"), data).await?;
            }
        }
        if let Some(glyphs) = &glyphs {
            self.same_host(glyphs)?;
            for stack in &fonts {
                for start in GLYPH_RANGES {
                    let range = format!("{start}-{}", start + 255);
                    let url = glyphs.replace("{fontstack}", &encode(stack)).replace("{range}", &range);
                    if let Some(data) = self.fetch(http, &url).await? {
                        put(format!("fonts/{stack}/{range}"), data).await?;
                    }
                }
            }
        }

        // The tiles, a few at a time.
        let mut pending = tiles(pack.bounds, pack.max_zoom);
        let mut running = tokio::task::JoinSet::new();
        let mut failed = 0u64;
        loop {
            while running.len() < PARALLEL {
                let Some((z, x, y)) = pending.next() else { break };
                let url = tile_url
                    .replace("{z}", &z.to_string())
                    .replace("{x}", &x.to_string())
                    .replace("{y}", &y.to_string());
                let http = http.clone();
                running.spawn(async move { ((z, x, y), fetch_retry(&http, &url).await) });
            }
            let Some(next) = running.join_next().await else { break };
            if job.cancel.load(Ordering::Relaxed) {
                return Err("cancelled".into());
            }
            let ((z, x, y), result) = next.map_err(|e| e.to_string())?;
            match result {
                Ok(Some(data)) => {
                    let conn = conn.clone();
                    // MBTiles numbers rows from the south.
                    let row = (1u32 << z) - 1 - y;
                    let bytes = &job.bytes;
                    // Stored gzipped, as MBTiles usually are: a third of the size.
                    let data = gzip(data);
                    bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
                    tokio::task::spawn_blocking(move || {
                        conn.lock()
                            .unwrap()
                            .execute("INSERT OR REPLACE INTO tiles VALUES (?1, ?2, ?3, ?4)", params![z, x, row, data])
                    })
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())?;
                }
                Ok(None) => {} // the sea, mostly
                Err(_) => failed += 1,
            }
            job.done.fetch_add(1, Ordering::Relaxed);
        }
        let total = job.done.load(Ordering::Relaxed);
        if failed * 50 > total {
            return Err(format!(
                "{failed} of {total} tiles could not be downloaded; check the connection and try again"
            ));
        }
        let conn = conn.lock().unwrap();
        conn.execute_batch("VACUUM;").map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Everything a pack's style names must come from the service the style
    /// came from.
    fn same_host(&self, url: &str) -> Result<(), String> {
        let origin = |u: &str| -> Option<String> {
            let u = reqwest::Url::parse(u).ok()?;
            Some(format!("{}://{}:{}", u.scheme(), u.host_str()?, u.port_or_known_default()?))
        };
        if origin(url).is_some() && origin(url) == origin(&self.upstream) {
            Ok(())
        } else {
            Err(format!("the map service pointed somewhere else: {url}"))
        }
    }

    async fn fetch(&self, http: &reqwest::Client, url: &str) -> Result<Option<Vec<u8>>, String> {
        self.same_host(url)?;
        fetch_retry(http, url).await
    }

    async fn fetch_json(&self, http: &reqwest::Client, url: &str) -> Result<Value, String> {
        let data = self.fetch(http, url).await?.ok_or_else(|| format!("{url} not found"))?;
        serde_json::from_slice(&data).map_err(|e| format!("{url}: {e}"))
    }

    /// Stop a download, or delete a pack.
    pub fn remove(&self, id: &str) -> Result<(), ApiError> {
        if !valid_id(id) {
            return Err(ApiError::NotFound("Map pack not found"));
        }
        if let Some(job) = self.jobs.lock().unwrap().remove(id) {
            job.cancel.store(true, Ordering::Relaxed);
            return Ok(());
        }
        match std::fs::remove_file(self.path(id)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(ApiError::NotFound("Map pack not found")),
            Err(e) => Err(ApiError::internal(e)),
        }
    }

    fn file(&self, id: &str) -> Result<PathBuf, ApiError> {
        let path = self.path(id);
        if valid_id(id) && path.is_file() { Ok(path) } else { Err(ApiError::NotFound("Map pack not found")) }
    }

    /// A tile, or `None` where the pack has none (outside it, or open sea).
    pub fn tile(&self, id: &str, z: u8, x: u32, y: u32) -> Result<Option<Found>, ApiError> {
        if z > 30 || y >= (1u32 << z) || x >= (1u32 << z) {
            return Ok(None);
        }
        let conn = open_ro(&self.file(id)?).map_err(ApiError::internal)?;
        let row = (1u32 << z) - 1 - y;
        let data: Option<Vec<u8>> = conn
            .query_row(
                "SELECT tile_data FROM tiles WHERE zoom_level = ?1 AND tile_column = ?2 AND tile_row = ?3",
                params![z, x, row],
                |r| r.get(0),
            )
            .optional()
            .map_err(ApiError::internal)?;
        Ok(data.map(|data| Found { gzipped: data.starts_with(&[0x1f, 0x8b]), data }))
    }

    pub fn stored(&self, id: &str, path: &str) -> Result<Option<Vec<u8>>, ApiError> {
        let conn = open_ro(&self.file(id)?).map_err(ApiError::internal)?;
        conn.query_row("SELECT data FROM files WHERE path = ?1", [path], |r| r.get(0))
            .optional()
            .map_err(ApiError::internal)
    }

    /// A style pointed at this server: `base` is how the browser reached it.
    pub fn style(&self, id: &str, name: &str, base: &str) -> Result<Value, ApiError> {
        let pack = read_pack(id, &self.file(id)?).map_err(ApiError::internal)?;
        let doc = self.stored(id, &format!("style/{name}"))?.ok_or(ApiError::NotFound("No such style in this pack"))?;
        let mut doc: Value = serde_json::from_slice(&doc).map_err(ApiError::internal)?;
        let at = format!("{base}/api/v2/maps/{id}");
        let attribution = doc.pointer("/sources/openmaptiles/attribution").cloned();
        doc["sources"] = json!({"openmaptiles": {
            "type": "vector",
            "tiles": [format!("{at}/tiles/{{z}}/{{x}}/{{y}}")],
            "minzoom": 0,
            "maxzoom": pack.max_zoom,
            "bounds": pack.bounds,
            "attribution": attribution.unwrap_or_else(|| json!("© OpenMapTiles © OpenStreetMap contributors")),
        }});
        // Only layers drawn from the pack, or from nothing (the background).
        if let Some(layers) = doc["layers"].as_array_mut() {
            layers.retain(|l| l.get("source").is_none_or(|s| s == "openmaptiles"));
        }
        if let Some(sprite) = doc["sprite"].as_str() {
            let name = sprite.rsplit('/').next().unwrap_or("sprite").to_owned();
            doc["sprite"] = json!(format!("{at}/sprite/{name}"));
        }
        doc["glyphs"] = json!(format!("{at}/fonts/{{fontstack}}/{{range}}.pbf"));
        Ok(doc)
    }
}

impl Job {
    fn snapshot(&self) -> MapPack {
        let mut p = self.pack.lock().unwrap().clone();
        if let Some(progress) = &mut p.progress {
            progress.done = self.done.load(Ordering::Relaxed);
        }
        p.bytes = self.bytes.load(Ordering::Relaxed);
        p
    }

    fn fail(&self, error: String) {
        let mut p = self.pack.lock().unwrap();
        p.progress = None;
        p.error = Some(error);
    }
}

/// `None` for a 404 or 204: the service has nothing there.
async fn fetch_retry(http: &reqwest::Client, url: &str) -> Result<Option<Vec<u8>>, String> {
    let mut last = String::new();
    for attempt in 0..3 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(400 * attempt)).await;
        }
        match http.get(url).timeout(std::time::Duration::from_secs(30)).send().await {
            Ok(r) if r.status() == reqwest::StatusCode::NOT_FOUND || r.status() == reqwest::StatusCode::NO_CONTENT => {
                return Ok(None);
            }
            Ok(r) if r.status().is_success() => match r.bytes().await {
                Ok(b) => return Ok(Some(b.to_vec())),
                Err(e) => last = e.to_string(),
            },
            Ok(r) => last = format!("{url}: {}", r.status()),
            Err(e) => last = e.to_string(),
        }
    }
    Err(last)
}

fn gzip(data: Vec<u8>) -> Vec<u8> {
    use std::io::Write;
    if data.starts_with(&[0x1f, 0x8b]) {
        return data;
    }
    let mut e = flate2::write::GzEncoder::new(Vec::with_capacity(data.len() / 3), flate2::Compression::default());
    match e.write_all(&data).and_then(|()| e.finish()) {
        Ok(out) => out,
        Err(_) => data,
    }
}

/// Percent-encode a font stack for a URL path.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b',' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_maths() {
        assert_eq!((tile_x(-180.0, 1), tile_x(179.9, 1)), (0, 1));
        assert_eq!((tile_y(85.0, 1), tile_y(-85.0, 1)), (0, 1));
        // Washington DC at zoom 14.
        assert_eq!((tile_x(-77.0365, 14), tile_y(38.8977, 14)), (4685, 6267));
        let dc = [-77.12, 38.80, -76.91, 38.99];
        assert_eq!(tile_count(dc, 14) as usize, tiles(dc, 14).count());
        assert!(tile_count(dc, 14) < 1000);
        assert!(tile_count([-125.0, 25.0, -66.0, 49.0], 14) > MAX_TILES);
        assert_eq!(encode("Noto Sans Regular"), "Noto%20Sans%20Regular");
        assert!(valid_id("3f2a9c1e") && !valid_id("../x") && !valid_id(""));
    }
}
