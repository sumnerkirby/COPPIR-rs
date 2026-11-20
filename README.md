# COPPIR

**Common Operational Picture Program for Incident Response**

COPPIR builds and runs a live common operational picture (a shared map of an incident) for incident response
tabletop exercises. It is one executable for macOS, Windows or Linux. It has no
accounts, no cloud service and no database server. Only map tiles, place search
and OpenStreetMap lookups use the network.

![COPPIR running a demo incident scenario in Washington DC](docs/screenshot.png)

---

## Contents

- [Getting started](#getting-started)
- [Exercises](#exercises)
- [Ready-made exercises](#ready-made-exercises)
- [The map](#the-map)
- [The topology graph](#the-topology-graph)
- [Assets and links](#assets-and-links)
- [SITREP strip](#sitrep-strip)
- [The script](#the-script)
- [Decision log](#decision-log)
- [Metrics and thresholds](#metrics-and-thresholds)
- [The room: players and observers](#the-room-players-and-observers)
- [Taxonomy](#taxonomy)
- [Look and banner](#look-and-banner)
- [Review and the after-action report](#review-and-the-after-action-report)
- [Undo](#undo)
- [Export](#export)
- [Keyboard shortcuts](#keyboard-shortcuts)
- [Development](#development)

---

## Getting started

### Installing

Download the installer for your system from the
[releases page](https://github.com/sumnerkirby/COPPIR-rs/releases):

| System | Installer |
|---|---|
| macOS 11 or later | `COPPIR_<version>_universal.dmg`: open it and drag COPPIR to Applications |
| Windows 10 or 11 | `coppir_<version>_x64-setup.exe`: installs for you alone, no administrator needed |
| Ubuntu, Debian | `coppir_<version>_amd64.deb`: `sudo apt install ./coppir_*_amd64.deb` |
| Other Linux | `coppir_<version>_x86_64.AppImage`: `chmod +x` it and run it |

Only the facilitator installs COPPIR. Players and observers join from a browser.

The installers are not signed with a paid certificate, so the first launch
needs one extra click:

- **macOS** says it cannot check the app. Open **System Settings → Privacy &
  Security** and click **Open Anyway**.
- **Windows** SmartScreen says it protected your PC. Click **More info**, then
  **Run anyway**.

Exercises and map packs live in the per-user data directory (see
[Options](#options)), so reinstalling or upgrading keeps them. Uninstalling
does too.

### Building from source

You need Rust 1.85 or newer and Node 22 or newer. On Linux, also install the
WebKitGTK development packages (`libwebkit2gtk-4.1-dev libgtk-3-dev` on
Debian and Ubuntu).

```bash
npm --prefix web ci && npm --prefix web run build
cargo run --release
```

That opens the desktop window. To run only the server and use your own browser,
add `--headless` and open the address it prints (`http://127.0.0.1:8000` by
default):

```bash
cargo run --release -- --headless
```

To build a server-only binary with no GUI dependencies:

```bash
cargo build --release --no-default-features
```

### Options

| Flag | Env var | Default | |
|---|---|---|---|
| `--port` | `COPPIR_PORT` | `8000` | If the port is taken, the app uses a free one |
| `--strict-port` | | off | Fail instead of falling back to another port |
| `--database` | `COPPIR_DATABASE` | `coppir.db` in the per-user data directory | Where exercises are stored |
| `--scenarios-dir` | `COPPIR_SCENARIOS_DIR` | `scenarios` in the per-user data directory | Where version 1 saved scenarios, so they can be imported |
| `--maps-dir` | `COPPIR_MAPS_DIR` | `maps` in the per-user data directory | Where offline map packs are kept |
| `--headless` | | off | Serve only; no window |

On Windows, run the app from a terminal to see `--headless` output and logs
there. Started from the Start menu it opens no console window.

The per-user data directory is `~/Library/Application Support/COPPIR` on macOS,
`%APPDATA%\COPPIR\data` on Windows and `~/.local/share/coppir` on Linux.
Logging follows `RUST_LOG`, for example `RUST_LOG=coppir=debug`.

### Loading the demo

To see the interface populated, run the seeder against a running instance:

```bash
cargo run --example seed_demo
```

It builds the exercise in the screenshot: a fictional incident in Washington DC
with 53 assets, dependencies, fired and queued injects, alert thresholds,
metrics, a cordon and a decision log. Each run creates a new exercise and prints
its address. To seed an instance on another port, set `COPPIR_API`.

---

## Exercises

The start page lists your exercises. From there you can:

- **Create** an empty exercise with the default taxonomy.
- **Open** one. You can bookmark its address (`/x/<id>`) or open it in a second
  window. Every window shows the same picture, live.
- **Duplicate** one to run it again from where it stands, or to start from a
  template.
- **Export** one as a `.coppir` file, and **import** that file on another
  machine. Import also accepts scenario `.json` files saved by version 1 or the
  Python app.
- **Delete** one, with its history.

COPPIR writes every change to disk before it appears on screen, so a crash or a
power cut loses nothing. When the start page finds scenarios left by version 1
in the scenarios folder, it offers to import each of them.

---

## Ready-made exercises

The start page offers four exercises you can run as they are or adapt. Each
comes with its own asset types and custom fields, teams, phases, a scripted
timeline of injects, a poll that branches the story, impact rules and an
exercise banner.

| Exercise | View | Length | Teams |
|---|---|---|---|
| **Hospital ransomware**: clinical systems fail one after another; isolate the network, keep patients safe, decide whether to pay | Graph | ~100 min | IT & Security, Clinical leadership, Legal & Comms |
| **Business email compromise**: a "CEO" asks for an urgent wire; follow the money, the mailbox and the vendor | Graph | ~75 min | Finance, IT Security, Legal |
| **Supply-chain compromise**: a vendor's update carries a backdoor into your monitoring platform | Graph | ~90 min | Security operations, Engineering, Customer & Legal |
| **Regional utility outage**: control-system intrusion during a heat wave; substations, hospitals, water and transit | Map | ~100 min | Utility, County emergency management, Health & water |

Starting one makes an ordinary exercise: change anything in it. Everything in
them is fictional. The templates are generated by `templates/generate.py`.

---

## The map

The map uses OpenFreeMap's dark vector tiles, which need no API key. Pan by
dragging and zoom with the scroll wheel. The toolbar at the top left holds:

| Tool | |
|---|---|
| **Search** | Find a place by name. Click a result to fly there, or **+ Asset** to add an asset at it. |
| **Add asset** (A) | Click the map to place a new asset. |
| **Measure** (M) | Click points to build a path; the running distance shows on the map. Double-click or Esc to finish. |
| **Draw** | Mark up the map with points, lines, freehand lines, polygons, rectangles or circles, in any colour. Every window sees your drawings, and COPPIR saves them with the exercise. Click a drawing to label it, recolour it or delete it. |
| **Lock** | Freeze panning and zooming, so a drawing stroke cannot drag the map. |
| **Layers** | Turn marker clustering, sector zones, links, drawings, asset names and the legend on or off, or show a heat map of either status. |

Right-click the map to add an asset there, search OpenStreetMap around that
point, measure from it, or copy its coordinates.

**Sector zones** outline each sector's assets and colour the area by the
sector's health. Hover a zone to list its assets. **Heat maps** weigh each asset
by how bad its status is, so trouble spots glow.

### Offline maps

Venue Wi-Fi fails. Save the map before the day, while you are online:

1. Pan and zoom the map to the exercise area.
2. In **Setup → Offline map**, click **Save the area on screen for offline use**.
3. Pick the detail, from region to streets and buildings. The form shows the
   tile count and the rough size before anything downloads.
4. Choose the pack under **Base map**.

A city at street detail is 10 to 30 MB. Players and the projector load the map
from your laptop, so nobody needs the internet. A pack is one `.mbtiles` file in
the maps folder. Copy it to another machine's maps folder to take the map with
you. If a chosen pack is deleted, the map falls back to the online one.

### Reading a marker

Each marker carries both statuses twice, so the picture reads without colour:

| | Colour | Second cue |
|---|---|---|
| Security (fill set) | Marker fill | Glyph badge: ✕ Compromised, ? Under Investigation, ▪ Contained, ◦ Monitored, ✓ Clean |
| Operational (ring set) | Marker ring | Ring style: solid Healthy, dashed Degraded, double Critical, dotted Offline |

The icon in the middle is the asset type. COPPIR builds the legend at the bottom
left from the exercise's own taxonomy, so it always matches the markers.

---

## The topology graph

The **Map / Graph** switch above the canvas shows the same assets as a network
of dependencies instead of places. Incident response is often about what
depends on what, and many assets (a domain controller, a mailbox, a vendor)
have no useful location. Players and the projector can switch too.

- Each node looks like its map marker: the fill and glyph for one status set,
  the ring colour and style for the other, and the type's icon.
- Dependencies are animated amber arrows from the provider to whatever relies
  on it. *Managed by* links are purple and *connects to* links are green.
- Drag a node to move it. Positions are saved with the exercise, so every
  screen sees the same arrangement.
- Drag from a node's lower dot to another node to add a dependency.
- **Arrange** lays the whole graph out by its dependencies, with providers
  above dependants. Undo puts the old layout back.

---

## Assets and links

The **Assets** tab lists everything on the map.

- **Find** by name or tag. The map shows only the matches. Press Enter to zoom
  to them.
- **Asset types** toggles show or hide whole types.
- **Bulk status change** sets one status on every asset, or every asset of one
  type, at once.
- **Import from OpenStreetMap** searches for real infrastructure near the map
  centre or a right-clicked point: hospitals, government buildings, power
  plants, fire and police stations, data centres, universities, banks, water
  systems, transport hubs and telecom sites. Choose which results to keep and
  add them in one step.

Select an asset on the map or in the list to edit it. You can rename it, set
each status, change its type or colour, fill in its custom fields, write
notes, add tags, or move it. The
editor also shows every status change with its time.

**Links** record how assets relate: *depends on*, *is managed by* or *connects
to*, with an optional label. Dependencies draw as dashed amber lines, and other
links as solid green lines. Removing an asset removes its links.

---

## SITREP strip

The strip under the title bar summarises the picture as it changes:

- the asset count, and a count for each value of each status set;
- one wheel per sector, showing the mean health score of its assets;
- your custom metrics;
- every alert threshold that is currently breached.

All of it comes from the taxonomy, so custom statuses and sectors appear without
any code change.

---

## The script

The **Script** tab holds everything that happens during the exercise: phases,
injects and polls. **Timeline** lays the same script out against the clock.

### The exercise clock and phases

The clock in the title bar belongs to the exercise, not to one window. Every
facilitator window, player phone and projector shows the same time. Start and
pause it with the play button. Its menu sets the time, begins any phase, or
resets for a new run.

**Phases** are the planned stages, such as Detection, Containment, Eradication
and Recovery, each with a planned length. Add them under **Phases** in the
Script tab. The title bar shows the current phase, and **→ next** begins the
next one.

### Injects

An inject is a scripted event. **+ Inject** takes:

- a title and the text players see;
- a severity: info, warning or critical;
- **Sent to**: everyone, chosen teams, or the facilitator only;
- **Fires**: when you fire it; at a time on the exercise clock; a number of
  minutes after a phase begins; or when something happens (an asset reaches a
  status, another inject fires, a threshold is breached, or a poll closes with
  a given answer);
- **When fired, also**: change an asset's status, show a hidden asset to a
  team, create a new asset, open a poll, or begin a phase;
- **Players should**: the actions you expect in response, each for a team or
  anyone. You judge them in the review, and the report scores them.
- facilitator notes, which players never see.

COPPIR fires scheduled and conditional injects on its own, the moment their
trigger is met. Chains resolve at once: if one inject's firing sets off
another, both fire together. Firing applies every effect, writes an automatic
log entry, and shows a banner in every window that may see it. An inject fires
only once.

**Fire now** fires any inject by hand. **Disarm** stops a scheduled or
conditional inject from firing by itself until you **Arm** it again. Undoing an
automatic firing also disarms the inject, so it does not fire again straight
away.

### Branching with polls

A poll puts a decision to the players: a question, a few answers, optionally a
written answer, and optionally a time limit. Polls start as drafts. Open one
yourself, or let an inject open it.

Players answer on their phones and can change their answer until the poll
closes. Each team sees its own team's answers. Everyone sees the totals once
it closes. To branch the script, give each path an inject that fires "when the
poll closes with" that answer. When the poll closes, by time or by your hand,
the winning path fires. On a tie, every tied path fires.

### The timeline

The timeline shows the planned phases as bands and each scheduled inject at its
minute, with a line for now. Drag an inject sideways to move it. Click one to
edit it. Injects that wait for a condition or for you are listed below it.

### Impact rules

Under **Setup**, impact rules spread a status change along links. For example:
"when Operational becomes Offline or Critical, anything that depends on it
becomes Degraded". The change follows the chain of dependencies, never makes
anything better than it already is, and writes one facilitator-only log entry
that lists what changed and why. One undo takes back the change and its
knock-on effects together.

---

## Decision log

The **Log** tab records decisions and actions. Type an entry, optionally attach
it to an asset, and press Enter. Fired injects log themselves, and COPPIR marks
those entries AUTO. The list follows new entries, and stops following while you
scroll up to read history. The **Timeline** view lays entries out in time, so
gaps and bursts of activity stand out. **CSV** downloads the whole log.

---

## Metrics and thresholds

In the **Setup** tab:

- **Metrics** are percentages you set by hand, such as comms readiness or staff
  available. They appear as wheels in the SITREP strip. Adjust them by ±5, click
  the wheel to type an exact value, or click the name to rename it.
- **Alert thresholds** watch a sector, or the average of all sectors. When its
  health drops below the threshold, the SITREP shows the alert and outlines the
  sector's wheel in red.

Metrics and thresholds belong to the exercise, so every window sees the same
values.

---

## The room: players and observers

By default only the machine running COPPIR can reach it. **Room** in the title
bar opens the exercise to other devices on the same network.

1. Add teams under **Setup**, for example IT, Legal and Comms.
2. Click **Room**, then **Open the room**. COPPIR shows an address, a QR code
   and two codes.
3. Players scan the QR code, or go to the address and type the **player code**.
   They enter a name and pick a team. Nothing needs installing.
4. For the projector, open the address on that machine and use the **observer
   code**. It shows a read-only screen with the map, the SITREP and the log.

**What players see.** Each asset and each inject has a *Visible to* setting:
everyone, chosen teams, or the facilitator only. A player's device receives
only what their team may see. Hidden assets, queued injects and facilitator
notes never leave your machine. Change an asset's visibility during the
exercise and it appears on, or vanishes from, that team's map at once.

**What players can do.** Players read their team's picture, receive the injects
sent to them, answer polls, and add entries to their team's log. Each entry carries the
player's name and team. You see every team's entries.

**Managing the room.** The Room panel lists who has joined and whether they are
connected. Move a player to another team or remove them there. **New codes**
replaces both codes; people already in stay in. **Close room** disconnects
everyone and stops listening on the network.

The connection is not encrypted. Open the room on a network you trust, such as
the venue's own or a hotspot from your phone.

---

## Taxonomy

**Taxonomy** in the title bar edits the exercise's vocabulary:

- **Statuses**: any number of status sets, each an ordered list of values with a
  colour, a glyph and a health score from 0 to 100. One set fills the markers,
  one draws their rings, and the rest show in the asset editor only.
- **Asset types**: a name, a colour and any [Lucide](https://lucide.dev/icons)
  icon.
- **Sectors**: groups of asset types, each with a health wheel driven by the
  status set you choose.
- **Fields**: extra details every asset can carry: text, a number, yes/no, a
  link, or a choice from a list. For example: an owner, an IP range, a
  recovery-time objective or a criticality tier. Mark a field for players and
  it appears on their devices; otherwise only you see it.

The default reproduces the original COPPIR vocabulary. Changes apply to every
window at once. If you remove a status value, assets that had it fall back to
the set's starting value. You cannot remove an asset type while assets still
use it.

---

## Look and banner

Under **Setup**, **Look and banner** sets how the exercise appears on every
screen: facilitator windows, player phones and the projector.

- **Theme**: Terminal (green on black), Midnight, Daylight (light, with a light
  map, for bright rooms) or High contrast.
- **Accent colour**: replaces the theme's highlight colour.
- **Organisation and logo**: shown beside the COPPIR name. Logos are PNG, JPEG,
  WebP, GIF or SVG, up to 256 KB.
- **Banner**: a strip of text at the top, bottom or both, such as
  "EXERCISE — EXERCISE — EXERCISE", so screenshots and projected screens cannot
  be mistaken for a real incident.

---

## Review and the after-action report

**Review** in the title bar opens the after-action view. It works during the
exercise as well as after it.

**Replay.** Play the exercise back on the map or the graph, at 10 seconds to
5 minutes of exercise time per second. Drag along the timeline to jump. Arrow
keys step from one event to the next, and the space bar plays and pauses. The
replay follows the exercise clock: setup sits at T+0, and pauses take no time.

**Lanes.** Below the canvas, each actor has a lane: the facilitator, the script,
each team, and the assets. A diamond is an inject, a dot is a log entry, a
square is a vote, a ring is a poll opening or closing. Hover a mark to read it;
click it to jump there. **What happened** lists the same marks as a feed, newest
first.

**Expected actions.** Each inject that fired lists the actions you expected,
with **Met**, **Partly** or **Missed** and a note. Click an inject's time to
replay from it and see what each team did next. A bar and a score sum up the
verdicts, overall or for one team; met counts fully, partly met half. Add or
remove expected actions here too, even after the inject fired.

**Hot wash.** Write the summary, the strengths and the areas for improvement.
Each box saves when you leave it.

**Report.** The **Report** menu opens the after-action report, prints it or
saves it as PDF, or downloads it as Markdown or HTML. It covers:

- the overview: date, exercise time, teams, injects fired, decisions, log size;
- the hot wash;
- expected actions with verdicts, scored per team;
- each decision with its votes and what people said;
- the phases, planned against actual;
- a timeline of the log, phases and decisions on the exercise clock;
- the assets that changed status, and how they ended.

The report carries the exercise's banner, logo and accent colour.

---

## Undo

**Undo** in the title bar, or Ctrl/⌘+Z, reverses your most recent change of any
kind: a status change, a deleted asset, a fired inject, a drawing. It reaches
back 100 changes. Players' log entries and poll answers are not undone, so
undo always takes back your own work. If a later change touched the same thing,
undo stops instead of overwriting that later work. For example, once players
have answered a poll, opening that poll cannot be undone. Press undo again to
go further back. Undo appends to the history rather than
erasing it, so the record of the exercise stays complete.

---

## Export

The **Export** menu offers:

- **Decision log (CSV)**, for the after-action report. COPPIR neutralises cells
  that would start a spreadsheet formula.
- **Briefing (HTML)**, a printable document with status counts, sector health,
  metrics, every asset and the last 30 log entries.
- The after-action report is under **Review → Report**.
- **Exercise file (.coppir)**, the whole exercise, to back up or move to another
  machine.
- **Duplicate exercise**.

In the desktop window, downloads go to your Downloads folder.

---

## Keyboard shortcuts

| Shortcut | Action |
|---|---|
| Ctrl/⌘+Z | Undo |
| Ctrl/⌘+K | Find an asset |
| Ctrl/⌘+L | Decision log |
| A | Add an asset |
| M | Measure |
| Esc | Cancel the current tool, or clear the selection |
| ? | Show this list |

In the review:

| Shortcut | Action |
|---|---|
| Space | Play or pause the replay |
| ← / → | Previous or next event |
| Esc | Back to the console |

---

## Development

```bash
cargo test --no-default-features              # Rust unit and API tests
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt
npm --prefix web run check                    # svelte-check; warnings are errors
npm --prefix web test                         # vitest
```

For frontend work, run the server with `cargo run -- --headless` and, in
another shell, `npm --prefix web run dev`. Vite serves the app with hot reload
at `http://localhost:5173` and forwards `/api` to the server. A debug build of
the Rust binary reads `web/dist/` from disk, so a fresh `npm run build` shows up
on reload without recompiling.

`cargo test` generates the TypeScript types in `web/src/lib/types/` from the
Rust types. Commit them with the Rust change. CI fails if they are stale.

See [ARCHITECTURE.md](ARCHITECTURE.md) for how the pieces fit together, and
[docs/DESIGN.md](docs/DESIGN.md) for where the project is going.

---

### Releases

`docs/RELEASING.md` covers versioning, tagging, the release workflow and how
to build an installer locally.

---

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
