//! Runtime configuration and third-party service settings.

use std::path::PathBuf;

pub const USER_AGENT: &str = concat!("COPPIR/", env!("CARGO_PKG_VERSION"));

/// Cap on returned OSM results, so a dense urban search (ATMs or telecom
/// masts in a city centre) cannot freeze the browser map.
pub const OSM_RESULT_CAP: usize = 200;

pub const NOMINATIM_URL: &str = "https://nominatim.openstreetmap.org/search";

/// Tried in order. The primary Overpass instance is rate-limited and sometimes
/// down; 429 and 504 (and connection errors) fall through to the next one.
pub const OVERPASS_MIRRORS: &[&str] = &[
    "https://overpass-api.de/api/interpreter",
    "https://overpass.kumi.systems/api/interpreter",
    "https://overpass.openstreetmap.fr/api/interpreter",
];

#[derive(Clone, Debug)]
pub struct Config {
    /// Where scenario files are written. Must already exist.
    pub scenarios_dir: PathBuf,
    pub nominatim_url: String,
    pub overpass_mirrors: Vec<String>,
}

impl Config {
    pub fn new(scenarios_dir: PathBuf) -> Self {
        Self {
            scenarios_dir,
            nominatim_url: NOMINATIM_URL.into(),
            overpass_mirrors: OVERPASS_MIRRORS.iter().map(|s| (*s).into()).collect(),
        }
    }
}

/// The per-user data directory for scenarios: `~/Library/Application
/// Support/COPPIR/scenarios` on macOS, `%APPDATA%\COPPIR\data\scenarios` on
/// Windows, `$XDG_DATA_HOME/coppir/scenarios` on Linux.
///
/// The Python build wrote next to the executable, which fails when the
/// executable sits somewhere read-only, such as inside a signed `.app`.
pub fn default_scenarios_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "COPPIR")
        .map(|d| d.data_dir().join("scenarios"))
        .unwrap_or_else(|| PathBuf::from("scenarios"))
}

/// OSM tag pairs per infrastructure category, as offered in the search panel.
pub const OSM_FILTERS: &[(&str, &[(&str, &str)])] = &[
    (
        "Hospitals",
        &[("amenity", "hospital"), ("healthcare", "hospital"), ("amenity", "clinic"), ("healthcare", "clinic")],
    ),
    (
        "Government Buildings",
        &[
            ("amenity", "townhall"),
            ("amenity", "courthouse"),
            ("building", "government"),
            ("office", "government"),
            ("amenity", "government"),
        ],
    ),
    ("Power Plants", &[("power", "plant"), ("power", "substation"), ("power", "generator")]),
    ("Fire Stations", &[("amenity", "fire_station")]),
    ("Police Stations", &[("amenity", "police")]),
    ("Data Centers", &[("building", "data_center"), ("telecom", "data_center"), ("facility", "data_center")]),
    ("Universities", &[("amenity", "university"), ("amenity", "college")]),
    ("Banks", &[("amenity", "bank"), ("amenity", "atm")]),
    (
        "Water Systems",
        &[
            ("man_made", "water_tower"),
            ("amenity", "water_works"),
            ("man_made", "water_works"),
            ("amenity", "water_point"),
            ("utility", "water"),
        ],
    ),
    (
        "Transportation Hubs",
        &[
            ("aeroway", "aerodrome"),
            ("railway", "station"),
            ("amenity", "bus_station"),
            ("public_transport", "station"),
            ("amenity", "ferry_terminal"),
        ],
    ),
    (
        "Telecom Infrastructure",
        &[
            ("man_made", "mast"),
            ("man_made", "tower"),
            ("communication", "mobile_phone"),
            ("telecom", "exchange"),
            ("building", "telecommunications"),
        ],
    ),
];

pub fn osm_tags(category: &str) -> Option<&'static [(&'static str, &'static str)]> {
    OSM_FILTERS.iter().find(|(name, _)| *name == category).map(|(_, tags)| *tags)
}
