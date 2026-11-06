//! Edges: the connections drawn between two pins.

mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

async fn two_pins(app: &TestApp) -> (String, String) {
    let a = app.pin(json!({"name": "Substation"})).await;
    let b = app.pin(json!({"name": "Hospital"})).await;
    (a["id"].as_str().unwrap().into(), b["id"].as_str().unwrap().into())
}

async fn edge(app: &TestApp, a: &str, b: &str) -> Value {
    let r = app.post("/api/edges", json!({"from_pid": a, "to_pid": b})).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    r.body
}

#[tokio::test]
async fn links_two_pins_with_defaults() {
    let app = TestApp::spawn().await;
    let (a, b) = two_pins(&app).await;
    let e = edge(&app, &a, &b).await;
    assert!(e["id"].is_string());
    assert_eq!((e["from_pid"].as_str(), e["to_pid"].as_str()), (Some(&*a), Some(&*b)));
    assert_eq!(e["edge_type"], "connection");
    assert_eq!(e["label"], "");
}

#[tokio::test]
async fn keeps_label_and_type() {
    let app = TestApp::spawn().await;
    let (a, b) = two_pins(&app).await;
    let e = app
        .post("/api/edges", json!({"from_pid": a, "to_pid": b, "label": "primary feed", "edge_type": "dependency"}))
        .await
        .body;
    assert_eq!(e["label"], "primary feed");
    assert_eq!(e["edge_type"], "dependency");
}

#[tokio::test]
async fn rejects_missing_endpoints_self_loops_and_long_labels() {
    let app = TestApp::spawn().await;
    let (a, b) = two_pins(&app).await;
    for body in [
        json!({"from_pid": "not-a-real-pin", "to_pid": b}),
        json!({"from_pid": a, "to_pid": "not-a-real-pin"}),
        json!({"from_pid": a, "to_pid": a}),
    ] {
        assert_eq!(app.post("/api/edges", body).await.status, StatusCode::BAD_REQUEST);
    }
    let r = app.post("/api/edges", json!({"from_pid": a, "to_pid": b, "label": "x".repeat(201)})).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(app.len("/api/edges").await, 0);
}

#[tokio::test]
async fn delete() {
    let app = TestApp::spawn().await;
    let (a, b) = two_pins(&app).await;
    let e = edge(&app, &a, &b).await;
    let url = format!("/api/edges/{}", e["id"].as_str().unwrap());
    assert_eq!(app.delete(&url).await.status, StatusCode::OK);
    assert_eq!(app.len("/api/edges").await, 0);
    assert_eq!(app.delete(&url).await.status, StatusCode::NOT_FOUND);
    // Removing an edge leaves its pins alone.
    assert_eq!(app.len("/api/pins").await, 2);
}

#[tokio::test]
async fn clearing_pins_clears_edges() {
    let app = TestApp::spawn().await;
    let (a, b) = two_pins(&app).await;
    edge(&app, &a, &b).await;
    app.delete("/api/pins").await;
    assert_eq!(app.len("/api/edges").await, 0);
}

#[tokio::test]
async fn deleting_a_pin_removes_every_edge_touching_it_and_nothing_else() {
    let app = TestApp::spawn().await;
    let mut ids = vec![];
    for n in ["hub", "s1", "s2", "c", "d"] {
        ids.push(app.pin(json!({"name": n})).await["id"].as_str().unwrap().to_owned());
    }
    edge(&app, &ids[0], &ids[1]).await;
    edge(&app, &ids[2], &ids[0]).await; // the other direction
    let survivor = edge(&app, &ids[3], &ids[4]).await;

    app.delete(&format!("/api/pins/{}", ids[0])).await;
    let remaining = app.get("/api/edges").await;
    assert_eq!(remaining.as_array().unwrap().len(), 1);
    assert_eq!(remaining[0]["id"], survivor["id"]);
}

#[tokio::test]
async fn orphans_do_not_reach_a_saved_scenario() {
    let app = TestApp::spawn().await;
    let (a, b) = two_pins(&app).await;
    edge(&app, &a, &b).await;
    app.delete(&format!("/api/pins/{a}")).await;

    app.post("/api/scenarios/save", json!({"name": "after delete"})).await;
    app.post_empty("/api/state/clear").await;
    app.post("/api/scenarios/load", json!({"name": "after delete"})).await;

    assert_eq!(app.len("/api/edges").await, 0);
    assert_eq!(app.pins().await.keys().cloned().collect::<Vec<_>>(), [b]);
}
