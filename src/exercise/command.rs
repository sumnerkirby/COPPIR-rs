//! Commands: what a client asks for. `decide` checks a command against the
//! current exercise and turns it into events, or refuses it. Nothing here
//! mutates state; that is `Exercise::apply`'s job, after the events are stored.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

use super::{
    event::Event,
    model::{
        Annotation, Asset, Effect, Exercise, FieldKind, GeoPoint, GraphPoint, Inject, Link, LinkKind, LogEntry, Metric,
        NewAsset, Severity, Shape, StatusChange, Taxonomy, Team, Threshold, Visibility,
    },
    taxonomy,
};
use crate::{
    error::ApiError,
    state::new_id,
    validate::{Checks, Validate},
};

pub const BULK_MAX: usize = 5000;

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
                    (statuses != a.statuses).then(|| Asset { statuses, ..a.clone() })
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
            if changed.is_empty() { vec![] } else { vec![Event::AssetsUpserted { assets: changed }] }
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
            c.nested(&["inject"], |c| {
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
                        Effect::CreateAsset { asset } => {
                            c.nested(&["asset"], |c| check_new_asset(ex, &ex.taxonomy, asset, c))
                        }
                    });
                }
            });
            c.finish()?;
            let NewInject { title, body, severity, audience, effects, notes } = inject;
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
            };
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
            let mut target = None;
            for effect in &inject.effects {
                match effect {
                    // An asset deleted, or a status removed from the taxonomy,
                    // since the inject was written is skipped, not an error.
                    Effect::SetStatus { asset, set, value } => {
                        let current = assets.iter().position(|a| &a.id == asset);
                        let Some(mut a) = current.map(|i| assets.remove(i)).or_else(|| ex.assets.get(asset).cloned())
                        else {
                            continue;
                        };
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
                }
            }
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
            vec![Event::InjectFired { inject, assets, entry }]
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
                if let Some(audience) = without(&i.audience) {
                    events.push(Event::InjectUpserted { inject: Inject { audience, ..i.clone() } });
                }
            }
            events
        }

        // `Live` handles undo itself, because it needs the history of changes.
        Command::Undo => return Err(ApiError::BadRequest("Undo is not available here".into())),
    };
    c.finish()?;
    Ok(events)
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
