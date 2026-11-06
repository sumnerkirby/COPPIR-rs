//! Request validation, and the extractor that runs it.
//!
//! Errors come back as HTTP 422 in the same shape FastAPI used —
//! `{"detail": [{"loc": [...], "msg": "...", "type": "..."}]}` — because the
//! frontend's `detailText()` already knows how to turn that into one line.

use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
    http::header::CONTENT_TYPE,
};
use serde::{Serialize, de::DeserializeOwned};

use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FieldError {
    pub loc: Vec<String>,
    pub msg: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
}

/// Accumulates every problem with a body, so the user sees all of them at once
/// rather than fixing one field per round trip.
#[derive(Default)]
pub struct Checks {
    prefix: Vec<String>,
    errors: Vec<FieldError>,
}

impl Checks {
    pub fn fail(&mut self, field: &str, msg: impl Into<String>) {
        let mut loc = vec!["body".to_owned()];
        loc.extend(self.prefix.iter().cloned());
        loc.push(field.to_owned());
        self.errors.push(FieldError { loc, msg: msg.into(), kind: "value_error" });
    }

    /// Run `f` with field names reported under `path`.
    pub fn nested(&mut self, path: &[&str], f: impl FnOnce(&mut Self)) {
        let depth = self.prefix.len();
        self.prefix.extend(path.iter().map(|s| (*s).to_owned()));
        f(self);
        self.prefix.truncate(depth);
    }

    /// A name-like field: trimmed in place, then required to be non-blank and
    /// at most `max` characters. A length minimum alone lets "   " through,
    /// which puts something on the map with no readable label.
    pub fn label(&mut self, field: &str, value: &mut String, max: usize) {
        let trimmed = value.trim();
        if trimmed.len() != value.len() {
            *value = trimmed.to_owned();
        }
        if value.is_empty() {
            self.fail(field, "must not be blank");
        } else {
            self.max_len(field, value, max);
        }
    }

    pub fn max_len(&mut self, field: &str, value: &str, max: usize) {
        if value.chars().count() > max {
            self.fail(field, format!("must be at most {max} characters"));
        }
    }

    pub fn coords(&mut self, lat: f64, lon: f64) {
        if !(-90.0..=90.0).contains(&lat) {
            self.fail("lat", "must be between -90 and 90");
        }
        if !(-180.0..=180.0).contains(&lon) {
            self.fail("lon", "must be between -180 and 180");
        }
    }

    pub fn color(&mut self, field: &str, value: Option<&str>) {
        if let Some(v) = value {
            if !is_hex_color(v) {
                self.fail(field, "must be #RRGGBB");
            }
        }
    }

    pub fn finish(&mut self) -> Result<(), ApiError> {
        let errors = std::mem::take(&mut self.errors);
        if errors.is_empty() { Ok(()) } else { Err(ApiError::Validation(errors)) }
    }
}

pub fn is_hex_color(v: &str) -> bool {
    v.len() == 7 && v.starts_with('#') && v[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

pub trait Validate {
    fn check(&mut self, c: &mut Checks);

    fn validate(&mut self) -> Result<(), ApiError> {
        let mut c = Checks::default();
        self.check(&mut c);
        c.finish()
    }
}

/// A JSON body that has been deserialized *and* validated.
pub struct Valid<T>(pub T);

impl<S, T> FromRequest<S> for Valid<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let is_json = req
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.split(';').next().is_some_and(|m| m.trim() == "application/json"));
        if !is_json {
            return Err(ApiError::UnsupportedMediaType);
        }
        let bytes = Bytes::from_request(req, state).await.map_err(|e| ApiError::BadRequest(e.body_text()))?;
        let mut value = parse::<T>(&bytes)?;
        value.validate()?;
        Ok(Valid(value))
    }
}

fn parse<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ApiError> {
    let de = &mut serde_json::Deserializer::from_slice(bytes);
    let value = serde_path_to_error::deserialize(&mut *de).map_err(|err| {
        let mut loc = vec!["body".to_owned()];
        let path = err.path().to_string();
        if path != "." {
            loc.extend(path.split('.').map(str::to_owned));
        }
        json_error(loc, err.inner())
    })?;
    de.end().map_err(|e| json_error(vec!["body".to_owned()], &e))?;
    Ok(value)
}

fn json_error(loc: Vec<String>, e: &serde_json::Error) -> ApiError {
    let kind = if e.is_syntax() || e.is_eof() { "json_invalid" } else { "type_error" };
    let msg = strip_position(&e.to_string()).to_owned();
    ApiError::Validation(vec![FieldError { loc, msg, kind }])
}

/// serde_json appends " at line 1 column 42", which means nothing to someone
/// filling in a form.
fn strip_position(msg: &str) -> &str {
    msg.rfind(" at line ").map_or(msg, |i| &msg[..i])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{PinIn, ThresholdIn};

    fn errors_for<T: DeserializeOwned + Validate>(json: &str) -> Vec<FieldError> {
        match parse::<T>(json.as_bytes()).and_then(|mut v| v.validate()) {
            Err(ApiError::Validation(e)) => e,
            Err(other) => panic!("unexpected error {other:?}"),
            Ok(()) => vec![],
        }
    }

    #[test]
    fn hex_colors() {
        assert!(is_hex_color("#a1B2c3"));
        for bad in ["red", "#ff0", "#gggggg", "a1b2c3#", "#a1b2c3d", ""] {
            assert!(!is_hex_color(bad), "{bad}");
        }
    }

    #[test]
    fn unknown_enum_names_the_field() {
        let e = errors_for::<PinIn>(r#"{"name":"x","lat":0,"lon":0,"status":"Melted"}"#);
        assert_eq!(e[0].loc, ["body", "status"]);
        assert!(e[0].msg.contains("Melted"), "{}", e[0].msg);
        assert!(!e[0].msg.contains("line"), "{}", e[0].msg);
    }

    #[test]
    fn reports_every_problem_at_once() {
        let e = errors_for::<PinIn>(r#"{"name":"  ","lat":91,"lon":-181,"pin_color":"red"}"#);
        let fields: Vec<_> = e.iter().map(|f| f.loc.last().unwrap().as_str()).collect();
        assert_eq!(fields, ["name", "lat", "lon", "pin_color"]);
    }

    #[test]
    fn trims_labels() {
        let mut p: PinIn = parse(br#"{"name":"  a  ","category":" b ","lat":0,"lon":0}"#).unwrap();
        p.validate().unwrap();
        assert_eq!((p.name.as_str(), p.category.as_str()), ("a", "b"));
    }

    #[test]
    fn missing_field() {
        let e = errors_for::<ThresholdIn>(r#"{"name":"x"}"#);
        assert!(e[0].msg.contains("sector"), "{}", e[0].msg);
    }

    #[test]
    fn trailing_garbage_is_rejected() {
        let e = errors_for::<ThresholdIn>(r#"{"name":"x","sector":"power"} junk"#);
        assert_eq!(e[0].kind, "json_invalid");
    }
}
