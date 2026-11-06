//! Real-time sync. Every change reaches every client; a client joining
//! mid-exercise is handed the whole picture immediately.

mod common;

use std::collections::HashSet;

use common::TestApp;
use serde_json::json;

#[tokio::test]
async fn connect_sends_full_state_with_the_existing_session() {
    let app = TestApp::spawn().await;
    let a = app.pin(json!({"name": "Substation"})).await;
    let b = app.pin(json!({"name": "Hospital"})).await;
    app.post("/api/edges", json!({"from_pid": a["id"], "to_pid": b["id"]})).await;
    app.inject(json!({})).await;
    app.post("/api/log", json!({"action": "declared"})).await;

    let mut ws = app.ws_raw().await;
    let msg = ws.recv().await;
    let keys: HashSet<_> = msg.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, HashSet::from(["type", "pins", "injects", "edges", "thresholds", "log"]));
    assert_eq!(msg["type"], "full_state");
    let pids: HashSet<_> = msg["pins"].as_object().unwrap().keys().cloned().collect();
    assert_eq!(pids, HashSet::from([a["id"].as_str().unwrap().to_owned(), b["id"].as_str().unwrap().to_owned()]));
    assert_eq!(msg["edges"].as_array().unwrap().len(), 1);
    assert_eq!(msg["injects"].as_array().unwrap().len(), 1);
    assert_eq!(msg["log"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn pin_events() {
    let app = TestApp::spawn().await;
    let mut ws = app.ws().await;

    let pin = app.pin(json!({"name": "New"})).await;
    let m = ws.recv().await;
    assert_eq!((m["type"].as_str(), m["pin"]["name"].as_str()), (Some("pin_add"), Some("New")));

    let url = format!("/api/pins/{}", pin["id"].as_str().unwrap());
    app.put(&url, json!({"status": "Contained"})).await;
    let m = ws.recv().await;
    assert_eq!((m["type"].as_str(), m["pin"]["status"].as_str()), (Some("pin_update"), Some("Contained")));

    app.delete(&url).await;
    let m = ws.recv().await;
    assert_eq!((m["type"].as_str(), &m["pid"]), (Some("pin_delete"), &pin["id"]));

    app.post("/api/pins/bulk", json!({"items": [{"name": "A", "category": "Banks", "lat": 1.0, "lon": 1.0}, {"name": "B", "category": "Banks", "lat": 2.0, "lon": 2.0}]})).await;
    let m = ws.recv().await;
    assert_eq!(m["type"], "bulk_add");
    assert_eq!(m["pins"].as_array().unwrap().len(), 2);

    app.post("/api/pins/bulk-status", json!({"category": "Banks", "status": "Compromised"})).await;
    let m = ws.recv().await;
    assert_eq!(m["type"], "bulk_update");
    assert!(m["pins"].as_array().unwrap().iter().all(|p| p["status"] == "Compromised"));

    app.delete("/api/pins").await;
    let m = ws.recv().await;
    assert_eq!(m["type"], "pins_cleared");
    assert_eq!(m["pids"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn edge_inject_threshold_and_log_events() {
    let app = TestApp::spawn().await;
    let a = app.pin(json!({"name": "a", "status": "Clean"})).await;
    let b = app.pin(json!({"name": "b"})).await;
    let mut ws = app.ws().await;

    let e = app.post("/api/edges", json!({"from_pid": a["id"], "to_pid": b["id"]})).await.body;
    assert_eq!(ws.recv().await["type"], "edge_add");
    app.delete(&format!("/api/edges/{}", e["id"].as_str().unwrap())).await;
    let m = ws.recv().await;
    assert_eq!((m["type"].as_str(), &m["eid"]), (Some("edge_delete"), &e["id"]));

    let i = app.inject(json!({"target_pid": a["id"], "target_status": "Compromised"})).await;
    assert_eq!(ws.recv().await["type"], "inject_queued");
    app.post_empty(&format!("/api/injects/{}/trigger", i["id"].as_str().unwrap())).await;
    let m = ws.recv().await;
    assert_eq!(m["type"], "inject_triggered");
    assert_eq!(m["pins"][a["id"].as_str().unwrap()]["status"], "Compromised");
    assert_eq!(m["log_entry"]["auto"], true);
    assert!(m["inject"]["triggered_at"].is_string());
    assert!(m["new_pin"].is_null());
    app.delete(&format!("/api/injects/{}", i["id"].as_str().unwrap())).await;
    let m = ws.recv().await;
    assert_eq!((m["type"].as_str(), &m["iid"]), (Some("inject_delete"), &i["id"]));

    let t = app.post("/api/thresholds", json!({"name": "Grid integrity", "sector": "power"})).await.body;
    let m = ws.recv().await;
    assert_eq!((m["type"].as_str(), m["threshold"]["name"].as_str()), (Some("threshold_add"), Some("Grid integrity")));
    app.delete(&format!("/api/thresholds/{}", t["id"].as_str().unwrap())).await;
    let m = ws.recv().await;
    assert_eq!((m["type"].as_str(), &m["tid"]), (Some("threshold_delete"), &t["id"]));

    app.post("/api/log", json!({"action": "declared"})).await;
    let m = ws.recv().await;
    assert_eq!((m["type"].as_str(), m["entry"]["action"].as_str()), (Some("log_entry"), Some("declared")));
}

#[tokio::test]
async fn failed_requests_broadcast_nothing() {
    let app = TestApp::spawn().await;
    let mut ws = app.ws().await;
    app.delete("/api/thresholds/nope").await;
    app.delete("/api/pins/nope").await;
    app.post("/api/pins", json!({"name": "", "lat": 0, "lon": 0})).await;
    app.post("/api/log", json!({"action": "sentinel"})).await;
    // A real message would have arrived ahead of this one.
    assert_eq!(ws.recv().await["type"], "log_entry");
}

#[tokio::test]
async fn scenario_load_and_clear_resync_everyone() {
    let app = TestApp::spawn().await;
    app.pin(json!({"name": "saved"})).await;
    let mut ws = app.ws().await;
    app.post("/api/scenarios/save", json!({"name": "Snap"})).await;

    app.post_empty("/api/state/clear").await;
    let m = ws.recv().await;
    assert_eq!(m["type"], "full_state");
    assert_eq!((m["pins"].clone(), m["log"].clone()), (json!({}), json!([])));

    app.post("/api/scenarios/load", json!({"name": "Snap"})).await;
    let m = ws.recv().await;
    assert_eq!(m["type"], "full_state");
    let names: Vec<_> = m["pins"].as_object().unwrap().values().map(|p| p["name"].clone()).collect();
    assert_eq!(names, [json!("saved")]);
}

#[tokio::test]
async fn every_window_sees_every_change_in_order() {
    let app = TestApp::spawn().await;
    let mut one = app.ws().await;
    let mut two = app.ws().await;
    for i in 0..20 {
        app.pin(json!({"name": format!("p{i}")})).await;
    }
    for ws in [&mut one, &mut two] {
        for i in 0..20 {
            let m = ws.recv().await;
            assert_eq!(m["pin"]["name"], format!("p{i}"));
        }
    }
    one.close().await;
    // The other client is unaffected by the first one leaving.
    app.post("/api/thresholds", json!({"name": "shared", "sector": "medical"})).await;
    assert_eq!(two.recv().await["type"], "threshold_add");
}
