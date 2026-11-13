//! The after-action report, as Markdown or as a printable HTML page.
//!
//! Both are rendered from one outline of [`Block`]s, so they always say the
//! same thing. Everything in it comes from the exercise as it stands and its
//! event log: the log supplies what the final state cannot, such as the
//! exercise clock at any past moment and which changes were undone.

use std::{collections::HashSet, fmt::Write};

use chrono::{DateTime, FixedOffset};

use crate::{
    exercise::{
        Envelope, Event, Exercise,
        model::{Clock, PollState, Verdict, Visibility},
    },
    live::ENGINE,
};

enum Block {
    H(u8, String),
    P(String),
    /// Label and value pairs.
    Facts(Vec<(&'static str, String)>),
    Table {
        head: Vec<&'static str>,
        rows: Vec<Vec<String>>,
    },
    /// Free text kept as written, line breaks and all.
    Text(String),
}

#[derive(Clone, Copy)]
pub enum Format {
    Markdown,
    Html,
}

pub fn render(ex: &Exercise, events: &[Envelope], generated: &str, format: Format) -> String {
    let blocks = outline(ex, events, generated);
    match format {
        Format::Markdown => markdown(&blocks),
        Format::Html => html(ex, &blocks),
    }
}

/// The exercise clock at any moment, rebuilt from the log. The clock can be
/// paused, set and reset, so wall time alone does not tell.
struct Clocks(Vec<(DateTime<FixedOffset>, Clock)>);

impl Clocks {
    fn new(events: &[Envelope]) -> Self {
        fn walk(at: DateTime<FixedOffset>, e: &Event, out: &mut Vec<(DateTime<FixedOffset>, Clock)>) {
            match e {
                Event::Created { exercise } => out.push((at, exercise.clock.clone())),
                Event::ClockChanged { clock } => out.push((at, clock.clone())),
                Event::Batch { events } => events.iter().for_each(|e| walk(at, e, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        for env in events {
            if let Ok(at) = DateTime::parse_from_rfc3339(&env.at) {
                walk(at, &env.event, &mut out);
            }
        }
        Clocks(out)
    }

    /// Milliseconds on the exercise clock at wall time `at`.
    fn at(&self, at: &str) -> u64 {
        let Ok(t) = DateTime::parse_from_rfc3339(at) else { return 0 };
        let idx = self.0.partition_point(|(when, _)| *when <= t);
        match idx.checked_sub(1) {
            Some(i) => self.0[i].1.elapsed_at(at),
            None => 0,
        }
    }
}

/// `T+1:05` for an hour and five minutes.
fn t_plus(ms: u64) -> String {
    let m = ms / 60_000;
    format!("T+{}:{:02}", m / 60, m % 60)
}

fn minutes(ms: u64) -> String {
    let m = (ms as f64 / 60_000.0).round() as u64;
    if m >= 60 { format!("{}h {:02}m", m / 60, m % 60) } else { format!("{m} min") }
}

fn date(at: &str) -> String {
    DateTime::parse_from_rfc3339(at).map(|d| d.format("%-d %B %Y").to_string()).unwrap_or_default()
}

fn outline(ex: &Exercise, events: &[Envelope], generated: &str) -> Vec<Block> {
    let clocks = Clocks::new(events);
    let end = events.last().map_or(ex.updated_at.as_str(), |e| e.at.as_str());
    let total = ex.clock.elapsed_at(end);
    let team_name = |id: &str| ex.teams.iter().find(|t| t.id == id).map_or_else(|| id.to_owned(), |t| t.name.clone());
    let mut b = Vec::new();

    // ── Overview
    b.push(Block::H(1, format!("{}: after-action report", ex.name)));
    let started = events.iter().find_map(|e| match &e.event {
        Event::ClockChanged { clock } if clock.running_since.is_some() => Some(e.at.as_str()),
        _ => None,
    });
    let fired = ex.injects.values().filter(|i| i.fired_at.is_some()).count();
    let asked = ex.polls.values().filter(|p| p.state != PollState::Draft).count();
    let undone: usize = events
        .iter()
        .map(|e| match &e.event {
            Event::Batch { events } => events.iter().filter(|e| matches!(e, Event::Undone { .. })).count(),
            Event::Undone { .. } => 1,
            _ => 0,
        })
        .sum();
    let mut facts = vec![];
    if !ex.branding.org.is_empty() {
        facts.push(("Organisation", ex.branding.org.clone()));
    }
    facts.push(("Date", date(started.unwrap_or(&ex.created_at))));
    facts.push(("Exercise time", minutes(total)));
    if !ex.teams.is_empty() {
        facts.push(("Teams", ex.teams.iter().map(|t| t.name.as_str()).collect::<Vec<_>>().join(", ")));
    }
    facts.push(("Injects", format!("{fired} of {} fired", ex.injects.len())));
    if !ex.polls.is_empty() {
        facts.push(("Decisions", format!("{asked} of {} put to the room", ex.polls.len())));
    }
    facts.push(("Log entries", ex.log.len().to_string()));
    if undone > 0 {
        facts.push(("Changes undone", undone.to_string()));
    }
    b.push(Block::Facts(facts));
    if !ex.description.is_empty() {
        b.push(Block::P(ex.description.clone()));
    }

    // ── The hot wash
    for (title, text) in [
        ("Summary", &ex.review.summary),
        ("Strengths", &ex.review.strengths),
        ("Areas for improvement", &ex.review.improvements),
    ] {
        if !text.trim().is_empty() {
            b.push(Block::H(2, title.into()));
            b.push(Block::Text(text.trim().to_owned()));
        }
    }

    // ── Expected actions
    let expected: Vec<_> = ex.injects.values().flat_map(|i| i.expected.iter().map(move |e| (i, e))).collect();
    if !expected.is_empty() {
        b.push(Block::H(2, "Expected actions".into()));
        let count = |team: Option<&str>, v: Option<Verdict>| {
            expected.iter().filter(|(_, e)| team.is_none_or(|t| e.team.as_deref() == Some(t)) && e.verdict == v).count()
        };
        let row = |name: String, team: Option<&str>| {
            let (met, part, missed, open) = (
                count(team, Some(Verdict::Met)),
                count(team, Some(Verdict::Partial)),
                count(team, Some(Verdict::Missed)),
                count(team, None),
            );
            let judged = met + part + missed;
            let score = if judged == 0 {
                "–".into()
            } else {
                format!("{:.0}%", (met as f64 + part as f64 / 2.0) * 100.0 / judged as f64)
            };
            vec![name, met.to_string(), part.to_string(), missed.to_string(), open.to_string(), score]
        };
        let mut rows = vec![row("All".into(), None)];
        for t in &ex.teams {
            if expected.iter().any(|(_, e)| e.team.as_ref() == Some(&t.id)) {
                rows.push(row(t.name.clone(), Some(&t.id)));
            }
        }
        b.push(Block::Table { head: vec!["", "Met", "Partly met", "Missed", "Not assessed", "Score"], rows });
        let rows = expected
            .iter()
            .map(|(i, e)| {
                vec![
                    i.title.clone(),
                    i.fired_at.as_deref().map_or_else(|| "not fired".into(), |at| t_plus(clocks.at(at))),
                    e.text.clone(),
                    e.team.as_deref().map_or_else(|| "Anyone".into(), team_name),
                    e.verdict.map_or("Not assessed", Verdict::label).to_owned(),
                    e.note.clone(),
                ]
            })
            .collect();
        b.push(Block::Table { head: vec!["Inject", "Fired", "Action", "Team", "Verdict", "Note"], rows });
    }

    // ── Decisions
    let polls: Vec<_> = ex.polls.values().filter(|p| p.state != PollState::Draft).collect();
    if !polls.is_empty() {
        b.push(Block::H(2, "Decisions".into()));
        for p in polls {
            b.push(Block::H(3, p.question.clone()));
            let mut facts = vec![];
            if let Some(at) = &p.opened_at {
                facts.push(("Opened", t_plus(clocks.at(at))));
            }
            match &p.closed_at {
                Some(at) => facts.push(("Closed", t_plus(clocks.at(at)))),
                None => facts.push(("Closed", "still open".into())),
            }
            facts.push(("Answers", p.responses.len().to_string()));
            b.push(Block::Facts(facts));
            if !p.options.is_empty() {
                let winners = p.winners();
                let rows = p
                    .options
                    .iter()
                    .map(|o| {
                        let won = p.state == PollState::Closed && winners.contains(&o.id.as_str());
                        vec![
                            if won { format!("{} (chosen)", o.label) } else { o.label.clone() },
                            p.results.get(&o.id).copied().unwrap_or(0).to_string(),
                        ]
                    })
                    .collect();
                b.push(Block::Table { head: vec!["Option", "Votes"], rows });
            }
            let said: Vec<_> = p.responses.iter().filter(|r| !r.text.trim().is_empty()).collect();
            if !said.is_empty() {
                let rows = said
                    .iter()
                    .map(|r| {
                        vec![r.name.clone(), r.team.as_deref().map_or_else(String::new, team_name), r.text.clone()]
                    })
                    .collect();
                b.push(Block::Table { head: vec!["Who", "Team", "Said"], rows });
            }
        }
    }

    // ── Phases
    if !ex.phase_log.is_empty() {
        b.push(Block::H(2, "Phases".into()));
        let rows = ex
            .phase_log
            .iter()
            .enumerate()
            .map(|(n, entry)| {
                let phase = ex.phases.iter().find(|p| p.id == entry.phase);
                let until = ex.phase_log.get(n + 1).map_or(total, |next| next.elapsed_ms);
                vec![
                    phase.map_or_else(|| "(removed)".into(), |p| p.name.clone()),
                    t_plus(entry.elapsed_ms),
                    minutes(until.saturating_sub(entry.elapsed_ms)),
                    phase.map_or_else(String::new, |p| format!("{} min", p.target_minutes)),
                ]
            })
            .collect();
        b.push(Block::Table { head: vec!["Phase", "Began", "Took", "Planned"], rows });
    }

    // ── Timeline: the log, phases and decisions in one list, by clock time.
    // An inject's own log line already says it fired, so it stands in for it.
    // The log does not say who wrote a line; the event that wrote it does.
    let mut by_script = HashSet::new();
    for env in events.iter().filter(|e| e.actor == ENGINE) {
        let mut stack = vec![&env.event];
        while let Some(e) = stack.pop() {
            match e {
                Event::Logged { entry } | Event::InjectFired { entry, .. } => {
                    by_script.insert(entry.id.as_str());
                }
                Event::Batch { events } => stack.extend(events),
                _ => {}
            }
        }
    }
    let mut lines: Vec<(u64, String, String, String)> = Vec::new();
    for entry in &ex.log {
        let who = match (&entry.visibility, entry.author.is_empty()) {
            (_, false) => entry.author.clone(),
            _ if by_script.contains(entry.id.as_str()) => "Script".into(),
            (Visibility::Facilitator, true) => "Facilitator (private)".into(),
            _ => "Facilitator".into(),
        };
        let mut what = entry.text.clone();
        if let Some(asset) = &entry.asset_name {
            what = format!("{what} ({asset})");
        }
        lines.push((clocks.at(&entry.at), entry.at.clone(), who, what));
    }
    for entry in &ex.phase_log {
        if let Some(p) = ex.phases.iter().find(|p| p.id == entry.phase) {
            lines.push((entry.elapsed_ms, entry.at.clone(), "Clock".into(), format!("Phase began: {}", p.name)));
        }
    }
    for p in ex.polls.values() {
        if let Some(at) = &p.opened_at {
            lines.push((
                clocks.at(at),
                at.clone(),
                "Facilitator".into(),
                format!("Decision put to the room: {}", p.question),
            ));
        }
        if let Some(at) = &p.closed_at {
            let won: Vec<&str> = p
                .winners()
                .iter()
                .filter_map(|id| p.options.iter().find(|o| o.id == *id).map(|o| o.label.as_str()))
                .collect();
            let result = if won.is_empty() { "no answer".to_owned() } else { won.join(" / ") };
            lines.push((clocks.at(at), at.clone(), "Room".into(), format!("Decided: {result}")));
        }
    }
    if !lines.is_empty() {
        lines.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        b.push(Block::H(2, "Timeline".into()));
        let rows = lines.into_iter().map(|(ms, _, who, what)| vec![t_plus(ms), who, what]).collect();
        b.push(Block::Table { head: vec!["Time", "Who", "What"], rows });
    }

    // ── Assets that changed. History starts with each asset's first
    // statuses, stamped with its creation; those are not changes.
    let changes = |a: &crate::exercise::model::Asset| a.history.iter().filter(|h| h.at != a.created_at).count();
    let mut changed: Vec<_> = ex.assets.values().filter(|a| changes(a) > 0).collect();
    if !changed.is_empty() {
        changed.sort_by_key(|a| std::cmp::Reverse(changes(a)));
        b.push(Block::H(2, "Assets that changed".into()));
        let label = |set: &str, value: &str| {
            ex.taxonomy
                .status_sets
                .iter()
                .find(|s| s.id == set)
                .and_then(|s| s.value(value).map(|v| format!("{}: {}", s.name, v.label)))
                .unwrap_or_else(|| format!("{set}: {value}"))
        };
        let rows = changed
            .iter()
            .map(|a| {
                let sets: HashSet<&str> =
                    a.history.iter().filter(|h| h.at != a.created_at).map(|h| h.set.as_str()).collect();
                let now: Vec<String> =
                    a.statuses.iter().filter(|(s, _)| sets.contains(s.as_str())).map(|(s, v)| label(s, v)).collect();
                vec![a.name.clone(), changes(a).to_string(), now.join(", ")]
            })
            .collect();
        b.push(Block::Table { head: vec!["Asset", "Changes", "Ended as"], rows });
    }

    b.push(Block::P(format!("Generated by COPPIR on {}.", date(generated))));
    b
}

// ───────────────────────────── Markdown ─────────────────────────────

fn markdown(blocks: &[Block]) -> String {
    let cell = |s: &str| s.replace('|', "\\|").replace(['\r', '\n'], " ");
    let mut out = String::new();
    for block in blocks {
        match block {
            Block::H(level, text) => {
                let _ = writeln!(out, "{} {}\n", "#".repeat(usize::from(*level)), cell(text));
            }
            Block::P(text) | Block::Text(text) => {
                let _ = writeln!(out, "{text}\n");
            }
            Block::Facts(facts) => {
                for (k, v) in facts {
                    let _ = writeln!(out, "- **{k}:** {}", cell(v));
                }
                out.push('\n');
            }
            Block::Table { head, rows } => {
                let _ = writeln!(out, "| {} |", head.join(" | "));
                let _ = writeln!(out, "|{}|", head.iter().map(|_| " --- ").collect::<Vec<_>>().join("|"));
                for row in rows {
                    let _ = writeln!(out, "| {} |", row.iter().map(|c| cell(c)).collect::<Vec<_>>().join(" | "));
                }
                out.push('\n');
            }
        }
    }
    out
}

// ───────────────────────────── HTML ─────────────────────────────

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// A colour fit to put in a stylesheet, or the fallback.
fn css_color<'a>(c: Option<&'a str>, fallback: &'a str) -> &'a str {
    match c {
        Some(c) if c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit()) => c,
        _ => fallback,
    }
}

fn html(ex: &Exercise, blocks: &[Block]) -> String {
    let accent = css_color(ex.branding.accent.as_deref(), "#1f6feb");
    let mut body = String::new();
    let banner = ex.branding.banner.as_ref().map(|b| {
        format!(
            "<div class=\"banner\" style=\"background:{}\">{}</div>",
            css_color(Some(&b.color), "#b8860b"),
            esc(&b.text)
        )
    });
    if let Some(b) = &banner {
        body.push_str(b);
    }
    if let Some(logo) = &ex.branding.logo {
        let _ = write!(body, "<img class=\"logo\" alt=\"\" src=\"{}\">", esc(logo));
    }
    for block in blocks {
        match block {
            Block::H(level, text) => {
                let _ = write!(body, "<h{level}>{}</h{level}>", esc(text));
            }
            Block::P(text) => {
                let _ = write!(body, "<p>{}</p>", esc(text));
            }
            Block::Text(text) => {
                let _ = write!(body, "<div class=\"text\">{}</div>", esc(text));
            }
            Block::Facts(facts) => {
                body.push_str("<dl>");
                for (k, v) in facts {
                    let _ = write!(body, "<dt>{}</dt><dd>{}</dd>", esc(k), esc(v));
                }
                body.push_str("</dl>");
            }
            Block::Table { head, rows } => {
                body.push_str("<table><thead><tr>");
                for h in head {
                    let _ = write!(body, "<th>{}</th>", esc(h));
                }
                body.push_str("</tr></thead><tbody>");
                for row in rows {
                    body.push_str("<tr>");
                    for c in row {
                        let class = match c.as_str() {
                            "Met" => " class=\"met\"",
                            "Partly met" => " class=\"partial\"",
                            "Missed" => " class=\"missed\"",
                            _ => "",
                        };
                        let _ = write!(body, "<td{class}>{}</td>", esc(c));
                    }
                    body.push_str("</tr>");
                }
                body.push_str("</tbody></table>");
            }
        }
    }
    if let Some(b) = &banner {
        body.push_str(b);
    }
    format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}: after-action report</title>
<style>
:root {{ --accent: {accent}; }}
body {{ font: 14px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif; color: #1b1f24; background: #fff; max-width: 960px; margin: 0 auto; padding: 24px 16px 48px; }}
h1 {{ font-size: 26px; margin: 8px 0 12px; border-bottom: 3px solid var(--accent); padding-bottom: 8px; }}
h2 {{ font-size: 18px; margin: 28px 0 8px; color: var(--accent); break-after: avoid; }}
h3 {{ font-size: 15px; margin: 16px 0 6px; break-after: avoid; }}
dl {{ display: grid; grid-template-columns: max-content 1fr; gap: 2px 16px; margin: 0 0 12px; }}
dt {{ color: #57606a; }} dd {{ margin: 0; }}
table {{ width: 100%; border-collapse: collapse; margin: 4px 0 12px; font-size: 13px; }}
th, td {{ text-align: left; vertical-align: top; padding: 5px 8px; border-bottom: 1px solid #d0d7de; }}
th {{ background: #f6f8fa; font-weight: 600; }}
tr {{ break-inside: avoid; }}
td.met {{ color: #1a7f37; font-weight: 600; }} td.partial {{ color: #9a6700; font-weight: 600; }} td.missed {{ color: #cf222e; font-weight: 600; }}
.text {{ white-space: pre-wrap; background: #f6f8fa; border-left: 3px solid var(--accent); padding: 8px 12px; }}
.banner {{ color: #fff; text-align: center; font-weight: 700; letter-spacing: 0.15em; padding: 3px; margin: 0 0 12px; -webkit-print-color-adjust: exact; print-color-adjust: exact; }}
.banner:last-child {{ margin: 24px 0 0; }}
.logo {{ max-height: 56px; max-width: 240px; float: right; }}
@media print {{ body {{ padding: 0; max-width: none; }} }}
</style></head>
<body>{body}</body></html>
"#,
        title = esc(&ex.name),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_follows_pauses() {
        let env = |seq, at: &str, clock: Clock| Envelope {
            seq,
            at: at.into(),
            actor: String::new(),
            event: Event::ClockChanged { clock },
        };
        let events = vec![
            env(
                1,
                "2026-01-01T10:00:00+00:00",
                Clock { elapsed_ms: 0, running_since: Some("2026-01-01T10:00:00+00:00".into()) },
            ),
            // Paused at ten minutes, for an hour.
            env(2, "2026-01-01T10:10:00+00:00", Clock { elapsed_ms: 600_000, running_since: None }),
            env(
                3,
                "2026-01-01T11:10:00+00:00",
                Clock { elapsed_ms: 600_000, running_since: Some("2026-01-01T11:10:00+00:00".into()) },
            ),
        ];
        let c = Clocks::new(&events);
        assert_eq!(c.at("2026-01-01T09:00:00+00:00"), 0);
        assert_eq!(c.at("2026-01-01T10:05:00+00:00"), 300_000);
        assert_eq!(c.at("2026-01-01T10:40:00+00:00"), 600_000);
        assert_eq!(c.at("2026-01-01T11:15:00+00:00"), 900_000);
        assert_eq!(t_plus(3_900_000), "T+1:05");
    }

    #[test]
    fn html_is_escaped() {
        assert_eq!(esc("<script>alert('x')</script>&"), "&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt;&amp;");
        assert_eq!(css_color(Some("red;}body{display:none"), "#000000"), "#000000");
    }
}
