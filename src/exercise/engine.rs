//! The script engine: what the server should do on its own, right now.
//!
//! [`due`] looks at the exercise and the time and returns ordinary commands:
//! fire the injects whose trigger has been met, close the polls whose time is
//! up. `Live` runs them like anyone else's commands, so they are validated,
//! logged, undoable and broadcast the same way. It asks again after every
//! change and once a second, so a chain of conditional injects resolves in
//! one go and a scheduled inject fires within a second of its time.

use super::{
    command::Command,
    model::{Condition, Exercise, PollState, Threshold, Trigger},
};

pub fn due(ex: &Exercise, now: &str) -> Vec<Command> {
    let mut out = Vec::new();
    // Polls first: a poll closing can be what a conditional inject waits for.
    for p in ex.polls.values().filter(|p| p.state == PollState::Open) {
        if p.closes_at.as_deref().is_some_and(|t| passed(t, now)) {
            out.push(Command::ClosePoll { id: p.id.clone() });
        }
    }
    if !out.is_empty() {
        return out;
    }
    let elapsed = ex.clock.elapsed_at(now);
    for i in ex.injects.values() {
        if i.fired_at.is_some() || !i.armed {
            continue;
        }
        let ready = match &i.trigger {
            Trigger::Manual => false,
            Trigger::At { minutes } => elapsed >= to_ms(*minutes),
            Trigger::Phase { phase, delay_minutes } => {
                // The latest time that phase began, if it has.
                ex.phase_log
                    .iter()
                    .rev()
                    .find(|e| &e.phase == phase)
                    .is_some_and(|e| elapsed >= e.elapsed_ms + to_ms(*delay_minutes))
            }
            Trigger::When { condition } => holds(ex, condition),
        };
        if ready {
            out.push(Command::FireInject { id: i.id.clone() });
        }
    }
    out
}

/// The next wall-clock moment something time-based could become due, in ms
/// of exercise time from now; used to decide whether the ticker needs this
/// exercise at all.
pub fn waiting_on_time(ex: &Exercise) -> bool {
    let timed = ex
        .injects
        .values()
        .any(|i| i.fired_at.is_none() && i.armed && matches!(i.trigger, Trigger::At { .. } | Trigger::Phase { .. }));
    (timed && ex.clock.running_since.is_some())
        || ex.polls.values().any(|p| p.state == PollState::Open && p.closes_at.is_some())
}

fn to_ms(minutes: f64) -> u64 {
    (minutes.max(0.0) * 60_000.0) as u64
}

fn passed(at: &str, now: &str) -> bool {
    match (chrono::DateTime::parse_from_rfc3339(at), chrono::DateTime::parse_from_rfc3339(now)) {
        (Ok(a), Ok(n)) => n >= a,
        _ => false,
    }
}

pub fn holds(ex: &Exercise, c: &Condition) -> bool {
    match c {
        Condition::AssetStatus { asset, set, value } => {
            ex.assets.get(asset).and_then(|a| a.statuses.get(set)).is_some_and(|v| v == value)
        }
        Condition::InjectFired { inject } => ex.injects.get(inject).is_some_and(|i| i.fired_at.is_some()),
        Condition::ThresholdBreached { threshold } => {
            ex.thresholds.iter().find(|t| &t.id == threshold).is_some_and(|t| breached(ex, t))
        }
        Condition::PollChoice { poll, option } => {
            ex.polls.get(poll).is_some_and(|p| p.state == PollState::Closed && p.winners().contains(&option.as_str()))
        }
    }
}

/// A sector's health: the mean score of its assets in the sector's status
/// set, or `None` with no assets. The same rule as the SITREP wheels.
pub fn sector_health(ex: &Exercise, sector: &str) -> Option<f64> {
    let s = ex.taxonomy.sectors.iter().find(|s| s.id == sector)?;
    let set = ex.taxonomy.status_sets.iter().find(|x| x.id == s.status_set)?;
    let scores: Vec<f64> = ex
        .assets
        .values()
        .filter(|a| s.asset_types.contains(&a.type_id))
        .map(|a| a.statuses.get(&set.id).and_then(|v| set.value(v)).map_or(0.0, |v| f64::from(v.score)))
        .collect();
    (!scores.is_empty()).then(|| scores.iter().sum::<f64>() / scores.len() as f64)
}

