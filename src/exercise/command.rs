//! Commands: what a client asks for. `decide` checks a command against the
//! current exercise and turns it into events, or refuses it. Nothing here
//! mutates state; that is `Exercise::apply`'s job, after the events are stored.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

use super::{
    event::Event,
    model::{
        Annotation, Asset, Branding, Clock, Condition, Effect, Exercise, Expected, FieldKind, GeoPoint, GraphPoint,
        ImpactRule, Inject, Link, LinkKind, LogEntry, Metric, NewAsset, Phase, PhaseEntry, Poll, PollOption,
        PollResponse, PollState, Review, Severity, Shape, StatusChange, Taxonomy, Team, Threshold, Trigger, Verdict,
        Visibility,
    },
    taxonomy,
    view::Audience,
};
use crate::{
    error::ApiError,
    state::new_id,
    validate::{Checks, Validate},
};

pub const BULK_MAX: usize = 5000;
/// A logo is a base64 data URL: 256 KB of image is about 350,000 characters.
const LOGO_MAX: usize = 350_000;
const EXPECTED_MAX: usize = 20;
const REVIEW_MAX: usize = 20_000;

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum Command {
    Rename {
        name: String,
        #[serde(default)]
        #[ts(optional)]
        description: Option<String>,
    },
    SetTaxonomy {
        taxonomy: Taxonomy,
    },
    AddAssets {
        assets: Vec<NewAsset>,
    },
    EditAsset {
        id: String,
        patch: AssetPatch,
    },
    /// Set one status set on many assets at once.
    SetStatus {
        ids: Vec<String>,
        set: String,
        value: String,
    },
    RemoveAssets {
        ids: Vec<String>,
    },
    AddLink {
        from: String,
        to: String,
        #[serde(default)]
        #[ts(optional)]
        kind: Option<LinkKind>,
        #[serde(default)]
        #[ts(optional)]
        label: Option<String>,
    },
    RemoveLink {
        id: String,
    },
    AddInject {
        inject: NewInject,
    },
    RemoveInject {
        id: String,
    },
    FireInject {
        id: String,
    },
    AddThreshold {
        name: String,
        #[serde(default)]
        #[ts(optional)]
        sector: Option<String>,
        below_pct: u8,
        #[serde(default)]
        #[ts(optional)]
        severity: Option<Severity>,
    },
    RemoveThreshold {
        id: String,
    },
    Log {
        text: String,
        #[serde(default)]
        #[ts(optional)]
        asset: Option<String>,
        #[serde(default)]
        #[ts(optional)]
        notes: Option<String>,
    },
    AddAnnotation {
        shape: Shape,
        #[ts(type = "GeoJSON.Geometry")]
        geometry: serde_json::Value,
        #[serde(default)]
        #[ts(optional)]
        label: Option<String>,
        #[serde(default)]
        #[ts(optional)]
        color: Option<String>,
    },
    EditAnnotation {
        id: String,
        #[serde(default)]
        #[ts(optional, type = "GeoJSON.Geometry")]
        geometry: Option<serde_json::Value>,
        #[serde(default)]
        #[ts(optional)]
        label: Option<String>,
        #[serde(default)]
        #[ts(optional)]
        color: Option<String>,
    },
    RemoveAnnotations {
        ids: Vec<String>,
    },
    AddMetric {
        name: String,
        #[serde(default)]
        #[ts(optional)]
        value: Option<u8>,
    },
    EditMetric {
        id: String,
        #[serde(default)]
        #[ts(optional)]
        name: Option<String>,
        #[serde(default)]
        #[ts(optional)]
        value: Option<u8>,
    },
    RemoveMetric {
        id: String,
    },
    AddTeam {
        name: String,
        #[serde(default)]
        #[ts(optional)]
        color: Option<String>,
    },
    EditTeam {
        id: String,
        #[serde(default)]
        #[ts(optional)]
        name: Option<String>,
        #[serde(default)]
        #[ts(optional)]
        color: Option<String>,
    },
    /// Anything visible only to this team loses it from its audience; what
    /// was visible to this team alone becomes facilitator-only, not public.
    RemoveTeam {
        id: String,
    },
    StartClock,
    PauseClock,
    /// Jump the clock to a point, running or not.
    SetClock {
        minutes: f64,
    },
    /// Back to zero, stopped, with no phase begun: ready to run again.
    ResetClock,
    /// Replace the planned phases. New ones may have an empty id.
    SetPhases {
        phases: Vec<Phase>,
    },
    EnterPhase {
        id: String,
    },
    /// Rewrite an inject that has not fired yet.
    EditInject {
        id: String,
        inject: NewInject,
    },
    ArmInject {
        id: String,
        armed: bool,
    },
    AddPoll {
        poll: NewPoll,
    },
    RemovePoll {
        id: String,
    },
    OpenPoll {
        id: String,
    },
    ClosePoll {
        id: String,
    },
    /// A player's answer. The server fills in who is answering.
    Respond {
        poll: String,
        #[serde(default)]
        #[ts(optional)]
        option: Option<String>,
        #[serde(default)]
        #[ts(optional)]
        text: Option<String>,
        #[serde(skip)]
        #[ts(skip)]
        by: Option<Responder>,
    },
    SetImpactRules {
        rules: Vec<ImpactRule>,
    },
    SetBranding {
        branding: Branding,
    },
    /// Place many assets on the topology graph at once (after an auto-layout).
    SetGraphPositions {
        positions: BTreeMap<String, GraphPoint>,
    },
    /// Replace an inject's expected actions, fired or not. Actions that keep
    /// their id keep their assessment.
    SetExpected {
        inject: String,
        expected: Vec<NewExpected>,
    },
    /// Judge one expected action. A null verdict clears it.
    Assess {
        inject: String,
        action: String,
        verdict: Option<Verdict>,
        #[serde(default)]
        #[ts(optional)]
        note: Option<String>,
    },
    SetReview {
        review: Review,
    },
    /// Reverse the most recent change that has not been undone yet.
    Undo,
}

