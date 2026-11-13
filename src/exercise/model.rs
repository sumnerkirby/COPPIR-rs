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
    /// The exercise clock, shared by everyone and kept by the server.
    #[serde(default)]
    pub clock: Clock,
    /// The planned stages, in order.
    #[serde(default)]
    pub phases: Vec<Phase>,
    /// Each time a phase began, in order. The last entry is the current phase.
    #[serde(default)]
    pub phase_log: Vec<PhaseEntry>,
    #[serde(default)]
    pub polls: IndexMap<String, Poll>,
    /// How a status change spreads along links.
    #[serde(default)]
    pub impact_rules: Vec<ImpactRule>,
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
    /// When the server fires it on its own. `Manual` waits for the facilitator.
    #[serde(default)]
    pub trigger: Trigger,
    /// A disarmed inject keeps its trigger but the server will not fire it.
    /// Undoing an automatic firing disarms it, so it does not fire again at once.
    #[serde(default = "yes")]
    pub armed: bool,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum Trigger {
    #[default]
    Manual,
    /// At a point on the exercise clock.
    At { minutes: f64 },
    /// A while after a phase begins.
    Phase { phase: String, delay_minutes: f64 },
    /// As soon as something becomes true.
    When { condition: Condition },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum Condition {
    /// An asset reaches a status.
    AssetStatus { asset: String, set: String, value: String },
    /// Another inject has fired: chains injects into a sequence.
    InjectFired { inject: String },
    /// An alert threshold is breached.
    ThresholdBreached { threshold: String },
    /// A poll has closed and this option won (or tied for first).
    PollChoice { poll: String, option: String },
}

/// The exercise clock. Elapsed time is `elapsed_ms`, plus the time since
/// `running_since` while it runs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Clock {
    #[ts(type = "number")]
    pub elapsed_ms: u64,
    #[serde(default)]
    pub running_since: Option<String>,
}

impl Clock {
    /// Elapsed exercise time at wall time `now` (RFC 3339).
    pub fn elapsed_at(&self, now: &str) -> u64 {
        let Some(since) = &self.running_since else { return self.elapsed_ms };
        let (Ok(a), Ok(b)) = (chrono::DateTime::parse_from_rfc3339(since), chrono::DateTime::parse_from_rfc3339(now))
        else {
            return self.elapsed_ms;
        };
        self.elapsed_ms + u64::try_from((b - a).num_milliseconds()).unwrap_or(0)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Phase {
    pub id: String,
    pub name: String,
    pub color: String,
    /// How long it is planned to last; drives the timeline, not the clock.
    pub target_minutes: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PhaseEntry {
    pub phase: String,
    pub at: String,
    #[ts(type = "number")]
    pub elapsed_ms: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PollState {
    #[default]
    Draft,
    Open,
    Closed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PollOption {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PollResponse {
    /// The participant's session id: one response each, the latest counts.
    pub participant: String,
    pub name: String,
    #[serde(default)]
    pub team: Option<String>,
    #[serde(default)]
    pub option: Option<String>,
    #[serde(default)]
    pub text: String,
    pub at: String,
}

/// A decision put to the players.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Poll {
    pub id: String,
    pub question: String,
    #[serde(default)]
    pub options: Vec<PollOption>,
    /// Whether players can add a free-text answer.
    #[serde(default)]
    pub allow_text: bool,
    #[serde(default)]
    pub audience: Visibility,
    #[serde(default)]
    pub state: PollState,
    /// Closes itself this long after opening; `None` waits for the facilitator.
    #[serde(default)]
    pub duration_minutes: Option<u32>,
    #[serde(default)]
    pub opened_at: Option<String>,
    #[serde(default)]
    pub closes_at: Option<String>,
    #[serde(default)]
    pub closed_at: Option<String>,
    #[serde(default)]
    pub responses: Vec<PollResponse>,
    /// Option id → count, kept current with `responses`.
    #[serde(default)]
    pub results: std::collections::BTreeMap<String, u32>,
}

impl Poll {
    /// The options with the most votes, if anyone voted.
    pub fn winners(&self) -> Vec<&str> {
        let max = self.results.values().copied().max().unwrap_or(0);
        if max == 0 {
            return vec![];
        }
        self.results.iter().filter(|(_, n)| **n == max).map(|(k, _)| k.as_str()).collect()
    }
}

/// When an asset's `set` status becomes one of `when`, everything that relies
/// on it through a link of this kind gets `then_value` in `then_set`, unless
/// it is already worse.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ImpactRule {
    pub id: String,
    pub kind: LinkKind,
    pub set: String,
    pub when: Vec<String>,
    pub then_set: String,
    pub then_value: String,
}

/// What firing an inject does to the picture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum Effect {
    SetStatus {
        asset: String,
        set: String,
        value: String,
    },
    CreateAsset {
        asset: NewAsset,
    },
    /// Change who can see an asset: bring a hidden one into play.
    RevealAsset {
        asset: String,
        visibility: Visibility,
    },
    OpenPoll {
        poll: String,
    },
    EnterPhase {
        phase: String,
    },
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
