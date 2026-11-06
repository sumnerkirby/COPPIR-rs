//! The v1 scenario file format, read-only. Scenarios saved by the v1 Rust app
//! and by the original Python app share it. Everything here exists to be
//! imported into a v2 exercise by `import_v1`.

use indexmap::IndexMap;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Default)]
pub enum Status {
    #[default]
    Clean,
    Monitored,
    Contained,
    #[serde(rename = "Under Investigation")]
    UnderInvestigation,
    Compromised,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Default)]
pub enum OpStatus {
    #[default]
    Healthy,
    Degraded,
    Critical,
    Offline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    #[default]
    Warning,
    Critical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
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

impl Sector {
    /// The matching sector id in the default taxonomy; `None` for "all".
    pub fn id(self) -> Option<&'static str> {
        Some(match self {
            Sector::All => return None,
            Sector::Medical => "medical",
            Sector::Government => "government",
            Sector::Power => "power",
            Sector::Emergency => "emergency",
            Sector::Civilian => "civilian",
            Sector::Financial => "financial",
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct StatusChange {
    pub status: Status,
    pub timestamp: String,
}

fn asset() -> String {
    "Asset".into()
}
fn under_investigation() -> Status {
    Status::UnderInvestigation
}
fn connection() -> String {
    "connection".into()
}

#[derive(Debug, Deserialize)]
pub struct Pin {
    pub id: String,
    pub name: String,
    #[serde(default = "asset")]
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

#[derive(Debug, Deserialize)]
pub struct Edge {
    pub id: String,
    pub from_pid: String,
    pub to_pid: String,
    #[serde(default)]
    pub label: String,
    #[serde(default = "connection")]
    pub edge_type: String,
}

#[derive(Debug, Deserialize)]
pub struct NewPin {
    pub name: String,
    #[serde(default = "asset")]
    pub category: String,
    pub lat: f64,
    pub lon: f64,
    #[serde(default = "under_investigation")]
    pub status: Status,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub struct Threshold {
    pub id: String,
    pub name: String,
    pub sector: Sector,
    pub below_pct: u8,
    #[serde(default)]
    pub severity: Severity,
}

#[derive(Debug, Deserialize)]
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
    #[serde(default)]
    pub auto: bool,
}

/// A whole v1 scenario file. Unknown keys (`name`, `saved_at`) are ignored.
#[derive(Debug, Default, Deserialize)]
pub struct Scenario {
    #[serde(default)]
    pub pins: IndexMap<String, Pin>,
    #[serde(default)]
    pub injects: IndexMap<String, Inject>,
    #[serde(default)]
    pub edges: IndexMap<String, Edge>,
    #[serde(default)]
    pub thresholds: Vec<Threshold>,
    #[serde(default)]
    pub decision_log: Vec<LogRecord>,
}

/// Keep alphanumerics, hyphens, underscores and spaces, trim, cap at 64: the
/// names the v1 app gave scenario files. Anything else cannot be one of ours.
pub fn safe_name(name: &str) -> String {
    let kept: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ' ')).collect();
    kept.trim().chars().take(64).collect::<String>().trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::safe_name;

    #[test]
    fn sanitising() {
        for (raw, want) in [
            ("Exercise One", "Exercise One"),
            ("../../../etc/passwd", "etcpasswd"),
            ("..\\..\\windows\\system32", "windowssystem32"),
            ("emoji \u{1F600} stripped", "emoji  stripped"),
            ("...", ""),
        ] {
            assert_eq!(safe_name(raw), want, "{raw:?}");
        }
    }
}
