//! Templates, branding, graph positions and custom fields.

mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

async fn cmd(app: &TestApp, id: &str, body: Value) -> common::Res {
    app.post(&format!("/api/v2/exercises/{id}/commands"), body).await
}

#[tokio::test]
async fn every_template_starts_a_working_exercise() {
    let app = TestApp::spawn().await;
    let list = app.get("/api/v2/templates").await;
    let ids: Vec<&str> = list.as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["ransomware", "bec", "supply-chain", "utility-outage"]);
    for t in list.as_array().unwrap() {
        assert!(t["assets"].as_u64().unwrap() >= 8 && t["minutes"].as_u64().unwrap() > 0, "{t}");
        let r = app.post(&format!("/api/v2/templates/{}", t["id"].as_str().unwrap()), json!({})).await;
        assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
        let ex = &r.body["exercise"];
        assert_eq!(ex["name"], t["name"]);
        assert_eq!(r.body["seq"], 1);
        // Each comes ready to run: start the clock and walk the phases.
        let id = ex["id"].as_str().unwrap();
        assert_eq!(cmd(&app, id, json!({"type": "start_clock"})).await.status, StatusCode::OK);
        for p in ex["phases"].as_array().unwrap() {
            let r = cmd(&app, id, json!({"type": "enter_phase", "id": p["id"]})).await;
            assert_eq!(r.status, StatusCode::OK, "{}", r.body);
        }
        // Jump past the end: every timed and phase injects must have fired,
        // by the engine, without anything else being done.
        assert_eq!(cmd(&app, id, json!({"type": "set_clock", "minutes": 500})).await.status, StatusCode::OK);
        let injects = app.get(&format!("/api/v2/exercises/{id}")).await["exercise"]["injects"].clone();
        for i in injects.as_object().unwrap().values() {
            let timed = matches!(i["trigger"]["type"].as_str(), Some("at" | "phase"));
            if timed {
                assert!(i["fired_at"].is_string(), "{}: {} did not fire", t["id"], i["title"]);
            }
        }
    }
    let named = app.post("/api/v2/templates/bec", json!({"name": "BEC drill, March"})).await;
    assert_eq!(named.body["exercise"]["name"], "BEC drill, March");
    assert_eq!(app.post("/api/v2/templates/nope", json!({})).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn branding_is_validated_shared_and_undoable() {
    let app = TestApp::spawn().await;
    let id = app.post("/api/v2/exercises", json!({"name": "Drill"})).await.body["exercise"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let png = "data:image/png;base64,iVBORw0KGgo=";
    let branding = json!({"theme": "daylight", "accent": "#0055ff", "org": " Contoso ", "logo": png,
                          "banner": {"text": "EXERCISE", "color": "#b8860b", "position": "both"}});
    let r = cmd(&app, &id, json!({"type": "set_branding", "branding": branding})).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let b = &app.get(&format!("/api/v2/exercises/{id}")).await["exercise"]["branding"];
    assert_eq!((b["theme"].as_str(), b["org"].as_str()), (Some("daylight"), Some("Contoso")));

    for bad in [
        json!({"logo": "data:text/html;base64,PHNjcmlwdD4="}),
        json!({"logo": format!("data:image/png;base64,{}", "A".repeat(400_000))}),
        json!({"accent": "blue"}),
        json!({"banner": {"text": "  ", "color": "#000000"}}),
        json!({"theme": "neon"}),
    ] {
        let r = cmd(&app, &id, json!({"type": "set_branding", "branding": bad.clone()})).await;
        assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{bad}");
    }
    cmd(&app, &id, json!({"type": "undo"})).await;
    assert_eq!(app.get(&format!("/api/v2/exercises/{id}")).await["exercise"]["branding"]["theme"], "terminal");
}

#[tokio::test]
async fn graph_positions_save_in_one_change() {
    let app = TestApp::spawn().await;
    let id = app.post("/api/v2/exercises", json!({"name": "Drill"})).await.body["exercise"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let r = cmd(
        &app,
        &id,
        json!({"type": "add_assets", "assets": [{"name": "a", "type_id": "asset"}, {"name": "b", "type_id": "asset"}]}),
    )
    .await;
    let ids: Vec<String> = r.body["events"][0]["event"]["assets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap().to_owned())
        .collect();
    let r = cmd(&app, &id, json!({"type": "set_graph_positions", "positions": {ids[0].clone(): {"x": 10, "y": 20}, ids[1].clone(): {"x": -5.5, "y": 0}}})).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["events"][0]["event"]["assets"].as_array().unwrap().len(), 2);
    // Unchanged positions are not a change.
    let again =
        cmd(&app, &id, json!({"type": "set_graph_positions", "positions": {ids[0].clone(): {"x": 10, "y": 20}}})).await;
    assert_eq!(again.body["events"], json!([]));
    let bad = cmd(&app, &id, json!({"type": "set_graph_positions", "positions": {"nope": {"x": 1, "y": 1}}})).await;
    assert_eq!(bad.status, StatusCode::NOT_FOUND);
    // One undo puts the whole layout back.
    cmd(&app, &id, json!({"type": "undo"})).await;
    let ex = app.get(&format!("/api/v2/exercises/{id}")).await["exercise"].clone();
    assert!(ex["assets"][&ids[0]]["graph"].is_null() && ex["assets"][&ids[1]]["graph"].is_null());
}

#[tokio::test]
async fn custom_fields_are_typed_and_follow_the_taxonomy() {
    let app = TestApp::spawn().await;
    let created = app.post("/api/v2/exercises", json!({"name": "Drill"})).await.body;
    let id = created["exercise"]["id"].as_str().unwrap().to_owned();
    let mut tax = created["exercise"]["taxonomy"].clone();
    tax["fields"] = json!([
        {"id": "owner", "label": "Owner", "kind": {"type": "text"}, "player_visible": true},
        {"id": "tier", "label": "Tier", "kind": {"type": "select", "options": ["1", "2"]}},
        {"id": "rto", "label": "RTO", "kind": {"type": "number"}},
    ]);
    assert_eq!(cmd(&app, &id, json!({"type": "set_taxonomy", "taxonomy": tax})).await.status, StatusCode::OK);
    let a = cmd(&app, &id, json!({"type": "add_assets", "assets": [{"name": "a", "type_id": "asset"}]})).await.body["events"][0]["event"]["assets"][0]["id"].clone();
    let edit = |fields: Value| cmd(&app, &id, json!({"type": "edit_asset", "id": a, "patch": {"fields": fields}}));
    assert_eq!(edit(json!({"owner": "IT", "tier": "2", "rto": 4})).await.status, StatusCode::OK);
    for bad in [json!({"tier": "3"}), json!({"rto": "four"}), json!({"nope": 1})] {
        assert_eq!(edit(bad.clone()).await.status, StatusCode::UNPROCESSABLE_ENTITY, "{bad}");
    }
    // Dropping option "2" and the RTO field drops those values from the asset.
    tax["fields"] = json!([
        {"id": "owner", "label": "Owner", "kind": {"type": "text"}, "player_visible": true},
        {"id": "tier", "label": "Tier", "kind": {"type": "select", "options": ["1"]}},
    ]);
    let r = cmd(&app, &id, json!({"type": "set_taxonomy", "taxonomy": tax})).await;
    assert_eq!(r.body["events"][0]["event"]["assets"][0]["fields"], json!({"owner": "IT"}));
}
