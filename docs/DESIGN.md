# COPPIR v2 Design

Status: milestone 7 done · 2026-11-14

## Goal

A tabletop exercise platform for incident response. One facilitator starts it
in seconds. The room joins from phones and laptops with no install. Every
exercise can be reshaped (taxonomies, fields, branding, scripted injects)
without touching code. It stays one self-contained binary.

## Decisions

| Area | Decision |
|---|---|
| Access | Facilitator hosts on a laptop. Players join over the LAN by link or QR code with a short join code. |
| Canvas | One set of assets, shown either as a geographic map or as a topology/dependency graph. |
| Scenarios | Scripted MSEL timeline, branching injects, player decisions and polls, after-action replay and report. |
| Roles | Facilitator, player and observer. Players belong to teams. Visibility is per team and enforced on the server. |
| Customization | Taxonomies, custom asset fields, built-in templates, branding and exercise banners. |
| Storage | Embedded SQLite with an append-only event log. |
| Maps | Keyless vector tiles online (OpenFreeMap) plus optional offline region packs, stored as MBTiles. |
| Frontend | TypeScript + Svelte 5 + MapLibre GL, built with Vite and embedded in the binary. |

## Concepts

**Exercise** — the authored package: settings, taxonomy, teams, assets, links,
zones, phases, injects and polls. A template is an exercise, and a `.coppir`
file holds one.

**Run** — one play-through of an exercise. It has its own clock and event log.
An exercise can be run many times. Each run keeps its own replay.

**Taxonomy** — per-exercise definitions that are hard-coded today:

- *Asset types*: name, icon, color, sector, default fields.
- *Status sets*: ordered values, each with a color, glyph and health score.
  "Security" and "Operational" ship as defaults. An exercise can rename them,
  change them, or add more, such as "Comms" or "Legal hold".
- *Sectors*: groups of asset types. Each sector drives a health wheel.
- *Fields*: a typed schema for custom asset properties (text, number, select,
  boolean, URL), each marked facilitator-only or visible to players.

**Asset** — type, name, status per status set, field values and tags. It has
an optional geographic position and an optional graph position, so the same
asset can appear in both views. Assets are visible to all teams, to some
teams, or only to the facilitator. A facilitator can reveal them during a run.

**Link** — a typed, directed relationship between assets: *depends on*,
*connects to* or *managed by*. Optional impact rules propagate status, for
example "if the upstream asset goes Offline, mark dependents Degraded".

**Inject** — a scripted event. It has:

- *Content*: title, a markdown body, and attachments such as a mock ransom
  note, a phishing email or a log excerpt.
- *Audience*: all teams or chosen ones.
- *Trigger*: manual, at an exercise time, at the start of a phase, or on a
  condition (an asset status, a threshold breach, a poll result, another
  inject having fired).
- *Effects*: set status, reveal or create an asset, open a poll, write to the
  log.
- *Facilitator notes and expected actions*, used to score the after-action
  report.

Facilitators build branching by chaining triggers and effects in a rule
builder. They do not write expressions.

**Poll / decision** — a prompt sent to teams: multiple choice or free text,
with an optional deadline. Responses are timestamped events. Results can
trigger injects.

**Phase** — a named stage (Detection, Containment, Eradication, Recovery) with
a target duration. Phases sit on the MSEL timeline.

## Architecture

```
coppir (single binary)
├── axum server
│   ├── /api/v2 (typed REST; TS types generated from Rust)
│   ├── /ws (one socket per client, filtered per role and team)
│   ├── auth (facilitator token, join codes, player sessions)
│   └── engine (clock, trigger evaluation, impact propagation)
├── store: SQLite (events, snapshots, exercises, attachments)
├── embedded web/dist (Svelte app)
├── embedded templates/*.coppir
└── desktop window (wry): facilitator console, QR panel
```

### Event store

- SQLite stores exercise definitions as versioned documents.
- A run is an append-only `events` table: sequence number, wall time,
  exercise-clock time, actor, kind, and a JSON payload.
- The server keeps live state in memory as a fold over the events. It takes a
  snapshot every N events so restarts are fast.
- Undo appends a compensating event. History is never rewritten.
- Replay folds the events up to any sequence number. The after-action report
  reads the same log.
- A crash loses nothing. SQLite runs in WAL mode, and the server commits each
  event before it broadcasts it.
- Export and import use a `.coppir` file: a zip of exercise JSON,
  attachments, and runs if chosen. The importer reads scenario JSON from v1
  and the Python version into a new exercise.

### Real-time protocol

- On connect, the client gets a snapshot with the current sequence number, then
  the events after it.
- On reconnect, the client sends its last sequence number and receives only
  what it missed. A full reload happens only if the gap is too large.
- The server filters each event for each connection by role and team. Hidden
  assets and injects never reach a player's device.
