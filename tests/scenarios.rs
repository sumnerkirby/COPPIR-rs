//! Scenario save/load/delete, filename sanitising, and the state reset.

mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::json;

async fn populate(app: &TestApp) {
    let a = app.pin(json!({"name": "Substation", "category": "Power Plants", "status": "Compromised"})).await;
    let b = app.pin(json!({"name": "Hospital", "category": "Hospitals"})).await;
    app.post("/api/edges", json!({"from_pid": a["id"], "to_pid": b["id"], "label": "primary feed"})).await;
    app.inject(json!({"title": "Outage"})).await;
    app.post("/api/thresholds", json!({"name": "Grid", "sector": "power", "below_pct": 40, "severity": "critical"}))
        .await;
    app.post("/api/log", json!({"action": "incident declared"})).await;
}

async fn counts(app: &TestApp) -> [usize; 5] {
    [
        app.len("/api/pins").await,
        app.len("/api/edges").await,
        app.len("/api/injects").await,
        app.len("/api/thresholds").await,
        app.len("/api/log").await,
    ]
}

#[tokio::test]
async fn save_list_and_overwrite() {
    let app = TestApp::spawn().await;
    populate(&app).await;
    let r = app.post("/api/scenarios/save", json!({"name": "Exercise One"})).await;
    assert_eq!((r.status, r.body), (StatusCode::OK, json!({"ok": true, "name": "Exercise One"})));
    assert_eq!(app.get("/api/scenarios").await, json!(["Exercise One"]));

    let written: Vec<_> = std::fs::read_dir(app.scenarios_dir()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(written, ["Exercise One.json"]);
    let on_disk: serde_json::Value =
        serde_json::from_slice(&std::fs::read(app.scenarios_dir().join("Exercise One.json")).unwrap()).unwrap();
    assert_eq!(on_disk["name"], "Exercise One");

    app.delete("/api/pins").await;
    app.pin(json!({"name": "only"})).await;
    app.post("/api/scenarios/save", json!({"name": "Exercise One"})).await;
    assert_eq!(app.get("/api/scenarios").await, json!(["Exercise One"]));
    app.post("/api/scenarios/load", json!({"name": "Exercise One"})).await;
    assert_eq!(app.len("/api/pins").await, 1);
}

#[tokio::test]
async fn listing_is_sorted() {
    let app = TestApp::spawn().await;
    for n in ["Charlie", "alpha", "Bravo"] {
        app.post("/api/scenarios/save", json!({"name": n})).await;
    }
    assert_eq!(app.get("/api/scenarios").await, json!(["Bravo", "Charlie", "alpha"]));
}

#[tokio::test]
async fn load_restores_every_store_and_replaces_rather_than_merges() {
    let app = TestApp::spawn().await;
    populate(&app).await;
    app.post("/api/scenarios/save", json!({"name": "Full"})).await;
    app.post_empty("/api/state/clear").await;
    app.pin(json!({"name": "added later"})).await;

    let r = app.post("/api/scenarios/load", json!({"name": "Full"})).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["count"], 2);
    assert_eq!(counts(&app).await, [2, 1, 1, 1, 1]);

    let pins = app.pins().await;
    assert!(pins.values().all(|p| p["name"] != "added later"));
    let sub = pins.values().find(|p| p["name"] == "Substation").unwrap();
    assert_eq!(sub["status"], "Compromised");
    assert_eq!(sub["category"], "Power Plants");
    assert_eq!(sub["status_history"][0]["status"], "Compromised");
}

