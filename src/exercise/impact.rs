//! Impact propagation: a status change spreading along links.
//!
//! An [`ImpactRule`] says, for example, "when an asset's Operational status
//! becomes Offline or Critical, everything that depends on it becomes
//! Degraded". Links run from the provider to the dependant (`from` → `to`), so
//! the rule follows them forwards, transitively, and never makes anything
//! better than it already is.

use std::collections::HashSet;

use super::model::{Asset, Exercise, StatusChange};

/// Spread the changes in `changed` through the exercise's impact rules,
/// adding every asset that changes as a result. Returns one line per
/// knock-on change, for the log.
pub fn propagate(ex: &Exercise, changed: &mut Vec<Asset>, now: &str) -> Vec<String> {
    if ex.impact_rules.is_empty() {
        return vec![];
    }
    let score = |set: &str, value: &str| -> Option<u8> {
        ex.taxonomy.status_sets.iter().find(|s| s.id == set)?.value(value).map(|v| v.score)
    };
    let label = |set: &str, value: &str| -> String {
        ex.taxonomy
            .status_sets
            .iter()
            .find(|s| s.id == set)
            .and_then(|s| s.value(value))
            .map_or_else(|| value.to_owned(), |v| v.label.clone())
    };

    let mut lines = Vec::new();
    // (asset, rule) pairs already spread, so a cycle of links stops.
    let mut done: HashSet<(String, String)> = HashSet::new();
    let mut queue: Vec<String> = changed.iter().map(|a| a.id.clone()).collect();

    while let Some(id) = queue.pop() {
        let Some(source) = changed.iter().find(|a| a.id == id).cloned() else { continue };
        for rule in &ex.impact_rules {
            let Some(current) = source.statuses.get(&rule.set) else { continue };
            if !rule.when.contains(current) || !done.insert((id.clone(), rule.id.clone())) {
                continue;
            }
            let Some(target_score) = score(&rule.then_set, &rule.then_value) else { continue };
            for link in ex.links.values().filter(|l| l.kind == rule.kind && l.from == id) {
                let dependant = match changed.iter().position(|a| a.id == link.to) {
                    Some(i) => changed.remove(i),
                    None => match ex.assets.get(&link.to) {
                        Some(a) => a.clone(),
                        None => continue,
                    },
                };
                let mut dependant = dependant;
                let now_value = dependant.statuses.get(&rule.then_set).cloned().unwrap_or_default();
                let worse = score(&rule.then_set, &now_value).is_none_or(|s| s > target_score);
                if worse {
                    dependant.statuses.insert(rule.then_set.clone(), rule.then_value.clone());
                    dependant.history.push(StatusChange {
                        set: rule.then_set.clone(),
                        value: rule.then_value.clone(),
                        at: now.to_owned(),
                    });
                    lines.push(format!(
                        "{} → {} (via {}, {})",
                        dependant.name,
                        label(&rule.then_set, &rule.then_value),
                        source.name,
                        label(&rule.set, current),
                    ));
                    queue.push(dependant.id.clone());
                }
                changed.push(dependant);
            }
        }
    }
    // Assets pulled in only to be checked, and left as they were, are not changes.
    changed.retain(|a| ex.assets.get(&a.id) != Some(a));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exercise::{
        Envelope, blank,
        command::{Command, decide},
        model::{ImpactRule, LinkKind},
    };
    use serde_json::json;

    fn run(ex: &mut Exercise, cmd: serde_json::Value) -> Vec<crate::exercise::Event> {
        let cmd: Command = serde_json::from_value(cmd).unwrap();
        let events = decide(ex, cmd, "2026-01-01T00:00:00+00:00").unwrap();
        for e in &events {
            ex.apply(&Envelope { seq: 0, at: "t".into(), actor: "x".into(), event: e.clone() });
        }
        events
    }

    /// grid → substation → hospital, and hospital → grid to make a cycle.
    fn chain() -> (Exercise, Vec<String>) {
        let mut ex = blank("Drill", "", "t0").unwrap();
        let ev = run(
            &mut ex,
            json!({"type": "add_assets", "assets": [
                {"name": "Grid", "type_id": "asset"},
                {"name": "Substation", "type_id": "asset"},
                {"name": "Hospital", "type_id": "asset", "statuses": {"operational": "critical"}},
                {"name": "Unrelated", "type_id": "asset"},
            ]}),
        );
        let crate::exercise::Event::AssetsUpserted { assets } = &ev[0] else { panic!() };
        let ids: Vec<String> = assets.iter().map(|a| a.id.clone()).collect();
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            run(&mut ex, json!({"type": "add_link", "from": ids[a], "to": ids[b], "kind": "depends_on"}));
        }
        ex.impact_rules = vec![ImpactRule {
            id: "r1".into(),
            kind: LinkKind::DependsOn,
            set: "operational".into(),
            when: vec!["offline".into(), "critical".into()],
            then_set: "operational".into(),
            then_value: "degraded".into(),
        }];
        (ex, ids)
    }

    #[test]
    fn spreads_downstream_without_improving_anything() {
        let (mut ex, ids) = chain();
        let events =
            run(&mut ex, json!({"type": "set_status", "ids": [ids[0]], "set": "operational", "value": "offline"}));
        assert_eq!(ex.assets[&ids[1]].statuses["operational"], "degraded");
        // The hospital was already worse (critical) than degraded: left alone.
        assert_eq!(ex.assets[&ids[2]].statuses["operational"], "critical");
        assert_eq!(ex.assets[&ids[3]].statuses["operational"], "healthy");
        // One batch: the change, its knock-on, and a log line explaining it.
        let crate::exercise::Event::AssetsUpserted { assets } = &events[0] else { panic!() };
        assert_eq!(assets.len(), 2);
        let crate::exercise::Event::Logged { entry } = &events[1] else { panic!() };
        assert!(entry.notes.contains("Substation → Degraded (via Grid, Offline)"), "{}", entry.notes);
    }

    #[test]
    fn degraded_does_not_trigger_the_rule_again() {
        let (mut ex, ids) = chain();
        // Substation becomes degraded, which is not in `when`, so the chain
        // stops there even though the hospital is downstream.
        let events =
            run(&mut ex, json!({"type": "set_status", "ids": [ids[0]], "set": "operational", "value": "critical"}));
        assert_eq!(events.len(), 2);
        assert_eq!(ex.assets[&ids[1]].statuses["operational"], "degraded");
    }

    #[test]
    fn a_cycle_terminates() {
        let (mut ex, ids) = chain();
        ex.impact_rules[0].when.push("degraded".into());
        ex.impact_rules[0].then_value = "critical".into();
        run(&mut ex, json!({"type": "set_status", "ids": [ids[0]], "set": "operational", "value": "offline"}));
        assert_eq!(ex.assets[&ids[1]].statuses["operational"], "critical");
        assert_eq!(ex.assets[&ids[2]].statuses["operational"], "critical");
        // Grid itself is already offline, which is worse than critical.
        assert_eq!(ex.assets[&ids[0]].statuses["operational"], "offline");
    }
}
