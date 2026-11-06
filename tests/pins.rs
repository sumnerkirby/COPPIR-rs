//! Pin CRUD, validation, and status history.

mod common;

use common::{TestApp, statuses};
use reqwest::StatusCode;
use serde_json::json;

#[tokio::test]
async fn create_returns_201_with_generated_fields() {
    let app = TestApp::spawn().await;
    let r = app
        .post(
            "/api/pins",
            json!({"name": "Rosslyn Substation", "category": "Power Plants", "lat": 38.8968, "lon": -77.0719}),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED);
    assert!(r.body["id"].as_str().is_some_and(|s| !s.is_empty()));
    assert!(r.body["pinned_at"].as_str().is_some_and(|s| !s.is_empty()));
    assert_eq!(r.body["name"], "Rosslyn Substation");
}

#[tokio::test]
async fn defaults_to_clean_and_healthy() {
    let app = TestApp::spawn().await;
    let pin = app.pin(json!({})).await;
    assert_eq!(pin["status"], "Clean");
    assert_eq!(pin["op_status"], "Healthy");
}

#[tokio::test]
async fn seeds_history_with_initial_status() {
    let app = TestApp::spawn().await;
    let pin = app.pin(json!({"status": "Compromised"})).await;
    assert_eq!(statuses(&pin["status_history"]), ["Compromised"]);
    assert!(pin["status_history"][0]["timestamp"].is_string());
}

#[tokio::test]
async fn collection_is_keyed_by_id_in_insertion_order() {
    let app = TestApp::spawn().await;
    let mut ids = vec![];
    for n in ["c", "a", "b"] {
        ids.push(app.pin(json!({"name": n})).await["id"].as_str().unwrap().to_owned());
    }
    let keys: Vec<_> = app.pins().await.keys().cloned().collect();
    assert_eq!(keys, ids);
}

#[tokio::test]
async fn rejects_bad_fields() {
    let app = TestApp::spawn().await;
    let cases = [
        ("status", json!("Melted")),
        ("op_status", json!("Vibing")),
        ("pin_color", json!("red")),
        ("pin_color", json!("#ff0")),
        ("lat", json!(91)),
        ("lat", json!(-91)),
        ("lon", json!(181)),
        ("lon", json!(-181)),
        ("lat", json!("north")),
        ("name", json!("")),
        ("name", json!("x".repeat(201))),
        ("notes", json!("x".repeat(2001))),
    ];
    for (field, value) in cases {
        let mut body = json!({"name": "x", "lat": 0.0, "lon": 0.0});
        body[field] = value.clone();
        let r = app.post("/api/pins", body).await;
        assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{field}={value}");
        assert_eq!(r.body["detail"][0]["loc"][1], field, "{}", r.body);
    }
    assert!(app.pins().await.is_empty());
}

#[tokio::test]
async fn accepts_boundary_coordinates() {
    let app = TestApp::spawn().await;
    for (lat, lon) in [(90, 180), (-90, -180), (0, 0)] {
        let r = app.post("/api/pins", json!({"name": "edge", "lat": lat, "lon": lon})).await;
        assert_eq!(r.status, StatusCode::CREATED, "{lat},{lon}");
    }
}

