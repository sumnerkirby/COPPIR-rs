//! Offline map packs, downloaded from a stand-in for the tile service.

mod common;

use std::time::Duration;

use axum::{Json, Router, extract::Path, http::StatusCode as AxumStatus, routing::get};
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

/// A tiny tile service shaped like OpenFreeMap. Tiles west of the meridian
/// exist; the rest is "sea" (404).
async fn upstream() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let b = base.clone();
    let style = move |Path(name): Path<String>| {
        let b = b.clone();
        async move {
            Json(json!({
                "version": 8, "name": name,
                "sources": {
                    "openmaptiles": {"type": "vector", "url": format!("{b}/planet")},
                    "ne2_shaded": {"type": "raster", "tiles": [format!("{b}/ne/{{z}}/{{x}}/{{y}}.png")]},
                },
                "sprite": format!("{b}/sprites/ofm"),
                "glyphs": format!("{b}/fonts/{{fontstack}}/{{range}}.pbf"),
                "layers": [
                    {"id": "bg", "type": "background"},
                    {"id": "shade", "type": "raster", "source": "ne2_shaded"},
                    {"id": "water", "type": "fill", "source": "openmaptiles", "source-layer": "water"},
                    {"id": "label", "type": "symbol", "source": "openmaptiles", "source-layer": "place",
                     "layout": {"text-font": ["Noto Sans Bold"]}},
                ],
            }))
        }
    };
    let b = base.clone();
    let app = Router::new()
        .route("/styles/{name}", get(style))
        .route(
            "/planet",
            get(move || async move { Json(json!({"tiles": [format!("{b}/planet/v1/{{z}}/{{x}}/{{y}}.pbf")]})) }),
        )
        .route(
            "/planet/v1/{z}/{x}/{y}",
            get(|Path((z, x, y)): Path<(u32, u32, String)>| async move {
                if x >= (1 << z) / 2 { Err(AxumStatus::NOT_FOUND) } else { Ok(format!("tile {z}/{x}/{y}")) }
            }),
        )
        .route("/sprites/{file}", get(|Path(f): Path<String>| async move { format!("sprite {f}") }))
        .route("/fonts/{stack}/{range}", get(|Path((s, r)): Path<(String, String)>| async move { format!("{s} {r}") }));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    base
}

async fn bytes(app: &TestApp, path: &str) -> (StatusCode, Vec<u8>) {
    let r = app.http.get(format!("{}{path}", app.base)).send().await.unwrap();
    (r.status(), r.bytes().await.unwrap().to_vec())
}

