//! Room access: joining, roles, what each team sees, and the LAN listener.

mod common;

use common::{TestApp, Ws};
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, http::HeaderValue};

struct Room {
    app: TestApp,
    id: String,
    player_code: String,
    observer_code: String,
    it: String,
    legal: String,
    urls: Vec<String>,
}

async fn cmd(app: &TestApp, id: &str, body: Value) -> Value {
    let r = app.post(&format!("/api/v2/exercises/{id}/commands"), body).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    r.body
}

/// An open room with two teams.
async fn room() -> Room {
    let app = TestApp::spawn().await;
    let id = app.post("/api/v2/exercises", json!({"name": "Drill"})).await.body["exercise"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let team = |r: Value| r["events"][0]["event"]["team"]["id"].as_str().unwrap().to_owned();
    let it = team(cmd(&app, &id, json!({"type": "add_team", "name": "IT"})).await);
    let legal = team(cmd(&app, &id, json!({"type": "add_team", "name": "Legal"})).await);
    let r = app.post("/api/v2/room/open", json!({"exercise_id": id})).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    Room {
        player_code: r.body["player_code"].as_str().unwrap().to_owned(),
        observer_code: r.body["observer_code"].as_str().unwrap().to_owned(),
        urls: r.body["urls"].as_array().unwrap().iter().map(|u| u.as_str().unwrap().to_owned()).collect(),
        app,
        id,
        it,
        legal,
    }
}

/// A joined participant: their cookie, sent by hand on each request.
struct Member {
    cookie: String,
}

impl Member {
    async fn join(room: &Room, code: &str, name: &str, team: Option<&str>) -> Self {
        let r = room
            .app
            .http
            .post(format!("{}/api/v2/join", room.app.base))
            .json(&json!({"code": code, "name": name, "team": team}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK, "{}", r.text().await.unwrap());
        let set = r.headers()["set-cookie"].to_str().unwrap().to_owned();
        assert!(set.contains("HttpOnly") && set.contains("SameSite=Strict"), "{set}");
        Self { cookie: set.split(';').next().unwrap().to_owned() }
    }

    async fn call(&self, app: &TestApp, method: Method, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut req = app.http.request(method, format!("{}{path}", app.base)).header("Cookie", &self.cookie);
        if let Some(b) = body {
            req = req.json(&b);
        }
        let r = req.send().await.unwrap();
        let status = r.status();
        (status, r.json().await.unwrap_or(Value::Null))
    }

    async fn get(&self, app: &TestApp, path: &str) -> Value {
        let (status, body) = self.call(app, Method::GET, path, None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    async fn ws(&self, room: &Room) -> Ws {
        let mut req = format!("ws://{}/api/v2/exercises/{}/ws", room.app.addr, room.id).into_client_request().unwrap();
        req.headers_mut().insert("Cookie", HeaderValue::from_str(&self.cookie).unwrap());
        Ws(tokio_tungstenite::connect_async(req).await.unwrap().0)
    }
}

fn asset_names(snapshot: &Value) -> Vec<String> {
    snapshot["exercise"]["assets"]
        .as_object()
        .unwrap()
        .values()
        .map(|a| a["name"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn this_machine_is_the_facilitator() {
    let app = TestApp::spawn().await;
    let me = app.get("/api/v2/me").await;
    assert_eq!(me["role"], "facilitator");
    assert!(app.get("/api/v2/room").await["open"].is_null());
}

#[tokio::test]
async fn joining_needs_a_valid_code_and_a_name() {
    let room = room().await;
    let app = &room.app;
    let info = app.get(&format!("/api/v2/join?code={}", room.player_code.to_lowercase())).await;
    assert_eq!((info["exercise"].as_str(), info["role"].as_str()), (Some("Drill"), Some("player")));
    assert_eq!(info["teams"].as_array().unwrap().len(), 2);
    assert_eq!(app.get(&format!("/api/v2/join?code={}", room.observer_code)).await["role"], "observer");

    for (body, status) in [
        (json!({"code": "WRONG1", "name": "Dana"}), 404),
        (json!({"code": room.player_code, "name": "  "}), 422),
        (json!({"code": room.player_code, "name": "Dana", "team": "no-such-team"}), 400),
    ] {
        assert_eq!(app.post("/api/v2/join", body.clone()).await.status.as_u16(), status, "{body}");
    }

    let dana = Member::join(&room, &room.player_code, "Dana", Some(&room.it)).await;
    let me = dana.get(app, "/api/v2/me").await;
    assert_eq!(me, json!({"role": "player", "name": "Dana", "team": room.it, "exercise_id": room.id}));

    let status = app.get("/api/v2/room").await;
    assert_eq!(status["participants"][0]["name"], "Dana");
    assert_eq!(status["participants"][0]["connected"], false);
    // The participant list never carries the session secret.
    assert!(!status.to_string().contains(dana.cookie.split('=').nth(1).unwrap()));
}

#[tokio::test]
async fn participants_cannot_manage_exercises() {
    let room = room().await;
    let app = &room.app;
    let dana = Member::join(&room, &room.player_code, "Dana", Some(&room.it)).await;
    let screen = Member::join(&room, &room.observer_code, "Screen", None).await;
    let ex = format!("/api/v2/exercises/{}", room.id);
    for who in [&dana, &screen] {
        for (method, path, body) in [
            (Method::GET, "/api/v2/exercises".to_owned(), None),
            (Method::POST, "/api/v2/exercises".to_owned(), Some(json!({"name": "mine"}))),
            (Method::DELETE, ex.clone(), None),
            (Method::GET, format!("{ex}/events"), None),
            (Method::GET, format!("{ex}/export"), None),
            (Method::GET, format!("{ex}/report"), None),
            (Method::GET, "/api/v2/maps".to_owned(), None),
            (
                Method::POST,
                "/api/v2/maps".to_owned(),
                Some(json!({"name": "x", "bounds": [0, 0, 1, 1], "max_zoom": 1})),
            ),
            (Method::POST, format!("{ex}/commands"), Some(json!({"type": "set_review", "review": {}}))),
            (Method::GET, "/api/v2/room".to_owned(), None),
            (Method::POST, "/api/v2/room/close".to_owned(), None),
            (Method::POST, format!("{ex}/commands"), Some(json!({"type": "add_assets", "assets": []}))),
            (Method::POST, format!("{ex}/commands"), Some(json!({"type": "undo"}))),
        ] {
            let (status, _) = who.call(app, method.clone(), &path, body).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}");
        }
    }
    // Nor can they reach an exercise they did not join.
    let other = app.post("/api/v2/exercises", json!({"name": "Other"})).await.body["exercise"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, _) = dana.call(app, Method::GET, &format!("/api/v2/exercises/{other}"), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // Observers cannot log.
    let (status, _) =
        screen.call(app, Method::POST, &format!("{ex}/commands"), Some(json!({"type": "log", "text": "hi"}))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn each_team_gets_its_own_snapshot() {
    let room = room().await;
    let app = &room.app;
    cmd(
        app,
        &room.id,
        json!({"type": "add_assets", "assets": [
            {"name": "public", "type_id": "asset"},
            {"name": "it-only", "type_id": "asset", "visibility": {"mode": "teams", "teams": [room.it]}},
            {"name": "hidden", "type_id": "asset", "notes": "the real cause", "visibility": {"mode": "facilitator"}},
        ]}),
    )
    .await;
    cmd(app, &room.id, json!({"type": "add_inject", "inject": {"title": "queued surprise", "notes": "secret plan"}}))
        .await;

    let path = format!("/api/v2/exercises/{}", room.id);
    let it = Member::join(&room, &room.player_code, "Dana", Some(&room.it)).await;
    let legal = Member::join(&room, &room.player_code, "Lee", Some(&room.legal)).await;
    let screen = Member::join(&room, &room.observer_code, "Screen", None).await;

    assert_eq!(asset_names(&it.get(app, &path).await), ["public", "it-only"]);
    assert_eq!(asset_names(&legal.get(app, &path).await), ["public"]);
    let seen = screen.get(app, &path).await;
    assert_eq!(asset_names(&seen), ["public"]);
    for secret in ["hidden", "the real cause", "queued surprise", "secret plan", "it-only"] {
        assert!(!seen.to_string().contains(secret), "observer snapshot leaks {secret:?}");
    }
    assert_eq!(asset_names(&app.get(&path).await).len(), 3, "the facilitator still sees everything");
}

#[tokio::test]
async fn live_views_reveal_hide_and_stay_quiet() {
    let room = room().await;
    let app = &room.app;
    let r = cmd(
        app,
        &room.id,
        json!({"type": "add_assets", "assets": [{"name": "hidden", "type_id": "asset", "visibility": {"mode": "facilitator"}}]}),
    )
    .await;
    let hidden = r["events"][0]["event"]["assets"][0]["id"].as_str().unwrap().to_owned();

    let it = Member::join(&room, &room.player_code, "Dana", Some(&room.it)).await;
    let legal = Member::join(&room, &room.player_code, "Lee", Some(&room.legal)).await;
    let (mut it_ws, mut legal_ws) = (it.ws(&room).await, legal.ws(&room).await);
    for ws in [&mut it_ws, &mut legal_ws] {
        let snap = ws.recv().await;
        assert_eq!(snap["kind"], "snapshot");
        assert_eq!(snap["exercise"]["assets"], json!({}));
    }
    assert_eq!(app.get("/api/v2/room").await["participants"][0]["connected"], true);

    // A change to something neither can see sends them nothing.
    cmd(app, &room.id, json!({"type": "set_status", "ids": [hidden], "set": "security", "value": "compromised"})).await;
    // Reveal it to IT only.
    cmd(
        app,
        &room.id,
        json!({"type": "edit_asset", "id": hidden, "patch": {"visibility": {"mode": "teams", "teams": [room.it]}}}),
    )
    .await;
    let patch = it_ws.recv().await;
    assert_eq!(patch["kind"], "patch");
    assert_eq!(patch["events"][0]["type"], "assets_upserted");
    assert_eq!(patch["events"][0]["assets"][0]["statuses"]["security"], "compromised");

    // Hide it again: IT is told it is gone. Then something public, which is
    // the first thing Legal ever hears.
    cmd(app, &room.id, json!({"type": "edit_asset", "id": hidden, "patch": {"visibility": {"mode": "facilitator"}}}))
        .await;
    assert_eq!(it_ws.recv().await["events"][0]["type"], "assets_removed");
    cmd(app, &room.id, json!({"type": "add_metric", "name": "Comms", "value": 50})).await;
    assert_eq!(legal_ws.recv().await["events"][0]["type"], "metric_upserted");
    assert_eq!(it_ws.recv().await["events"][0]["type"], "metric_upserted");
}

#[tokio::test]
async fn fired_injects_reach_only_their_audience() {
    let room = room().await;
    let app = &room.app;
    let r = cmd(
        app,
        &room.id,
        json!({"type": "add_inject", "inject": {"title": "Subpoena", "body": "served at 09:00", "notes": "watch for panic",
               "audience": {"mode": "teams", "teams": [room.legal]}}}),
    )
    .await;
    let iid = r["events"][0]["event"]["inject"]["id"].clone();
    let it = Member::join(&room, &room.player_code, "Dana", Some(&room.it)).await;
    let legal = Member::join(&room, &room.player_code, "Lee", Some(&room.legal)).await;
    let (mut it_ws, mut legal_ws) = (it.ws(&room).await, legal.ws(&room).await);
    it_ws.recv().await;
    legal_ws.recv().await;

    cmd(app, &room.id, json!({"type": "fire_inject", "id": iid})).await;
    let patch = legal_ws.recv().await;
    let inject = patch["events"].as_array().unwrap().iter().find(|e| e["type"] == "inject_upserted").unwrap();
    assert_eq!(inject["inject"]["body"], "served at 09:00");
    assert_eq!(inject["inject"]["notes"], "");
    assert!(patch["events"].as_array().unwrap().iter().any(|e| e["type"] == "logged"));

    // IT hears nothing about it; the next public change is their next message.
    cmd(app, &room.id, json!({"type": "log", "text": "all hands"})).await;
    let next = it_ws.recv().await;
    assert_eq!(next["events"][0]["entry"]["text"], "all hands");
}

#[tokio::test]
async fn player_log_entries_are_stamped_and_stay_with_the_team() {
    let room = room().await;
    let app = &room.app;
    let it = Member::join(&room, &room.player_code, "Dana", Some(&room.it)).await;
    let legal = Member::join(&room, &room.player_code, "Lee", Some(&room.legal)).await;
    let commands = format!("/api/v2/exercises/{}/commands", room.id);

    let (status, receipt) =
        it.call(app, Method::POST, &commands, Some(json!({"type": "log", "text": "Isolating the VLAN"}))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(receipt["events"], json!([]), "a player gets a receipt, not the raw event");

    let path = format!("/api/v2/exercises/{}", room.id);
    let entry = app.get(&path).await["exercise"]["log"][0].clone();
    assert_eq!(entry["author"], "Dana (IT)");
    assert_eq!(entry["visibility"], json!({"mode": "teams", "teams": [room.it]}));
    assert_eq!(it.get(app, &path).await["exercise"]["log"][0]["text"], "Isolating the VLAN");
    assert_eq!(legal.get(app, &path).await["exercise"]["log"], json!([]));

    // A player cannot attach an entry to an asset they cannot see.
    let r = cmd(app, &room.id, json!({"type": "add_assets", "assets": [{"name": "h", "type_id": "asset", "visibility": {"mode": "facilitator"}}]})).await;
    let hidden = r["events"][0]["event"]["assets"][0]["id"].clone();
    let (status, _) =
        it.call(app, Method::POST, &commands, Some(json!({"type": "log", "text": "x", "asset": hidden}))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn moving_and_kicking_take_effect_at_once() {
    let room = room().await;
    let app = &room.app;
    let dana = Member::join(&room, &room.player_code, "Dana", Some(&room.it)).await;
    let mut ws = dana.ws(&room).await;
    ws.recv().await;
    let pid = app.get("/api/v2/room").await["participants"][0]["id"].as_str().unwrap().to_owned();

    // Moving teams hangs up the socket so the client reconnects into its new view.
    let r =
        app.call(Method::PATCH, &format!("/api/v2/room/participants/{pid}"), Some(json!({"team": room.legal}))).await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{}", r.body);
    assert!(ws.closed().await);
    assert_eq!(dana.get(app, "/api/v2/me").await["team"], room.legal.as_str());

    let mut ws = dana.ws(&room).await;
    ws.recv().await;
    assert_eq!(app.delete(&format!("/api/v2/room/participants/{pid}")).await.status, StatusCode::NO_CONTENT);
    assert!(ws.closed().await);
    assert_eq!(app.get("/api/v2/room").await["participants"], json!([]));
}

#[tokio::test]
async fn closing_the_room_ends_every_session() {
    let room = room().await;
    let app = &room.app;
    let dana = Member::join(&room, &room.player_code, "Dana", None).await;
    let mut ws = dana.ws(&room).await;
    ws.recv().await;
    assert_eq!(app.post_empty("/api/v2/room/close").await.status, StatusCode::NO_CONTENT);
    assert!(ws.closed().await);
    assert!(app.get("/api/v2/room").await["open"].is_null());
    assert_eq!(
        app.post("/api/v2/join", json!({"code": room.player_code, "name": "Late"})).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn deleting_the_exercise_closes_its_room() {
    let room = room().await;
    let app = &room.app;
    let dana = Member::join(&room, &room.player_code, "Dana", None).await;
    let mut ws = dana.ws(&room).await;
    ws.recv().await;
    assert_eq!(app.delete(&format!("/api/v2/exercises/{}", room.id)).await.status, StatusCode::NO_CONTENT);
    assert!(ws.closed().await);
    let status = app.get("/api/v2/room").await;
    assert!(status["open"].is_null(), "{status}");
    assert_eq!(status["participants"], json!([]));
    assert_eq!(
        app.post("/api/v2/join", json!({"code": room.player_code, "name": "Late"})).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn deleting_another_exercise_leaves_the_room_open() {
    let room = room().await;
    let app = &room.app;
    let other = app.post("/api/v2/exercises", json!({"name": "Other"})).await.body["exercise"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(app.delete(&format!("/api/v2/exercises/{other}")).await.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get("/api/v2/room").await["open"]["exercise_id"], room.id.as_str());
}

#[tokio::test]
async fn the_qr_code_is_an_svg_of_the_join_link() {
    let room = room().await;
    let r = room.app.http.get(format!("{}/api/v2/room/qr.svg", room.app.base)).send().await.unwrap();
    if room.urls.is_empty() {
        assert_eq!(r.status(), StatusCode::CONFLICT, "no network address, so nothing to encode");
    } else {
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(r.headers()["content-type"], "image/svg+xml");
        assert!(r.text().await.unwrap().contains("<svg"));
    }
}

/// Over the real network address (when this machine has one): strangers get
/// the app and the join page, and nothing else until they join.
#[tokio::test]
async fn over_the_lan_only_members_get_in() {
    let room = room().await;
    let Some(lan) = room.urls.first() else {
        eprintln!("no LAN address on this machine; skipping");
        return;
    };
    let http = &room.app.http;
    let get = |path: &str| http.get(format!("{lan}{path}")).send();

    let app_page = get("/").await.unwrap().status();
    assert!(app_page == StatusCode::OK || app_page == StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(get("/api/v2/me").await.unwrap().status(), StatusCode::UNAUTHORIZED);
    assert_eq!(get("/api/v2/exercises").await.unwrap().status(), StatusCode::UNAUTHORIZED);
    assert_eq!(get(&format!("/api/v2/exercises/{}", room.id)).await.unwrap().status(), StatusCode::UNAUTHORIZED);
    assert_eq!(get("/api/v2/room").await.unwrap().status(), StatusCode::UNAUTHORIZED);

    let r = http
        .post(format!("{lan}/api/v2/join"))
        .json(&json!({"code": room.player_code, "name": "Phone"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let cookie = r.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap().to_owned();
    let me: Value =
        http.get(format!("{lan}/api/v2/me")).header("Cookie", &cookie).send().await.unwrap().json().await.unwrap();
    assert_eq!(me["role"], "player");
    // Still not the facilitator.
    let r = http.get(format!("{lan}/api/v2/exercises")).header("Cookie", &cookie).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);

    // Once the room closes, the address stops answering as the app.
    room.app.post_empty("/api/v2/room/close").await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let after = http.get(format!("{lan}/api/v2/me")).send().await;
    assert!(after.is_err() || after.unwrap().status() != StatusCode::OK);
}
