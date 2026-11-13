//! The v2 domain: an exercise and everything in it.
//!
//! Every type here derives `TS`, and `cargo test` writes it to
//! `web/src/lib/types/` so the frontend is typed against exactly what the
//! server sends.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// An authored exercise: its taxonomy, teams, assets, links and script.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Exercise {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub taxonomy: Taxonomy,
    #[serde(default)]
    pub teams: Vec<Team>,
    #[serde(default)]
    pub assets: IndexMap<String, Asset>,
    #[serde(default)]
    pub links: IndexMap<String, Link>,
    #[serde(default)]
    pub injects: IndexMap<String, Inject>,
    #[serde(default)]
    pub thresholds: Vec<Threshold>,
    #[serde(default)]
    pub log: Vec<LogEntry>,
    /// Shapes drawn on the map: zones, perimeters, routes, markers.
    #[serde(default)]
    pub annotations: IndexMap<String, Annotation>,
    /// Facilitator-defined percentage wheels (resources, comms readiness…).
    #[serde(default)]
    pub metrics: Vec<Metric>,
    pub created_at: String,
    pub updated_at: String,
}

// ───────────────────────────── taxonomy ─────────────────────────────

/// The per-exercise vocabulary: what kinds of assets exist, which statuses
/// they carry, and how they roll up into sectors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Taxonomy {
    pub asset_types: Vec<AssetType>,
    pub status_sets: Vec<StatusSet>,
    pub sectors: Vec<Sector>,
    #[serde(default)]
    pub fields: Vec<FieldDef>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AssetType {
    pub id: String,
    pub name: String,
    /// A Lucide icon name, e.g. `hospital` or `server`.
    pub icon: String,
    pub color: String,
}

/// An ordered set of statuses, e.g. Security: Clean … Compromised.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StatusSet {
    pub id: String,
    pub name: String,
    pub values: Vec<StatusValue>,
    /// The value a new asset starts with.
    pub default: String,
    /// How the canvas shows this set on a marker.
    pub display: StatusDisplay,
}

impl StatusSet {
    pub fn value(&self, id: &str) -> Option<&StatusValue> {
        self.values.iter().find(|v| v.id == id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum StatusDisplay {
    /// The marker's fill colour and glyph.
    Fill,
    /// The marker's ring colour and line style.
    Ring,
    /// Shown in the asset's details only.
    Detail,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StatusValue {
    pub id: String,
    pub label: String,
    pub color: String,
    /// A second cue besides colour, for colour-blind readers.
    pub glyph: String,
    /// 0 (worst) to 100 (best); sector health is the mean of these.
    pub score: u8,
}

/// A group of asset types with one health wheel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Sector {
    pub id: String,
    pub name: String,
    pub color: String,
    pub asset_types: Vec<String>,
    /// Which status set's scores drive the wheel.
    pub status_set: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FieldDef {
    pub id: String,
    pub label: String,
    pub kind: FieldKind,
    #[serde(default)]
    pub player_visible: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum FieldKind {
    Text,
    Number,
    Bool,
    Url,
    Select { options: Vec<String> },
}

// ───────────────────────────── people ─────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub color: String,
}

/// Who can see an asset or inject.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(tag = "mode", rename_all = "snake_case")]
#[ts(export)]
pub enum Visibility {
    #[default]
    All,
    Teams {
        teams: Vec<String>,
    },
    Facilitator,
}

// ───────────────────────────── the picture ─────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GeoPoint {
    pub lat: f64,
    pub lon: f64,
}

/// Position on the topology canvas.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GraphPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Asset {
    pub id: String,
    pub name: String,
    pub type_id: String,
    #[serde(default)]
    pub geo: Option<GeoPoint>,
    #[serde(default)]
    pub graph: Option<GraphPoint>,
    /// Status set id → status value id. Always holds every set in the taxonomy.
    pub statuses: BTreeMap<String, String>,
    #[serde(default)]
    pub history: Vec<StatusChange>,
    /// Custom field id → value.
    #[serde(default)]
    pub fields: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: String,
    /// Overrides the type colour on the canvas.
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub visibility: Visibility,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StatusChange {
    pub set: String,
    pub value: String,
    pub at: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum LinkKind {
    /// `to` cannot work without `from`.
    DependsOn,
    #[default]
    ConnectsTo,
    /// `to` is managed by `from`.
    ManagedBy,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Link {
    pub id: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub kind: LinkKind,
    #[serde(default)]
    pub label: String,
}

// ───────────────────────────── the script ─────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Severity {
    Info,
    #[default]
    Warning,
    Critical,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Inject {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub severity: Severity,
    #[serde(default)]
    pub audience: Visibility,
    #[serde(default)]
    pub effects: Vec<Effect>,
    /// For the facilitator only.
    #[serde(default)]
    pub notes: String,
    pub created_at: String,
    #[serde(default)]
    pub fired_at: Option<String>,
}

/// What firing an inject does to the picture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum Effect {
    SetStatus { asset: String, set: String, value: String },
    CreateAsset { asset: NewAsset },
}

/// The fields a caller supplies for a new asset; the server fills in the rest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NewAsset {
    pub name: String,
    pub type_id: String,
    #[serde(default)]
    #[ts(optional)]
    pub geo: Option<GeoPoint>,
    #[serde(default)]
    #[ts(optional)]
    pub graph: Option<GraphPoint>,
    /// Overrides of the status sets' defaults.
    #[serde(default)]
    #[ts(optional, as = "Option<BTreeMap<String, String>>")]
    pub statuses: BTreeMap<String, String>,
    #[serde(default)]
    #[ts(optional, as = "Option<String>")]
    pub notes: String,
    #[serde(default)]
    #[ts(optional)]
    pub color: Option<String>,
    #[serde(default)]
    #[ts(optional, as = "Option<Vec<String>>")]
    pub tags: Vec<String>,
    #[serde(default)]
    #[ts(optional, as = "Option<Visibility>")]
    pub visibility: Visibility,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Threshold {
    pub id: String,
    pub name: String,
    /// `None` watches every sector together.
    #[serde(default)]
    pub sector: Option<String>,
    pub below_pct: u8,
    #[serde(default)]
    pub severity: Severity,
}

/// The drawing tool a shape came from, so it can be edited the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Shape {
    Point,
    Line,
    Freehand,
    Polygon,
    Rectangle,
    Circle,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Annotation {
    pub id: String,
    pub shape: Shape,
    /// A GeoJSON geometry: Point, LineString or Polygon.
    #[ts(type = "GeoJSON.Geometry")]
    pub geometry: serde_json::Value,
    #[serde(default)]
    pub label: String,
    pub color: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Metric {
    pub id: String,
    pub name: String,
    /// 0–100.
    pub value: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LogEntry {
    pub id: String,
    pub at: String,
    pub text: String,
    #[serde(default)]
    pub asset: Option<String>,
    /// Kept beside the id so the entry still reads after the asset is deleted.
    #[serde(default)]
    pub asset_name: Option<String>,
    #[serde(default)]
    pub notes: String,
    /// Written by the system (an inject firing) rather than by a person.
    #[serde(default)]
    pub auto: bool,
    /// Who wrote it, for entries from players: "Dana (Legal)". Empty for the
    /// facilitator's own entries.
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub visibility: Visibility,
}
