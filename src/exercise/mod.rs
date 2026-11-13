//! The v2 exercise model: data, the events that change it, and the commands
//! that produce those events.
//!
//! The flow for every change is `Command` → [`decide`] → `Vec<Event>` →
//! stored → [`Exercise::apply`] → broadcast. Only `decide` can refuse;
//! `apply` never fails.

pub mod command;
pub mod event;
pub mod import_v1;
pub mod legacy;
pub mod model;
pub mod taxonomy;
pub mod undo;
pub mod view;

pub use command::{Command, decide};
pub use event::{Envelope, Event};
pub use model::Exercise;

use crate::{error::ApiError, validate::Checks};

/// Make an exercise from outside (an exported file, a copy) safe to adopt: a
/// fresh id, a valid taxonomy, every asset's type known and statuses fitted,
/// no link to a missing asset.
pub fn adopt(mut ex: Exercise, name: Option<&str>, now: &str) -> Result<Exercise, ApiError> {
    let mut c = Checks::default();
    if let Some(n) = name {
        ex.name = n.to_owned();
    }
    c.label("name", &mut ex.name, 200);
    c.nested(&["exercise"], |c| taxonomy::check(&mut ex.taxonomy, c));
    c.finish()?;
    let types: std::collections::HashSet<_> = ex.taxonomy.asset_types.iter().map(|t| t.id.clone()).collect();
    if let Some(a) = ex.assets.values().find(|a| !types.contains(&a.type_id)) {
        return Err(ApiError::BadRequest(format!("Asset {:?} has unknown type {:?}", a.name, a.type_id)));
    }
    for a in ex.assets.values_mut() {
        a.statuses = command::fit_statuses(&ex.taxonomy, &a.statuses);
    }
    let assets = &ex.assets;
    ex.links.retain(|_, l| assets.contains_key(&l.from) && assets.contains_key(&l.to) && l.from != l.to);
    ex.id = crate::state::new_id();
    ex.created_at = now.to_owned();
    ex.updated_at = now.to_owned();
    Ok(ex)
}

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
        annotations: Default::default(),
        metrics: vec![],
        created_at: now.to_owned(),
        updated_at: now.to_owned(),
    })
}
