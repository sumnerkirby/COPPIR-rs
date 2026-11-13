//! After the exercise: expected actions, the hot wash, and the report.

mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

async fn cmd(app: &TestApp, id: &str, body: Value) -> common::Res {
    app.post(&format!("/api/v2/exercises/{id}/commands"), body).await
}

async fn exercise(app: &TestApp, id: &str) -> Value {
    app.get(&format!("/api/v2/exercises/{id}")).await["exercise"].clone()
}

async fn text(app: &TestApp, path: &str) -> (StatusCode, String, reqwest::header::HeaderMap) {
    let r = app.http.get(format!("{}{path}", app.base)).send().await.unwrap();
    let (status, headers) = (r.status(), r.headers().clone());
    (status, r.text().await.unwrap(), headers)
}

/// An exercise with two teams and one inject that expects two actions.
async fn drill(app: &TestApp) -> (String, String, String, String) {
    let id = app.post("/api/v2/exercises", json!({"name": "Drill <b>"})).await.body["exercise"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let team = |r: common::Res| r.body["events"][0]["event"]["team"]["id"].as_str().unwrap().to_owned();
    let it = team(cmd(app, &id, json!({"type": "add_team", "name": "IT"})).await);
    let legal = team(cmd(app, &id, json!({"type": "add_team", "name": "Legal"})).await);
    let r = cmd(
        app,
        &id,
        json!({"type": "add_inject", "inject": {"title": "Ransom note", "expected": [
            {"text": "Isolate the file server", "team": it},
            {"text": "Brief counsel", "team": legal},
        ]}}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let inject = r.body["events"][0]["event"]["inject"]["id"].as_str().unwrap().to_owned();
    (id, inject, it, legal)
}

#[tokio::test]
async fn expected_actions_are_assessed_and_keep_their_verdicts() {
    let app = TestApp::spawn().await;
    let (id, inject, it, legal) = drill(&app).await;
    let ex = exercise(&app, &id).await;
    let actions = ex["injects"][&inject]["expected"].as_array().unwrap().clone();
    assert_eq!(actions.len(), 2);
    let (isolate, brief) = (actions[0]["id"].clone(), actions[1]["id"].clone());

    let assess = |action: Value, verdict: Value| {
        cmd(
            &app,
            &id,
            json!({"type": "assess", "inject": inject, "action": action, "verdict": verdict, "note": " slow "}),
        )
    };
    assert_eq!(assess(isolate.clone(), json!("met")).await.status, StatusCode::OK);
    assert_eq!(assess(json!("nope"), json!("met")).await.status, StatusCode::NOT_FOUND);
    assert_eq!(assess(isolate.clone(), json!("great")).await.status, StatusCode::UNPROCESSABLE_ENTITY);
    // The same verdict again is not a change.
    assert_eq!(assess(isolate.clone(), json!("met")).await.body["events"], json!([]));

    // Editing the list keeps the assessed action by id, drops one, adds one.
    let r = cmd(
        &app,
        &id,
        json!({"type": "set_expected", "inject": inject, "expected": [
            {"id": isolate, "text": "Isolate the file servers", "team": it},
            {"text": "Notify the regulator"},
        ]}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let list = exercise(&app, &id).await["injects"][&inject]["expected"].clone();
    assert_eq!(list[0]["verdict"], "met");
    assert_eq!(list[0]["note"], "slow");
    assert_eq!(list[0]["text"], "Isolate the file servers");
    assert!(list[1]["verdict"].is_null() && list[1]["team"].is_null() && list[1]["id"] != brief);

    // Unknown teams and empty text are refused.
    for bad in [json!([{"text": "x", "team": "nope"}]), json!([{"text": "  "}])] {
        let r = cmd(&app, &id, json!({"type": "set_expected", "inject": inject, "expected": bad})).await;
        assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{bad}");
    }
    // An edit that leaves `expected` out keeps the list.
    let r = cmd(&app, &id, json!({"type": "edit_inject", "id": inject, "inject": {"title": "Ransom note v2"}})).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(exercise(&app, &id).await["injects"][&inject]["expected"], list);

    // A fired inject can still be assessed.
    cmd(&app, &id, json!({"type": "fire_inject", "id": inject})).await;
    assert_eq!(assess(list[1]["id"].clone(), json!("missed")).await.status, StatusCode::OK);
    // Removing a team leaves its actions for anyone.
    cmd(&app, &id, json!({"type": "remove_team", "id": it})).await;
    assert!(exercise(&app, &id).await["injects"][&inject]["expected"][0]["team"].is_null());
    let _ = legal;
}

#[tokio::test]
async fn the_review_is_saved_and_undoable() {
    let app = TestApp::spawn().await;
    let (id, ..) = drill(&app).await;
    let review = json!({"summary": "Went well.", "strengths": "Fast isolation", "improvements": "Call legal sooner"});
    assert_eq!(cmd(&app, &id, json!({"type": "set_review", "review": review})).await.status, StatusCode::OK);
    assert_eq!(exercise(&app, &id).await["review"], review);
    let long = json!({"summary": "x".repeat(20_001)});
    assert_eq!(
        cmd(&app, &id, json!({"type": "set_review", "review": long})).await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    cmd(&app, &id, json!({"type": "undo"})).await;
    assert_eq!(exercise(&app, &id).await["review"]["summary"], "");
}

#[tokio::test]
async fn the_report_tells_the_story_in_markdown_and_html() {
    let app = TestApp::spawn().await;
    let (id, inject, ..) = drill(&app).await;
    let ex = exercise(&app, &id).await;
    let isolate = ex["injects"][&inject]["expected"][0]["id"].clone();
    cmd(&app, &id, json!({"type": "start_clock"})).await;
    cmd(&app, &id, json!({"type": "fire_inject", "id": inject})).await;
    cmd(&app, &id, json!({"type": "assess", "inject": inject, "action": isolate, "verdict": "partial"})).await;
    cmd(&app, &id, json!({"type": "log", "text": "<script>alert(1)</script> | pipes"})).await;
    let poll =
        cmd(&app, &id, json!({"type": "add_poll", "poll": {"question": "Pay?", "options": ["Yes", "No"]}})).await;
    let poll = poll.body["events"][0]["event"]["poll"]["id"].clone();
    cmd(&app, &id, json!({"type": "open_poll", "id": poll})).await;
    cmd(&app, &id, json!({"type": "close_poll", "id": poll})).await;
    cmd(&app, &id, json!({"type": "set_review", "review": {"strengths": "Calm under pressure"}})).await;
    // One asset changes, one does not; one inject is fired by the script.
    let r = cmd(&app, &id, json!({"type": "add_assets", "assets": [{"name": "Mail server", "type_id": "asset"}, {"name": "Quiet box", "type_id": "asset"}]})).await;
    let mail = r.body["events"][0]["event"]["assets"][0]["id"].clone();
    cmd(&app, &id, json!({"type": "set_status", "ids": [mail], "set": "security", "value": "compromised"})).await;
    cmd(
        &app,
        &id,
        json!({"type": "add_inject", "inject": {"title": "Scheduled", "trigger": {"type": "at", "minutes": 0}}}),
    )
    .await;

    let path = format!("/api/v2/exercises/{id}/report");
    let (status, md, headers) = text(&app, &format!("{path}?format=md&download=true")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers["content-type"].to_str().unwrap().starts_with("text/markdown"));
    assert!(headers["content-disposition"].to_str().unwrap().starts_with("attachment; filename=\"Drill _b_ AAR.md\""));
    for want in [
        "# Drill <b>: after-action report",
        "## Strengths",
        "Calm under pressure",
        "## Expected actions",
        "| All | 0 | 1 | 0 | 1 | 50% |",
        "| Ransom note | T+0:00 | Isolate the file server | IT | Partly met |",
        "## Decisions",
        "### Pay?",
        "\\| pipes",
        "## Timeline",
        "INJECT FIRED: Ransom note",
        "| Script | INJECT FIRED: Scheduled |",
        "| Mail server | 1 | Security: Compromised |",
        "- **Decisions:** 1 of 1 put to the room",
    ] {
        assert!(md.contains(want), "missing {want:?} in\n{md}");
    }
    assert!(!md.contains("Quiet box"), "an asset that never changed is listed");

    let (status, html, headers) = text(&app, &path).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers["content-disposition"].to_str().unwrap().starts_with("inline"));
    assert!(headers["content-security-policy"].to_str().unwrap().contains("default-src 'none'"));
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;") && !html.contains("<script>"));
    assert!(html.contains("<td class=\"partial\">Partly met</td>"));
    assert!(html.contains("<title>Drill &lt;b&gt;: after-action report</title>"));

    assert_eq!(
        app.call(reqwest::Method::GET, &format!("{path}?format=pdf"), None).await.status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        app.call(reqwest::Method::GET, "/api/v2/exercises/nope/report", None).await.status,
        StatusCode::NOT_FOUND
    );
}
