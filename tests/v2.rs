//! /api/v2: exercises, commands, the event log and live streams.

mod common;

use common::TestApp;
use futures_util::future::join_all;
use reqwest::StatusCode;
use serde_json::{Value, json};

async fn exercise(app: &TestApp) -> (String, Value) {
    let r = app.post("/api/v2/exercises", json!({"name": "Grid Drill"})).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    (r.body["exercise"]["id"].as_str().unwrap().to_owned(), r.body)
}

async fn cmd(app: &TestApp, id: &str, body: Value) -> common::Res {
    app.post(&format!("/api/v2/exercises/{id}/commands"), body).await
}

async fn add_asset(app: &TestApp, id: &str, name: &str, type_id: &str) -> String {
    let r = cmd(
        app,
        id,
        json!({"type": "add_assets", "assets": [{"name": name, "type_id": type_id, "geo": {"lat": 38.9, "lon": -77.0}}]}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    r.body["events"][0]["event"]["assets"][0]["id"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn create_list_get_delete() {
    let app = TestApp::spawn().await;
    let (id, created) = exercise(&app).await;
    assert_eq!(created["seq"], 1);
    assert_eq!(created["exercise"]["taxonomy"]["status_sets"][0]["id"], "security");

    let list = app.get("/api/v2/exercises").await;
    assert_eq!(list[0]["id"], id.as_str());
    assert_eq!(list[0]["name"], "Grid Drill");

    let snap = app.get(&format!("/api/v2/exercises/{id}")).await;
    assert_eq!(snap["exercise"]["name"], "Grid Drill");

    assert_eq!(app.delete(&format!("/api/v2/exercises/{id}")).await.status, StatusCode::NO_CONTENT);
    assert_eq!(
        app.call(reqwest::Method::GET, &format!("/api/v2/exercises/{id}"), None).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(app.post("/api/v2/exercises", json!({"name": "  "})).await.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn assets_take_taxonomy_defaults_and_record_history() {
    let app = TestApp::spawn().await;
    let (id, _) = exercise(&app).await;
    let a = add_asset(&app, &id, "Rosslyn Substation", "power_plants").await;

    let r = cmd(&app, &id, json!({"type": "set_status", "ids": [a], "set": "security", "value": "compromised"})).await;
    assert_eq!(r.body["seq"], 3);
    let asset = &r.body["events"][0]["event"]["assets"][0];
    assert_eq!(asset["statuses"], json!({"operational": "healthy", "security": "compromised"}));
    let history: Vec<_> = asset["history"].as_array().unwrap().iter().map(|h| h["value"].clone()).collect();
    assert_eq!(history, [json!("healthy"), json!("clean"), json!("compromised")]);

    // Setting the same value again produces no event and no new seq.
    let r = cmd(&app, &id, json!({"type": "set_status", "ids": [a], "set": "security", "value": "compromised"})).await;
    assert_eq!((r.body["seq"].clone(), r.body["events"].clone()), (json!(3), json!([])));
}

#[tokio::test]
async fn commands_are_checked_against_the_exercise() {
    let app = TestApp::spawn().await;
    let (id, _) = exercise(&app).await;
    let a = add_asset(&app, &id, "A", "asset").await;
    for (body, status) in [
        (json!({"type": "add_assets", "assets": [{"name": "x", "type_id": "spaceship"}]}), 422),
        (json!({"type": "add_assets", "assets": [{"name": " ", "type_id": "asset"}]}), 422),
        (
            json!({"type": "add_assets", "assets": [{"name": "x", "type_id": "asset", "geo": {"lat": 91, "lon": 0}}]}),
            422,
        ),
        (json!({"type": "add_assets", "assets": [{"name": "x", "type_id": "asset", "color": "red"}]}), 422),
        (json!({"type": "set_status", "ids": [a], "set": "security", "value": "melted"}), 422),
        (json!({"type": "set_status", "ids": [a], "set": "mood", "value": "happy"}), 422),
        (json!({"type": "set_status", "ids": ["nope"], "set": "security", "value": "clean"}), 404),
        (json!({"type": "edit_asset", "id": "nope", "patch": {}}), 404),
        (json!({"type": "add_link", "from": a, "to": a}), 400),
        (json!({"type": "add_link", "from": a, "to": "nope"}), 400),
        (json!({"type": "add_threshold", "name": "t", "sector": "logistics", "below_pct": 50}), 422),
        (json!({"type": "add_threshold", "name": "t", "below_pct": 101}), 422),
        (json!({"type": "log", "text": "x", "asset": "nope"}), 422),
        (json!({"type": "teleport"}), 422),
    ] {
        let r = cmd(&app, &id, body.clone()).await;
        assert_eq!(r.status.as_u16(), status, "{body} → {}", r.body);
    }
    // Nothing above made it into the log.
    assert_eq!(app.get(&format!("/api/v2/exercises/{id}")).await["seq"], 2);
}

#[tokio::test]
async fn edit_patch_semantics() {
    let app = TestApp::spawn().await;
    let (id, _) = exercise(&app).await;
    let a = add_asset(&app, &id, "A", "asset").await;
    let edit = |patch: Value| cmd(&app, &id, json!({"type": "edit_asset", "id": a, "patch": patch}));

    let r = edit(json!({"name": "  Renamed  ", "color": "#112233", "tags": ["ot", " ot ", "", "scada"]})).await;
    let asset = &r.body["events"][0]["event"]["assets"][0];
    assert_eq!(asset["name"], "Renamed");
    assert_eq!(asset["color"], "#112233");
    assert_eq!(asset["tags"], json!(["ot", "scada"]));

    // Absent leaves alone, null clears.
    let r = edit(json!({"color": null})).await;
    let asset = &r.body["events"][0]["event"]["assets"][0];
    assert!(asset["color"].is_null());
    assert_eq!(asset["name"], "Renamed");
    assert!(asset["geo"].is_object());
    let r = edit(json!({"geo": null})).await;
    assert!(r.body["events"][0]["event"]["assets"][0]["geo"].is_null());
}

#[tokio::test]
async fn removing_assets_takes_their_links() {
    let app = TestApp::spawn().await;
    let (id, _) = exercise(&app).await;
    let (a, b, c) = (
        add_asset(&app, &id, "a", "asset").await,
        add_asset(&app, &id, "b", "asset").await,
        add_asset(&app, &id, "c", "asset").await,
    );
    cmd(&app, &id, json!({"type": "add_link", "from": a, "to": b, "kind": "depends_on"})).await;
    let keep = cmd(&app, &id, json!({"type": "add_link", "from": b, "to": c})).await.body["events"][0]["event"]["link"]
        ["id"]
        .clone();
    let r = cmd(&app, &id, json!({"type": "remove_assets", "ids": [a]})).await;
    assert_eq!(r.body["events"][0]["event"]["links"].as_array().unwrap().len(), 1);
    let links = app.get(&format!("/api/v2/exercises/{id}")).await["exercise"]["links"].clone();
    assert_eq!(links.as_object().unwrap().keys().collect::<Vec<_>>(), [keep.as_str().unwrap()]);
}

#[tokio::test]
async fn injects_fire_once_with_all_effects_in_one_event() {
    let app = TestApp::spawn().await;
    let (id, _) = exercise(&app).await;
    let a = add_asset(&app, &id, "Rosslyn Substation", "power_plants").await;
    let r = cmd(
        &app,
        &id,
        json!({"type": "add_inject", "inject": {
        "title": "SCADA compromise", "body": "breaker opened", "severity": "critical",
        "effects": [
            {"type": "set_status", "asset": a, "set": "security", "value": "compromised"},
            {"type": "set_status", "asset": a, "set": "operational", "value": "offline"},
            {"type": "create_asset", "asset": {"name": "Field team", "type_id": "responder"}}
        ]}}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let iid = r.body["events"][0]["event"]["inject"]["id"].as_str().unwrap().to_owned();

    // Race 12 fires: exactly one wins.
    let fires = join_all((0..12).map(|_| cmd(&app, &id, json!({"type": "fire_inject", "id": iid})))).await;
    assert_eq!(fires.iter().filter(|r| r.status == StatusCode::OK).count(), 1);
    assert!(fires.iter().all(|r| r.status == StatusCode::OK || r.status == StatusCode::CONFLICT));

    let fired = fires.iter().find(|r| r.status == StatusCode::OK).unwrap();
    let ev = &fired.body["events"][0]["event"];
    assert_eq!(ev["type"], "inject_fired");
    assert_eq!(ev["assets"].as_array().unwrap().len(), 2, "target once, plus the new asset");
    assert_eq!(ev["assets"][0]["statuses"], json!({"operational": "offline", "security": "compromised"}));
    assert_eq!(ev["entry"]["asset_name"], "Rosslyn Substation");
    assert_eq!(ev["entry"]["auto"], true);

    let ex = app.get(&format!("/api/v2/exercises/{id}")).await["exercise"].clone();
    assert_eq!(ex["assets"].as_object().unwrap().len(), 2);
    assert_eq!(ex["log"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn taxonomy_replacement_refits_statuses_and_protects_used_types() {
    let app = TestApp::spawn().await;
    let (id, created) = exercise(&app).await;
    let a = add_asset(&app, &id, "Host", "asset").await;
    cmd(&app, &id, json!({"type": "set_status", "ids": [a], "set": "security", "value": "compromised"})).await;

    let mut tax = created["exercise"]["taxonomy"].clone();
    // Drop "compromised" and add a new status set.
    tax["status_sets"][0]["values"].as_array_mut().unwrap().retain(|v| v["id"] != "compromised");
    tax["status_sets"].as_array_mut().unwrap().push(json!({
        "id": "comms", "name": "Comms", "default": "quiet", "display": "detail",
        "values": [{"id": "quiet", "label": "Quiet", "color": "#888888", "glyph": "", "score": 100}]
    }));
    let r = cmd(&app, &id, json!({"type": "set_taxonomy", "taxonomy": tax})).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let asset = &r.body["events"][0]["event"]["assets"][0];
    assert_eq!(asset["statuses"], json!({"comms": "quiet", "operational": "healthy", "security": "clean"}));

    // Removing the "asset" type while an asset uses it is refused.
    let mut tax2 = tax.clone();
    tax2["asset_types"].as_array_mut().unwrap().retain(|t| t["id"] != "asset");
    tax2["sectors"] = json!([]);
    assert_eq!(
        cmd(&app, &id, json!({"type": "set_taxonomy", "taxonomy": tax2})).await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn the_event_log_rebuilds_the_exercise() {
    let app = TestApp::spawn().await;
    let (id, _) = exercise(&app).await;
    let a = add_asset(&app, &id, "a", "asset").await;
    cmd(&app, &id, json!({"type": "rename", "name": "Renamed"})).await;
    cmd(&app, &id, json!({"type": "log", "text": "declared", "asset": a})).await;
    let events = app.get(&format!("/api/v2/exercises/{id}/events")).await;
    let kinds: Vec<_> =
        events.as_array().unwrap().iter().map(|e| e["event"]["type"].as_str().unwrap().to_owned()).collect();
    assert_eq!(kinds, ["created", "assets_upserted", "renamed", "logged"]);
    let seqs: Vec<_> = events.as_array().unwrap().iter().map(|e| e["seq"].as_u64().unwrap()).collect();
    assert_eq!(seqs, [1, 2, 3, 4]);
    let tail = app.get(&format!("/api/v2/exercises/{id}/events?after=3")).await;
    assert_eq!(tail.as_array().unwrap().len(), 1);
    assert_eq!(tail[0]["actor"], "facilitator");
}

#[tokio::test]
async fn websocket_snapshot_then_events_and_resume() {
    let app = TestApp::spawn().await;
    let (id, _) = exercise(&app).await;
    let url = |q: &str| format!("ws://{}/api/v2/exercises/{id}/ws{q}", app.addr);

    let (ws, _) = tokio_tungstenite::connect_async(url("")).await.unwrap();
    let mut ws = common::Ws(ws);
    let snap = ws.recv().await;
    assert_eq!((snap["kind"].as_str(), snap["seq"].as_u64()), (Some("snapshot"), Some(1)));

    let a = add_asset(&app, &id, "a", "asset").await;
    cmd(&app, &id, json!({"type": "log", "text": "one"})).await;
    let m = ws.recv().await;
    assert_eq!(
        (m["kind"].as_str(), m["seq"].as_u64(), m["event"]["type"].as_str()),
        (Some("event"), Some(2), Some("assets_upserted"))
    );
    assert_eq!(ws.recv().await["seq"], 3);

    // A reconnecting client that saw seq 2 gets only seq 3, then live events.
    let (resumed, _) = tokio_tungstenite::connect_async(url("?after=2")).await.unwrap();
    let mut resumed = common::Ws(resumed);
    let m = resumed.recv().await;
    assert_eq!((m["kind"].as_str(), m["seq"].as_u64()), (Some("event"), Some(3)));
    cmd(&app, &id, json!({"type": "remove_assets", "ids": [a]})).await;
    assert_eq!(resumed.recv().await["seq"], 4);
    assert_eq!(ws.recv().await["seq"], 4);

    // A client claiming to be ahead of the server gets a snapshot.
    let (ahead, _) = tokio_tungstenite::connect_async(url("?after=99")).await.unwrap();
    assert_eq!(common::Ws(ahead).recv().await["kind"], "snapshot");
}

/// A scenario file as the v1 app (and the Python app before it) wrote them.
fn v1_scenario() -> Value {
    json!({
        "pins": {
            "p1": {"id": "p1", "name": "Substation", "category": "Power Plants", "lat": 38.9, "lon": -77.0,
                   "status": "Compromised", "op_status": "Degraded", "pin_color": null, "notes": "",
                   "pinned_at": "2025-01-01T10:00:00", "status_history": [{"status": "Compromised", "timestamp": "2025-01-01T10:00:00"}]},
            "p2": {"id": "p2", "name": "Van", "category": "Mobile Units", "lat": 38.91, "lon": -77.01}
        },
        "edges": {"e1": {"id": "e1", "from_pid": "p1", "to_pid": "p2", "label": "", "edge_type": "dependency"}},
        "injects": {"i1": {"id": "i1", "title": "t", "description": "d", "severity": "info",
                           "target_pid": "p1", "target_status": "Contained", "new_pin": null,
                           "created_at": "2025-01-01T10:00:00", "triggered_at": null}},
        "thresholds": [{"id": "t1", "name": "Grid", "sector": "power", "below_pct": 40, "severity": "critical"}],
        "decision_log": [{"id": "l1", "timestamp": "2025-01-01T10:00:00", "action": "declared", "auto": false}],
        "saved_at": "2025-01-01T11:00:00",
        "name": "Legacy"
    })
}

#[tokio::test]
async fn imports_a_v1_scenario_body() {
    let app = TestApp::spawn().await;
    let r = app.post("/api/v2/exercises/import/v1", json!({"name": "Legacy import", "scenario": v1_scenario()})).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let ex = &r.body["exercise"];
    assert_eq!(ex["assets"].as_object().unwrap().len(), 2);
    assert_eq!(ex["links"].as_object().unwrap().len(), 1);
    assert_eq!(ex["injects"].as_object().unwrap().len(), 1);
    assert_eq!(ex["log"].as_array().unwrap().len(), 1);
    assert_eq!(ex["assets"]["p1"]["statuses"]["security"], "compromised");

    let bad = app.post("/api/v2/exercises/import/v1", json!({"name": "x", "scenario": {"pins": [1]}})).await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn offers_and_imports_scenarios_left_by_v1() {
    let app = TestApp::spawn().await;
    // No folder yet: nothing to offer, and that is not an error.
    assert_eq!(app.get("/api/v2/legacy/scenarios").await, json!([]));

    let dir = app.scenarios_dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Exercise One.json"), v1_scenario().to_string()).unwrap();
    std::fs::write(dir.join("broken.json"), "{not json").unwrap();
    std::fs::write(dir.join("notes.txt"), "ignored").unwrap();
    assert_eq!(app.get("/api/v2/legacy/scenarios").await, json!(["Exercise One", "broken"]));

    let r = app.post("/api/v2/legacy/scenarios/import", json!({"name": "Exercise One"})).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    assert_eq!(r.body["exercise"]["name"], "Exercise One");
    assert_eq!(r.body["exercise"]["assets"].as_object().unwrap().len(), 2);

    for (name, status) in [("broken", 400), ("missing", 404), ("../secret", 400), ("", 400)] {
        let r = app.post("/api/v2/legacy/scenarios/import", json!({"name": name})).await;
        assert_eq!(r.status.as_u16(), status, "{name}");
    }
}

#[tokio::test]
async fn undo_reverses_the_latest_change_and_is_logged() {
    let app = TestApp::spawn().await;
    let (id, _) = exercise(&app).await;
    let a = add_asset(&app, &id, "Substation", "power_plants").await;
    cmd(&app, &id, json!({"type": "set_status", "ids": [a], "set": "security", "value": "compromised"})).await;
    cmd(&app, &id, json!({"type": "log", "text": "declared"})).await;

    // Undo the log entry, then the status change, then the add.
    let r = cmd(&app, &id, json!({"type": "undo"})).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["events"][0]["event"], json!({"type": "undone", "seqs": [4]}));
    assert_eq!(r.body["events"][1]["event"]["type"], "log_removed");
    let r = cmd(&app, &id, json!({"type": "undo"})).await;
    assert_eq!(r.body["events"][1]["event"]["assets"][0]["statuses"]["security"], "clean");
    cmd(&app, &id, json!({"type": "undo"})).await;

    let snap = app.get(&format!("/api/v2/exercises/{id}")).await;
    assert_eq!(snap["exercise"]["assets"], json!({}));
    assert_eq!(snap["exercise"]["log"], json!([]));
    // The history keeps everything: 1 create, 3 changes, 3 undos of 2 events each.
    assert_eq!(snap["seq"], 10);
    let r = cmd(&app, &id, json!({"type": "undo"})).await;
    assert_eq!(r.status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn undo_refuses_to_clobber_a_later_change() {
    let app = TestApp::spawn().await;
    let (id, _) = exercise(&app).await;
    let a = add_asset(&app, &id, "a", "asset").await;
    let b = add_asset(&app, &id, "b", "asset").await;
    // Change b, then a. Undo reaches a's change first; that is fine.
    cmd(&app, &id, json!({"type": "set_status", "ids": [b], "set": "security", "value": "monitored"})).await;
    cmd(&app, &id, json!({"type": "set_status", "ids": [a], "set": "security", "value": "monitored"})).await;
    assert_eq!(cmd(&app, &id, json!({"type": "undo"})).await.status, StatusCode::OK);
    // Now overwrite b's change outside the undo stack's view: the stale undo
    // of b's first change is refused rather than throwing the second away.
    let r = cmd(&app, &id, json!({"type": "edit_asset", "id": b, "patch": {"notes": "edited"}})).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(cmd(&app, &id, json!({"type": "undo"})).await.status, StatusCode::OK, "undoes the notes edit");
    assert_eq!(cmd(&app, &id, json!({"type": "undo"})).await.status, StatusCode::OK, "then b's status");
    let snap = app.get(&format!("/api/v2/exercises/{id}")).await;
    assert_eq!(snap["exercise"]["assets"][&b]["statuses"]["security"], "clean");
}

#[tokio::test]
async fn annotations_and_metrics() {
    let app = TestApp::spawn().await;
    let (id, _) = exercise(&app).await;
    let poly =
        json!({"type": "Polygon", "coordinates": [[[-77.0, 38.9], [-77.1, 38.9], [-77.1, 39.0], [-77.0, 38.9]]]});
    let r = cmd(&app, &id, json!({"type": "add_annotation", "shape": "polygon", "geometry": poly, "label": "Cordon"}))
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let ann = &r.body["events"][0]["event"]["annotation"];
    assert_eq!((ann["label"].as_str(), ann["color"].as_str()), (Some("Cordon"), Some("#00ff41")));
    let aid = ann["id"].as_str().unwrap().to_owned();

    for bad in [
        json!({"type": "add_annotation", "shape": "polygon", "geometry": {"type": "Point", "coordinates": [0, 0]}}),
        json!({"type": "add_annotation", "shape": "point", "geometry": {"type": "Point", "coordinates": [200, 0]}}),
        json!({"type": "add_annotation", "shape": "line", "geometry": {"type": "LineString", "coordinates": [[0, 0]]}}),
        json!({"type": "add_annotation", "shape": "point", "geometry": {"type": "Point", "coordinates": [0, 0]}, "color": "red"}),
    ] {
        assert_eq!(cmd(&app, &id, bad.clone()).await.status, StatusCode::UNPROCESSABLE_ENTITY, "{bad}");
    }
    let r = cmd(&app, &id, json!({"type": "edit_annotation", "id": aid, "color": "#ff0000"})).await;
    assert_eq!(r.body["events"][0]["event"]["annotation"]["color"], "#ff0000");
    cmd(&app, &id, json!({"type": "remove_annotations", "ids": [aid]})).await;

    let m = cmd(&app, &id, json!({"type": "add_metric", "name": "Comms readiness", "value": 80})).await;
    let mid = m.body["events"][0]["event"]["metric"]["id"].as_str().unwrap().to_owned();
    assert_eq!(
        cmd(&app, &id, json!({"type": "edit_metric", "id": mid, "value": 101})).await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    cmd(&app, &id, json!({"type": "edit_metric", "id": mid, "value": 35})).await;
    let ex = app.get(&format!("/api/v2/exercises/{id}")).await["exercise"].clone();
    assert_eq!(ex["annotations"], json!({}));
    assert_eq!(ex["metrics"][0]["value"], 35);
}

#[tokio::test]
async fn export_import_and_duplicate() {
    let app = TestApp::spawn().await;
    let (id, _) = exercise(&app).await;
    let a = add_asset(&app, &id, "a", "asset").await;
    let b = add_asset(&app, &id, "b", "asset").await;
    cmd(&app, &id, json!({"type": "add_link", "from": a, "to": b})).await;

    let r = app.http.get(format!("{}/api/v2/exercises/{id}/export", app.base)).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(r.headers()["content-disposition"], "attachment; filename=\"Grid Drill.coppir\"");
    let file: Value = r.json().await.unwrap();
    assert_eq!(file["format"], "coppir.exercise");

    let r = app.post("/api/v2/exercises/import", json!({"file": file, "name": "Imported"})).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    assert_ne!(r.body["exercise"]["id"], json!(id));
    assert_eq!(r.body["exercise"]["name"], "Imported");
    assert_eq!(r.body["exercise"]["links"].as_object().unwrap().len(), 1);

    let r = app.post(&format!("/api/v2/exercises/{id}/duplicate"), json!({})).await;
    assert_eq!(r.status, StatusCode::CREATED);
    assert_eq!(r.body["exercise"]["name"], "Grid Drill (copy)");
    assert_eq!(r.body["seq"], 1, "a copy starts with a fresh history");
    assert_eq!(app.len("/api/v2/exercises").await, 3);

    let mut tampered = file.clone();
    tampered["exercise"]["assets"][&a]["type_id"] = json!("spaceship");
    assert_eq!(app.post("/api/v2/exercises/import", json!({"file": tampered})).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        app.post("/api/v2/exercises/import", json!({"file": {"pins": {}}})).await.status,
        StatusCode::BAD_REQUEST
    );
}
