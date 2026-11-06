//! The v2 exercise model: data, the events that change it, and the commands
//! that produce those events.
//!
//! The flow for every change is `Command` → [`decide`] → `Vec<Event>` →
//! stored → [`Exercise::apply`] → broadcast. Only `decide` can refuse;
//! `apply` never fails.

pub mod command;
pub mod event;
pub mod import_v1;
pub mod model;
pub mod taxonomy;

pub use command::{Command, decide};
pub use event::{Envelope, Event};
pub use model::Exercise;

use crate::{error::ApiError, validate::Checks};

/// A brand-new, empty exercise with the default taxonomy.
pub fn blank(name: &str, description: &str, now: &str) -> Result<Exercise, ApiError> {
    let mut name = name.to_owned();
    let mut c = Checks::default();
    c.label("name", &mut name, 200);
    c.max_len("description", description, 4000);
    c.finish()?;
    Ok(Exercise {
        id: crate::state::new_id(),
        name,
        description: description.to_owned(),
        taxonomy: taxonomy::default_taxonomy(),
        teams: vec![],
        assets: Default::default(),
        links: Default::default(),
        injects: Default::default(),
        thresholds: vec![],
        log: vec![],
        created_at: now.to_owned(),
        updated_at: now.to_owned(),
    })
}
