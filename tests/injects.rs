//! Injects: pre-authored events an exercise controller fires during a run.

mod common;

use common::{TestApp, statuses};
use reqwest::StatusCode;
use serde_json::json;

fn id(v: &serde_json::Value) -> String {
    v["id"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn starts_untriggered_with_warning_severity() {
    let app = TestApp::spawn().await;
    let i = app.inject(json!({})).await;
    assert!(i["id"].is_string() && i["created_at"].is_string());
    assert!(i["triggered_at"].is_null());
    assert_eq!(i["severity"], "warning");
    assert_eq!(app.get("/api/injects").await[0]["id"], i["id"]);
}

#[tokio::test]
async fn accepts_each_severity() {
    let app = TestApp::spawn().await;
    for s in ["info", "warning", "critical"] {
        assert_eq!(app.inject(json!({"severity": s})).await["severity"], s);
    }
}

#[tokio::test]
async fn rejects_bad_bodies() {
    let app = TestApp::spawn().await;
    for extra in [
        json!({"severity": "catastrophic"}),
        json!({"target_status": "Melted"}),
        json!({"new_pin": {"name": "spawn", "lat": 999, "lon": 0}}),
        json!({"new_pin": {"name": "   ", "lat": 1.0, "lon": 1.0}}),
        json!({"title": "  "}),
    ] {
        let mut body = json!({"title": "x", "description": "y"});
        common::merge(&mut body, extra.clone());
        assert_eq!(app.post("/api/injects", body).await.status, StatusCode::UNPROCESSABLE_ENTITY, "{extra}");
    }
}

#[tokio::test]
async fn delete() {
    let app = TestApp::spawn().await;
    let i = app.inject(json!({})).await;
    assert_eq!(app.delete(&format!("/api/injects/{}", id(&i))).await.status, StatusCode::OK);
    assert_eq!(app.len("/api/injects").await, 0);
    assert_eq!(app.delete("/api/injects/nope").await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn trigger_does_everything_at_once() {
    let app = TestApp::spawn().await;
    let pin = app.pin(json!({"name": "Rosslyn Substation", "status": "Clean"})).await;
    let i = app
        .inject(json!({
            "title": "Substation outage", "description": "grid down",
            "target_pid": id(&pin), "target_status": "Compromised",
            "new_pin": {"name": "Relocated CP", "category": "Asset", "lat": 38.9, "lon": -77.0},
        }))
        .await;
    let r = app.post_empty(&format!("/api/injects/{}/trigger", id(&i))).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.body["triggered_at"].is_string());

    let pins = app.pins().await;
    assert_eq!(pins.len(), 2);
    let target = &pins[&id(&pin)];
    assert_eq!(statuses(&target["status_history"]), ["Clean", "Compromised"]);
    let spawned = pins.values().find(|p| p["name"] == "Relocated CP").unwrap();
    assert_eq!(spawned["status"], "Under Investigation"); // the documented default
    assert_eq!(spawned["op_status"], "Healthy");

    let log = app.get("/api/log").await;
    assert_eq!(log.as_array().unwrap().len(), 1);
    assert_eq!(log[0]["auto"], true);
    assert!(log[0]["action"].as_str().unwrap().contains("Substation outage"));
    assert_eq!(log[0]["notes"], "grid down");
    assert_eq!(log[0]["asset_name"], "Rosslyn Substation");
    assert_eq!(log[0]["asset_pid"], id(&pin));
}

#[tokio::test]
async fn trigger_unknown_is_404_and_missing_target_is_tolerated() {
    let app = TestApp::spawn().await;
    assert_eq!(app.post_empty("/api/injects/nope/trigger").await.status, StatusCode::NOT_FOUND);
    let i = app.inject(json!({"target_pid": "deleted-pin", "target_status": "Compromised"})).await;
    assert_eq!(app.post_empty(&format!("/api/injects/{}/trigger", id(&i))).await.status, StatusCode::OK);
}

#[tokio::test]
async fn cannot_be_triggered_twice() {
    let app = TestApp::spawn().await;
    let pin = app.pin(json!({"status": "Clean"})).await;
    let i = app
        .inject(json!({"target_pid": id(&pin), "target_status": "Compromised", "new_pin": {"name": "CP", "lat": 1.0, "lon": 1.0}}))
        .await;
    let url = format!("/api/injects/{}/trigger", id(&i));
    let fired = app.post_empty(&url).await.body["triggered_at"].clone();
    app.put(&format!("/api/pins/{}", id(&pin)), json!({"status": "Contained"})).await;

    for _ in 0..3 {
        let r = app.post_empty(&url).await;
        assert_eq!(r.status, StatusCode::CONFLICT);
        assert!(r.body["detail"].as_str().unwrap().contains("already been triggered"));
    }
    assert_eq!(app.len("/api/pins").await, 2);
    assert_eq!(app.len("/api/log").await, 1);
    assert_eq!(app.pins().await[&id(&pin)]["status"], "Contained");
    assert_eq!(app.get("/api/injects").await[0]["triggered_at"], fired);
}

#[tokio::test]
async fn concurrent_triggers_fire_once() {
    // The double-click case, raced for real: exactly one request may win.
    let app = TestApp::spawn().await;
    let i = app.inject(json!({"new_pin": {"name": "CP", "lat": 1.0, "lon": 1.0}})).await;
    let url = format!("/api/injects/{}/trigger", id(&i));
    let results = futures_util::future::join_all((0..16).map(|_| app.post_empty(&url))).await;
    let ok = results.iter().filter(|r| r.status == StatusCode::OK).count();
    assert_eq!(ok, 1);
    assert_eq!(app.len("/api/pins").await, 1);
    assert_eq!(app.len("/api/log").await, 1);
}