#[tokio::test]
async fn a_pack_downloads_and_serves_the_whole_map() {
    let tiles = upstream().await;
    let app = TestApp::spawn_with(|c| c.tiles_url = tiles.clone()).await;

    // The whole world at zoom 0-2: 1 + 4 + 16 tiles, the eastern half sea.
    let r = app.post("/api/v2/maps", json!({"name": "World", "bounds": [-180, -85, 180, 85], "max_zoom": 2})).await;
    assert_eq!(r.status, StatusCode::ACCEPTED, "{}", r.body);
    let id = r.body["id"].as_str().unwrap().to_owned();
    assert_eq!(r.body["progress"]["total"], 21);

    let mut pack = Value::Null;
    for _ in 0..100 {
        pack = app.get("/api/v2/maps").await[0].clone();
        if pack["progress"].is_null() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(pack["error"].is_null(), "{pack}");
    assert_eq!(pack["tiles"], 10, "{pack}"); // zoom 0 is the one tile, and it is "sea"
    assert!(pack["bytes"].as_u64().unwrap() > 0);
    assert!(app.dir.path().join(format!("maps/{id}.mbtiles")).is_file());

    // The style points at this server only, and keeps only the pack's layers.
    let style = app.get(&format!("/api/v2/maps/{id}/style/dark")).await;
    let text = style.to_string();
    assert!(!text.contains(&tiles), "{text}");
    let at = format!("{}/api/v2/maps/{id}", app.base);
    assert_eq!(style["sources"]["openmaptiles"]["tiles"][0], format!("{at}/tiles/{{z}}/{{x}}/{{y}}"));
    assert_eq!(style["sprite"], format!("{at}/sprite/ofm"));
    assert_eq!(style["glyphs"], format!("{at}/fonts/{{fontstack}}/{{range}}.pbf"));
    assert_eq!(style["layers"].as_array().unwrap().len(), 3);
    assert_eq!(style["sources"].as_object().unwrap().len(), 1);

    // Tiles come back by XYZ address, stored and sent gzipped; sea is an empty 204.
    let r = app.http.get(format!("{}/api/v2/maps/{id}/tiles/2/1/3", app.base)).send().await.unwrap();
    assert_eq!(r.headers()["content-encoding"], "gzip");
    let mut tile = String::new();
    std::io::Read::read_to_string(&mut flate2::read::GzDecoder::new(&r.bytes().await.unwrap()[..]), &mut tile).unwrap();
    assert_eq!(tile, "tile 2/1/3.pbf");
    assert_eq!(bytes(&app, &format!("/api/v2/maps/{id}/tiles/2/3/3")).await.0, StatusCode::NO_CONTENT);
    assert_eq!(bytes(&app, &format!("/api/v2/maps/{id}/tiles/9/0/0")).await.0, StatusCode::NO_CONTENT);
    let (status, body) = bytes(&app, &format!("/api/v2/maps/{id}/sprite/ofm@2x.png")).await;
    assert_eq!((status, body), (StatusCode::OK, b"sprite ofm@2x.png".to_vec()));
    // Both the style's font and the app's own label font were saved.
    let (_, body) = bytes(&app, &format!("/api/v2/maps/{id}/fonts/Noto%20Sans%20Bold/0-255.pbf")).await;
    assert_eq!(body, b"Noto Sans Bold 0-255.pbf");
    let (_, body) = bytes(&app, &format!("/api/v2/maps/{id}/fonts/Noto%20Sans%20Regular/256-511.pbf")).await;
    assert_eq!(body, b"Noto Sans Regular 256-511.pbf");
    // A range it did not save is empty, not an error.
    assert_eq!(
        bytes(&app, &format!("/api/v2/maps/{id}/fonts/Noto%20Sans%20Bold/65280-65535.pbf")).await,
        (StatusCode::OK, vec![])
    );

    // The exercise can be pointed at it.
    let ex = app.post("/api/v2/exercises", json!({"name": "Drill"})).await.body["exercise"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let r = app
        .post(
            &format!("/api/v2/exercises/{ex}/commands"),
            json!({"type": "set_branding", "branding": {"map_pack": id}}),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);

    assert_eq!(app.delete(&format!("/api/v2/maps/{id}")).await.status, StatusCode::NO_CONTENT);
    assert_eq!(app.len("/api/v2/maps").await, 0);
    assert_eq!(bytes(&app, &format!("/api/v2/maps/{id}/style/dark")).await.0, StatusCode::NOT_FOUND);
    assert_eq!(app.delete(&format!("/api/v2/maps/{id}")).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn bad_requests_and_failed_downloads_are_reported() {
    let app = TestApp::spawn().await;
    for bad in [
        json!({"name": "x", "bounds": [10, 0, 5, 1], "max_zoom": 10}),
        json!({"name": "x", "bounds": [0, 0, 1, 1], "max_zoom": 15}),
        json!({"name": " ", "bounds": [0, 0, 1, 1], "max_zoom": 10}),
        json!({"name": "USA", "bounds": [-125, 25, -66, 49], "max_zoom": 14}),
    ] {
        assert_eq!(app.post("/api/v2/maps", bad.clone()).await.status, StatusCode::UNPROCESSABLE_ENTITY, "{bad}");
    }
    // Nothing listens on the stand-in address, so this one fails.
    let r = app.post("/api/v2/maps", json!({"name": "Nowhere", "bounds": [0, 0, 1, 1], "max_zoom": 1})).await;
    assert_eq!(r.status, StatusCode::ACCEPTED);
    let mut pack = Value::Null;
    for _ in 0..100 {
        pack = app.get("/api/v2/maps").await[0].clone();
        if pack["error"].is_string() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(pack["error"].is_string() && pack["progress"].is_null(), "{pack}");
    // Dismissing it clears it.
    assert_eq!(
        app.delete(&format!("/api/v2/maps/{}", pack["id"].as_str().unwrap())).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(app.len("/api/v2/maps").await, 0);
    // Ids are checked before they reach the file system.
    assert_eq!(
        app.call(reqwest::Method::GET, "/api/v2/maps/..%2Fcoppir/style/dark", None).await.status,
        StatusCode::NOT_FOUND
    );
}
