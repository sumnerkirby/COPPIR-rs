//! Undo by compensation: the inverse of a change is more events, appended like
//! any other. History is never rewritten, so the after-action log shows both
//! the mistake and its correction.

use super::{
    event::Event,
    model::{Asset, Exercise},
};

/// The events that reverse `e`, given the exercise as it was just before `e`.
pub fn inverse(before: &Exercise, e: &Event) -> Vec<Event> {
    let mut out = Vec::new();
    match e {
        // Neither can be undone: there is no state before creation, and an
        // undo is not itself undoable.
        Event::Created { .. } | Event::Undone { .. } | Event::LogRemoved { .. } | Event::Batch { .. } => {}
        Event::TeamUpserted { team } => out.push(match before.teams.iter().find(|t| t.id == team.id) {
            Some(old) => Event::TeamUpserted { team: old.clone() },
            None => Event::TeamRemoved { id: team.id.clone() },
        }),
        Event::TeamRemoved { id } => {
            if let Some(old) = before.teams.iter().find(|t| &t.id == id) {
                out.push(Event::TeamUpserted { team: old.clone() });
            }
        }
        Event::Renamed { .. } => {
            out.push(Event::Renamed { name: before.name.clone(), description: before.description.clone() })
        }
        Event::TaxonomyReplaced { assets, .. } => {
            out.push(Event::TaxonomyReplaced { taxonomy: before.taxonomy.clone(), assets: previous(before, assets).0 })
        }
        Event::AssetsUpserted { assets } => restore_assets(before, assets, &mut out),
        Event::AssetsRemoved { ids, links } => {
            let assets: Vec<Asset> = ids.iter().filter_map(|id| before.assets.get(id).cloned()).collect();
            out.push(Event::AssetsUpserted { assets });
            for id in links {
                if let Some(link) = before.links.get(id) {
                    out.push(Event::LinkUpserted { link: link.clone() });
                }
            }
        }
        Event::LinkUpserted { link } => out.push(match before.links.get(&link.id) {
            Some(old) => Event::LinkUpserted { link: old.clone() },
            None => Event::LinkRemoved { id: link.id.clone() },
        }),
        Event::LinkRemoved { id } => {
            if let Some(old) = before.links.get(id) {
                out.push(Event::LinkUpserted { link: old.clone() });
            }
        }
        Event::InjectUpserted { inject } => out.push(match before.injects.get(&inject.id) {
            Some(old) => Event::InjectUpserted { inject: old.clone() },
            None => Event::InjectRemoved { id: inject.id.clone() },
        }),
        Event::InjectRemoved { id } => {
            if let Some(old) = before.injects.get(id) {
                out.push(Event::InjectUpserted { inject: old.clone() });
            }
        }
        Event::InjectFired { inject, assets, entry } => {
            if let Some(old) = before.injects.get(&inject.id) {
                out.push(Event::InjectUpserted { inject: old.clone() });
            }
            restore_assets(before, assets, &mut out);
            out.push(Event::LogRemoved { id: entry.id.clone() });
        }
        Event::ThresholdUpserted { threshold } => {
            out.push(match before.thresholds.iter().find(|t| t.id == threshold.id) {
                Some(old) => Event::ThresholdUpserted { threshold: old.clone() },
                None => Event::ThresholdRemoved { id: threshold.id.clone() },
            })
        }
        Event::ThresholdRemoved { id } => {
            if let Some(old) = before.thresholds.iter().find(|t| &t.id == id) {
                out.push(Event::ThresholdUpserted { threshold: old.clone() });
            }
        }
        Event::Logged { entry } => out.push(Event::LogRemoved { id: entry.id.clone() }),
        Event::AnnotationUpserted { annotation } => out.push(match before.annotations.get(&annotation.id) {
            Some(old) => Event::AnnotationUpserted { annotation: old.clone() },
            None => Event::AnnotationsRemoved { ids: vec![annotation.id.clone()] },
        }),
        Event::AnnotationsRemoved { ids } => {
            for id in ids {
                if let Some(old) = before.annotations.get(id) {
                    out.push(Event::AnnotationUpserted { annotation: old.clone() });
                }
            }
        }
        Event::MetricUpserted { metric } => out.push(match before.metrics.iter().find(|m| m.id == metric.id) {
            Some(old) => Event::MetricUpserted { metric: old.clone() },
            None => Event::MetricRemoved { id: metric.id.clone() },
        }),
        Event::MetricRemoved { id } => {
            if let Some(old) = before.metrics.iter().find(|m| &m.id == id) {
                out.push(Event::MetricUpserted { metric: old.clone() });
            }
        }
    }
    out
}