pub fn breached(ex: &Exercise, t: &Threshold) -> bool {
    let score = match &t.sector {
        Some(s) => sector_health(ex, s),
        None => {
            let all: Vec<f64> = ex.taxonomy.sectors.iter().filter_map(|s| sector_health(ex, &s.id)).collect();
            (!all.is_empty()).then(|| all.iter().sum::<f64>() / all.len() as f64)
        }
    };
    score.is_some_and(|s| s < f64::from(t.below_pct))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exercise::{Envelope, Event, blank, command::Responder, command::decide};
    use serde_json::json;

    const T0: &str = "2026-01-01T10:00:00+00:00";

    fn at(min: i64) -> String {
        (chrono::DateTime::parse_from_rfc3339(T0).unwrap() + chrono::Duration::minutes(min))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
    }

    fn run_at(ex: &mut Exercise, cmd: Command, now: &str) -> Vec<Event> {
        let events = decide(ex, cmd, now).unwrap();
        for e in &events {
            ex.apply(&Envelope { seq: 0, at: now.into(), actor: "x".into(), event: e.clone() });
        }
        events
    }

    fn run(ex: &mut Exercise, cmd: serde_json::Value, now: &str) -> Vec<Event> {
        run_at(ex, serde_json::from_value(cmd).unwrap(), now)
    }

    /// Run the engine until it has nothing more to do, as `Live` does.
    fn settle(ex: &mut Exercise, now: &str) -> Vec<String> {
        let mut fired = vec![];
        for _ in 0..20 {
            let due = due(ex, now);
            if due.is_empty() {
                break;
            }
            for cmd in due {
                if let Command::FireInject { id } = &cmd {
                    fired.push(ex.injects[id].title.clone());
                }
                run_at(ex, cmd, now);
            }
        }
        fired
    }

    fn inject(ex: &mut Exercise, title: &str, trigger: serde_json::Value) -> String {
        let ev = run(ex, json!({"type": "add_inject", "inject": {"title": title, "trigger": trigger}}), T0);
        let Event::InjectUpserted { inject } = &ev[0] else { panic!() };
        inject.id.clone()
    }

    #[test]
    fn timed_injects_fire_on_the_exercise_clock_not_the_wall_clock() {
        let mut ex = blank("Drill", "", T0).unwrap();
        inject(&mut ex, "at 10", json!({"type": "at", "minutes": 10}));
        inject(&mut ex, "manual", json!({"type": "manual"}));
        // Not started: an hour of wall time does nothing.
        assert!(settle(&mut ex, &at(60)).is_empty());
        run(&mut ex, json!({"type": "start_clock"}), &at(60));
        assert!(settle(&mut ex, &at(65)).is_empty());
        run(&mut ex, json!({"type": "pause_clock"}), &at(66));
        assert_eq!(ex.clock.elapsed_ms, 6 * 60_000);
        // Paused: still nothing, however long.
        assert!(settle(&mut ex, &at(200)).is_empty());
        run(&mut ex, json!({"type": "start_clock"}), &at(200));
        assert_eq!(settle(&mut ex, &at(204)), ["at 10"]);
        assert!(settle(&mut ex, &at(300)).is_empty(), "fires once");
    }

    #[test]
    fn phase_triggers_count_from_when_the_phase_began() {
        let mut ex = blank("Drill", "", T0).unwrap();
        run(
            &mut ex,
            json!({"type": "set_phases", "phases": [
                {"id": "", "name": "Detection", "color": "#3399ff", "target_minutes": 30},
                {"id": "", "name": "Containment", "color": "#ffaa00", "target_minutes": 45}]}),
            T0,
        );
        let containment = ex.phases[1].id.clone();
        inject(&mut ex, "5 into containment", json!({"type": "phase", "phase": containment, "delay_minutes": 5}));
        run(&mut ex, json!({"type": "start_clock"}), T0);
        assert!(settle(&mut ex, &at(40)).is_empty(), "phase not begun");
        run(&mut ex, json!({"type": "enter_phase", "id": containment}), &at(40));
        assert!(settle(&mut ex, &at(44)).is_empty());
        assert_eq!(settle(&mut ex, &at(45)), ["5 into containment"]);
    }

    #[test]
    fn conditions_chain_and_branch_on_a_poll() {
        let mut ex = blank("Drill", "", T0).unwrap();
        let ev =
            run(&mut ex, json!({"type": "add_assets", "assets": [{"name": "Grid", "type_id": "power_plants"}]}), T0);
        let Event::AssetsUpserted { assets } = &ev[0] else { panic!() };
        let grid = assets[0].id.clone();
        let ev = run(
            &mut ex,
            json!({"type": "add_poll", "poll": {"question": "Pay the ransom?", "options": ["Pay", "Refuse"], "duration_minutes": 5}}),
            T0,
        );
        let Event::PollUpserted { poll } = &ev[0] else { panic!() };
        let (poll_id, pay, refuse) = (poll.id.clone(), poll.options[0].id.clone(), poll.options[1].id.clone());

        let first = inject(
            &mut ex,
            "grid down",
            json!({"type": "when", "condition": {"type": "asset_status", "asset": grid, "set": "security", "value": "compromised"}}),
        );
        // Firing the first opens the poll.
        run(
            &mut ex,
            json!({"type": "edit_inject", "id": first, "inject": {"title": "grid down", "effects": [{"type": "open_poll", "poll": poll_id}],
                   "trigger": {"type": "when", "condition": {"type": "asset_status", "asset": grid, "set": "security", "value": "compromised"}}}}),
            T0,
        );
        inject(&mut ex, "follow-up", json!({"type": "when", "condition": {"type": "inject_fired", "inject": first}}));
        inject(
            &mut ex,
            "they paid",
            json!({"type": "when", "condition": {"type": "poll_choice", "poll": poll_id, "option": pay}}),
        );
        inject(
            &mut ex,
            "they refused",
            json!({"type": "when", "condition": {"type": "poll_choice", "poll": poll_id, "option": refuse}}),
        );

        run(&mut ex, json!({"type": "set_status", "ids": [grid], "set": "security", "value": "compromised"}), &at(1));
        let fired = settle(&mut ex, &at(1));
        assert_eq!(fired, ["grid down", "follow-up"], "a chain resolves in one go");
        assert_eq!(ex.polls[&poll_id].state, PollState::Open);

        for (who, choice) in [("a", &refuse), ("b", &refuse), ("c", &pay)] {
            let by = Responder { participant: who.into(), name: who.into(), team: None };
            run_at(
                &mut ex,
                Command::Respond { poll: poll_id.clone(), option: Some(choice.clone()), text: None, by: Some(by) },
                &at(2),
            );
        }
        // Changing your mind replaces your answer rather than adding one.
        let by = Responder { participant: "c".into(), name: "c".into(), team: None };
        run_at(
            &mut ex,
            Command::Respond { poll: poll_id.clone(), option: Some(refuse.clone()), text: None, by: Some(by) },
            &at(3),
        );
        assert_eq!(ex.polls[&poll_id].results[&refuse], 3);
        assert!(settle(&mut ex, &at(4)).is_empty(), "still open");
        // Five minutes after opening, the poll closes itself and the branch fires.
        assert_eq!(settle(&mut ex, &at(6)), ["they refused"]);
        assert_eq!(ex.polls[&poll_id].state, PollState::Closed);
    }

    #[test]
    fn thresholds_trigger_and_disarmed_injects_wait() {
        let mut ex = blank("Drill", "", T0).unwrap();
        let ev = run(&mut ex, json!({"type": "add_assets", "assets": [{"name": "H", "type_id": "hospitals"}]}), T0);
        let Event::AssetsUpserted { assets } = &ev[0] else { panic!() };
        let h = assets[0].id.clone();
        let ev = run(
            &mut ex,
            json!({"type": "add_threshold", "name": "Medical low", "sector": "medical", "below_pct": 50}),
            T0,
        );
        let Event::ThresholdUpserted { threshold } = &ev[0] else { panic!() };
        let id = inject(
            &mut ex,
            "divert",
            json!({"type": "when", "condition": {"type": "threshold_breached", "threshold": threshold.id}}),
        );
        run(&mut ex, json!({"type": "arm_inject", "id": id, "armed": false}), T0);
        run(&mut ex, json!({"type": "set_status", "ids": [h], "set": "security", "value": "compromised"}), T0);
        assert!(settle(&mut ex, T0).is_empty(), "disarmed");
        run(&mut ex, json!({"type": "arm_inject", "id": id, "armed": true}), T0);
        assert_eq!(settle(&mut ex, T0), ["divert"]);
    }
}
