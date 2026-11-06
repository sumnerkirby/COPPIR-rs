//! A real server on a random port, with its own scenarios directory, for each
//! test. State is per-process in the app, so isolation is free: every test
//! gets a fresh `AppState`.

#![allow(dead_code)]

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use reqwest::{Method, StatusCode};
use serde_json::Value;
use tempfile::TempDir;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message};

pub struct TestApp {
    pub base: String,
    pub addr: String,
    pub http: reqwest::Client,
    pub dir: TempDir,
}

pub struct Res {
    pub status: StatusCode,
    pub body: Value,
}

impl TestApp {
    pub async fn spawn() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let scenarios = dir.path().join("scenarios");
        let listener = coppir::bind(0, false).await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let mut config = coppir::Config::new(scenarios);
        // Nothing in the suite should reach the internet.
        config.nominatim_url = "http://127.0.0.1:9/".into();
        config.overpass_mirrors = vec!["http://127.0.0.1:9/".into()];
        tokio::spawn(coppir::serve(listener, config, std::future::pending()));
        Self { base: format!("http://{addr}"), addr, http: reqwest::Client::new(), dir }
    }

    pub fn scenarios_dir(&self) -> std::path::PathBuf {
        self.dir.path().join("scenarios")
    }

    pub async fn call(&self, method: Method, path: &str, body: Option<Value>) -> Res {
        let mut req = self.http.request(method, format!("{}{path}", self.base));
        if let Some(b) = body {
            req = req.json(&b);
        }
        let r = req.send().await.unwrap();
        let status = r.status();
        let text = r.text().await.unwrap();
        let body = serde_json::from_str(&text).unwrap_or(Value::String(text));
        Res { status, body }
    }

    pub async fn get(&self, path: &str) -> Value {
        let r = self.call(Method::GET, path, None).await;
        assert_eq!(r.status, StatusCode::OK, "GET {path}: {}", r.body);
        r.body
    }
    pub async fn post(&self, path: &str, body: Value) -> Res {
        self.call(Method::POST, path, Some(body)).await
    }
    pub async fn post_empty(&self, path: &str) -> Res {
        self.call(Method::POST, path, None).await
    }
    pub async fn put(&self, path: &str, body: Value) -> Res {
        self.call(Method::PUT, path, Some(body)).await
    }
    pub async fn delete(&self, path: &str) -> Res {
        self.call(Method::DELETE, path, None).await
    }

    pub async fn len(&self, path: &str) -> usize {
        let v = self.get(path).await;
        v.as_array().map(Vec::len).or_else(|| v.as_object().map(|o| o.len())).unwrap()
    }
}

pub struct Ws(pub WebSocketStream<MaybeTlsStream<TcpStream>>);

impl Ws {
    pub async fn recv(&mut self) -> Value {
        loop {
            let msg = tokio::time::timeout(Duration::from_secs(5), self.0.next())
                .await
                .expect("timed out waiting for a websocket message")
                .expect("stream ended")
                .unwrap();
            if let Message::Text(t) = msg {
                return serde_json::from_str(&t).unwrap();
            }
        }
    }

    pub async fn close(mut self) {
        let _ = self.0.send(Message::Close(None)).await;
    }
}

pub fn merge(base: &mut Value, overrides: Value) {
    if let (Some(b), Value::Object(o)) = (base.as_object_mut(), overrides) {
        b.extend(o);
    }
}
