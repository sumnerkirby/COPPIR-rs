//! Outbound lookups: Nominatim geocoding and Overpass infrastructure search.
//! The only routes that leave the machine.

use std::{collections::HashSet, fmt::Write, time::Duration};

use axum::{Json, extract::State};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::{
    config::{self, OSM_RESULT_CAP},
    error::ApiError,
    state::AppState,
    validate::{Checks, Valid, Validate},
};

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct GeoReq {
    pub query: String,
}

impl Validate for GeoReq {
    fn check(&mut self, c: &mut Checks) {
        c.label("query", &mut self.query, 300);
    }
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct OsmReq {
    pub lat: f64,
    pub lon: f64,
    #[serde(default = "default_radius")]
    #[ts(optional, as = "Option<f64>")]
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

/// The infrastructure categories an OSM search can look for.
pub async fn categories() -> Json<Vec<&'static str>> {
    Json(config::OSM_FILTERS.iter().map(|(name, _)| *name).collect())
}

const METRES_PER_MILE: f64 = 1609.344;

/// A place the geocoder found.
#[derive(Debug, Serialize, PartialEq, TS)]
#[ts(export)]
pub struct Place {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    /// What sort of place, e.g. "hospital" or "city".
    pub kind: String,
}

pub async fn geocode(State(app): State<AppState>, Valid(req): Valid<GeoReq>) -> Result<Json<Vec<Place>>, ApiError> {
    let res = app
        .http
        .get(&app.config.nominatim_url)
        .query(&[("q", req.query.as_str()), ("format", "json"), ("limit", "8")])
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| ApiError::Upstream(format!("Geocoder unavailable: {e}")))?;
    let body: Value = res.json().await.map_err(|e| ApiError::Upstream(format!("Geocoder sent a bad response: {e}")))?;
    Ok(Json(parse_places(&body)))
}

/// Nominatim sends coordinates as strings; skip anything without both.
pub fn parse_places(body: &Value) -> Vec<Place> {
    let num = |v: Option<&Value>| match v {
        Some(Value::String(s)) => s.parse::<f64>().ok(),
        Some(v) => v.as_f64(),
        None => None,
    };
    body.as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            Some(Place {
                name: r.get("display_name")?.as_str()?.to_owned(),
                lat: num(r.get("lat"))?,
                lon: num(r.get("lon"))?,
                kind: r.get("type").and_then(Value::as_str).unwrap_or("place").replace('_', " "),
            })
        })
        .collect()
}

#[derive(Debug, Serialize, PartialEq, TS)]
#[ts(export)]
pub struct OsmResult {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub category: String,
}

pub async fn search(State(app): State<AppState>, Valid(req): Valid<OsmReq>) -> Result<Json<Vec<OsmResult>>, ApiError> {
    let tags = config::osm_tags(&req.category)
        .ok_or_else(|| ApiError::BadRequest(format!("Unknown category: {}", req.category)))?;
    let query = overpass_query(tags, req.lat, req.lon, req.radius_mi);

    let mut last_err = String::from("no mirrors configured");
    for mirror in &app.config.overpass_mirrors {
        let res = app.http.post(mirror).form(&[("data", query.as_str())]).timeout(Duration::from_secs(50)).send().await;
        let res = match res {
            Ok(r) if matches!(r.status(), StatusCode::TOO_MANY_REQUESTS | StatusCode::GATEWAY_TIMEOUT) => {
                last_err = format!("{mirror} returned {}", r.status());
                continue;
            }
            Ok(r) => r,
            Err(e) => {
                last_err = e.to_string();
                continue;
            }
        };
        let data: Value = match res.error_for_status() {
            Ok(r) => match r.json().await {
                Ok(v) => v,
                Err(e) => {
                    last_err = format!("{mirror} sent a bad response: {e}");
                    continue;
                }
            },
            Err(e) => {
                last_err = e.to_string();
                continue;
            }
        };
        return Ok(Json(parse_elements(&req.category, &data)));
    }
    Err(ApiError::Upstream(format!("Overpass unavailable: {last_err}")))
}