// Commands are validated against the exercise in `decide`, not on their own.
impl Validate for Command {
    fn check(&mut self, _: &mut Checks) {}
}

/// Absent leaves a field alone; `null` clears it; a value sets it.
#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AssetPatch {
    #[ts(optional)]
    pub name: Option<String>,
    #[ts(optional)]
    pub type_id: Option<String>,
    #[serde(default, deserialize_with = "some")]
    #[ts(optional)]
    pub geo: Option<Option<GeoPoint>>,
    #[serde(default, deserialize_with = "some")]
    #[ts(optional)]
    pub graph: Option<Option<GraphPoint>>,
    #[ts(optional)]
    pub notes: Option<String>,
    #[serde(default, deserialize_with = "some")]
    #[ts(optional)]
    pub color: Option<Option<String>>,
    #[ts(optional)]
    pub tags: Option<Vec<String>>,
    #[ts(optional)]
    pub visibility: Option<Visibility>,
    #[ts(optional)]
    pub fields: Option<BTreeMap<String, serde_json::Value>>,
}

/// Wrap a present value (including `null`) in `Some`, so a missing key and an
/// explicit `null` deserialize differently.
fn some<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NewInject {
    pub title: String,
    #[serde(default)]
    #[ts(optional, as = "Option<String>")]
    pub body: String,
    #[serde(default)]
    #[ts(optional, as = "Option<Severity>")]
    pub severity: Severity,
    #[serde(default)]
    #[ts(optional, as = "Option<Visibility>")]
    pub audience: Visibility,
    #[serde(default)]
    #[ts(optional, as = "Option<Vec<Effect>>")]
    pub effects: Vec<Effect>,
    #[serde(default)]
    #[ts(optional, as = "Option<String>")]
    pub notes: String,
    #[serde(default)]
    #[ts(optional, as = "Option<Trigger>")]
    pub trigger: Trigger,
    /// Left out, an edit keeps the inject's expected actions as they are.
    #[serde(default)]
    #[ts(optional)]
    pub expected: Option<Vec<NewExpected>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NewExpected {
    /// The id of an existing action to keep, or empty for a new one.
    #[serde(default)]
    #[ts(optional, as = "Option<String>")]
    pub id: String,
    pub text: String,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub team: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NewPoll {
    pub question: String,
    /// Option labels, in order.
    #[serde(default)]
    #[ts(optional, as = "Option<Vec<String>>")]
    pub options: Vec<String>,
    #[serde(default)]
    #[ts(optional, as = "Option<bool>")]
    pub allow_text: bool,
    #[serde(default)]
    #[ts(optional, as = "Option<Visibility>")]
    pub audience: Visibility,
    #[serde(default)]
    #[ts(optional)]
    pub duration_minutes: Option<u32>,
}

/// Who is answering a poll; set by the server from the player's session.
#[derive(Clone, Debug, PartialEq)]
pub struct Responder {
    pub participant: String,
    pub name: String,
    pub team: Option<String>,
}

const NAME_MAX: usize = 200;
const TEXT_MAX: usize = 4000;

fn not_found(what: &'static str) -> ApiError {
    ApiError::NotFound(what)
}

/// Turn a command into events, or explain why not.
pub fn decide(ex: &Exercise, cmd: Command, now: &str) -> Result<Vec<Event>, ApiError> {
    let mut c = Checks::default();
    let events = match cmd {
        Command::Rename { mut name, description } => {
            c.label("name", &mut name, NAME_MAX);
            let description = description.unwrap_or_else(|| ex.description.clone());
            c.max_len("description", &description, TEXT_MAX);
            vec![Event::Renamed { name, description }]
        }

        Command::SetTaxonomy { mut taxonomy } => {
            c.nested(&["taxonomy"], |c| taxonomy::check(&mut taxonomy, c));
            let types: HashSet<_> = taxonomy.asset_types.iter().map(|t| t.id.as_str()).collect();
            let orphaned = ex.assets.values().filter(|a| !types.contains(a.type_id.as_str())).count();
            if orphaned > 0 {
                c.fail("taxonomy", format!("{orphaned} asset(s) use an asset type this taxonomy removes"));
            }
            c.finish()?;
            let assets = ex
                .assets
                .values()
                .filter_map(|a| {
                    let statuses = fit_statuses(&taxonomy, &a.statuses);
                    let fields = fit_fields(&taxonomy, &a.fields);
                    (statuses != a.statuses || fields != a.fields).then(|| Asset { statuses, fields, ..a.clone() })
                })
                .collect();
            vec![Event::TaxonomyReplaced { taxonomy, assets }]
        }

        Command::AddAssets { mut assets } => {
            if assets.len() > BULK_MAX {
                c.fail("assets", format!("at most {BULK_MAX} per request"));
            }
            for (i, a) in assets.iter_mut().enumerate() {
                c.nested(&["assets", &i.to_string()], |c| check_new_asset(ex, &ex.taxonomy, a, c));
            }
            c.finish()?;
            let assets = assets.into_iter().map(|a| build_asset(&ex.taxonomy, a, now)).collect();
            vec![Event::AssetsUpserted { assets }]
        }

        Command::EditAsset { id, patch } => {
            let mut a = ex.assets.get(&id).ok_or(not_found("Asset not found"))?.clone();
            if let Some(mut name) = patch.name {
                c.label("name", &mut name, NAME_MAX);
                a.name = name;
            }
            if let Some(t) = patch.type_id {
                if ex.taxonomy.asset_types.iter().all(|x| x.id != t) {
                    c.fail("type_id", format!("unknown asset type {t:?}"));
                }
                a.type_id = t;
            }
            if let Some(geo) = patch.geo {
                if let Some(g) = geo {
                    c.coords(g.lat, g.lon);
                }
                a.geo = geo;
            }
            if let Some(graph) = patch.graph {
                a.graph = graph;
            }
            if let Some(notes) = patch.notes {
                c.max_len("notes", &notes, TEXT_MAX);
                a.notes = notes;
            }
            if let Some(color) = patch.color {
                c.color("color", color.as_deref());
                a.color = color;
            }
            if let Some(tags) = patch.tags {
                a.tags = clean_tags(tags, &mut c);
            }
            if let Some(v) = patch.visibility {
                check_visibility(ex, &v, "visibility", &mut c);
                a.visibility = v;
            }
            if let Some(fields) = patch.fields {
                check_fields(&ex.taxonomy, &fields, &mut c);
                a.fields = fields;
            }
            c.finish()?;
            vec![Event::AssetsUpserted { assets: vec![a] }]
        }

        Command::SetStatus { ids, set, value } => {
            check_status(&ex.taxonomy, &set, &value, "value", &mut c);
            c.finish()?;
            let mut changed = Vec::new();
            for id in &ids {
                let a = ex.assets.get(id).ok_or(not_found("Asset not found"))?;
                if a.statuses.get(&set) != Some(&value) {
                    let mut a = a.clone();
                    set_status(&mut a, &set, &value, now);
                    changed.push(a);
                }
            }
            if changed.is_empty() {
                return Ok(vec![]);
            }
            let impact = super::impact::propagate(ex, &mut changed, now);
            let mut events = vec![Event::AssetsUpserted { assets: changed }];
            events.extend(impact_log(impact, now));
            events
        }

        Command::RemoveAssets { ids } => {
            for id in &ids {
                if !ex.assets.contains_key(id) {
                    return Err(not_found("Asset not found"));
                }
            }
            let gone: HashSet<_> = ids.iter().collect();
            let links = ex
                .links
                .values()
                .filter(|l| gone.contains(&l.from) || gone.contains(&l.to))
                .map(|l| l.id.clone())
                .collect();
            vec![Event::AssetsRemoved { ids, links }]
        }

        Command::AddLink { from, to, kind, label } => {
            if !ex.assets.contains_key(&from) || !ex.assets.contains_key(&to) {
                return Err(ApiError::BadRequest("Both assets must exist".into()));
            }
            if from == to {
                return Err(ApiError::BadRequest("A link has to join two different assets".into()));
            }
            let label = label.unwrap_or_default();
            c.max_len("label", &label, NAME_MAX);
            c.finish()?;
            vec![Event::LinkUpserted { link: Link { id: new_id(), from, to, kind: kind.unwrap_or_default(), label } }]
        }

        Command::RemoveLink { id } => {
            ex.links.get(&id).ok_or(not_found("Link not found"))?;
            vec![Event::LinkRemoved { id }]
        }

        Command::AddInject { mut inject } => {
            c.nested(&["inject"], |c| check_inject(ex, &mut inject, c));
            let expected = merge_expected(ex, &[], inject.expected.take().unwrap_or_default(), &mut c);
            c.finish()?;
            let NewInject { title, body, severity, audience, effects, notes, trigger, .. } = inject;
            let inject = Inject {
                id: new_id(),
                title,
                body,
                severity,
                audience,
                effects,
                notes,
                created_at: now.to_owned(),
                fired_at: None,
                trigger,
                armed: true,
                expected,
            };
            vec![Event::InjectUpserted { inject }]
        }

        Command::EditInject { id, mut inject } => {
            let old = ex.injects.get(&id).ok_or(not_found("Inject not found"))?;
            if old.fired_at.is_some() {
                return Err(ApiError::Conflict("A fired inject cannot be changed"));
            }
            c.nested(&["inject"], |c| check_inject(ex, &mut inject, c));
            if matches!(&inject.trigger, Trigger::When { condition: Condition::InjectFired { inject } } if *inject == id)
            {
                c.fail("trigger", "an inject cannot wait for itself");
            }
            let expected = match inject.expected.take() {
                Some(new) => merge_expected(ex, &old.expected, new, &mut c),
                None => old.expected.clone(),
            };
            c.finish()?;
            let NewInject { title, body, severity, audience, effects, notes, trigger, .. } = inject;
            let inject = Inject { title, body, severity, audience, effects, notes, trigger, expected, ..old.clone() };
            vec![Event::InjectUpserted { inject }]
        }

        Command::SetExpected { inject, expected } => {
            let old = ex.injects.get(&inject).ok_or(not_found("Inject not found"))?;
            let expected = merge_expected(ex, &old.expected, expected, &mut c);
            c.finish()?;
            if expected == old.expected {
                return Ok(vec![]);
            }
            vec![Event::InjectUpserted { inject: Inject { expected, ..old.clone() } }]
        }

        Command::Assess { inject, action, verdict, note } => {
            let old = ex.injects.get(&inject).ok_or(not_found("Inject not found"))?;
            let mut i = old.clone();
            let e = i.expected.iter_mut().find(|e| e.id == action).ok_or(not_found("Expected action not found"))?;
            e.verdict = verdict;
            if let Some(note) = note {
                c.max_len("note", &note, TEXT_MAX);
                e.note = note.trim().to_owned();
            }
            c.finish()?;
            if &i == old {
                return Ok(vec![]);
            }
            vec![Event::InjectUpserted { inject: i }]
        }

        Command::SetReview { review } => {
            c.max_len("summary", &review.summary, REVIEW_MAX);
            c.max_len("strengths", &review.strengths, REVIEW_MAX);
            c.max_len("improvements", &review.improvements, REVIEW_MAX);
            c.finish()?;
            if review == ex.review {
                return Ok(vec![]);
            }
            vec![Event::ReviewChanged { review }]
        }

        Command::ArmInject { id, armed } => {
            let mut inject = ex.injects.get(&id).ok_or(not_found("Inject not found"))?.clone();
            if inject.armed == armed {
                return Ok(vec![]);
            }
            inject.armed = armed;
            vec![Event::InjectUpserted { inject }]
        }

        Command::RemoveInject { id } => {
            ex.injects.get(&id).ok_or(not_found("Inject not found"))?;
            vec![Event::InjectRemoved { id }]
        }

        Command::FireInject { id } => {
            let mut inject = ex.injects.get(&id).ok_or(not_found("Inject not found"))?.clone();
            if inject.fired_at.is_some() {
                return Err(ApiError::Conflict("Inject has already been fired"));
            }
            inject.fired_at = Some(now.to_owned());
            let mut assets: Vec<Asset> = Vec::new();
            let mut extra = Vec::new();
            let mut target = None;
            // Take an asset to change: the version an earlier effect already
            // changed, if any. An asset deleted since the inject was written
            // is skipped, not an error.
            let take = |assets: &mut Vec<Asset>, id: &str| -> Option<Asset> {
                match assets.iter().position(|a| a.id == id) {
                    Some(i) => Some(assets.remove(i)),
                    None => ex.assets.get(id).cloned(),
                }
            };
            for effect in &inject.effects {
                match effect {
                    Effect::SetStatus { asset, set, value } => {
                        let Some(mut a) = take(&mut assets, asset) else { continue };
                        target.get_or_insert_with(|| (a.id.clone(), a.name.clone()));
                        if ex.taxonomy.status_sets.iter().any(|s| &s.id == set && s.value(value).is_some()) {
                            set_status(&mut a, set, value, now);
                        }
                        assets.push(a);
                    }
                    Effect::CreateAsset { asset } => {
                        let mut new = asset.clone();
                        // The type may have gone since; fall back to the first one.
                        if ex.taxonomy.asset_types.iter().all(|t| t.id != new.type_id) {
                            new.type_id = ex.taxonomy.asset_types[0].id.clone();
                        }
                        assets.push(build_asset(&ex.taxonomy, new, now));
                    }
                    Effect::RevealAsset { asset, visibility } => {
                        let Some(mut a) = take(&mut assets, asset) else { continue };
                        a.visibility = visibility.clone();
                        assets.push(a);
                    }
                    Effect::OpenPoll { poll } => {
                        if let Some(p) = ex.polls.get(poll).filter(|p| p.state == PollState::Draft) {
                            extra.push(Event::PollUpserted { poll: open_poll(p.clone(), now) });
                        }
                    }
                    Effect::EnterPhase { phase } => {
                        if ex.phases.iter().any(|p| &p.id == phase) {
                            extra.push(enter_phase(ex, phase, now));
                        }
                    }
                }
            }
            let impact = super::impact::propagate(ex, &mut assets, now);
            let (asset, asset_name) = target.unzip();
            let entry = LogEntry {
                id: new_id(),
                at: now.to_owned(),
                text: format!("INJECT FIRED: {}", inject.title),
                asset,
                asset_name,
                notes: inject.body.clone(),
                auto: true,
                author: String::new(),
                // The log line is as visible as the inject that wrote it.
                visibility: inject.audience.clone(),
            };
            let mut events = vec![Event::InjectFired { inject, assets, entry }];
            events.extend(extra);
            events.extend(impact_log(impact, now));
            events
        }

        Command::StartClock => {
            if ex.clock.running_since.is_some() {
                return Ok(vec![]);
            }
            let clock = Clock { elapsed_ms: ex.clock.elapsed_ms, running_since: Some(now.to_owned()) };
            vec![Event::ClockChanged { clock }]
        }

        Command::PauseClock => {
            if ex.clock.running_since.is_none() {
                return Ok(vec![]);
            }
            vec![Event::ClockChanged { clock: Clock { elapsed_ms: ex.clock.elapsed_at(now), running_since: None } }]
        }

        Command::SetClock { minutes } => {
            if !(0.0..=100_000.0).contains(&minutes) {
                c.fail("minutes", "must be between 0 and 100000");
            }
            c.finish()?;
            let running_since = ex.clock.running_since.as_ref().map(|_| now.to_owned());
            vec![Event::ClockChanged { clock: Clock { elapsed_ms: (minutes * 60_000.0) as u64, running_since } }]
        }

        Command::ResetClock => {
            vec![Event::ClockChanged { clock: Clock::default() }, Event::PhaseLogReplaced { log: vec![] }]
        }

        Command::SetPhases { mut phases } => {
            if phases.len() > 30 {
                c.fail("phases", "at most 30 phases");
            }
            let mut seen = HashSet::new();
            for (i, p) in phases.iter_mut().enumerate() {
                c.nested(&["phases", &i.to_string()], |c| {
                    c.label("name", &mut p.name, 100);
                    c.color("color", Some(&p.color));
                    if p.target_minutes > 10_000 {
                        c.fail("target_minutes", "at most 10000");
                    }
                });
                if p.id.is_empty() {
                    p.id = new_id();
                }
                if !seen.insert(p.id.clone()) {
                    c.fail("phases", "duplicate phase id");
                }
            }
            let kept: HashSet<&str> = phases.iter().map(|p| p.id.as_str()).collect();
            for i in ex.injects.values().filter(|i| i.fired_at.is_none()) {
                let uses = match &i.trigger {
                    Trigger::Phase { phase, .. } => Some(phase),
                    _ => i.effects.iter().find_map(|e| match e {
                        Effect::EnterPhase { phase } => Some(phase),
                        _ => None,
                    }),
                };
                if uses.is_some_and(|p| !kept.contains(p.as_str())) {
                    c.fail("phases", format!("the inject {:?} still uses a phase this removes", i.title));
                }
            }
            c.finish()?;
            vec![Event::PhasesReplaced { phases }]
        }

        Command::EnterPhase { id } => {
            if ex.phases.iter().all(|p| p.id != id) {
                return Err(not_found("Phase not found"));
            }
            vec![enter_phase(ex, &id, now)]
        }

        Command::AddPoll { mut poll } => {
            c.label("question", &mut poll.question, 500);
            let mut labels = Vec::new();
            for (i, o) in poll.options.iter_mut().enumerate() {
                c.nested(&["options", &i.to_string()], |c| c.label("label", o, 200));
                labels.push(o.clone());
            }
            if labels.is_empty() && !poll.allow_text {
                c.fail("options", "give at least one option, or allow free text");
            }
            if labels.len() > 12 {
                c.fail("options", "at most 12 options");
            }
            if poll.duration_minutes.is_some_and(|m| m == 0 || m > 1440) {
                c.fail("duration_minutes", "must be between 1 and 1440");
            }
            check_visibility(ex, &poll.audience, "audience", &mut c);
            c.finish()?;
            let poll = Poll {
                id: new_id(),
                question: poll.question,
                options: labels.into_iter().map(|label| PollOption { id: new_id(), label }).collect(),
                allow_text: poll.allow_text,
                audience: poll.audience,
                state: PollState::Draft,
                duration_minutes: poll.duration_minutes,
                opened_at: None,
                closes_at: None,
                closed_at: None,
                responses: vec![],
                results: BTreeMap::new(),
            };
            vec![Event::PollUpserted { poll }]
        }

        Command::RemovePoll { id } => {
            ex.polls.get(&id).ok_or(not_found("Poll not found"))?;
            vec![Event::PollRemoved { id }]
        }

        Command::OpenPoll { id } => {
            let p = ex.polls.get(&id).ok_or(not_found("Poll not found"))?;
            if p.state == PollState::Open {
                return Ok(vec![]);
            }
            // Reopening a closed poll keeps its answers.
            vec![Event::PollUpserted { poll: open_poll(p.clone(), now) }]
        }

        Command::ClosePoll { id } => {
            let mut p = ex.polls.get(&id).ok_or(not_found("Poll not found"))?.clone();
            if p.state != PollState::Open {
                return Ok(vec![]);
            }
            p.state = PollState::Closed;
            p.closed_at = Some(now.to_owned());
            vec![Event::PollUpserted { poll: p }]
        }

        Command::Respond { poll, option, text, by } => {
            let by = by.ok_or(ApiError::Forbidden("Only players answer polls"))?;
            let mut p = ex.polls.get(&poll).ok_or(not_found("Poll not found"))?.clone();
            if !(Audience::Player { team: by.team.clone() }).sees(&p.audience) {
                return Err(not_found("Poll not found"));
            }
            if p.state != PollState::Open {
                return Err(ApiError::Conflict("This poll is not open"));
            }
            let text = text.unwrap_or_default().trim().to_owned();
            if option.as_ref().is_some_and(|o| p.options.iter().all(|x| &x.id != o)) {
                c.fail("option", "not one of this poll's options");
            }
            if !text.is_empty() && !p.allow_text {
                c.fail("text", "this poll takes no free text");
            }
            c.max_len("text", &text, 1000);
            if option.is_none() && text.is_empty() {
                c.fail("option", "choose an option or write an answer");
            }
            c.finish()?;
            p.responses.retain(|r| r.participant != by.participant);
            p.responses.push(PollResponse {
                participant: by.participant,
                name: by.name,
                team: by.team,
                option,
                text,
                at: now.to_owned(),
            });
            p.results = BTreeMap::new();
            for r in &p.responses {
                if let Some(o) = &r.option {
                    *p.results.entry(o.clone()).or_default() += 1;
                }
            }
            vec![Event::PollUpserted { poll: p }]
        }

        Command::SetImpactRules { mut rules } => {
            if rules.len() > 50 {
                c.fail("rules", "at most 50 rules");
            }
            for (i, r) in rules.iter_mut().enumerate() {
                c.nested(&["rules", &i.to_string()], |c| {
                    if r.when.is_empty() {
                        c.fail("when", "choose at least one status");
                    }
                    for w in &r.when {
                        check_status(&ex.taxonomy, &r.set, w, "when", c);
                    }
                    check_status(&ex.taxonomy, &r.then_set, &r.then_value, "then_value", c);
                });
                if r.id.is_empty() {
                    r.id = new_id();
                }
            }
            c.finish()?;
            vec![Event::ImpactRulesReplaced { rules }]
        }

        Command::AddThreshold { mut name, sector, below_pct, severity } => {
            c.label("name", &mut name, NAME_MAX);
            if below_pct > 100 {
                c.fail("below_pct", "must be between 0 and 100");
            }
            if let Some(s) = &sector {
                if ex.taxonomy.sectors.iter().all(|x| &x.id != s) {
                    c.fail("sector", format!("unknown sector {s:?}"));
                }
            }
            c.finish()?;
            let threshold = Threshold { id: new_id(), name, sector, below_pct, severity: severity.unwrap_or_default() };
            vec![Event::ThresholdUpserted { threshold }]
        }

        Command::RemoveThreshold { id } => {
            if ex.thresholds.iter().all(|t| t.id != id) {
                return Err(not_found("Threshold not found"));
            }
            vec![Event::ThresholdRemoved { id }]
        }

        Command::Log { mut text, asset, notes } => {
            c.label("text", &mut text, 500);
            let notes = notes.unwrap_or_default();
            c.max_len("notes", &notes, TEXT_MAX);
            let asset_name = match &asset {
                Some(id) => match ex.assets.get(id) {
                    Some(a) => Some(a.name.clone()),
                    None => {
                        c.fail("asset", "unknown asset");
                        None
                    }
                },
                None => None,
            };
            c.finish()?;
            let entry = LogEntry {
                id: new_id(),
                at: now.to_owned(),
                text,
                asset,
                asset_name,
                notes,
                auto: false,
                author: String::new(),
                visibility: Visibility::All,
            };
            vec![Event::Logged { entry }]
        }

        Command::AddAnnotation { shape, geometry, label, color } => {
            let label = label.unwrap_or_default();
            let color = color.unwrap_or_else(|| DEFAULT_ANNOTATION_COLOR.to_owned());
            c.max_len("label", &label, NAME_MAX);
            c.color("color", Some(&color));
            check_geometry(shape, &geometry, &mut c);
            c.finish()?;
            let annotation = Annotation { id: new_id(), shape, geometry, label, color, created_at: now.to_owned() };
            vec![Event::AnnotationUpserted { annotation }]
        }

        Command::EditAnnotation { id, geometry, label, color } => {
            let mut a = ex.annotations.get(&id).ok_or(not_found("Annotation not found"))?.clone();
            if let Some(g) = geometry {
                check_geometry(a.shape, &g, &mut c);
                a.geometry = g;
            }
            if let Some(l) = label {
                c.max_len("label", &l, NAME_MAX);
                a.label = l;
            }
            if let Some(col) = color {
                c.color("color", Some(&col));
                a.color = col;
            }
            c.finish()?;
            vec![Event::AnnotationUpserted { annotation: a }]
        }

        Command::RemoveAnnotations { ids } => {
            if ids.iter().any(|id| !ex.annotations.contains_key(id)) {
                return Err(not_found("Annotation not found"));
            }
            if ids.is_empty() { vec![] } else { vec![Event::AnnotationsRemoved { ids }] }
        }

        Command::AddMetric { mut name, value } => {
            c.label("name", &mut name, 100);
            let value = value.unwrap_or(100);
            if value > 100 {
                c.fail("value", "must be between 0 and 100");
            }
            c.finish()?;
            vec![Event::MetricUpserted { metric: Metric { id: new_id(), name, value } }]
        }

        Command::EditMetric { id, name, value } => {
            let mut m = ex.metrics.iter().find(|m| m.id == id).ok_or(not_found("Metric not found"))?.clone();
            if let Some(mut n) = name {
                c.label("name", &mut n, 100);
                m.name = n;
            }
            if let Some(v) = value {
                if v > 100 {
                    c.fail("value", "must be between 0 and 100");
                }
                m.value = v;
            }
            c.finish()?;
            vec![Event::MetricUpserted { metric: m }]
        }

        Command::RemoveMetric { id } => {
            if ex.metrics.iter().all(|m| m.id != id) {
                return Err(not_found("Metric not found"));
            }
            vec![Event::MetricRemoved { id }]
        }

        Command::AddTeam { mut name, color } => {
            c.label("name", &mut name, 100);
            let color = color.unwrap_or_else(|| TEAM_COLORS[ex.teams.len() % TEAM_COLORS.len()].to_owned());
            c.color("color", Some(&color));
            if ex.teams.len() >= 24 {
                c.fail("name", "at most 24 teams");
            }
            c.finish()?;
            vec![Event::TeamUpserted { team: Team { id: new_id(), name, color } }]
        }

        Command::EditTeam { id, name, color } => {
            let mut t = ex.teams.iter().find(|t| t.id == id).ok_or(not_found("Team not found"))?.clone();
            if let Some(mut n) = name {
                c.label("name", &mut n, 100);
                t.name = n;
            }
            if let Some(col) = color {
                c.color("color", Some(&col));
                t.color = col;
            }
            c.finish()?;
            vec![Event::TeamUpserted { team: t }]
        }

        Command::RemoveTeam { id } => {
            if ex.teams.iter().all(|t| t.id != id) {
                return Err(not_found("Team not found"));
            }
            let without = |v: &Visibility| -> Option<Visibility> {
                let Visibility::Teams { teams } = v else { return None };
                if !teams.contains(&id) {
                    return None;
                }
                let rest: Vec<String> = teams.iter().filter(|t| **t != id).cloned().collect();
                Some(if rest.is_empty() { Visibility::Facilitator } else { Visibility::Teams { teams: rest } })
            };
            let assets: Vec<Asset> = ex
                .assets
                .values()
                .filter_map(|a| without(&a.visibility).map(|visibility| Asset { visibility, ..a.clone() }))
                .collect();
            let mut events = vec![Event::TeamRemoved { id: id.clone() }];
            if !assets.is_empty() {
                events.push(Event::AssetsUpserted { assets });
            }
            for i in ex.injects.values() {
                let audience = without(&i.audience);
                let named = i.expected.iter().any(|e| e.team.as_ref() == Some(&id));
                if audience.is_some() || named {
                    let mut i = i.clone();
                    if let Some(audience) = audience {
                        i.audience = audience;
                    }
                    for e in i.expected.iter_mut().filter(|e| e.team.as_ref() == Some(&id)) {
                        e.team = None;
                    }
                    events.push(Event::InjectUpserted { inject: i });
                }
            }
            events
        }

        Command::SetBranding { mut branding } => {
            c.max_len("org", &branding.org, 100);
            branding.org = branding.org.trim().to_owned();
            c.color("accent", branding.accent.as_deref());
            if let Some(logo) = &branding.logo {
                let ok_type = ["png", "jpeg", "webp", "svg+xml", "gif"]
                    .iter()
                    .any(|t| logo.starts_with(&format!("data:image/{t};base64,")));
                if !ok_type {
                    c.fail("logo", "must be a PNG, JPEG, WebP, GIF or SVG image");
                } else if logo.len() > LOGO_MAX {
                    c.fail("logo", "must be under 256 KB");
                }
            }
            if let Some(p) = &branding.map_pack
                && (p.is_empty() || p.len() > 64 || !p.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-'))
            {
                c.fail("map_pack", "not a map pack id");
            }
            if let Some(b) = &mut branding.banner {
                c.nested(&["banner"], |c| {
                    c.label("text", &mut b.text, 120);
                    c.color("color", Some(&b.color));
                });
            }
            c.finish()?;
            if branding == ex.branding {
                return Ok(vec![]);
            }
            vec![Event::BrandingChanged { branding }]
        }

        Command::SetGraphPositions { positions } => {
            if positions.len() > BULK_MAX {
                c.fail("positions", format!("at most {BULK_MAX} per request"));
            }
            if positions.values().any(|p| !p.x.is_finite() || !p.y.is_finite() || p.x.abs() > 1e7 || p.y.abs() > 1e7) {
                c.fail("positions", "positions must be finite numbers");
            }
            c.finish()?;
            let mut assets = Vec::new();
            for (id, p) in positions {
                let a = ex.assets.get(&id).ok_or(not_found("Asset not found"))?;
                if a.graph != Some(p) {
                    assets.push(Asset { graph: Some(p), ..a.clone() });
                }
            }
            if assets.is_empty() { vec![] } else { vec![Event::AssetsUpserted { assets }] }
        }

        // `Live` handles undo itself, because it needs the history of changes.
        Command::Undo => return Err(ApiError::BadRequest("Undo is not available here".into())),
    };
    c.finish()?;
    Ok(events)
}

/// An inject's expected actions after an edit. Actions that keep their id
/// keep their verdict and note; the rest are new.
fn merge_expected(ex: &Exercise, old: &[Expected], new: Vec<NewExpected>, c: &mut Checks) -> Vec<Expected> {
    if new.len() > EXPECTED_MAX {
        c.fail("expected", format!("at most {EXPECTED_MAX} per inject"));
    }
    new.into_iter()
        .enumerate()
        .map(|(n, mut e)| {
            c.nested(&["expected", &n.to_string()], |c| {
                c.label("text", &mut e.text, 500);
                if let Some(t) = &e.team
                    && ex.teams.iter().all(|x| &x.id != t)
                {
                    c.fail("team", "unknown team");
                }
            });
            match old.iter().find(|o| !e.id.is_empty() && o.id == e.id) {
                Some(o) => Expected { text: e.text, team: e.team, ..o.clone() },
                None => Expected { id: new_id(), text: e.text, team: e.team, verdict: None, note: String::new() },
            }
        })
        .collect()
}

/// A facilitator-only log line listing knock-on changes, if there were any.
fn impact_log(lines: Vec<String>, now: &str) -> Option<Event> {
    if lines.is_empty() {
        return None;
    }
    let entry = LogEntry {
        id: new_id(),
        at: now.to_owned(),
        text: format!("IMPACT: {} asset(s) affected", lines.len()),
        asset: None,
        asset_name: None,
        notes: lines.join("\n"),
        auto: true,
        author: String::new(),
        // The lines can name assets players cannot see.
        visibility: Visibility::Facilitator,
    };
    Some(Event::Logged { entry })
}

fn open_poll(mut p: Poll, now: &str) -> Poll {
    p.state = PollState::Open;
    p.opened_at = Some(now.to_owned());
    p.closed_at = None;
    p.closes_at = p.duration_minutes.and_then(|m| {
        let start = chrono::DateTime::parse_from_rfc3339(now).ok()?;
        Some((start + chrono::Duration::minutes(i64::from(m))).to_rfc3339_opts(chrono::SecondsFormat::Micros, false))
    });
    p
}

fn enter_phase(ex: &Exercise, phase: &str, now: &str) -> Event {
    Event::PhaseEntered {
        entry: PhaseEntry { phase: phase.to_owned(), at: now.to_owned(), elapsed_ms: ex.clock.elapsed_at(now) },
    }
}

/// Checks shared by adding and editing an inject.
fn check_inject(ex: &Exercise, inject: &mut NewInject, c: &mut Checks) {
    c.label("title", &mut inject.title, NAME_MAX);
    c.max_len("body", &inject.body, TEXT_MAX);
    c.max_len("notes", &inject.notes, TEXT_MAX);
    check_visibility(ex, &inject.audience, "audience", c);
    for (i, e) in inject.effects.iter_mut().enumerate() {
        c.nested(&["effects", &i.to_string()], |c| match e {
            Effect::SetStatus { asset, set, value } => {
                if !ex.assets.contains_key(asset) {
                    c.fail("asset", "unknown asset");
                }
                check_status(&ex.taxonomy, set, value, "value", c);
            }
            Effect::CreateAsset { asset } => c.nested(&["asset"], |c| check_new_asset(ex, &ex.taxonomy, asset, c)),
            Effect::RevealAsset { asset, visibility } => {
                if !ex.assets.contains_key(asset) {
                    c.fail("asset", "unknown asset");
                }
                check_visibility(ex, visibility, "visibility", c);
            }
            Effect::OpenPoll { poll } => {
                if !ex.polls.contains_key(poll) {
                    c.fail("poll", "unknown poll");
                }
            }
            Effect::EnterPhase { phase } => {
                if ex.phases.iter().all(|p| &p.id != phase) {
                    c.fail("phase", "unknown phase");
                }
            }
        });
    }
    c.nested(&["trigger"], |c| match &inject.trigger {
        Trigger::Manual => {}
        Trigger::At { minutes } => {
            if !(0.0..=100_000.0).contains(minutes) {
                c.fail("minutes", "must be between 0 and 100000");
            }
        }
        Trigger::Phase { phase, delay_minutes } => {
            if ex.phases.iter().all(|p| &p.id != phase) {
                c.fail("phase", "unknown phase");
            }
            if !(0.0..=100_000.0).contains(delay_minutes) {
                c.fail("delay_minutes", "must be between 0 and 100000");
            }
        }
        Trigger::When { condition } => match condition {
            Condition::AssetStatus { asset, set, value } => {
                if !ex.assets.contains_key(asset) {
                    c.fail("asset", "unknown asset");
                }
                check_status(&ex.taxonomy, set, value, "value", c);
            }
            Condition::InjectFired { inject } => {
                if !ex.injects.contains_key(inject) {
                    c.fail("inject", "unknown inject");
                }
            }
            Condition::ThresholdBreached { threshold } => {
                if ex.thresholds.iter().all(|t| &t.id != threshold) {
                    c.fail("threshold", "unknown threshold");
                }
            }
            Condition::PollChoice { poll, option } => match ex.polls.get(poll) {
                None => c.fail("poll", "unknown poll"),
                Some(p) if p.options.iter().all(|o| &o.id != option) => {
                    c.fail("option", "not one of the poll's options")
                }
                Some(_) => {}
            },
        },
    });
}

pub const DEFAULT_ANNOTATION_COLOR: &str = "#00ff41";
const TEAM_COLORS: &[&str] = &["#3399ff", "#ffaa00", "#aa88ff", "#00ccaa", "#ff6688", "#cccc33"];

/// The most points one shape may have. A freehand stroke samples densely, but
/// thousands of points is a stuck mouse, not a drawing.
const MAX_POINTS: usize = 10_000;

/// A GeoJSON geometry of the kind `shape` draws, with every position on Earth.
fn check_geometry(shape: Shape, g: &serde_json::Value, c: &mut Checks) {
    use serde_json::Value;
    let want = match shape {
        Shape::Point => "Point",
        Shape::Line | Shape::Freehand => "LineString",
        Shape::Polygon | Shape::Rectangle | Shape::Circle => "Polygon",
    };
    if g.get("type").and_then(Value::as_str) != Some(want) {
        c.fail("geometry", format!("a {shape:?} needs a GeoJSON {want}"));
        return;
    }
    let position = |p: &Value| -> bool {
        let Some(xy) = p.as_array() else { return false };
        matches!(
            (xy.first().and_then(Value::as_f64), xy.get(1).and_then(Value::as_f64)),
            (Some(lon), Some(lat)) if (-180.0..=180.0).contains(&lon) && (-90.0..=90.0).contains(&lat)
        )
    };
    let line = |l: &Value, min: usize| -> Option<usize> {
        let pts = l.as_array()?;
        (pts.len() >= min && pts.iter().all(position)).then_some(pts.len())
    };
    let coords = g.get("coordinates");
    let points = match (want, coords) {
        ("Point", Some(p)) if position(p) => Some(1),
        ("LineString", Some(l)) => line(l, 2),
        ("Polygon", Some(Value::Array(rings))) if !rings.is_empty() => {
            rings.iter().map(|r| line(r, 4)).sum::<Option<usize>>()
        }
        _ => None,
    };
    match points {
        None => c.fail("geometry", format!("invalid {want} coordinates")),
        Some(n) if n > MAX_POINTS => c.fail("geometry", format!("at most {MAX_POINTS} points")),
        Some(_) => {}
    }
}

fn set_status(a: &mut Asset, set: &str, value: &str, now: &str) {
    if a.statuses.get(set).map(String::as_str) != Some(value) {
        a.history.push(StatusChange { set: set.to_owned(), value: value.to_owned(), at: now.to_owned() });
        a.statuses.insert(set.to_owned(), value.to_owned());
    }
}

/// Field values that still fit the taxonomy: known fields, and select values
/// that are still among the options. Anything else is dropped.
pub fn fit_fields(t: &Taxonomy, fields: &BTreeMap<String, serde_json::Value>) -> BTreeMap<String, serde_json::Value> {
    fields
        .iter()
        .filter(|(id, v)| {
            t.fields.iter().find(|f| &f.id == *id).is_some_and(|f| match (&f.kind, v) {
                (FieldKind::Select { options }, serde_json::Value::String(s)) => options.contains(s),
                (FieldKind::Select { .. }, _) => false,
                _ => true,
            })
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Every set in `t`, keeping current values that are still valid and falling
/// back to each set's default otherwise.
pub fn fit_statuses(t: &Taxonomy, current: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    t.status_sets
        .iter()
        .map(|s| {
            let v = current.get(&s.id).filter(|v| s.value(v).is_some()).unwrap_or(&s.default);
            (s.id.clone(), v.clone())
        })
        .collect()
}

pub fn build_asset(t: &Taxonomy, a: NewAsset, now: &str) -> Asset {
    let statuses = fit_statuses(t, &a.statuses);
    let history =
        statuses.iter().map(|(s, v)| StatusChange { set: s.clone(), value: v.clone(), at: now.into() }).collect();
    Asset {
        id: new_id(),
        name: a.name,
        type_id: a.type_id,
        geo: a.geo,
        graph: a.graph,
        statuses,
        history,
        fields: BTreeMap::new(),
        tags: a.tags,
        notes: a.notes,
        color: a.color,
        visibility: a.visibility,
        created_at: now.to_owned(),
    }
}

fn check_new_asset(ex: &Exercise, t: &Taxonomy, a: &mut NewAsset, c: &mut Checks) {
    c.label("name", &mut a.name, NAME_MAX);
    if t.asset_types.iter().all(|x| x.id != a.type_id) {
        c.fail("type_id", format!("unknown asset type {:?}", a.type_id));
    }
    if let Some(g) = a.geo {
        c.coords(g.lat, g.lon);
    }
    for (set, value) in &a.statuses {
        check_status(t, set, value, "statuses", c);
    }
    c.max_len("notes", &a.notes, TEXT_MAX);
    c.color("color", a.color.as_deref());
    a.tags = clean_tags(std::mem::take(&mut a.tags), c);
    check_visibility(ex, &a.visibility, "visibility", c);
}

fn check_status(t: &Taxonomy, set: &str, value: &str, field: &str, c: &mut Checks) {
    match t.status_sets.iter().find(|s| s.id == set) {
        None => c.fail(field, format!("unknown status set {set:?}")),
        Some(s) if s.value(value).is_none() => c.fail(field, format!("{value:?} is not a {} status", s.name)),
        Some(_) => {}
    }
}

fn check_visibility(ex: &Exercise, v: &Visibility, field: &str, c: &mut Checks) {
    if let Visibility::Teams { teams } = v {
        for t in teams {
            if ex.teams.iter().all(|x| &x.id != t) {
                c.fail(field, format!("unknown team {t:?}"));
            }
        }
    }
}

fn check_fields(t: &Taxonomy, fields: &BTreeMap<String, serde_json::Value>, c: &mut Checks) {
    use serde_json::Value;
    for (id, v) in fields {
        let Some(def) = t.fields.iter().find(|f| &f.id == id) else {
            c.fail("fields", format!("unknown field {id:?}"));
            continue;
        };
        let ok = match (&def.kind, v) {
            (_, Value::Null) => true,
            (FieldKind::Text | FieldKind::Url, Value::String(s)) => s.chars().count() <= TEXT_MAX,
            (FieldKind::Number, Value::Number(_)) | (FieldKind::Bool, Value::Bool(_)) => true,
            (FieldKind::Select { options }, Value::String(s)) => options.contains(s),
            _ => false,
        };
        if !ok {
            c.fail("fields", format!("invalid value for {}", def.label));
        }
    }
}

fn clean_tags(tags: Vec<String>, c: &mut Checks) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in tags {
        let t = t.trim().to_owned();
        if t.is_empty() || out.contains(&t) {
            continue;
        }
        c.max_len("tags", &t, 50);
        out.push(t);
    }
    if out.len() > 32 {
        c.fail("tags", "at most 32 tags");
    }
    out
}
