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
//! Each run creates a new exercise named "DC Grid Incident Demo" and prints
//! its address. Point it at another instance with `COPPIR_API`.

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
    async fn post(&self, path: &str, body: Value) -> Result<Value> {
        let res = self.http.post(format!("{}{path}", self.base)).json(&body).send().await?;
        let status = res.status();
        let body: Value = res.json().await?;
        if !status.is_success() {
            return Err(format!("POST {path} -> {status}: {body}").into());
        }
        Ok(body)
    }
}

/// "Under Investigation" -> "under_investigation": the default taxonomy's ids.
fn value_id(label: &str) -> String {
    label.to_lowercase().replace(' ', "_")
}

#[tokio::main]
async fn main() -> Result {
    let demo: Demo = serde_json::from_str(include_str!("demo.json"))?;
    let base = std::env::var("COPPIR_API").unwrap_or_else(|_| "http://127.0.0.1:8000".into());
    let api = Api { http: reqwest::Client::builder().timeout(Duration::from_secs(30)).build()?, base };

    if api.http.get(format!("{}/api/health", api.base)).timeout(Duration::from_secs(3)).send().await.is_err() {
        return Err(format!("No COPPIR instance reachable at {}. Start one with: cargo run", api.base).into());
    }

    let taxonomy: Value = api.http.get(format!("{}/api/v2/taxonomy/default", api.base)).send().await?.json().await?;
    let type_id = |name: &str| -> Value {
        taxonomy["asset_types"]
            .as_array()
            .and_then(|ts| ts.iter().find(|t| t["name"] == name))
            .map_or(json!("asset"), |t| t["id"].clone())
    };

    let created = api.post("/api/v2/exercises", json!({"name": demo.scenario})).await?;
    let id = created["exercise"]["id"].as_str().unwrap_or_default().to_owned();
    let path = format!("/api/v2/exercises/{id}/commands");
    let cmd = |body: Value| api.post(&path, body);

    let assets: Vec<Value> = demo
        .pins
        .iter()
        .map(|p| {
            json!({
                "name": p["name"],
                "type_id": type_id(p["category"].as_str().unwrap_or_default()),
                "geo": {"lat": p["lat"], "lon": p["lon"]},
                "statuses": {
                    "security": value_id(p["status"].as_str().unwrap_or("Clean")),
                    "operational": value_id(p["op_status"].as_str().unwrap_or("Healthy")),
                },
            })
        })
        .collect();
    let added = cmd(json!({"type": "add_assets", "assets": assets})).await?;
    let mut ids = HashMap::new();
    for a in added["events"][0]["event"]["assets"].as_array().into_iter().flatten() {
        ids.insert(a["name"].as_str().unwrap_or_default().to_owned(), a["id"].clone());
    }
    println!("assets:      {}", ids.len());

    for e in &demo.edges {
        cmd(
            json!({"type": "add_link", "from": ids[&e.from], "to": ids[&e.to], "label": e.label, "kind": "depends_on"}),
        )
        .await?;
    }
    println!("links:       {}", demo.edges.len());

    let (mut fired, mut queued) = (0, 0);
    for inj in &demo.injects {
        let mut effects = vec![];
        if let (Some(target), Some(status)) = (&inj.target, &inj.target_status) {
            effects.push(
                json!({"type": "set_status", "asset": ids[target], "set": "security", "value": value_id(status)}),
            );
        }
        let body = json!({"type": "add_inject", "inject": {
            "title": inj.title, "body": inj.description, "severity": inj.severity, "effects": effects,
        }});
        let r = cmd(body).await?;
        if inj.fire {
            let iid = r["events"][0]["event"]["inject"]["id"].clone();
            cmd(json!({"type": "fire_inject", "id": iid})).await?;
            fired += 1;
            // Spread the timestamps so the timeline has some shape to it.
            tokio::time::sleep(Duration::from_millis(200)).await;
        } else {
            queued += 1;
        }
    }
    println!("injects:     {fired} fired, {queued} queued");

    for t in &demo.thresholds {
        let mut t = t.clone();
        t["type"] = json!("add_threshold");
        cmd(t).await?;
    }
    println!("thresholds:  {}", demo.thresholds.len());

    for entry in &demo.log {
        let mut body = json!({"type": "log", "text": entry.action});
        if let Some(asset) = &entry.asset {
            body["asset"] = ids[asset].clone();
        }
        cmd(body).await?;
    }
    println!("log:         {} manual entries", demo.log.len());

    for (name, value) in [("Comms readiness", 60), ("Field teams available", 75), ("Backup generation", 40)] {
        cmd(json!({"type": "add_metric", "name": name, "value": value})).await?;
    }
    // A cordon around the Rosslyn substation.
    let (lat, lon) = (38.8968_f64, -77.0719_f64);
    let ring: Vec<Value> = (0..=32)
        .map(|i| {
            let a = f64::from(i) / 32.0 * std::f64::consts::TAU;
            json!([lon + 0.006 * a.cos() / lat.to_radians().cos(), lat + 0.006 * a.sin()])
        })
        .collect();
    cmd(json!({"type": "add_annotation", "shape": "circle", "label": "Rosslyn cordon", "color": "#ff4d4d",
               "geometry": {"type": "Polygon", "coordinates": [ring]}}))
    .await?;
    println!("extras:      3 metrics, 1 cordon");

    // The script: phases, an impact rule, and a branch on a decision.
    cmd(json!({"type": "set_phases", "phases": [
        {"id": "", "name": "Detection", "color": "#3399ff", "target_minutes": 20},
        {"id": "", "name": "Containment", "color": "#ffaa00", "target_minutes": 30},
        {"id": "", "name": "Eradication", "color": "#ff6633", "target_minutes": 25},
        {"id": "", "name": "Recovery", "color": "#33cc99", "target_minutes": 20},
    ]}))
    .await?;
    cmd(json!({"type": "set_impact_rules", "rules": [{"id": "", "kind": "depends_on", "set": "operational",
        "when": ["offline", "critical"], "then_set": "operational", "then_value": "degraded"}]}))
    .await?;
    let poll = cmd(json!({"type": "add_poll", "poll": {"question": "The water authority demands payment. Do you pay?",
        "options": ["Pay the ransom", "Refuse and restore from backup"], "allow_text": true, "duration_minutes": 10}}))
    .await?;
    let poll = &poll["events"][0]["event"]["poll"];
    let (poll_id, pay, refuse) =
        (poll["id"].clone(), poll["options"][0]["id"].clone(), poll["options"][1]["id"].clone());
    cmd(json!({"type": "add_inject", "inject": {"title": "Extortion demand", "severity": "critical",
        "body": "A second note sets a 10-minute deadline and asks for 40 BTC.",
        "trigger": {"type": "at", "minutes": 25}, "effects": [{"type": "open_poll", "poll": poll_id}]}}))
    .await?;
    cmd(json!({"type": "add_inject", "inject": {"title": "Payment traced", "severity": "warning",
        "body": "Treasury flags the payment; decryptor only partly works.",
        "trigger": {"type": "when", "condition": {"type": "poll_choice", "poll": poll_id, "option": pay}}}}))
    .await?;
    cmd(json!({"type": "add_inject", "inject": {"title": "Data leak threatened", "severity": "critical",
        "body": "Attackers post a sample of customer records and threaten to publish the rest.",
        "trigger": {"type": "when", "condition": {"type": "poll_choice", "poll": poll_id, "option": refuse}}}}))
    .await?;
    println!("script:      4 phases, 1 impact rule, 1 poll with two branches");
    println!("open:        {}/x/{id}", api.base);
    Ok(())
}
