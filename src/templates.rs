//! Ready-made exercises, compiled into the binary. Each is a `.coppir` file
//! under `templates/`, written by `templates/generate.py`. Starting one makes
//! an ordinary exercise the facilitator can change freely.

use std::sync::OnceLock;

use serde::Serialize;
use ts_rs::TS;

use crate::exercise::Exercise;

const FILES: &[(&str, &str)] = &[
    ("ransomware", include_str!("../templates/ransomware.coppir")),
    ("bec", include_str!("../templates/bec.coppir")),
    ("supply-chain", include_str!("../templates/supply-chain.coppir")),
    ("utility-outage", include_str!("../templates/utility-outage.coppir")),
];

#[derive(Clone, Debug, Serialize, TS)]
#[ts(export)]
pub struct TemplateSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    #[ts(type = "number")]
    pub assets: usize,
    #[ts(type = "number")]
    pub injects: usize,
    #[ts(type = "number")]
    pub teams: usize,
    #[ts(type = "number")]
    pub phases: usize,
    #[ts(type = "number")]
    pub minutes: u32,
}

fn parsed() -> &'static Vec<(&'static str, Exercise)> {
    static CELL: OnceLock<Vec<(&'static str, Exercise)>> = OnceLock::new();
    CELL.get_or_init(|| {
        FILES
            .iter()
            .map(|(id, text)| {
                let file: crate::routes::v2::ExerciseFile =
                    serde_json::from_str(text).unwrap_or_else(|e| panic!("template {id} is malformed: {e}"));
                (*id, file.exercise)
            })
            .collect()
    })
}

pub fn list() -> Vec<TemplateSummary> {
    parsed()
        .iter()
        .map(|(id, ex)| TemplateSummary {
            id: (*id).to_owned(),
            name: ex.name.clone(),
            description: ex.description.clone(),
            assets: ex.assets.len(),
            injects: ex.injects.len(),
            teams: ex.teams.len(),
            phases: ex.phases.len(),
            minutes: ex.phases.iter().map(|p| p.target_minutes).sum(),
        })
        .collect()
}

pub fn get(id: &str) -> Option<Exercise> {
    parsed().iter().find(|(t, _)| *t == id).map(|(_, ex)| ex.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exercise::{
        Command, adopt,
        command::{NewExpected, NewInject, decide},
    };

    /// Every template adopts cleanly, and every inject in it would be accepted
    /// if written by hand: all its assets, phases, polls and conditions exist.
    #[test]
    fn every_template_is_valid() {
        assert_eq!(list().len(), FILES.len());
        for (id, ex) in parsed() {
            let ex = adopt(ex.clone(), None, "2026-01-01T00:00:00+00:00").unwrap_or_else(|e| panic!("{id}: {e:?}"));
            assert!(!ex.description.is_empty(), "{id} needs a description");
            assert!(ex.assets.len() >= 8, "{id} is too thin");
            assert!(!ex.phases.is_empty() && !ex.teams.is_empty() && !ex.polls.is_empty(), "{id}");
            for i in ex.injects.values() {
                let inject = NewInject {
                    title: i.title.clone(),
                    body: i.body.clone(),
                    severity: i.severity,
                    audience: i.audience.clone(),
                    effects: i.effects.clone(),
                    notes: i.notes.clone(),
                    trigger: i.trigger.clone(),
                    expected: Some(
                        i.expected
                            .iter()
                            .map(|e| NewExpected { id: e.id.clone(), text: e.text.clone(), team: e.team.clone() })
                            .collect(),
                    ),
                };
                decide(&ex, Command::EditInject { id: i.id.clone(), inject }, "2026-01-01T00:00:00+00:00")
                    .unwrap_or_else(|e| panic!("{id}: inject {:?} is invalid: {e:?}", i.title));
            }
            for l in ex.links.values() {
                assert!(ex.assets.contains_key(&l.from) && ex.assets.contains_key(&l.to), "{id}: dangling link");
            }
            let rules = Command::SetImpactRules { rules: ex.impact_rules.clone() };
            decide(&ex, rules, "2026-01-01T00:00:00+00:00").unwrap_or_else(|e| panic!("{id}: impact rules: {e:?}"));
            let branding = Command::SetBranding { branding: ex.branding.clone() };
            decide(&ex, branding, "2026-01-01T00:00:00+00:00").unwrap_or_else(|e| panic!("{id}: branding: {e:?}"));
        }
    }
}
