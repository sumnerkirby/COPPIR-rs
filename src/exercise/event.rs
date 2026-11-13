//! Events: the append-only record of everything that happened to an exercise.
//!
//! Events carry *resulting state*, not instructions. `AssetsUpserted` holds
//! the whole asset after the change, so applying an event is a plain upsert or
//! remove. That keeps replay deterministic and keeps the TypeScript reducer in
//! the browser trivial enough that it cannot drift from this one.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::model::{
    Annotation, Asset, Branding, Clock, Exercise, ImpactRule, Inject, Link, LogEntry, Metric, Phase, PhaseEntry, Poll,
    Taxonomy, Team, Threshold,
};

// Events are built, serialized and dropped; none lives in a large collection
// for long, so the gap between the smallest and largest variant costs nothing.
// The one that holds a whole exercise is boxed anyway.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum Event {
    /// The first event of every exercise: its complete starting state.
    Created {
        // Boxed: every other event is small, and this one holds everything.
        exercise: Box<Exercise>,
    },
    Renamed {
        name: String,
        description: String,
    },
    /// Assets whose statuses had to be remapped to fit come along with it.
    TaxonomyReplaced {
        taxonomy: Taxonomy,
        assets: Vec<Asset>,
    },
    AssetsUpserted {
        assets: Vec<Asset>,
    },
    /// Links that touched the removed assets go with them.
    AssetsRemoved {
        ids: Vec<String>,
        links: Vec<String>,
    },
    LinkUpserted {
        link: Link,
    },
    LinkRemoved {
        id: String,
    },
    InjectUpserted {
        inject: Inject,
    },
    InjectRemoved {
        id: String,
    },
    /// One fact for the after-action report: the inject, every asset it
    /// changed or created, and the log line it wrote.
    InjectFired {
        inject: Inject,
        assets: Vec<Asset>,
        entry: LogEntry,
    },
    ThresholdUpserted {
        threshold: Threshold,
    },
    ThresholdRemoved {
        id: String,
    },
    Logged {
        entry: LogEntry,
    },
    /// Only produced by undoing a `Logged` or `InjectFired`.
    LogRemoved {
        id: String,
    },
    AnnotationUpserted {
        annotation: Annotation,
    },
    AnnotationsRemoved {
        ids: Vec<String>,
    },
    MetricUpserted {
        metric: Metric,
    },
    MetricRemoved {
        id: String,
    },
    TeamUpserted {
        team: Team,
    },
    /// Assets and injects that named the team are re-sent without it, as
    /// ordinary upserts in the same batch.
    TeamRemoved {
        id: String,
    },
    ClockChanged {
        clock: Clock,
    },
    PhasesReplaced {
        phases: Vec<Phase>,
    },
    PhaseEntered {
        entry: PhaseEntry,
    },
    PhaseLogReplaced {
        log: Vec<PhaseEntry>,
    },
    PollUpserted {
        poll: Poll,
    },
    PollRemoved {
        id: String,
    },
    ImpactRulesReplaced {
        rules: Vec<ImpactRule>,
    },
    BrandingChanged {
        branding: Branding,
    },
    /// Several changes delivered as one. Sent to players and observers, whose
    /// view of a change is computed for them rather than replayed from the log.
    Batch {
        events: Vec<Event>,
    },
    /// Marks that the events after it, in the same batch, reverse the changes
    /// made at `seqs`. Changes no state; it is there for the after-action log.
    Undone {
        #[ts(type = "number[]")]
        seqs: Vec<u64>,
    },
}

/// An event as stored and sent: numbered, timestamped, attributed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Envelope {
    #[ts(type = "number")]
    pub seq: u64,
    pub at: String,
    pub actor: String,
    pub event: Event,
}

