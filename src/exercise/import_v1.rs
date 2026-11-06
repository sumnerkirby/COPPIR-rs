//! Turn a v1 scenario (from this app's v1 API or the Python app) into a v2
//! exercise. Categories the default taxonomy does not know become new asset
//! types, so nothing is dropped.

use std::collections::BTreeMap;

use super::legacy::{self as v1, OpStatus, Scenario, Status};
use super::{
    model::{
        Asset, AssetType, Effect, Exercise, GeoPoint, Inject, Link, LinkKind, LogEntry, NewAsset, Severity,
        StatusChange, Threshold,
    },
    taxonomy::{OPERATIONAL, SECURITY, slug},
};
use crate::error::ApiError;

fn security(s: Status) -> &'static str {
    match s {
        Status::Clean => "clean",
        Status::Monitored => "monitored",
        Status::Contained => "contained",
        Status::UnderInvestigation => "under_investigation",
        Status::Compromised => "compromised",
    }
}

fn operational(s: OpStatus) -> &'static str {
    match s {
        OpStatus::Healthy => "healthy",
        OpStatus::Degraded => "degraded",
        OpStatus::Critical => "critical",
        OpStatus::Offline => "offline",
    }
}

fn severity(s: v1::Severity) -> Severity {
    match s {
        v1::Severity::Info => Severity::Info,
        v1::Severity::Warning => Severity::Warning,
        v1::Severity::Critical => Severity::Critical,
    }
}

