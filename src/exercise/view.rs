//! What each audience may see.
//!
//! The facilitator sees the exercise as it is. Everyone else gets a
//! *projection*: a copy with everything hidden from them removed. Hidden
//! assets, unfired injects and facilitator notes never leave the server, so a
//! curious player cannot find them in the page's data.
//!
//! When the exercise changes, each audience's projection is recomputed and
//! [`diff`]ed against the previous one. The difference, as ordinary events,
//! is what that audience is sent. This is why an asset revealed to a team
//! simply appears for them, and one hidden again simply disappears.

use std::collections::HashSet;

use super::{
    event::Event,
    model::{Exercise, Visibility},
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Audience {
    Facilitator,
    /// The projector: only what everyone may see.
    Observer,
    Player {
        team: Option<String>,
    },
}

impl Audience {
    pub fn sees(&self, v: &Visibility) -> bool {
        match (self, v) {
            (Audience::Facilitator, _) | (_, Visibility::All) => true,
            (Audience::Player { team: Some(t) }, Visibility::Teams { teams }) => teams.contains(t),
            _ => false,
        }
    }
}

/// The exercise as `audience` may see it.
pub fn project(ex: &Exercise, audience: &Audience) -> Exercise {
    if *audience == Audience::Facilitator {
        return ex.clone();
    }
    let hidden_fields: HashSet<&str> =
        ex.taxonomy.fields.iter().filter(|f| !f.player_visible).map(|f| f.id.as_str()).collect();

    let mut out = Exercise {
        assets: Default::default(),
        links: Default::default(),
        injects: Default::default(),
        log: vec![],
        ..ex.clone()
    };
    for a in ex.assets.values().filter(|a| audience.sees(&a.visibility)) {
        let mut a = a.clone();
        a.fields.retain(|k, _| !hidden_fields.contains(k.as_str()));
        // Who else can see an asset is the facilitator's business.
        a.visibility = Visibility::All;
        out.assets.insert(a.id.clone(), a);
    }
    for l in ex.links.values() {
        if out.assets.contains_key(&l.from) && out.assets.contains_key(&l.to) {
            out.links.insert(l.id.clone(), l.clone());
        }
    }
    // Only injects that have fired, and then only their public face: no
    // facilitator notes, and no effects, which could name hidden assets.
    for i in ex.injects.values().filter(|i| i.fired_at.is_some() && audience.sees(&i.audience)) {
        let mut i = i.clone();
        i.notes.clear();
        i.effects.clear();
        i.audience = Visibility::All;
        out.injects.insert(i.id.clone(), i);
    }
    out.log = ex
        .log
        .iter()
        .filter(|e| audience.sees(&e.visibility))
        .map(|e| {
            let mut e = e.clone();
            // A log line about an asset this audience cannot see keeps its
            // text but not the link.
            if e.asset.as_ref().is_some_and(|id| !out.assets.contains_key(id)) {
                e.asset = None;
                e.asset_name = None;
            }
            e.visibility = Visibility::All;
            e
        })
        .collect();
    out
}

/// The events that turn `before` into `after`.
pub fn diff(before: &Exercise, after: &Exercise) -> Vec<Event> {
    let mut out = Vec::new();
    if before.name != after.name || before.description != after.description {
        out.push(Event::Renamed { name: after.name.clone(), description: after.description.clone() });
    }

    let changed_assets: Vec<_> =
        after.assets.values().filter(|a| before.assets.get(&a.id) != Some(*a)).cloned().collect();
    if before.taxonomy != after.taxonomy {
        out.push(Event::TaxonomyReplaced { taxonomy: after.taxonomy.clone(), assets: changed_assets });
    } else if !changed_assets.is_empty() {
        out.push(Event::AssetsUpserted { assets: changed_assets });
    }
    let gone_assets: Vec<String> = before.assets.keys().filter(|id| !after.assets.contains_key(*id)).cloned().collect();
    let gone_links: Vec<String> = before.links.keys().filter(|id| !after.links.contains_key(*id)).cloned().collect();
    if !gone_assets.is_empty() {
        out.push(Event::AssetsRemoved { ids: gone_assets, links: gone_links });
    } else {
        out.extend(gone_links.into_iter().map(|id| Event::LinkRemoved { id }));
    }
    for l in after.links.values().filter(|l| before.links.get(&l.id) != Some(*l)) {
        out.push(Event::LinkUpserted { link: l.clone() });
    }

    for t in after.teams.iter().filter(|t| !before.teams.contains(t)) {
        out.push(Event::TeamUpserted { team: t.clone() });
    }
    for t in before.teams.iter().filter(|t| after.teams.iter().all(|x| x.id != t.id)) {
        out.push(Event::TeamRemoved { id: t.id.clone() });
    }

    for i in after.injects.values().filter(|i| before.injects.get(&i.id) != Some(*i)) {
        out.push(Event::InjectUpserted { inject: i.clone() });
    }
    for id in before.injects.keys().filter(|id| !after.injects.contains_key(*id)) {
        out.push(Event::InjectRemoved { id: id.clone() });
    }

    for t in after.thresholds.iter().filter(|t| !before.thresholds.contains(t)) {
        out.push(Event::ThresholdUpserted { threshold: t.clone() });
    }
    for t in before.thresholds.iter().filter(|t| after.thresholds.iter().all(|x| x.id != t.id)) {
        out.push(Event::ThresholdRemoved { id: t.id.clone() });
    }

    // The log is ordered, and a line can change (it loses its asset link when
    // that asset leaves this view). From the first line that differs, drop
    // the old tail and send the new one, so order is preserved.
    let same = before.log.iter().zip(&after.log).take_while(|(a, b)| a == b).count();
    for e in &before.log[same..] {
        out.push(Event::LogRemoved { id: e.id.clone() });
    }
    for e in &after.log[same..] {
        out.push(Event::Logged { entry: e.clone() });
    }

    for a in after.annotations.values().filter(|a| before.annotations.get(&a.id) != Some(*a)) {
        out.push(Event::AnnotationUpserted { annotation: a.clone() });
    }
    let gone: Vec<String> =
        before.annotations.keys().filter(|id| !after.annotations.contains_key(*id)).cloned().collect();
    if !gone.is_empty() {
        out.push(Event::AnnotationsRemoved { ids: gone });
    }

    for m in after.metrics.iter().filter(|m| !before.metrics.contains(m)) {
        out.push(Event::MetricUpserted { metric: m.clone() });
    }
    for m in before.metrics.iter().filter(|m| after.metrics.iter().all(|x| x.id != m.id)) {
        out.push(Event::MetricRemoved { id: m.id.clone() });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exercise::{Command, Envelope, blank, decide};
    use serde_json::json;

    fn run(ex: &mut Exercise, cmd: serde_json::Value) -> Vec<Event> {
        let cmd: Command = serde_json::from_value(cmd).unwrap();
        let events = decide(ex, cmd, "t").unwrap();
        for e in &events {
            ex.apply(&Envelope { seq: 0, at: "t".into(), actor: "x".into(), event: e.clone() });
        }
        events
    }

    fn first_id(events: &[Event]) -> String {
        match &events[0] {
            Event::AssetsUpserted { assets } => assets[0].id.clone(),
            Event::TeamUpserted { team } => team.id.clone(),
            Event::InjectUpserted { inject } => inject.id.clone(),
            other => panic!("{other:?}"),
        }
    }

    /// An exercise with two teams, one asset each can see alone, a public
    /// asset, a hidden one, and links between them.
    fn fixture() -> (Exercise, String, String) {
        let mut ex = blank("Drill", "", "t0").unwrap();
        let it = first_id(&run(&mut ex, json!({"type": "add_team", "name": "IT"})));
        let legal = first_id(&run(&mut ex, json!({"type": "add_team", "name": "Legal"})));
        let ev = run(
            &mut ex,
            json!({"type": "add_assets", "assets": [
                {"name": "public", "type_id": "asset"},
                {"name": "it-only", "type_id": "asset", "visibility": {"mode": "teams", "teams": [it]}},
                {"name": "legal-only", "type_id": "asset", "visibility": {"mode": "teams", "teams": [legal]}},
                {"name": "hidden", "type_id": "asset", "visibility": {"mode": "facilitator"}},
            ]}),
        );
        let Event::AssetsUpserted { assets } = &ev[0] else { panic!() };
        let ids: Vec<_> = assets.iter().map(|a| a.id.clone()).collect();
        run(&mut ex, json!({"type": "add_link", "from": ids[0], "to": ids[1]}));
        run(&mut ex, json!({"type": "add_link", "from": ids[0], "to": ids[3]}));
        (ex, it, legal)
    }

    fn names(ex: &Exercise) -> Vec<&str> {
        ex.assets.values().map(|a| a.name.as_str()).collect()
    }

    #[test]
    fn each_audience_sees_only_its_own() {
        let (ex, it, legal) = fixture();
        assert_eq!(project(&ex, &Audience::Facilitator), ex);
        assert_eq!(names(&project(&ex, &Audience::Observer)), ["public"]);
        assert_eq!(names(&project(&ex, &Audience::Player { team: None })), ["public"]);
        let it_view = project(&ex, &Audience::Player { team: Some(it) });
        assert_eq!(names(&it_view), ["public", "it-only"]);
        assert_eq!(it_view.links.len(), 1, "the link to the hidden asset is gone");
        assert_eq!(names(&project(&ex, &Audience::Player { team: Some(legal) })), ["public", "legal-only"]);
        // Nothing in a projection says who else can see what.
        assert!(it_view.assets.values().all(|a| a.visibility == Visibility::All));
    }

    #[test]
    fn injects_show_only_once_fired_and_without_their_workings() {
        let (mut ex, it, legal) = fixture();
        let target = ex.assets.values().find(|a| a.name == "hidden").unwrap().id.clone();
        let iid = first_id(&run(
            &mut ex,
            json!({"type": "add_inject", "inject": {
                "title": "Ransom note", "body": "pay up", "notes": "expect escalation to legal",
                "audience": {"mode": "teams", "teams": [it]},
                "effects": [{"type": "set_status", "asset": target, "set": "security", "value": "compromised"}]}}),
        ));
        let player = Audience::Player { team: Some(it) };
        assert!(project(&ex, &player).injects.is_empty(), "queued injects are secret");

        run(&mut ex, json!({"type": "fire_inject", "id": iid}));
        let view = project(&ex, &player);
        let inject = &view.injects[&iid];
        assert_eq!(inject.body, "pay up");
        assert!(inject.notes.is_empty() && inject.effects.is_empty());
        // The automatic log line follows the inject's audience, and does not
        // name the hidden asset it changed.
        assert_eq!(view.log.len(), 1);
        assert_eq!(view.log[0].asset, None);
        let other = project(&ex, &Audience::Player { team: Some(legal) });
        assert!(other.injects.is_empty() && other.log.is_empty());
        assert!(!serde_json::to_string(&view).unwrap().contains("escalation"));
    }

    /// Applying `diff(a, b)` to `a` gives `b`, for every audience, across a
    /// sequence of changes that reveal, hide, edit and remove things.
    #[test]
    fn diff_carries_a_view_from_one_state_to_the_next() {
        let (mut ex, it, legal) = fixture();
        let hidden = ex.assets.values().find(|a| a.name == "hidden").unwrap().id.clone();
        let it_only = ex.assets.values().find(|a| a.name == "it-only").unwrap().id.clone();
        let public = ex.assets.values().find(|a| a.name == "public").unwrap().id.clone();
        let audiences = [
            Audience::Observer,
            Audience::Player { team: Some(it.clone()) },
            Audience::Player { team: Some(legal.clone()) },
        ];
        let mut views: Vec<Exercise> = audiences.iter().map(|a| project(&ex, a)).collect();

        let steps = [
            json!({"type": "edit_asset", "id": hidden, "patch": {"visibility": {"mode": "teams", "teams": [it, legal]}}}),
            json!({"type": "set_status", "ids": [hidden, public], "set": "security", "value": "compromised"}),
            json!({"type": "edit_asset", "id": it_only, "patch": {"visibility": {"mode": "facilitator"}}}),
            json!({"type": "log", "text": "declared", "asset": public}),
            json!({"type": "add_metric", "name": "Comms", "value": 40}),
            json!({"type": "add_threshold", "name": "t", "below_pct": 50}),
            json!({"type": "rename", "name": "Renamed"}),
            json!({"type": "remove_team", "id": legal}),
            json!({"type": "remove_assets", "ids": [public]}),
            json!({"type": "add_annotation", "shape": "point", "geometry": {"type": "Point", "coordinates": [1.0, 2.0]}}),
        ];
        for step in steps {
            run(&mut ex, step.clone());
            for (audience, view) in audiences.iter().zip(views.iter_mut()) {
                let target = project(&ex, audience);
                for event in diff(view, &target) {
                    view.apply(&Envelope { seq: 0, at: "t".into(), actor: String::new(), event });
                }
                // Order within the maps can differ after a reveal; contents may not.
                view.assets.sort_keys();
                view.links.sort_keys();
                let mut want = target.clone();
                want.assets.sort_keys();
                want.links.sort_keys();
                view.updated_at.clone_from(&want.updated_at);
                assert_eq!(*view, want, "{audience:?} after {step}");
            }
        }
    }

    #[test]
    fn nothing_changed_means_nothing_sent() {
        let (ex, it, _) = fixture();
        let view = project(&ex, &Audience::Player { team: Some(it) });
        assert!(diff(&view, &view).is_empty());
    }
}