/// Split `assets` into the previous versions of those that existed, and the
/// ids of those that are new.
fn previous(before: &Exercise, assets: &[Asset]) -> (Vec<Asset>, Vec<String>) {
    let mut old = Vec::new();
    let mut new = Vec::new();
    for a in assets {
        match before.assets.get(&a.id) {
            Some(prev) => old.push(prev.clone()),
            None => new.push(a.id.clone()),
        }
    }
    (old, new)
}

fn restore_assets(before: &Exercise, assets: &[Asset], out: &mut Vec<Event>) {
    let (old, new) = previous(before, assets);
    if !old.is_empty() {
        out.push(Event::AssetsUpserted { assets: old });
    }
    if !new.is_empty() {
        // New assets cannot have links yet: links are added by later commands.
        out.push(Event::AssetsRemoved { ids: new, links: vec![] });
    }
}

/// Whether everything `events` produced is still as they left it. Undoing a
/// change that something later has already overwritten would silently throw
/// that later work away, so it is refused instead.
pub fn still_current(now: &Exercise, events: &[Event]) -> bool {
    events.iter().all(|e| match e {
        Event::AssetsUpserted { assets } | Event::TaxonomyReplaced { assets, .. } => {
            assets.iter().all(|a| now.assets.get(&a.id) == Some(a))
        }
        Event::InjectFired { inject, assets, .. } => {
            now.injects.get(&inject.id) == Some(inject) && assets.iter().all(|a| now.assets.get(&a.id) == Some(a))
        }
        Event::AssetsRemoved { ids, .. } => ids.iter().all(|id| !now.assets.contains_key(id)),
        Event::LinkUpserted { link } => now.links.get(&link.id) == Some(link),
        Event::LinkRemoved { id } => !now.links.contains_key(id),
        Event::InjectUpserted { inject } => now.injects.get(&inject.id) == Some(inject),
        Event::InjectRemoved { id } => !now.injects.contains_key(id),
        Event::ThresholdUpserted { threshold } => now.thresholds.iter().any(|t| t == threshold),
        Event::ThresholdRemoved { id } => now.thresholds.iter().all(|t| &t.id != id),
        Event::AnnotationUpserted { annotation } => now.annotations.get(&annotation.id) == Some(annotation),
        Event::AnnotationsRemoved { ids } => ids.iter().all(|id| !now.annotations.contains_key(id)),
        Event::MetricUpserted { metric } => now.metrics.iter().any(|m| m == metric),
        Event::MetricRemoved { id } => now.metrics.iter().all(|m| &m.id != id),
        Event::Renamed { name, .. } => &now.name == name,
        Event::TeamUpserted { team } => now.teams.iter().any(|t| t == team),
        Event::TeamRemoved { id } => now.teams.iter().all(|t| &t.id != id),
        Event::Logged { .. }
        | Event::LogRemoved { .. }
        | Event::Created { .. }
        | Event::Undone { .. }
        | Event::Batch { .. } => true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exercise::{Command, Envelope, blank, decide, model::NewAsset};

    fn run(ex: &mut Exercise, cmd: Command) -> (Vec<Event>, Vec<Event>) {
        let events = decide(ex, cmd, "t").unwrap();
        let mut inv = Vec::new();
        for e in &events {
            let mut group = inverse(ex, e);
            ex.apply(&Envelope { seq: 0, at: "t".into(), actor: "x".into(), event: e.clone() });
            group.reverse();
            inv.extend(group);
        }
        inv.reverse();
        (events, inv)
    }

    fn undo(ex: &mut Exercise, inv: Vec<Event>) {
        for e in inv {
            ex.apply(&Envelope { seq: 0, at: "t".into(), actor: "x".into(), event: e });
        }
    }

    fn new_asset(name: &str) -> NewAsset {
        serde_json::from_value(serde_json::json!({"name": name, "type_id": "asset"})).unwrap()
    }

    /// Every undoable command, applied then undone, leaves the exercise as it
    /// was (apart from `updated_at`).
    #[test]
    fn undo_restores_the_previous_state() {
        let mut ex = blank("Drill", "", "t0").unwrap();
        let (ev, _) = run(&mut ex, Command::AddAssets { assets: vec![new_asset("a"), new_asset("b")] });
        let Event::AssetsUpserted { assets } = &ev[0] else { panic!() };
        let (a, b) = (assets[0].id.clone(), assets[1].id.clone());
        run(&mut ex, Command::AddLink { from: a.clone(), to: b.clone(), kind: None, label: None });
        let inject: crate::exercise::command::NewInject = serde_json::from_value(serde_json::json!({
            "title": "t",
            "effects": [{"type": "set_status", "asset": a, "set": "security", "value": "compromised"},
                        {"type": "create_asset", "asset": {"name": "spawn", "type_id": "asset"}}]
        }))
        .unwrap();
        let (ev, _) = run(&mut ex, Command::AddInject { inject });
        let Event::InjectUpserted { inject } = &ev[0] else { panic!() };
        let iid = inject.id.clone();

        let commands = vec![
            Command::Rename { name: "New".into(), description: Some("d".into()) },
            Command::SetStatus { ids: vec![a.clone(), b.clone()], set: "security".into(), value: "contained".into() },
            Command::RemoveAssets { ids: vec![a.clone()] },
            Command::FireInject { id: iid.clone() },
            Command::RemoveInject { id: iid.clone() },
            Command::Log { text: "x".into(), asset: None, notes: None },
            Command::AddThreshold { name: "t".into(), sector: None, below_pct: 50, severity: None },
            Command::AddMetric { name: "Comms".into(), value: Some(40) },
            Command::AddAnnotation {
                shape: crate::exercise::model::Shape::Point,
                geometry: serde_json::json!({"type": "Point", "coordinates": [1.0, 2.0]}),
                label: None,
                color: None,
            },
        ];
        for cmd in commands {
            let label = format!("{cmd:?}");
            let mut before = ex.clone();
            let (events, inv) = run(&mut ex, cmd);
            assert!(still_current(&ex, &events), "{label}");
            undo(&mut ex, inv);
            before.updated_at.clone_from(&ex.updated_at);
            // Undo re-inserts at the end, so compare contents, not order.
            ex.assets.sort_keys();
            before.assets.sort_keys();
            ex.links.sort_keys();
            before.links.sort_keys();
            assert_eq!(ex, before, "{label}");
        }
    }

    #[test]
    fn a_later_overwrite_makes_an_undo_stale() {
        let mut ex = blank("Drill", "", "t0").unwrap();
        let (ev, _) = run(&mut ex, Command::AddAssets { assets: vec![new_asset("a")] });
        let Event::AssetsUpserted { assets } = &ev[0] else { panic!() };
        let id = assets[0].id.clone();
        let (first, _) = run(
            &mut ex,
            Command::SetStatus { ids: vec![id.clone()], set: "security".into(), value: "monitored".into() },
        );
        assert!(still_current(&ex, &first));
        run(&mut ex, Command::SetStatus { ids: vec![id], set: "security".into(), value: "compromised".into() });
        assert!(!still_current(&ex, &first));
    }
}
