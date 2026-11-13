//! Scripted exercises over the API: the clock, scheduled and conditional
//! injects firing on their own, undo, and players answering polls.

mod common;

use std::time::Duration;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

async fn exercise(app: &TestApp) -> String {
    app.post("/api/v2/exercises", json!({"name": "Drill"})).await.body["exercise"]["id"].as_str().unwrap().to_owned()
}

async fn cmd(app: &TestApp, id: &str, body: Value) -> Value {
    let r = app.post(&format!("/api/v2/exercises/{id}/commands"), body.clone()).await;
    assert_eq!(r.status, StatusCode::OK, "{body} → {}", r.body);
    r.body
}

async fn get(app: &TestApp, id: &str) -> Value {
    app.get(&format!("/api/v2/exercises/{id}")).await["exercise"].clone()
}

fn first(body: &Value, key: &str) -> String {
    body["events"][0]["event"][key]["id"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn a_scheduled_inject_fires_by_itself_when_the_clock_reaches_it() {
    let app = TestApp::spawn().await;
    let id = exercise(&app).await;
    // 0.02 minutes is 1.2 seconds of exercise time.
    let r = cmd(
        &app,
        &id,
        json!({"type": "add_inject", "inject": {"title": "Ransom note", "trigger": {"type": "at", "minutes": 0.02}}}),
    )
    .await;
    let iid = first(&r, "inject");
    cmd(&app, &id, json!({"type": "start_clock"})).await;

    let mut fired = false;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if get(&app, &id).await["injects"][&iid]["fired_at"].is_string() {
            fired = true;
            break;
        }
    }
    assert!(fired, "the ticker should have fired it within a few seconds");
    let events = app.get(&format!("/api/v2/exercises/{id}/events")).await;
    let fire = events.as_array().unwrap().iter().find(|e| e["event"]["type"] == "inject_fired").unwrap();
    assert_eq!(fire["actor"], "engine");
}

#[tokio::test]
async fn conditions_resolve_inside_the_request_that_meets_them() {
    let app = TestApp::spawn().await;
    let id = exercise(&app).await;
    let r =
        cmd(&app, &id, json!({"type": "add_assets", "assets": [{"name": "Grid", "type_id": "power_plants"}]})).await;
    let grid = r["events"][0]["event"]["assets"][0]["id"].as_str().unwrap().to_owned();
    let cond = json!({"type": "when", "condition": {"type": "asset_status", "asset": grid, "set": "security", "value": "compromised"}});
    let a = first(
        &cmd(&app, &id, json!({"type": "add_inject", "inject": {"title": "A", "trigger": cond}})).await,
        "inject",
    );
    let b = first(
        &cmd(&app, &id, json!({"type": "add_inject", "inject": {"title": "B", "trigger": {"type": "when", "condition": {"type": "inject_fired", "inject": a}}}})).await,
        "inject",
    );

    cmd(&app, &id, json!({"type": "set_status", "ids": [grid], "set": "security", "value": "compromised"})).await;
    // No waiting: by the time the request returns, A and then B have fired.
    let ex = get(&app, &id).await;
    assert!(ex["injects"][&a]["fired_at"].is_string());
    assert!(ex["injects"][&b]["fired_at"].is_string());
    assert_eq!(ex["log"].as_array().unwrap().len(), 2);

    // Undo takes back B's firing, and disarms it so it does not fire again.
    cmd(&app, &id, json!({"type": "undo"})).await;
    let ex = get(&app, &id).await;
    assert!(ex["injects"][&b]["fired_at"].is_null());
    assert_eq!(ex["injects"][&b]["armed"], false);
    cmd(&app, &id, json!({"type": "log", "text": "anything"})).await;
    assert!(get(&app, &id).await["injects"][&b]["fired_at"].is_null(), "still waiting, disarmed");
    // Re-arming it fires it straight away: its condition still holds.
    cmd(&app, &id, json!({"type": "arm_inject", "id": b, "armed": true})).await;
    assert!(get(&app, &id).await["injects"][&b]["fired_at"].is_string());
}

#[tokio::test]
async fn impact_spreads_along_dependencies() {
    let app = TestApp::spawn().await;
    let id = exercise(&app).await;
    let r = cmd(&app, &id, json!({"type": "add_assets", "assets": [{"name": "Substation", "type_id": "power_plants"}, {"name": "Hospital", "type_id": "hospitals"}]})).await;
    let assets = r["events"][0]["event"]["assets"].as_array().unwrap();
    let (sub, hosp) = (assets[0]["id"].as_str().unwrap().to_owned(), assets[1]["id"].as_str().unwrap().to_owned());
    cmd(&app, &id, json!({"type": "add_link", "from": sub, "to": hosp, "kind": "depends_on"})).await;
    cmd(
        &app,
        &id,
        json!({"type": "set_impact_rules", "rules": [{"id": "", "kind": "depends_on", "set": "operational", "when": ["offline"],
               "then_set": "operational", "then_value": "degraded"}]}),
    )
    .await;
    cmd(&app, &id, json!({"type": "set_status", "ids": [sub], "set": "operational", "value": "offline"})).await;
    let ex = get(&app, &id).await;
    assert_eq!(ex["assets"][&hosp]["statuses"]["operational"], "degraded");
    let impact = ex["log"].as_array().unwrap().last().unwrap().clone();
    assert!(impact["text"].as_str().unwrap().starts_with("IMPACT"));
    assert_eq!(impact["visibility"], json!({"mode": "facilitator"}));
    // One undo takes back the change and its knock-on together.
    cmd(&app, &id, json!({"type": "undo"})).await;
    assert_eq!(get(&app, &id).await["assets"][&hosp]["statuses"]["operational"], "healthy");
}

#[tokio::test]
async fn players_answer_polls_and_the_answer_can_branch_the_script() {
    let app = TestApp::spawn().await;
    let id = exercise(&app).await;
    let team = |r: Value| r["events"][0]["event"]["team"]["id"].as_str().unwrap().to_owned();
    let it = team(cmd(&app, &id, json!({"type": "add_team", "name": "IT"})).await);
    let legal = team(cmd(&app, &id, json!({"type": "add_team", "name": "Legal"})).await);
    let r = cmd(
        &app,
        &id,
        json!({"type": "add_poll", "poll": {"question": "Pay?", "options": ["Pay", "Refuse"], "allow_text": true}}),
    )
    .await;
    let poll = r["events"][0]["event"]["poll"].clone();
    let (pid, pay, refuse) = (
        poll["id"].as_str().unwrap().to_owned(),
        poll["options"][0]["id"].as_str().unwrap().to_owned(),
        poll["options"][1]["id"].as_str().unwrap().to_owned(),
    );
    let branch = |opt: &str, title: &str| json!({"type": "add_inject", "inject": {"title": title, "trigger": {"type": "when", "condition": {"type": "poll_choice", "poll": pid, "option": opt}}}});
    let paid = first(&cmd(&app, &id, branch(&pay, "They paid")).await, "inject");
    let refused = first(&cmd(&app, &id, branch(&refuse, "They refused")).await, "inject");

    let room = app.post("/api/v2/room/open", json!({"exercise_id": id})).await.body;
    let code = room["player_code"].as_str().unwrap();
    let join = |name: &'static str, team: String| {
        let app = &app;
        async move {
            let r = app
                .http
                .post(format!("{}/api/v2/join", app.base))
                .json(&json!({"code": code, "name": name, "team": team}))
                .send()
                .await
                .unwrap();
            r.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap().to_owned()
        }
    };
    let (dana, lee) = (join("Dana", it.clone()).await, join("Lee", legal.clone()).await);
    let as_player = |cookie: &str, body: Value| {
        app.http
            .post(format!("{}/api/v2/exercises/{id}/commands", app.base))
            .header("Cookie", cookie)
            .json(&body)
            .send()
    };
    let view = |cookie: String| {
        let app = &app;
        let id = id.clone();
        async move {
            let r = app
                .http
                .get(format!("{}/api/v2/exercises/{id}", app.base))
                .header("Cookie", cookie)
                .send()
                .await
                .unwrap();
            r.json::<Value>().await.unwrap()["exercise"].clone()
        }
    };

    // A draft poll is invisible and cannot be answered.
    assert_eq!(view(dana.clone()).await["polls"], json!({}));
    let r = as_player(&dana, json!({"type": "respond", "poll": pid, "option": pay})).await.unwrap();
    assert_eq!(r.status(), StatusCode::CONFLICT);

    cmd(&app, &id, json!({"type": "open_poll", "id": pid})).await;
    for (cookie, option, text) in [(&dana, &refuse, ""), (&lee, &refuse, "never"), (&dana, &pay, "changed my mind")] {
        let r =
            as_player(cookie, json!({"type": "respond", "poll": pid, "option": option, "text": text})).await.unwrap();
        assert_eq!(r.status(), StatusCode::OK);
    }
    // Players cannot set who they are answering as.
    let r = as_player(&dana, json!({"type": "respond", "poll": pid, "option": pay, "by": {"participant": "x"}}))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let facilitator = get(&app, &id).await;
    let responses = facilitator["polls"][&pid]["responses"].as_array().unwrap();
    assert_eq!(responses.len(), 2, "one answer each, the latest counts");
    assert!(responses.iter().all(|r| r["name"] == "Dana" || r["name"] == "Lee"));

    // Each team sees its own answers, and no totals while the poll is open.
    let seen = view(dana.clone()).await["polls"][&pid].clone();
    assert_eq!(seen["responses"].as_array().unwrap().len(), 1);
    assert_eq!(seen["responses"][0]["name"], "Dana");
    assert_eq!(seen["results"], json!({}));

    // A tie: both branches' conditions hold, and both fire.
    cmd(&app, &id, json!({"type": "close_poll", "id": pid})).await;
    let ex = get(&app, &id).await;
    assert!(ex["injects"][&paid]["fired_at"].is_string());
    assert!(ex["injects"][&refused]["fired_at"].is_string());
    assert_eq!(view(lee).await["polls"][&pid]["results"], json!({pay.as_str(): 1, refuse.as_str(): 1}));
    // The facilitator cannot answer.
    let r = app
        .post(&format!("/api/v2/exercises/{id}/commands"), json!({"type": "respond", "poll": pid, "option": pay}))
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn players_see_the_clock_and_phases_but_not_how_the_script_works() {
    let app = TestApp::spawn().await;
    let id = exercise(&app).await;
    cmd(&app, &id, json!({"type": "set_phases", "phases": [{"id": "", "name": "Detection", "color": "#3399ff", "target_minutes": 20}]})).await;
    let phase = get(&app, &id).await["phases"][0]["id"].as_str().unwrap().to_owned();
    cmd(
        &app,
        &id,
        json!({"type": "add_inject", "inject": {"title": "Secret", "trigger": {"type": "at", "minutes": 999}}}),
    )
    .await;
    cmd(&app, &id, json!({"type": "enter_phase", "id": phase})).await;
    cmd(&app, &id, json!({"type": "start_clock"})).await;
    cmd(&app, &id, json!({"type": "set_impact_rules", "rules": [{"id": "", "kind": "depends_on", "set": "operational", "when": ["offline"], "then_set": "operational", "then_value": "degraded"}]})).await;

    let room = app.post("/api/v2/room/open", json!({"exercise_id": id})).await.body;
    let r = app
        .http
        .post(format!("{}/api/v2/join", app.base))
        .json(&json!({"code": room["observer_code"], "name": "Screen"}))
        .send()
        .await
        .unwrap();
    let cookie = r.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap().to_owned();
    let seen: Value = app
        .http
        .get(format!("{}/api/v2/exercises/{id}", app.base))
        .header("Cookie", cookie)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ex = &seen["exercise"];
    assert!(ex["clock"]["running_since"].is_string());
    assert_eq!(ex["phases"][0]["name"], "Detection");
    assert_eq!(ex["phase_log"][0]["phase"], phase.as_str());
    assert_eq!(ex["impact_rules"], json!([]));
    assert_eq!(ex["injects"], json!({}));
    assert!(!seen.to_string().contains("Secret"));
}

#[tokio::test]
async fn script_commands_are_validated() {
    let app = TestApp::spawn().await;
    let id = exercise(&app).await;
    for body in [
        json!({"type": "add_inject", "inject": {"title": "x", "trigger": {"type": "phase", "phase": "nope", "delay_minutes": 0}}}),
        json!({"type": "add_inject", "inject": {"title": "x", "trigger": {"type": "at", "minutes": -1}}}),
        json!({"type": "add_inject", "inject": {"title": "x", "trigger": {"type": "when", "condition": {"type": "inject_fired", "inject": "nope"}}}}),
        json!({"type": "add_inject", "inject": {"title": "x", "effects": [{"type": "open_poll", "poll": "nope"}]}}),
        json!({"type": "add_poll", "poll": {"question": "?", "options": []}}),
        json!({"type": "add_poll", "poll": {"question": "?", "options": ["a"], "duration_minutes": 0}}),
        json!({"type": "set_phases", "phases": [{"id": "", "name": " ", "color": "#3399ff", "target_minutes": 10}]}),
        json!({"type": "set_clock", "minutes": -5}),
    ] {
        let r = app.post(&format!("/api/v2/exercises/{id}/commands"), body.clone()).await;
        assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    }
    // A phase an inject depends on cannot be removed.
    cmd(&app, &id, json!({"type": "set_phases", "phases": [{"id": "p1", "name": "One", "color": "#3399ff", "target_minutes": 10}]})).await;
    cmd(&app, &id, json!({"type": "add_inject", "inject": {"title": "x", "trigger": {"type": "phase", "phase": "p1", "delay_minutes": 1}}})).await;
    let r = app.post(&format!("/api/v2/exercises/{id}/commands"), json!({"type": "set_phases", "phases": []})).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
}
