//! The default taxonomy, which reproduces the v1 app's fixed vocabulary, and
//! the checks any replacement taxonomy has to pass.

use std::collections::HashSet;

use super::model::{AssetType, Sector, StatusDisplay, StatusSet, StatusValue, Taxonomy};
use crate::validate::Checks;

pub const SECURITY: &str = "security";
pub const OPERATIONAL: &str = "operational";

fn value(id: &str, label: &str, color: &str, glyph: &str, score: u8) -> StatusValue {
    StatusValue { id: id.into(), label: label.into(), color: color.into(), glyph: glyph.into(), score }
}

fn asset_type(id: &str, name: &str, icon: &str, color: &str) -> AssetType {
    AssetType { id: id.into(), name: name.into(), icon: icon.into(), color: color.into() }
}

fn sector(id: &str, name: &str, color: &str, types: &[&str]) -> Sector {
    Sector {
        id: id.into(),
        name: name.into(),
        color: color.into(),
        asset_types: types.iter().map(|s| (*s).into()).collect(),
        status_set: SECURITY.into(),
    }
}

/// The v1 statuses, categories and sectors, with the same colours, glyphs and
/// scores, so an imported scenario looks and scores the way it did.
pub fn default_taxonomy() -> Taxonomy {
    Taxonomy {
        status_sets: vec![
            StatusSet {
                id: SECURITY.into(),
                name: "Security".into(),
                values: vec![
                    value("clean", "Clean", "#33cc33", "✓", 100),
                    value("monitored", "Monitored", "#3399ff", "◦", 70),
                    value("contained", "Contained", "#ff6600", "▪", 40),
                    value("under_investigation", "Under Investigation", "#ffaa00", "?", 15),
                    value("compromised", "Compromised", "#ff3333", "✕", 0),
                ],
                default: "clean".into(),
                display: StatusDisplay::Fill,
            },
            StatusSet {
                id: OPERATIONAL.into(),
                name: "Operational".into(),
                values: vec![
                    value("healthy", "Healthy", "#00ff41", "solid", 100),
                    value("degraded", "Degraded", "#ffd700", "dashed", 60),
                    value("critical", "Critical", "#ff4500", "double", 25),
                    value("offline", "Offline", "#888888", "dotted", 0),
                ],
                default: "healthy".into(),
                display: StatusDisplay::Ring,
            },
        ],
        asset_types: vec![
            asset_type("hospitals", "Hospitals", "hospital", "#3399ff"),
            asset_type("government_buildings", "Government Buildings", "landmark", "#aa88ff"),
            asset_type("power_plants", "Power Plants", "zap", "#ffdd00"),
            asset_type("fire_stations", "Fire Stations", "flame", "#ff4444"),
            asset_type("police_stations", "Police Stations", "shield", "#ff4444"),
            asset_type("water_systems", "Water Systems", "droplet", "#00ccaa"),
            asset_type("transportation_hubs", "Transportation Hubs", "plane", "#00ccaa"),
            asset_type("telecom_infrastructure", "Telecom Infrastructure", "radio-tower", "#00ccaa"),
            asset_type("data_centers", "Data Centers", "database", "#00ccaa"),
            asset_type("universities", "Universities", "graduation-cap", "#00ccaa"),
            asset_type("banks", "Banks", "landmark", "#ffb000"),
            asset_type("asset", "Asset", "server", "#888888"),
            asset_type("responder", "Responder", "user-check", "#888888"),
            asset_type("poi", "POI", "map-pin", "#888888"),
        ],
        sectors: vec![
            sector("medical", "Medical", "#3399ff", &["hospitals"]),
            sector("government", "Government", "#aa88ff", &["government_buildings"]),
            sector("power", "Power", "#ffdd00", &["power_plants"]),
            sector("emergency", "Emergency", "#ff4444", &["fire_stations", "police_stations"]),
            sector(
                "civilian",
                "Civilian",
                "#00ccaa",
                &["water_systems", "transportation_hubs", "telecom_infrastructure", "universities", "data_centers"],
            ),
            sector("financial", "Financial", "#ffb000", &["banks"]),
        ],
        fields: vec![],
    }
}

/// Lowercase ASCII with underscores, for ids derived from a display name.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() { "item".into() } else { out }
}