#[tokio::test]
async fn rejects_a_body_that_is_not_json() {
    let app = TestApp::spawn().await;
    let r = app.http.post(format!("{}/api/pins", app.base)).body("name=x").send().await.unwrap();
    assert_eq!(r.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let r = app
        .http
        .post(format!("{}/api/pins", app.base))
        .header("content-type", "application/json")
        .body("{not json")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn partial_update_leaves_other_fields_alone() {
    let app = TestApp::spawn().await;
    let pin = app.pin(json!({"name": "Original", "notes": "keep me"})).await;
    let r = app.put(&format!("/api/pins/{}", pin["id"].as_str().unwrap()), json!({"status": "Contained"})).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["status"], "Contained");
    assert_eq!(r.body["name"], "Original");
    assert_eq!(r.body["notes"], "keep me");
}

#[tokio::test]
async fn status_changes_append_history_but_rewrites_do_not() {
    let app = TestApp::spawn().await;
    let pin = app.pin(json!({"status": "Clean"})).await;
    let url = format!("/api/pins/{}", pin["id"].as_str().unwrap());
    app.put(&url, json!({"status": "Monitored"})).await;
    app.put(&url, json!({"status": "Monitored"})).await;
    let r = app.put(&url, json!({"status": "Compromised"})).await;
    assert_eq!(statuses(&r.body["status_history"]), ["Clean", "Monitored", "Compromised"]);

    // op_status is not part of the security history.
    let r = app.put(&url, json!({"op_status": "Critical"})).await;
    assert_eq!(r.body["op_status"], "Critical");
    assert_eq!(r.body["status_history"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn update_unknown_pin_is_404_and_bad_status_is_422() {
    let app = TestApp::spawn().await;
    assert_eq!(app.put("/api/pins/nope", json!({"status": "Clean"})).await.status, StatusCode::NOT_FOUND);
    let pin = app.pin(json!({})).await;
    let r = app.put(&format!("/api/pins/{}", pin["id"].as_str().unwrap()), json!({"status": "Toast"})).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn delete_and_clear() {
    let app = TestApp::spawn().await;
    let pin = app.pin(json!({})).await;
    let url = format!("/api/pins/{}", pin["id"].as_str().unwrap());
    assert_eq!(app.delete(&url).await.status, StatusCode::OK);
    assert!(app.pins().await.is_empty());
    assert_eq!(app.delete(&url).await.status, StatusCode::NOT_FOUND);

    for i in 0..3 {
        app.pin(json!({"name": format!("pin {i}")})).await;
    }
    let r = app.delete("/api/pins").await;
    assert_eq!(r.body, json!({"ok": true, "count": 3}));
    assert!(app.pins().await.is_empty());
}

#[tokio::test]
async fn bulk_create_applies_shared_status() {
    let app = TestApp::spawn().await;
    let r = app
        .post(
            "/api/pins/bulk",
            json!({
                "items": [
                    {"name": "A", "category": "Hospitals", "lat": 1.0, "lon": 1.0},
                    {"name": "B", "category": "Hospitals", "lat": 2.0, "lon": 2.0},
                ],
                "status": "Monitored", "op_status": "Degraded", "pin_color": "#112233",
            }),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED);
    let created = r.body.as_array().unwrap();
    assert_eq!(created.len(), 2);
    for p in created {
        assert_eq!((p["status"].as_str(), p["op_status"].as_str()), (Some("Monitored"), Some("Degraded")));
        assert_eq!(p["pin_color"], "#112233");
    }
}

#[tokio::test]
async fn bulk_create_empty_and_invalid() {
    let app = TestApp::spawn().await;
    let r = app.post("/api/pins/bulk", json!({"items": []})).await;
    assert_eq!((r.status, r.body.clone()), (StatusCode::CREATED, json!([])));

    // One bad item rejects the whole batch.
    let r = app
        .post(
            "/api/pins/bulk",
            json!({"items": [{"name": "fine", "lat": 1.0, "lon": 1.0}, {"name": "  ", "lat": 2.0, "lon": 2.0}]}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(r.body["detail"][0]["loc"], json!(["body", "items", "1", "name"]));
    assert!(app.pins().await.is_empty());
}

#[tokio::test]
async fn bulk_status_by_category() {
    let app = TestApp::spawn().await;
    let hospital = app.pin(json!({"name": "H", "category": "Hospitals", "status": "Clean"})).await;
    let bank = app.pin(json!({"name": "B", "category": "Banks"})).await;

    let r = app.post("/api/pins/bulk-status", json!({"category": "Hospitals", "status": "Compromised"})).await;
    assert_eq!(r.body["count"], 1);
    let pins = app.pins().await;
    let h = &pins[hospital["id"].as_str().unwrap()];
    assert_eq!(h["status"], "Compromised");
    assert_eq!(statuses(&h["status_history"]), ["Clean", "Compromised"]);
    assert_eq!(pins[bank["id"].as_str().unwrap()]["status"], "Clean");

    let r = app.post("/api/pins/bulk-status", json!({"op_status": "Offline"})).await;
    assert_eq!(r.body["count"], 2);
    assert!(app.pins().await.values().all(|p| p["op_status"] == "Offline"));
}

#[tokio::test]
async fn bulk_status_needs_something_to_set() {
    let app = TestApp::spawn().await;
    let r = app.post("/api/pins/bulk-status", json!({"category": "Banks"})).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn blank_names_and_categories_are_rejected() {
    let app = TestApp::spawn().await;
    for v in ["", "   ", "\t", "\n  "] {
        let r = app.post("/api/pins", json!({"name": v, "lat": 1.0, "lon": 1.0})).await;
        assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{v:?}");
        let r = app.post("/api/pins", json!({"name": "ok", "category": v, "lat": 1.0, "lon": 1.0})).await;
        assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{v:?}");
    }

    let pin = app.pin(json!({"name": "Rosslyn Substation", "category": "Power Plants"})).await;
    let url = format!("/api/pins/{}", pin["id"].as_str().unwrap());
    for field in ["name", "category"] {
        for v in ["", "   "] {
            assert_eq!(app.put(&url, json!({ field: v })).await.status, StatusCode::UNPROCESSABLE_ENTITY);
        }
    }
    let stored = &app.pins().await[pin["id"].as_str().unwrap()];
    assert_eq!(
        (stored["name"].as_str(), stored["category"].as_str()),
        (Some("Rosslyn Substation"), Some("Power Plants"))
    );
}

#[tokio::test]
async fn surrounding_whitespace_is_trimmed() {
    let app = TestApp::spawn().await;
    let pin = app.pin(json!({"name": "  Rosslyn Substation  ", "category": " Power Plants "})).await;
    assert_eq!(pin["name"], "Rosslyn Substation");
    assert_eq!(pin["category"], "Power Plants");
}