impl Exercise {
    /// Apply one event. Infallible by design: commands are checked before
    /// their events exist, so anything in the log is already known-good.
    pub fn apply(&mut self, env: &Envelope) {
        self.updated_at.clone_from(&env.at);
        match &env.event {
            Event::Created { exercise } => {
                *self = (**exercise).clone();
                self.updated_at.clone_from(&env.at);
            }
            Event::Renamed { name, description } => {
                self.name.clone_from(name);
                self.description.clone_from(description);
            }
            Event::TaxonomyReplaced { taxonomy, assets } => {
                self.taxonomy = taxonomy.clone();
                self.upsert_assets(assets);
            }
            Event::AssetsUpserted { assets } => self.upsert_assets(assets),
            Event::AssetsRemoved { ids, links } => {
                for id in ids {
                    self.assets.shift_remove(id);
                }
                for id in links {
                    self.links.shift_remove(id);
                }
            }
            Event::LinkUpserted { link } => {
                self.links.insert(link.id.clone(), link.clone());
            }
            Event::LinkRemoved { id } => {
                self.links.shift_remove(id);
            }
            Event::InjectUpserted { inject } => {
                self.injects.insert(inject.id.clone(), inject.clone());
            }
            Event::InjectRemoved { id } => {
                self.injects.shift_remove(id);
            }
            Event::InjectFired { inject, assets, entry } => {
                self.injects.insert(inject.id.clone(), inject.clone());
                self.upsert_assets(assets);
                self.log.push(entry.clone());
            }
            Event::ThresholdUpserted { threshold } => match self.thresholds.iter_mut().find(|t| t.id == threshold.id) {
                Some(t) => *t = threshold.clone(),
                None => self.thresholds.push(threshold.clone()),
            },
            Event::ThresholdRemoved { id } => self.thresholds.retain(|t| &t.id != id),
            Event::Logged { entry } => self.log.push(entry.clone()),
            Event::LogRemoved { id } => self.log.retain(|e| &e.id != id),
            Event::AnnotationUpserted { annotation } => {
                self.annotations.insert(annotation.id.clone(), annotation.clone());
            }
            Event::AnnotationsRemoved { ids } => {
                for id in ids {
                    self.annotations.shift_remove(id);
                }
            }
            Event::MetricUpserted { metric } => match self.metrics.iter_mut().find(|m| m.id == metric.id) {
                Some(m) => *m = metric.clone(),
                None => self.metrics.push(metric.clone()),
            },
            Event::MetricRemoved { id } => self.metrics.retain(|m| &m.id != id),
            Event::TeamUpserted { team } => match self.teams.iter_mut().find(|t| t.id == team.id) {
                Some(t) => *t = team.clone(),
                None => self.teams.push(team.clone()),
            },
            Event::TeamRemoved { id } => self.teams.retain(|t| &t.id != id),
            Event::ClockChanged { clock } => self.clock = clock.clone(),
            Event::PhasesReplaced { phases } => self.phases.clone_from(phases),
            Event::PhaseEntered { entry } => self.phase_log.push(entry.clone()),
            Event::PhaseLogReplaced { log } => self.phase_log.clone_from(log),
            Event::PollUpserted { poll } => {
                self.polls.insert(poll.id.clone(), poll.clone());
            }
            Event::PollRemoved { id } => {
                self.polls.shift_remove(id);
            }
            Event::ImpactRulesReplaced { rules } => self.impact_rules.clone_from(rules),
            Event::BrandingChanged { branding } => self.branding = branding.clone(),
            Event::Batch { events } => {
                for event in events {
                    self.apply(&Envelope {
                        seq: env.seq,
                        at: env.at.clone(),
                        actor: env.actor.clone(),
                        event: event.clone(),
                    });
                }
            }
            Event::Undone { .. } => {}
        }
    }

    fn upsert_assets(&mut self, assets: &[Asset]) {
        for a in assets {
            self.assets.insert(a.id.clone(), a.clone());
        }
    }

    /// Rebuild an exercise from its full log.
    pub fn replay<'a>(events: impl IntoIterator<Item = &'a Envelope>) -> Option<Self> {
        let mut iter = events.into_iter();
        let first = iter.next()?;
        let Event::Created { exercise } = &first.event else { return None };
        let mut ex = (**exercise).clone();
        ex.apply(first);
        for env in iter {
            ex.apply(env);
        }
        Some(ex)
    }
}
