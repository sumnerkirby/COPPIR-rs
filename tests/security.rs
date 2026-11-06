//! Only this machine's own pages may drive the API, and the app serves itself.

mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::json;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, http::HeaderValue};

async fn exercise(app: &TestApp) -> String {
    let r = app.post("/api/v2/exercises", json!({"name": "Drill"})).await;
    r.body["exercise"]["id"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn cross_origin_writes_are_refused() {
    let app = TestApp::spawn().await;
    let id = exercise(&app).await;
    // A form post from another site needs no preflight. Deleting is the worst
    // thing one could do.
    let r = app
        .http
        .delete(format!("{}/api/v2/exercises/{id}", app.base))
        .header("Origin", "https://evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    assert_eq!(app.len("/api/v2/exercises").await, 1);
}

#[tokio::test]
async fn same_origin_requests_pass() {
    let app = TestApp::spawn().await;
    for origin in [app.base.clone(), app.base.replace("127.0.0.1", "localhost")] {
        let host = origin.trim_start_matches("http://").to_owned();
        let r = app
            .http
            .post(format!("{}/api/v2/exercises", app.base))
            .header("Origin", &origin)
            .header("Host", &host)
            .json(&json!({"name": "ok"}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::CREATED, "{origin}");
    }
}

#[tokio::test]
async fn rebinding_hosts_are_refused() {
    let app = TestApp::spawn().await;
    let port = app.addr.rsplit(':').next().unwrap();
    let r = app
        .http
        .get(format!("{}/api/v2/exercises", app.base))
        .header("Host", format!("attacker.example:{port}"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn cross_origin_websockets_are_refused() {
    let app = TestApp::spawn().await;
    let id = exercise(&app).await;
    let mut req = format!("ws://{}/api/v2/exercises/{id}/ws", app.addr).into_client_request().unwrap();
    req.headers_mut().insert("Origin", HeaderValue::from_static("https://evil.example"));
    assert!(tokio_tungstenite::connect_async(req).await.is_err());
}

#[tokio::test]
async fn json_bodies_must_say_so() {
    let app = TestApp::spawn().await;
    let r = app.http.post(format!("{}/api/v2/exercises", app.base)).body(r#"{"name":"x"}"#).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn serves_the_app_with_client_side_routes() {
    let app = TestApp::spawn().await;
    // Built or not, the root and any client route answer the same way, and
    // unknown API paths and files are plain 404s.
    let root = app.http.get(&app.base).send().await.unwrap().status();
    assert!(root == StatusCode::OK || root == StatusCode::SERVICE_UNAVAILABLE, "{root}");
    let route = app.http.get(format!("{}/x/some-exercise", app.base)).send().await.unwrap().status();
    assert_eq!(route, root);
    for missing in ["/nope.js", "/api/v1/pins", "/api/nothing"] {
        let s = app.http.get(format!("{}{missing}", app.base)).send().await.unwrap().status();
        assert_eq!(s, StatusCode::NOT_FOUND, "{missing}");
    }
}

#[tokio::test]
async fn outbound_failures_are_502_not_500() {
    let app = TestApp::spawn().await;
    let r = app.post("/api/v2/geocode", json!({"query": "Washington DC"})).await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY);
    let r = app.post("/api/v2/osm/search", json!({"lat": 38.9, "lon": -77.0, "category": "Hospitals"})).await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY);
    let r = app.post("/api/v2/osm/search", json!({"lat": 38.9, "lon": -77.0, "category": "Bakeries"})).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let r = app
        .post("/api/v2/osm/search", json!({"lat": 38.9, "lon": -77.0, "radius_mi": 0, "category": "Hospitals"}))
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let cats = app.get("/api/v2/osm/categories").await;
    assert!(cats.as_array().unwrap().iter().any(|c| c == "Hospitals"));
}
