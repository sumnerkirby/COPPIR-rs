//! Alert thresholds and the decision log.

mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::json;

#[tokio::test]
async fn threshold_create_and_defaults() {
    let app = TestApp::spawn().await;
    let r = app
        .post(
            "/api/thresholds",
            json!({"name": "Grid integrity", "sector": "power", "below_pct": 40, "severity": "critical"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED);
    assert!(r.body["id"].is_string());
    assert_eq!(r.body["below_pct"], 40);

    let t = app.post("/api/thresholds", json!({"name": "Watch", "sector": "medical"})).await.body;
    assert_eq!(t["below_pct"], 50);
    assert_eq!(t["severity"], "warning");
}

#[tokio::test]
async fn threshold_validation() {
    let app = TestApp::spawn().await;
    for sector in ["all", "medical", "government", "power", "emergency", "civilian", "financial"] {
        assert_eq!(
            app.post("/api/thresholds", json!({"name": "s", "sector": sector})).await.status,
            StatusCode::CREATED
        );
    }
    assert_eq!(
        app.post("/api/thresholds", json!({"name": "s", "sector": "logistics"})).await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    for pct in [-1, 101] {
        let r = app.post("/api/thresholds", json!({"name": "s", "sector": "power", "below_pct": pct})).await;
        assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(r.body["detail"][0]["loc"][1], "below_pct");
    }
    for pct in [0, 100] {
        let r = app.post("/api/thresholds", json!({"name": "s", "sector": "power", "below_pct": pct})).await;
        assert_eq!(r.status, StatusCode::CREATED);
    }
}

#[tokio::test]
async fn threshold_delete_removes_only_that_one() {
    let app = TestApp::spawn().await;
    let a = app.post("/api/thresholds", json!({"name": "a", "sector": "power"})).await.body;
    let b = app.post("/api/thresholds", json!({"name": "b", "sector": "medical"})).await.body;
    assert_eq!(app.delete(&format!("/api/thresholds/{}", a["id"].as_str().unwrap())).await.status, StatusCode::OK);
    let left = app.get("/api/thresholds").await;
    assert_eq!(left.as_array().unwrap().len(), 1);
    assert_eq!(left[0]["id"], b["id"]);
    assert_eq!(app.delete("/api/thresholds/nope").await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn manual_log_entries() {
    let app = TestApp::spawn().await;
    assert_eq!(app.get("/api/log").await, json!([]));
    let r = app.post("/api/log", json!({"action": "incident declared"})).await;
    assert_eq!(r.status, StatusCode::CREATED);
    assert_eq!(r.body["auto"], false);
    assert!(r.body["id"].is_string() && r.body["timestamp"].is_string());

    let pin = app.pin(json!({"name": "Rosslyn Substation"})).await;
    let e = app
        .post("/api/log", json!({"action": "IR team dispatched", "asset_name": pin["name"], "asset_pid": pin["id"]}))
        .await
        .body;
    assert_eq!(e["asset_name"], "Rosslyn Substation");
    assert_eq!(e["asset_pid"], pin["id"]);

    for n in ["third", "fourth"] {
        app.post("/api/log", json!({"action": n})).await;
    }
    let actions: Vec<_> = app
        .get("/api/log")
        .await
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["action"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(actions, ["incident declared", "IR team dispatched", "third", "fourth"]);
}

#[tokio::test]
async fn log_validation() {
    let app = TestApp::spawn().await;
    for action in [json!(""), json!("   "), json!("x".repeat(501))] {
        assert_eq!(app.post("/api/log", json!({"action": action})).await.status, StatusCode::UNPROCESSABLE_ENTITY);
    }
}

#[tokio::test]
async fn timestamps_keep_the_shape_the_frontend_slices() {
    // app.js shows `triggered_at.slice(11, 19)` as HH:MM:SS.
    let app = TestApp::spawn().await;
    let ts = app.post("/api/log", json!({"action": "x"})).await.body["timestamp"].as_str().unwrap().to_owned();
    let hms = &ts[11..19];
    assert!(hms.as_bytes()[2] == b':' && hms.as_bytes()[5] == b':', "{ts}");
}
