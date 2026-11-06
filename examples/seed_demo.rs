//! Seed a running COPPIR instance with a demo incident scenario.
//!
//! A fictional incident in Washington DC: a SCADA compromise at a power
//! substation cascades through a fiber cut into transit signalling, with the
//! surrounding assets at varying states of health. Asset names and coordinates
//! are real OpenStreetMap data; everything else in `demo.json` is made up.
//! Statuses are picked so each sector wheel lands somewhere different.
//!
//! ```text
//! cargo run -- --headless              # one shell
//! cargo run --example seed_demo        # another
//! ```
//!
//! This clears whatever is loaded, then saves the result as a scenario named
//! "DC Grid Incident Demo". Point it elsewhere with `COPPIR_API`.

use std::{collections::HashMap, time::Duration};

use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Demo {
    scenario: String,
    pins: Vec<Value>,
    edges: Vec<DemoEdge>,
    injects: Vec<DemoInject>,
    log: Vec<DemoLog>,
    thresholds: Vec<Value>,
}

#[derive(Deserialize)]
struct DemoEdge {
    from: String,
    to: String,
    label: String,
}

#[derive(Deserialize)]
struct DemoInject {
    title: String,
    description: String,
    severity: String,
    fire: bool,
    target: Option<String>,
    target_status: Option<String>,
}

#[derive(Deserialize)]
struct DemoLog {
    action: String,
    asset: Option<String>,
}

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Api {
    base: String,
    http: reqwest::Client,
}

impl Api {
    async fn post(&self, path: &str, body: Option<Value>) -> Result<Value> {
        let mut req = self.http.post(format!("{}{path}", self.base));
        if let Some(b) = body {
            req = req.json(&b);
        }
        let res = req.send().await?;
        let status = res.status();
        let body: Value = res.json().await?;
        if !status.is_success() {
            return Err(format!("POST {path} -> {status}: {body}").into());
        }
        Ok(body)
    }
}

#[tokio::main]
async fn main() -> Result {
    let demo: Demo = serde_json::from_str(include_str!("demo.json"))?;
    let base = std::env::var("COPPIR_API").unwrap_or_else(|_| "http://127.0.0.1:8000".into());
    let api = Api { http: reqwest::Client::builder().timeout(Duration::from_secs(30)).build()?, base };

    if api.http.get(format!("{}/api/health", api.base)).timeout(Duration::from_secs(3)).send().await.is_err() {
        return Err(
            format!("No COPPIR instance reachable at {}. Start one with: cargo run -- --headless", api.base).into()
        );
    }

    api.post("/api/state/clear", None).await?;

    let mut ids = HashMap::new();
    for pin in &demo.pins {
        let created = api.post("/api/pins", Some(pin.clone())).await?;
        ids.insert(pin["name"].as_str().unwrap_or_default().to_owned(), created["id"].clone());
    }
    println!("pins:       {}", ids.len());

    for e in &demo.edges {
        api.post(
            "/api/edges",
            Some(json!({"from_pid": ids[&e.from], "to_pid": ids[&e.to], "label": e.label, "edge_type": "dependency"})),
        )
        .await?;
    }
    println!("edges:      {}", demo.edges.len());

    let (mut fired, mut queued) = (0, 0);
    for inj in &demo.injects {
        let mut body = json!({"title": inj.title, "description": inj.description, "severity": inj.severity});
        if let Some(target) = &inj.target {
            body["target_pid"] = ids[target].clone();
            body["target_status"] = json!(inj.target_status);
        }
        let created = api.post("/api/injects", Some(body)).await?;
        if inj.fire {
            let iid = created["id"].as_str().unwrap_or_default();
            api.post(&format!("/api/injects/{iid}/trigger"), None).await?;
            fired += 1;
            // Spread the timestamps so the log timeline has some shape to it.
            tokio::time::sleep(Duration::from_millis(200)).await;
        } else {
            queued += 1;
        }
    }
    println!("injects:    {fired} fired, {queued} queued");

    for t in &demo.thresholds {
        api.post("/api/thresholds", Some(t.clone())).await?;
    }
    println!("thresholds: {}", demo.thresholds.len());

    for entry in &demo.log {
        let mut body = json!({"action": entry.action});
        if let Some(asset) = &entry.asset {
            body["asset_name"] = json!(asset);
            body["asset_pid"] = ids[asset].clone();
        }
        api.post("/api/log", Some(body)).await?;
    }
    println!("log:        {} manual entries", demo.log.len());

    let saved = api.post("/api/scenarios/save", Some(json!({"name": demo.scenario}))).await?;
    println!("scenario:   {saved}");
    Ok(())
}