#[tokio::test]
async fn unknown_scenarios_are_404() {
    let app = TestApp::spawn().await;
    assert_eq!(app.post("/api/scenarios/load", json!({"name": "never saved"})).await.status, StatusCode::NOT_FOUND);
    assert_eq!(app.delete("/api/scenarios/nothing-here").await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_leaves_loaded_state_alone() {
    let app = TestApp::spawn().await;
    app.pin(json!({"name": "still here"})).await;
    app.post("/api/scenarios/save", json!({"name": "Temp Name"})).await;
    assert_eq!(app.delete("/api/scenarios/Temp%20Name").await.status, StatusCode::OK);
    assert_eq!(app.get("/api/scenarios").await, json!([]));
    assert_eq!(app.len("/api/pins").await, 1);
}

#[tokio::test]
async fn names_are_sanitised() {
    let app = TestApp::spawn().await;
    for (raw, stored) in [
        ("Exercise One", "Exercise One"),
        ("drop/slashes", "dropslashes"),
        ("dots...everywhere", "dotseverywhere"),
        ("  padded  ", "padded"),
        ("emoji \u{1F600} stripped", "emoji  stripped"),
    ] {
        let r = app.post("/api/scenarios/save", json!({"name": raw})).await;
        assert_eq!(r.status, StatusCode::OK, "{raw:?}");
        assert_eq!(r.body["name"], stored);
    }
    assert_eq!(app.post("/api/scenarios/save", json!({"name": "..."})).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(app.post("/api/scenarios/save", json!({"name": "NUL"})).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        app.post("/api/scenarios/save", json!({"name": "x".repeat(65)})).await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn traversal_cannot_escape_the_directory() {
    let app = TestApp::spawn().await;
    for attack in [
        "../../../etc/passwd",
        "..%2F..%2Fetc%2Fpasswd",
        "/etc/passwd",
        "....//....//etc/passwd",
        "..\\..\\windows\\system32",
    ] {
        app.post("/api/scenarios/save", json!({"name": attack})).await;
    }
    for entry in walk(app.dir.path()) {
        assert_eq!(entry.parent().unwrap(), app.scenarios_dir(), "{}", entry.display());
    }

    let outside = app.dir.path().join("secret.json");
    std::fs::write(&outside, r#"{"pins": {"x": {"id": "x", "name": "leaked", "lat": 0, "lon": 0}}}"#).unwrap();
    let r = app.post("/api/scenarios/load", json!({"name": "../secret"})).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(app.len("/api/pins").await, 0);
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else if p.file_name() != Some("secret.json".as_ref()) {
            out.push(p);
        }
    }
    out
}

#[tokio::test]
async fn a_damaged_file_is_refused_without_touching_state() {
    let app = TestApp::spawn().await;
    app.pin(json!({"name": "keep"})).await;
    std::fs::write(app.scenarios_dir().join("broken.json"), "{\"pins\": [1, 2").unwrap();
    let r = app.post("/api/scenarios/load", json!({"name": "broken"})).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert!(r.body["detail"].as_str().unwrap().contains("damaged"));
    assert_eq!(app.len("/api/pins").await, 1);
}

#[tokio::test]
async fn loads_a_scenario_saved_by_the_python_version() {
    // Naive timestamps, no op_status on inject-spawned pins, a dangling edge:
    // all things the Python app could write.
    let app = TestApp::spawn().await;
    let legacy = json!({
        "pins": {
            "p1": {"id": "p1", "name": "Sub", "category": "Power Plants", "lat": 1.0, "lon": 2.0,
                   "status": "Clean", "op_status": "Degraded", "pin_color": null, "notes": "",
                   "pinned_at": "2025-01-01T10:00:00.000001",
                   "status_history": [{"status": "Clean", "timestamp": "2025-01-01T10:00:00.000001"}]},
            "p2": {"id": "p2", "name": "Spawned", "category": "Asset", "lat": 1.0, "lon": 2.0,
                   "status": "Under Investigation", "pin_color": null, "notes": "",
                   "pinned_at": "2025-01-01T10:05:00", "status_history": []}
        },
        "injects": {"i1": {"id": "i1", "title": "t", "description": "d", "severity": "info",
                           "target_pid": null, "target_status": null, "new_pin": null,
                           "created_at": "2025-01-01T10:00:00", "triggered_at": null}},
        "edges": {"e1": {"id": "e1", "from_pid": "p1", "to_pid": "p2", "label": "", "edge_type": "connection"},
                  "e2": {"id": "e2", "from_pid": "p1", "to_pid": "gone", "label": "", "edge_type": "connection"}},
        "thresholds": [{"id": "t1", "name": "Grid", "sector": "power", "below_pct": 40, "severity": "critical"}],
        "decision_log": [{"id": "l1", "timestamp": "2025-01-01T10:00:00", "action": "declared",
                          "asset_name": null, "asset_pid": null, "notes": "", "auto": false}],
        "saved_at": "2025-01-01T11:00:00",
        "name": "Legacy"
    });
    std::fs::write(app.scenarios_dir().join("Legacy.json"), legacy.to_string()).unwrap();
    let r = app.post("/api/scenarios/load", json!({"name": "Legacy"})).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(counts(&app).await, [2, 1, 1, 1, 1]);
    assert_eq!(app.pins().await["p2"]["op_status"], "Healthy");
}

#[tokio::test]
async fn clear_empties_every_store_but_keeps_files() {
    let app = TestApp::spawn().await;
    populate(&app).await;
    app.post("/api/scenarios/save", json!({"name": "Survives"})).await;
    let r = app.post_empty("/api/state/clear").await;
    assert_eq!(r.body, json!({"ok": true}));
    assert_eq!(counts(&app).await, [0; 5]);
    assert_eq!(app.get("/api/scenarios").await, json!(["Survives"]));
}