fn id_ok(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Structural checks: ids unique and well-formed, every reference resolves,
/// colours are `#rrggbb`, every set has a valid default.
pub fn check(t: &mut Taxonomy, c: &mut Checks) {
    let ids = |kind: &str, list: Vec<&str>, c: &mut Checks| {
        let mut seen = HashSet::new();
        for id in list {
            if !id_ok(id) {
                c.fail(kind, format!("id {id:?} must be 1-64 letters, digits, '_' or '-'"));
            } else if !seen.insert(id.to_owned()) {
                c.fail(kind, format!("duplicate id {id:?}"));
            }
        }
    };
    ids("asset_types", t.asset_types.iter().map(|a| a.id.as_str()).collect(), c);
    ids("status_sets", t.status_sets.iter().map(|s| s.id.as_str()).collect(), c);
    ids("sectors", t.sectors.iter().map(|s| s.id.as_str()).collect(), c);
    ids("fields", t.fields.iter().map(|f| f.id.as_str()).collect(), c);

    if t.status_sets.is_empty() {
        c.fail("status_sets", "at least one status set is required");
    }
    if t.asset_types.is_empty() {
        c.fail("asset_types", "at least one asset type is required");
    }
    for (i, a) in t.asset_types.iter_mut().enumerate() {
        c.nested(&["asset_types", &i.to_string()], |c| {
            c.label("name", &mut a.name, 100);
            c.color("color", Some(&a.color));
        });
    }
    for (i, s) in t.status_sets.iter_mut().enumerate() {
        c.nested(&["status_sets", &i.to_string()], |c| {
            c.label("name", &mut s.name, 100);
            if s.values.is_empty() {
                c.fail("values", "at least one value is required");
            }
            ids("values", s.values.iter().map(|v| v.id.as_str()).collect(), c);
            for (j, v) in s.values.iter_mut().enumerate() {
                c.nested(&["values", &j.to_string()], |c| {
                    c.label("label", &mut v.label, 100);
                    c.color("color", Some(&v.color));
                    c.max_len("glyph", &v.glyph, 16);
                    if v.score > 100 {
                        c.fail("score", "must be between 0 and 100");
                    }
                });
            }
            if s.value(&s.default).is_none() {
                c.fail("default", format!("{:?} is not one of this set's values", s.default));
            }
        });
    }
    let type_ids: HashSet<_> = t.asset_types.iter().map(|a| a.id.clone()).collect();
    let set_ids: HashSet<_> = t.status_sets.iter().map(|s| s.id.clone()).collect();
    for (i, s) in t.sectors.iter_mut().enumerate() {
        c.nested(&["sectors", &i.to_string()], |c| {
            c.label("name", &mut s.name, 100);
            c.color("color", Some(&s.color));
            for ty in &s.asset_types {
                if !type_ids.contains(ty) {
                    c.fail("asset_types", format!("unknown asset type {ty:?}"));
                }
            }
            if !set_ids.contains(&s.status_set) {
                c.fail("status_set", format!("unknown status set {:?}", s.status_set));
            }
        });
    }
    for (i, f) in t.fields.iter_mut().enumerate() {
        c.nested(&["fields", &i.to_string()], |c| c.label("label", &mut f.label, 100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::Validate;

    struct T(Taxonomy);
    impl Validate for T {
        fn check(&mut self, c: &mut Checks) {
            check(&mut self.0, c);
        }
    }

    #[test]
    fn default_taxonomy_is_valid() {
        T(default_taxonomy()).validate().unwrap();
    }

    #[test]
    fn catches_dangling_references_and_duplicates() {
        let mut t = default_taxonomy();
        t.sectors[0].asset_types.push("nope".into());
        t.status_sets[0].default = "missing".into();
        t.asset_types.push(t.asset_types[0].clone());
        let Err(crate::error::ApiError::Validation(errs)) = T(t).validate() else { panic!() };
        let msgs: Vec<_> = errs.iter().map(|e| e.msg.as_str()).collect();
        assert!(msgs.iter().any(|m| m.contains("nope")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("missing")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("duplicate")), "{msgs:?}");
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Government Buildings"), "government_buildings");
        assert_eq!(slug("  Wi-Fi / LAN  "), "wi_fi_lan");
        assert_eq!(slug("!!!"), "item");
    }
}