/// Parse v1 scenario JSON and build an exercise from it.
pub fn import(json: &serde_json::Value, name: &str, now: &str) -> Result<Exercise, ApiError> {
    let session: Scenario = serde_json::from_value(json.clone())
        .map_err(|e| ApiError::BadRequest(format!("Not a v1 scenario file: {e}")))?;
    let mut ex = super::blank(name, "Imported from a v1 scenario.", now)?;

    // Map every category to an asset type, adding the ones we have not seen.
    let type_of = |category: &str, ex: &mut Exercise| -> String {
        if let Some(t) = ex.taxonomy.asset_types.iter().find(|t| t.name.eq_ignore_ascii_case(category)) {
            return t.id.clone();
        }
        let mut id = slug(category);
        while ex.taxonomy.asset_types.iter().any(|t| t.id == id) {
            id.push('_');
        }
        ex.taxonomy.asset_types.push(AssetType {
            id: id.clone(),
            name: category.to_owned(),
            icon: "circle".into(),
            color: "#888888".into(),
        });
        id
    };

    for pin in session.pins.values() {
        let type_id = type_of(&pin.category, &mut ex);
        let statuses = BTreeMap::from([
            (SECURITY.to_owned(), security(pin.status).to_owned()),
            (OPERATIONAL.to_owned(), operational(pin.op_status).to_owned()),
        ]);
        let mut history: Vec<StatusChange> = pin
            .status_history
            .iter()
            .map(|h| StatusChange { set: SECURITY.into(), value: security(h.status).into(), at: h.timestamp.clone() })
            .collect();
        history.push(StatusChange {
            set: OPERATIONAL.into(),
            value: operational(pin.op_status).into(),
            at: pin.pinned_at.clone(),
        });
        ex.assets.insert(
            pin.id.clone(),
            Asset {
                id: pin.id.clone(),
                name: pin.name.clone(),
                type_id,
                geo: Some(GeoPoint { lat: pin.lat, lon: pin.lon }),
                graph: None,
                statuses,
                history,
                fields: BTreeMap::new(),
                tags: vec![],
                notes: pin.notes.clone(),
                color: pin.pin_color.clone(),
                visibility: Default::default(),
                created_at: if pin.pinned_at.is_empty() { now.to_owned() } else { pin.pinned_at.clone() },
            },
        );
    }

    for e in session.edges.values() {
        if !ex.assets.contains_key(&e.from_pid) || !ex.assets.contains_key(&e.to_pid) || e.from_pid == e.to_pid {
            continue;
        }
        let kind = if e.edge_type == "dependency" { LinkKind::DependsOn } else { LinkKind::ConnectsTo };
        ex.links.insert(
            e.id.clone(),
            Link { id: e.id.clone(), from: e.from_pid.clone(), to: e.to_pid.clone(), kind, label: e.label.clone() },
        );
    }

    for i in session.injects.values() {
        let mut effects = Vec::new();
        if let (Some(asset), Some(status)) = (&i.target_pid, i.target_status) {
            effects.push(Effect::SetStatus {
                asset: asset.clone(),
                set: SECURITY.into(),
                value: security(status).into(),
            });
        }
        if let Some(np) = &i.new_pin {
            let type_id = type_of(&np.category, &mut ex);
            effects.push(Effect::CreateAsset {
                asset: NewAsset {
                    name: np.name.clone(),
                    type_id,
                    geo: Some(GeoPoint { lat: np.lat, lon: np.lon }),
                    graph: None,
                    statuses: BTreeMap::from([(SECURITY.to_owned(), security(np.status).to_owned())]),
                    notes: String::new(),
                    color: None,
                    tags: vec![],
                    visibility: Default::default(),
                },
            });
        }
        ex.injects.insert(
            i.id.clone(),
            Inject {
                id: i.id.clone(),
                title: i.title.clone(),
                body: i.description.clone(),
                severity: severity(i.severity),
                audience: Default::default(),
                effects,
                notes: String::new(),
                created_at: i.created_at.clone(),
                fired_at: i.triggered_at.clone(),
            },
        );
    }

    ex.thresholds = session
        .thresholds
        .iter()
        .map(|t| Threshold {
            id: t.id.clone(),
            name: t.name.clone(),
            sector: t.sector.id().map(str::to_owned),
            below_pct: t.below_pct.min(100),
            severity: severity(t.severity),
        })
        .collect();

    ex.log = session
        .decision_log
        .iter()
        .map(|l| LogEntry {
            id: l.id.clone(),
            at: l.timestamp.clone(),
            text: l.action.clone(),
            asset: l.asset_pid.clone(),
            asset_name: l.asset_name.clone(),
            notes: l.notes.clone(),
            auto: l.auto,
        })
        .collect();

    Ok(ex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn imports_everything_and_invents_missing_types() {
        let v1 = json!({
            "pins": {
                "p1": {"id": "p1", "name": "Sub", "category": "Power Plants", "lat": 1.0, "lon": 2.0,
                       "status": "Compromised", "op_status": "Degraded", "pinned_at": "t0",
                       "status_history": [{"status": "Clean", "timestamp": "t0"}, {"status": "Compromised", "timestamp": "t1"}]},
                "p2": {"id": "p2", "name": "Van", "category": "Mobile Units", "lat": 1.0, "lon": 2.0}
            },
            "edges": {"e1": {"id": "e1", "from_pid": "p1", "to_pid": "p2", "edge_type": "dependency"},
                      "e2": {"id": "e2", "from_pid": "p1", "to_pid": "gone"}},
            "injects": {"i1": {"id": "i1", "title": "t", "description": "d", "target_pid": "p1",
                               "target_status": "Contained", "new_pin": {"name": "CP", "category": "Asset", "lat": 0, "lon": 0}}},
            "thresholds": [{"id": "t1", "name": "All", "sector": "all", "below_pct": 50},
                           {"id": "t2", "name": "Grid", "sector": "power", "below_pct": 40}],
            "decision_log": [{"id": "l1", "timestamp": "t2", "action": "declared"}]
        });
        let ex = import(&v1, "Legacy", "now").unwrap();
        assert_eq!(ex.assets.len(), 2);
        let sub = &ex.assets["p1"];
        assert_eq!(sub.type_id, "power_plants");
        assert_eq!(sub.statuses[SECURITY], "compromised");
        assert_eq!(sub.statuses[OPERATIONAL], "degraded");
        assert_eq!(ex.assets["p2"].type_id, "mobile_units");
        assert!(ex.taxonomy.asset_types.iter().any(|t| t.name == "Mobile Units"));
        assert_eq!(ex.links.len(), 1);
        assert_eq!(ex.links["e1"].kind, LinkKind::DependsOn);
        assert_eq!(ex.injects["i1"].effects.len(), 2);
        assert_eq!(ex.thresholds[0].sector, None);
        assert_eq!(ex.thresholds[1].sector.as_deref(), Some("power"));
        assert_eq!(ex.log[0].text, "declared");
    }

    #[test]
    fn rejects_garbage() {
        assert!(import(&json!({"pins": [1, 2]}), "x", "now").is_err());
    }
}