- The server keeps today's lock-and-publish ordering guarantee.

### Access and auth

- **Facilitator**: the desktop window holds an admin token. A facilitator can
  pair a remote device by scanning a one-time admin QR code.
- **Open to room**: an explicit toggle binds the LAN interface. It shows the
  URL, a QR code, and a 6-character join code that the facilitator can rotate.
- **Players** enter a display name and pick a team or receive one. They get
  an HttpOnly session cookie. The facilitator can rename, move or kick
  players.
- **Observers** join with a separate code and get a read-only projector view.
- The Host/Origin guard extends to the LAN address. The server rate limits
  join attempts.
- LAN traffic is plain HTTP on the first pass. TLS with a self-signed or
  provided certificate is a later option. Phones warn about self-signed
  certificates, which is why it is not the default.

### Frontend (`web/`)

Svelte 5 + TypeScript + Vite. `cargo build` embeds `web/dist`, and CI builds
it first. Contributors who only work on Rust can use the prebuilt bundle
from CI.

| View | For | Main features |
|---|---|---|
| **Author** | Facilitator before the exercise | Taxonomy and field editors, asset library, map and graph editing, MSEL builder, rule builder, team visibility matrix, template picker |
| **Console** | Facilitator during the run | Live canvas, clock (start, pause, jump to phase), inject queue with "fire now", decision feed, team presence, quick status edits, undo |
| **Player** | Teams, phone-first | Their team's partial picture, an inject inbox with attachments, decision prompts, team notes and log |
| **Projector** | Observers and big screens | SITREP wheels, canvas, inject ticker, clock, exercise banner. Kiosk mode. |
| **Review** | Everyone after the run | Replay scrubber, timeline lanes per team, expected vs actual actions, exportable AAR (HTML, print-to-PDF, Markdown) |

**Libraries**

- **Map**: MapLibre GL JS with OpenFreeMap styles, and terra-draw for zones,
  perimeters and markup. Offline packs are MBTiles files the server
  downloads and serves. PMTiles was the first plan, but SQLite is already in the
  binary, and serving tiles by URL needs nothing new in the browser.
- **Graph**: Svelte Flow (`@xyflow/svelte`) with dagre auto-layout. (ELK was the first plan; dagre gives the same layered layout at a fraction of the size.)
- **Geocoding**: Photon (komoot) first, falling back to Nominatim. Overpass
  import stays.
- **Theming**: CSS design tokens. Branding swaps the tokens and the logo. The
  current terminal-green look becomes one theme among several.

### Performance targets

- Cold start to usable window in under 1 s. The binary is under 30 MB,
  excluding offline map packs.
- 5,000 assets render smoothly on the map. 500 nodes render smoothly in the
  graph.
- 50 connected clients. An event reaches every device in under 100 ms on a
  LAN.

## Milestones

Each milestone ships something runnable and keeps the test suite green.

1. ✅ **Foundations**: SQLite event store, the v2 domain model with a default
   taxonomy that reproduces today's behavior, v1 scenario import, typed
   `/api/v2` with generated TS types, and a `web/` scaffold that renders a
   loaded exercise on MapLibre.
2. ✅ **Facilitator parity**: the new UI matches the current app (assets, links,
   log, injects, thresholds, zones and markup, exports, undo). Taxonomy
   editing. The vanilla-JS frontend and v1 API are removed.
3. ✅ **Room access**: LAN mode, QR codes and join codes, roles, teams,
   server-side visibility, the phone player view and the projector view.
4. ✅ **Scripted exercises**: exercise clock, phases, the MSEL builder,
   scheduled and conditional injects, impact propagation, polls and
   decisions.
5. ✅ **Topology and customization**: the graph view, custom fields, branding
   and banners, and built-in templates (ransomware, BEC, supply-chain
   compromise, regional utility outage).
6. ✅ **Review**: replay with team lanes, expected actions and verdicts, the
   hot wash, the AAR report (HTML, print-to-PDF, Markdown), and offline map
   packs.
7. ✅ **Packaging**: installers for macOS (universal `.dmg`), Windows (NSIS)
   and Linux (`.deb`, AppImage), built and drafted as a GitHub release from a
   version tag. Signing with paid certificates is deferred. The macOS app is
   signed ad hoc, and `docs/RELEASING.md` says how to add real signing later.

## Open questions

Each has a default. I will proceed with it unless you say otherwise.

1. **Remote facilitator**: pairing a second device as co-facilitator. Default:
   yes, in milestone 3.
2. **Inject attachments**: images, PDFs and plain text, capped at 20 MB per
   exercise. Default: yes.
3. **Player identity**: display name only, with no accounts. Default: yes.
4. **v1 API**: removed in milestone 2, with v1 scenarios still importable.
   Default: yes.
5. **Node in the build**: required to build the frontend, while Rust-only
   contributors use the CI bundle. Default: yes.
