//! Outbound lookups: Nominatim geocoding and Overpass infrastructure search.
//! The only routes that leave the machine.

use std::{collections::HashSet, fmt::Write, time::Duration};

use axum::{Json, extract::State};
use reqwest::StatusCode;
use serde::Serialize;
use serde_json::Value;

use crate::{
    config::{self, OSM_RESULT_CAP},
    error::ApiError,
    models::{GeoReq, OsmReq},
    state::AppState,
    validate::Valid,
};

const METRES_PER_MILE: f64 = 1609.344;

pub async fn geocode(State(app): State<AppState>, Valid(req): Valid<GeoReq>) -> Result<Json<Value>, ApiError> {
    let res = app
        .http
        .get(&app.config.nominatim_url)
        .query(&[("q", req.query.as_str()), ("format", "json"), ("limit", "5")])
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| ApiError::Upstream(format!("Geocoder unavailable: {e}")))?;
    let body = res.json().await.map_err(|e| ApiError::Upstream(format!("Geocoder sent a bad response: {e}")))?;
    Ok(Json(body))
}

#[derive(Debug, Serialize, PartialEq)]
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
