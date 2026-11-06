//! Only this machine's own pages may drive the API.

mod common;

use common::TestApp;
use reqwest::StatusCode;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, http::HeaderValue};

#[tokio::test]
async fn cross_origin_writes_are_refused() {
    let app = TestApp::spawn().await;
    app.pin(serde_json::json!({})).await;
    // A form post from another site needs no preflight and carries no JSON —
    // exactly what /api/state/clear accepts.
    let r = app
        .http
        .post(format!("{}/api/state/clear", app.base))
        .header("Origin", "https://evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    assert_eq!(app.len("/api/pins").await, 1);
}

#[tokio::test]
async fn same_origin_requests_pass() {
    let app = TestApp::spawn().await;
    for origin in [app.base.clone(), app.base.replace("127.0.0.1", "localhost")] {
        let host = origin.trim_start_matches("http://").to_owned();
        let r = app
            .http
            .post(format!("{}/api/state/clear", app.base))
            .header("Origin", &origin)
            .header("Host", &host)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK, "{origin}");
    }
}

#[tokio::test]
async fn rebinding_hosts_are_refused() {
    let app = TestApp::spawn().await;
    let port = app.addr.rsplit(':').next().unwrap();
    let r = app
        .http
        .get(format!("{}/api/pins", app.base))
        .header("Host", format!("attacker.example:{port}"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn cross_origin_websockets_are_refused() {
    let app = TestApp::spawn().await;
    let mut req = format!("ws://{}/ws", app.addr).into_client_request().unwrap();
    req.headers_mut().insert("Origin", HeaderValue::from_static("https://evil.example"));
    assert!(tokio_tungstenite::connect_async(req).await.is_err());
}

#[tokio::test]
async fn serves_the_embedded_frontend() {
    let app = TestApp::spawn().await;
    let r = app.http.get(&app.base).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert!(r.headers()["content-type"].to_str().unwrap().starts_with("text/html"));
    let etag = r.headers()["etag"].clone();
    assert!(r.text().await.unwrap().contains("COPPIR"));

    let again = app.http.get(&app.base).header("If-None-Match", etag).send().await.unwrap();
    assert_eq!(again.status(), StatusCode::NOT_MODIFIED);

    let js = app.http.get(format!("{}/js/api.js", app.base)).send().await.unwrap();
    assert!(js.headers()["content-type"].to_str().unwrap().contains("javascript"));
    assert_eq!(app.http.get(format!("{}/nope.js", app.base)).send().await.unwrap().status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn outbound_failures_are_502_not_500() {
    let app = TestApp::spawn().await;
    let r = app.post("/api/geocode", serde_json::json!({"query": "Washington DC"})).await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY);
    let r = app.post("/api/search/osm", serde_json::json!({"lat": 38.9, "lon": -77.0, "category": "Hospitals"})).await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY);
    let r = app.post("/api/search/osm", serde_json::json!({"lat": 38.9, "lon": -77.0, "category": "Bakeries"})).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let r = app
        .post(
            "/api/search/osm",
            serde_json::json!({"lat": 38.9, "lon": -77.0, "radius_mi": 0, "category": "Hospitals"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
}