/// `nwr` covers nodes, ways *and* relations; large sites such as hospital
/// campuses are often mapped as multipolygon relations, which the Python
/// version's node+way query missed. `out center` returns a centroid for ways
/// and relations instead of their full geometry, keeping responses small.
pub fn overpass_query(tags: &[(&str, &str)], lat: f64, lon: f64, radius_mi: f64) -> String {
    let radius_m = (radius_mi * METRES_PER_MILE).round() as i64;
    let mut body = String::new();
    for (k, v) in tags {
        let _ = write!(body, r#"nwr["{k}"="{v}"](around:{radius_m},{lat},{lon});"#);
    }
    format!("[out:json][timeout:40];({body});out center qt;")
}

/// Turn an Overpass response into pins-to-be: named, deduplicated by rounded
/// coordinate (a node and the way around it often mark the same site), capped.
pub fn parse_elements(category: &str, data: &Value) -> Vec<OsmResult> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let Some(elements) = data.get("elements").and_then(Value::as_array) else { return out };
    for el in elements {
        if out.len() >= OSM_RESULT_CAP {
            break;
        }
        let coord = |k: &str| {
            el.get(k)
                .and_then(Value::as_f64)
                .or_else(|| el.get("center").and_then(|c| c.get(k)).and_then(Value::as_f64))
        };
        // Python tested `if lat and lon`, which also threw away anything on
        // the equator or the prime meridian.
        let (Some(lat), Some(lon)) = (coord("lat"), coord("lon")) else { continue };
        let key = ((lat * 1e5).round() as i64, (lon * 1e5).round() as i64);
        if !seen.insert(key) {
            continue;
        }
        let tags = el.get("tags");
        let tag = |k: &str| tags.and_then(|t| t.get(k)).and_then(Value::as_str).filter(|s| !s.trim().is_empty());
        let name = tag("name")
            .or_else(|| tag("operator"))
            .or_else(|| tag("brand"))
            .map_or_else(|| format!("Unnamed {category}"), str::to_owned);
        out.push(OsmResult { name, lat, lon, category: category.to_owned() });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn query_covers_every_tag() {
        let q = overpass_query(&[("amenity", "bank"), ("amenity", "atm")], 38.9, -77.0, 1.0);
        assert_eq!(
            q,
            r#"[out:json][timeout:40];(nwr["amenity"="bank"](around:1609,38.9,-77);nwr["amenity"="atm"](around:1609,38.9,-77););out center qt;"#
        );
    }

    #[test]
    fn parses_nominatim_places() {
        let body = json!([
            {"display_name": "Washington, DC", "lat": "38.9", "lon": "-77.03", "type": "city"},
            {"display_name": "No coordinates"},
            {"display_name": "Numeric", "lat": 1.5, "lon": 2.5, "type": "fire_station"}
        ]);
        let got = parse_places(&body);
        assert_eq!(got.len(), 2);
        assert_eq!((got[0].lat, got[0].lon), (38.9, -77.03));
        assert_eq!(got[1].kind, "fire station");
    }

    #[test]
    fn every_category_has_tags() {
        for (name, tags) in config::OSM_FILTERS {
            assert!(!tags.is_empty(), "{name}");
        }
    }

    #[test]
    fn parses_names_centres_and_duplicates() {
        let data = json!({"elements": [
            {"lat": 1.0, "lon": 2.0, "tags": {"name": "General"}},
            {"center": {"lat": 1.000001, "lon": 2.000001}, "tags": {"name": "General (way)"}},
            {"lat": 0.0, "lon": 0.0, "tags": {"operator": "Null Island Clinic"}},
            {"lat": 5.0, "lon": 5.0, "tags": {"name": "  ", "brand": "BrandCo"}},
            {"lat": 6.0, "lon": 6.0},
            {"tags": {"name": "no position"}},
        ]});
        let got = parse_elements("Hospitals", &data);
        let names: Vec<_> = got.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["General", "Null Island Clinic", "BrandCo", "Unnamed Hospitals"]);
    }

    #[test]
    fn caps_results() {
        let elements: Vec<_> = (0..500).map(|i| json!({"lat": i as f64 / 100.0, "lon": 0.5})).collect();
        assert_eq!(parse_elements("Banks", &json!({ "elements": elements })).len(), OSM_RESULT_CAP);
    }
}
