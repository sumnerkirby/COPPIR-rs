//! The domain types: what is stored, and the request bodies that create it.
//!
//! Enum-style fields (`status`, `op_status`, `severity`, `sector`) are real
//! enums, so an unknown value fails to deserialize and never reaches state.
//! Everything else a request carries is checked by its [`Validate`] impl
//! before a handler sees it.

use serde::{Deserialize, Serialize};

use crate::validate::{Checks, Validate};

// ───────────────────────────── enums ─────────────────────────────

/// Security status: how far an asset is from a known-good state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Status {
    #[default]
    Clean,
    Monitored,
    Contained,
    #[serde(rename = "Under Investigation")]
    UnderInvestigation,
    Compromised,
}

/// Operational status: whether the asset is still doing its job.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OpStatus {
    #[default]
    Healthy,
    Degraded,
    Critical,
    Offline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    #[default]
    Warning,
    Critical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sector {
    All,
    Medical,
    Government,
    Power,
    Emergency,
    Civilian,
    Financial,
}

// ───────────────────────────── stored records ─────────────────────────────
//
// Every optional-looking field carries `#[serde(default)]` so scenarios saved
// by the Python version, which were looser about what a pin held, still load.

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusChange {
    pub status: Status,
    pub timestamp: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pin {
    pub id: String,
    pub name: String,
    #[serde(default = "default_category")]
    pub category: String,
    pub lat: f64,
    pub lon: f64,
    #[serde(default)]
    pub status: Status,
    #[serde(default)]
    pub op_status: OpStatus,
    #[serde(default)]
    pub pin_color: Option<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub pinned_at: String,
    #[serde(default)]
    pub status_history: Vec<StatusChange>,
}

impl Pin {
    /// Set the security status, recording it in the history only if it changed.
    /// The history is a record of changes, not of writes.
    pub fn set_status(&mut self, status: Status, at: &str) {
        if self.status != status {
            self.status_history.push(StatusChange { status, timestamp: at.to_owned() });
        }
        self.status = status;
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    pub from_pid: String,
    pub to_pid: String,
    #[serde(default)]
    pub label: String,
    #[serde(default = "default_edge_type")]
    pub edge_type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inject {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub severity: Severity,
    #[serde(default)]
    pub target_pid: Option<String>,
    #[serde(default)]
    pub target_status: Option<Status>,
    #[serde(default)]
    pub new_pin: Option<NewPin>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub triggered_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Threshold {
    pub id: String,
    pub name: String,
    pub sector: Sector,
    pub below_pct: u8,
    #[serde(default)]
    pub severity: Severity,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogRecord {
    pub id: String,
    pub timestamp: String,
    pub action: String,
    #[serde(default)]
    pub asset_name: Option<String>,
    #[serde(default)]
    pub asset_pid: Option<String>,
    #[serde(default)]
    pub notes: String,
    /// True for entries the system wrote (inject triggers), false for manual ones.
    #[serde(default)]
    pub auto: bool,
}

fn default_category() -> String {
    "Asset".into()
}
fn default_edge_type() -> String {
    "connection".into()
}
fn default_new_pin_status() -> Status {
    Status::UnderInvestigation
}
fn default_below_pct() -> i64 {
    50
}

// ───────────────────────────── request bodies ─────────────────────────────

const NAME_MAX: usize = 200;
const CATEGORY_MAX: usize = 100;
const NOTES_MAX: usize = 2000;

#[derive(Debug, Deserialize)]
pub struct PinIn {
    pub name: String,
    #[serde(default = "default_category")]
    pub category: String,
    pub lat: f64,
    pub lon: f64,
    #[serde(default)]
    pub status: Status,
    #[serde(default)]
    pub op_status: OpStatus,
    #[serde(default)]
    pub pin_color: Option<String>,
    #[serde(default)]
    pub notes: String,
}

impl Validate for PinIn {
    fn check(&mut self, c: &mut Checks) {
        c.label("name", &mut self.name, NAME_MAX);
        c.label("category", &mut self.category, CATEGORY_MAX);
        c.coords(self.lat, self.lon);
        c.color("pin_color", self.pin_color.as_deref());
        c.max_len("notes", &self.notes, NOTES_MAX);
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct PinUpdate {
    pub name: Option<String>,
    pub category: Option<String>,
    pub status: Option<Status>,
    pub op_status: Option<OpStatus>,
    pub pin_color: Option<String>,
    pub notes: Option<String>,
}

impl Validate for PinUpdate {
    fn check(&mut self, c: &mut Checks) {
        if let Some(name) = &mut self.name {
            c.label("name", name, NAME_MAX);
        }
        if let Some(category) = &mut self.category {
            c.label("category", category, CATEGORY_MAX);
        }
        c.color("pin_color", self.pin_color.as_deref());
        if let Some(notes) = &self.notes {
            c.max_len("notes", notes, NOTES_MAX);
        }
    }
}

/// A pin an inject spawns when it fires.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewPin {
    pub name: String,
    #[serde(default = "default_category")]
    pub category: String,
    pub lat: f64,
    pub lon: f64,
    #[serde(default = "default_new_pin_status")]
    pub status: Status,
}

impl Validate for NewPin {
    fn check(&mut self, c: &mut Checks) {
        c.label("name", &mut self.name, NAME_MAX);
        c.label("category", &mut self.category, CATEGORY_MAX);
        c.coords(self.lat, self.lon);
    }
}

#[derive(Debug, Deserialize)]
pub struct BulkPinItem {
    pub name: String,
    #[serde(default = "default_category")]
    pub category: String,
    pub lat: f64,
    pub lon: f64,
}

impl Validate for BulkPinItem {
    fn check(&mut self, c: &mut Checks) {
        c.label("name", &mut self.name, NAME_MAX);
        c.label("category", &mut self.category, CATEGORY_MAX);
        c.coords(self.lat, self.lon);
    }
}

/// The most pins one bulk request may create. An Overpass search is capped at
/// a few hundred, so this only stops a runaway client.
pub const BULK_MAX: usize = 5000;

#[derive(Debug, Deserialize)]
pub struct BulkCreate {
    #[serde(default)]
    pub items: Vec<BulkPinItem>,
    #[serde(default)]
    pub status: Status,
    #[serde(default)]
    pub op_status: OpStatus,
    #[serde(default)]
    pub pin_color: Option<String>,
}

impl Validate for BulkCreate {
    fn check(&mut self, c: &mut Checks) {
        if self.items.len() > BULK_MAX {
            c.fail("items", format!("at most {BULK_MAX} items per request"));
        }
        for (i, item) in self.items.iter_mut().enumerate() {
            c.nested(&["items", &i.to_string()], |c| item.check(c));
        }
        c.color("pin_color", self.pin_color.as_deref());
    }
}

#[derive(Debug, Deserialize)]
pub struct BulkStatusReq {
    pub status: Option<Status>,
    pub op_status: Option<OpStatus>,
    pub category: Option<String>,
}

impl Validate for BulkStatusReq {
    fn check(&mut self, c: &mut Checks) {
        if self.status.is_none() && self.op_status.is_none() {
            c.fail("status", "give a status, an op_status, or both");
        }
        if let Some(cat) = &self.category {
            c.max_len("category", cat, CATEGORY_MAX);
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct EdgeIn {
    pub from_pid: String,
    pub to_pid: String,
    #[serde(default)]
    pub label: String,
    #[serde(default = "default_edge_type")]
    pub edge_type: String,
}

impl Validate for EdgeIn {
    fn check(&mut self, c: &mut Checks) {
        c.max_len("label", &self.label, NAME_MAX);
        c.label("edge_type", &mut self.edge_type, 50);
    }
}

#[derive(Debug, Deserialize)]
pub struct InjectIn {
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub severity: Severity,
    pub target_pid: Option<String>,
    pub target_status: Option<Status>,
    pub new_pin: Option<NewPin>,
}

impl Validate for InjectIn {
    fn check(&mut self, c: &mut Checks) {
        c.label("title", &mut self.title, NAME_MAX);
        c.max_len("description", &self.description, NOTES_MAX);
        if let Some(np) = &mut self.new_pin {
            c.nested(&["new_pin"], |c| np.check(c));
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LogEntryIn {
    pub action: String,
    pub asset_name: Option<String>,
    pub asset_pid: Option<String>,
    #[serde(default)]
    pub notes: String,
}

impl Validate for LogEntryIn {
    fn check(&mut self, c: &mut Checks) {
        c.label("action", &mut self.action, 500);
        if let Some(n) = &self.asset_name {
            c.max_len("asset_name", n, NAME_MAX);
        }
        c.max_len("notes", &self.notes, NOTES_MAX);
    }
}

#[derive(Debug, Deserialize)]
pub struct ThresholdIn {
    pub name: String,
    pub sector: Sector,
    // Signed and wide so -1 or 101 reach the range check and get a message
    // about the range, rather than a deserializer complaint about the type.
    #[serde(default = "default_below_pct")]
    pub below_pct: i64,
    #[serde(default)]
    pub severity: Severity,
}

impl Validate for ThresholdIn {
    fn check(&mut self, c: &mut Checks) {
        c.label("name", &mut self.name, NAME_MAX);
        if !(0..=100).contains(&self.below_pct) {
            c.fail("below_pct", "must be between 0 and 100");
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ScenarioReq {
    pub name: String,
}

impl Validate for ScenarioReq {
    fn check(&mut self, c: &mut Checks) {
        c.label("name", &mut self.name, 64);
    }
}

#[derive(Debug, Deserialize)]
pub struct GeoReq {
    pub query: String,
}

impl Validate for GeoReq {
    fn check(&mut self, c: &mut Checks) {
        c.label("query", &mut self.query, 300);
    }
}

#[derive(Debug, Deserialize)]
pub struct OsmReq {
    pub lat: f64,
    pub lon: f64,
    #[serde(default = "default_radius")]
    pub radius_mi: f64,
    pub category: String,
}

fn default_radius() -> f64 {
    15.0
}

impl Validate for OsmReq {
    fn check(&mut self, c: &mut Checks) {
        c.coords(self.lat, self.lon);
        if !(self.radius_mi > 0.0 && self.radius_mi <= 100.0) {
            c.fail("radius_mi", "must be greater than 0 and at most 100");
        }
    }
}
