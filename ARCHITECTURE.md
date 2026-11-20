# COPPIR Architecture

## Overview

COPPIR is one executable. It contains an HTTP and websocket server, the web
frontend, and (with the default `desktop` feature) a native window that shows
that frontend. Exercises live in one SQLite file as append-only event logs.

```
┌──────────────────────────────────────────────┐
│  coppir (one executable)                     │
│                                              │
│  main thread: native window (tao + wry)      │
│  ┌────────────────────────────────────────┐  │
│  │  Frontend: Svelte 5 + TS + MapLibre    │  │
│  │  embedded from web/dist/               │  │
│  └───────────────┬────────────────────────┘  │
│                  │ HTTP + WebSocket          │
│                  │ 127.0.0.1:8000            │
│  ┌───────────────▼────────────────────────┐  │
│  │  Server: axum on tokio workers         │  │
│  │  commands → events → SQLite → clients  │  │
│  └───────────────┬────────────────────────┘  │
│                  ▼                           │
│        coppir.db (SQLite, WAL)               │
└──────────────────────────────────────────────┘
```

## Startup (`src/main.rs`)

1. Parse flags (`--port`, `--strict-port`, `--database`, `--scenarios-dir`,
   `--headless`).
2. Build a multi-threaded tokio runtime and bind `127.0.0.1:<port>`. If the
   port is taken, bind a free one instead, unless `--strict-port` is set.
3. Open the database, creating it and its folder if needed.
4. **Headless:** serve on the current thread until Ctrl+C.
   **Desktop:** spawn the server on the runtime's worker threads. Then run the
   window's event loop on the main thread, which macOS requires. Closing the
   window ends the process.

## Modules

| Module | Role |
|---|---|
| `lib.rs` | `bind()` and `serve()`, which `main` and the tests share |
| `config.rs` | Runtime `Config`, the data directory, Overpass mirrors and OSM tag filters |
| `exercise/model.rs` | The domain: `Exercise`, `Taxonomy`, `Asset`, `Link`, `Inject`, `Threshold`, `LogEntry`, `Annotation`, `Metric` |
| `exercise/taxonomy.rs` | The default taxonomy and taxonomy validation |
| `exercise/command.rs` | `Command` and `decide`: check a command against the exercise and produce events |
| `exercise/event.rs` | `Event`, `Envelope` and `Exercise::apply` |
| `exercise/undo.rs` | The inverse of each event, and whether an undo is still safe |
| `exercise/import_v1.rs`, `legacy.rs` | Read version 1 and Python scenario files into an exercise |
| `store.rs` | SQLite: the `exercises` index, append-only `events`, `snapshots` |
| `exercise/engine.rs` | The script engine: which injects are due and which polls have run out of time |
| `exercise/impact.rs` | Spreads a status change along links by the exercise's impact rules |
| `templates.rs` | The built-in exercises, `templates/*.coppir` compiled into the binary |
| `report.rs` | The after-action report, rendered from one outline to Markdown and to HTML |
| `maps.rs` | Offline map packs: downloading a region into an MBTiles file, and reading tiles, styles, sprites and glyphs back |
| `routes/maps.rs` | `/api/v2/maps`: list, download and delete packs (facilitator); styles, tiles, sprites and fonts (anyone in the room) |
| `exercise/view.rs` | What each audience may see: `project` an exercise, `diff` two projections into events |
| `live.rs` | One room per open exercise: the in-memory state, the undo stack, the broadcast channel and one view per connected audience |
| `room.rs` | Join codes, participant sessions, rate limiting, and this machine's LAN addresses |
| `auth.rs` | `Viewer`: who a request comes from, and the facilitator-only middleware |
| `routes/room.rs` | Open and close the room, QR code, join, leave, manage participants |
| `routes/v2.rs` | `/api/v2`: exercises, commands, events, websocket, export and import |
| `routes/osm.rs` | Place search (Nominatim) and infrastructure search (Overpass) |
| `validate.rs` | The `Valid<T>` JSON extractor and the `Checks` accumulator |
| `error.rs` | `ApiError`, rendered as `{"detail": ...}` |
| `guard.rs` | Refuses non-loopback `Host` headers and foreign `Origin`s |
| `assets.rs` | Serves the embedded frontend, with client-side route fallback and ETags |
| `desktop.rs` | The window (feature `desktop`) |

## How a change happens

```
Command ─► decide() ─► Vec<Event> ─► number them ─► apply to a copy
        ─► commit to SQLite ─► swap the copy in ─► push undo entry ─► broadcast
```

`Live::execute` does all of that under the exercise's lock. It applies events
in place. If the SQLite write then fails, it reloads the exercise from disk, so
memory never stays ahead of the log. Responses are gzip-compressed, and hashed
frontend assets are served as immutable.

- **Only `decide` can refuse.** It checks the command against the current
  exercise (does the asset exist, is the status value in the taxonomy). It
  returns 404, 409 or 422 without touching anything. `apply` never fails.
- **Events carry results.** An event holds entities as they are *after* the
  change: `AssetsUpserted` carries whole assets. Applying one is a plain upsert
  or removal, in Rust (`Exercise::apply`) and in the browser
  (`web/src/lib/reducer.ts`). Replay stays deterministic. The two reducers
  are too simple to drift apart.
- **Commit before broadcast.** The server builds the new state on a copy and
  writes it to SQLite. Only then does it swap the copy in and send it. A failed write leaves memory as it
  was. No client sees a change that a crash could take back.
- **One order.** The lock covers the whole sequence. So every client sees
  events in `seq` order, and a client that subscribes takes its snapshot and
  subscription at the same moment.

### The script engine

`engine::due` looks at the exercise and the wall clock and returns ordinary
commands: `ClosePoll` for polls past their deadline, then `FireInject` for each
armed inject whose trigger is met. A trigger is one of these:

- a time on the exercise clock;
- some minutes after a phase began, measured from the latest `phase_log`
  entry for that phase;
- a condition: an asset status, another inject having fired, a threshold
  breached, or a closed poll's winning answer.

After every command, `Live` runs the engine under the same lock until it
returns nothing, capped at 50 rounds. Each command it runs is decided and
committed like anyone else's, with actor `engine`, and is undoable on its own.
A chain of conditional injects therefore resolves within the request that
started it. A background task calls `Live::tick` once a second. It runs the
engine only for exercises waiting on time: a running clock with scheduled
injects, or an open poll with a deadline.

The clock is `elapsed_ms` plus the time since `running_since`. The server
computes it from the wall clock; browsers do the same between updates.

Undoing an automatic firing would leave the inject's condition still true, so
the inverse also disarms the inject. The facilitator re-arms it on purpose.

**Impact rules.** `SetStatus` and `FireInject` pass the assets they change to
`impact::propagate`. It follows links of the rule's kind from provider to
dependant, transitively, and sets a dependant only when the new value scores
lower than its current one. A set of visited (asset, rule) pairs stops cycles.
The knock-on changes join the same `AssetsUpserted` event, with one
facilitator-only `Logged` entry listing them, so one undo reverses everything.

**Polls.** A response stores the participant's session id, name and team; the
latest response from each participant counts, and `results` is recomputed with
every response. Players get the poll once it is open, their own team's
answers, and totals only once it closes.

### Undo

Before `apply` runs each event, `undo::inverse` computes the events that
reverse it from the state just before it. The room keeps the last 100 changes
with their inverses, in memory. `Command::Undo` pops the newest one. It checks
with `undo::still_current` that nothing later has overwritten what it touched.
Then it appends `Undone { seqs }` followed by the inverse events. History is never
rewritten. The undo stack does not survive a restart; the log does.

## Room access

Until the facilitator opens the room, the app listens on loopback only.

**Who is asking (`auth.rs`).** A request with a valid `coppir_session` cookie is
that participant. Otherwise a request from a loopback peer is the facilitator.
Anything else gets a 401, and the client shows the join page. The cookie wins
over being local, so the host can join as a player in a second browser profile.

**Opening (`routes/room.rs`).** `POST /api/v2/room/open` binds one listener per
LAN IPv4 address, on the loopback port when it is free there. Each serves the
same router. The room holds a `watch` sender; dropping it on close stops the
listeners. The host guard then also accepts those literal `ip:port` hosts.

**Joining (`room.rs`).** A six-character code (no lookalike characters) admits
a player or an observer. `POST /api/v2/join` returns an `HttpOnly`,
`SameSite=Strict` cookie. Sessions live in memory. Each address gets 12 code
attempts a minute: a wrong code counts wherever it is tried, a right one only
when it joins, so the join page looking a code up first costs nothing.

**Views (`exercise/view.rs`, `live.rs`).** The facilitator receives the raw
event stream. Every other audience (observer, or a player on a given team)
gets a *projection*: the exercise with hidden assets, their links, unfired
injects, inject notes and effects, and other teams' log lines removed. After
each command the room recomputes the projection for every audience that has a
client connected, diffs it against the previous one, and sends the difference
as a `patch` message. Revealing an asset to a team therefore arrives as an
ordinary upsert, and hiding it as a removal. Hidden data never reaches a
participant's device.

**Participant commands.** Players may send `Log` and `Respond` only. For
`Respond`, the server fills in who is answering; the request cannot set it. The server stamps the
entry with the player's name and team and limits its visibility to that team.
Observers may send nothing.

**Moving and removing.** Changing a player's team, kicking them, or closing the
room fires the session's `Notify`. Its websockets close. A moved client
reconnects and receives the snapshot of its new view.

## Storage (`store.rs`)

| Table | Contents |
|---|---|
| `exercises` | One row per exercise: name, timestamps, head `seq`, asset count. Updated in the same transaction as its events. |
| `events` | `(exercise_id, seq)` → time, actor, JSON event. Append-only. |
| `snapshots` | The folded exercise every 250 events. The newest two are kept. |

Loading an exercise reads the newest snapshot and applies the events after it.
SQLite runs in WAL mode with `synchronous=NORMAL`. A committed event survives
an app crash; a power cut can lose the last few milliseconds. `user_version`
holds the schema version. The store refuses a database from a newer build.

## API

Every route is under `/api/`. Participants may call only `me`, `join`, `leave`,
and the snapshot, commands and websocket routes of their own exercise. The
rest are the facilitator's. Any other path serves the frontend, with
`index.html` for client-side routes such as `/x/<id>`.

| Method | Path | |
|---|---|---|
| GET | `/api/health` | Liveness and version |
| GET | `/api/v2/taxonomy/default` | The default taxonomy |
| GET, POST | `/api/v2/exercises` | List / create |
| GET, DELETE | `/api/v2/exercises/{id}` | Snapshot (`{seq, exercise}`) / delete with its history |
| POST | `/api/v2/exercises/{id}/commands` | Run one `Command`; returns the events it produced |
| GET | `/api/v2/exercises/{id}/events?after=` | The event log |
| GET | `/api/v2/exercises/{id}/ws?after=` | Live stream |
| GET | `/api/v2/exercises/{id}/export` | Download a `.coppir` file |
| GET | `/api/v2/exercises/{id}/report?format=html\|md&download=` | The after-action report |
| POST | `/api/v2/exercises/{id}/duplicate` | Copy, with a fresh history |
| POST | `/api/v2/exercises/import` | Create from a `.coppir` file |
| POST | `/api/v2/exercises/import/v1` | Create from a version 1 scenario's JSON |
| GET | `/api/v2/templates` | The built-in exercises |
| POST | `/api/v2/templates/{id}` | Start an exercise from one |
| GET | `/api/v2/legacy/scenarios` | Version 1 scenario files waiting in the scenarios folder |
| POST | `/api/v2/legacy/scenarios/import` | Import one of them by name |
| GET | `/api/v2/me` | The caller's role, name, team and exercise |
| GET, POST | `/api/v2/join` | What a code admits to / join and receive a session cookie |
| POST | `/api/v2/leave` | End the caller's session |
| GET | `/api/v2/room` | Room status and participants |
| POST | `/api/v2/room/open`, `/close`, `/codes` | Open, close, or issue new codes |
| GET | `/api/v2/room/qr.svg` | QR code of the join link |
| PATCH, DELETE | `/api/v2/room/participants/{id}` | Move to a team or rename / remove |
| POST | `/api/v2/geocode` | Place search |
| GET | `/api/v2/osm/categories` | Infrastructure categories |
| POST | `/api/v2/osm/search` | Infrastructure near a point |
| GET, POST | `/api/v2/maps` | List offline map packs / start downloading one |
| DELETE | `/api/v2/maps/{id}` | Stop a download or delete a pack |
| GET | `/api/v2/maps/{id}/style/{name}` | A pack's style, its URLs pointed at this server |
| GET | `/api/v2/maps/{id}/tiles/{z}/{x}/{y}`, `/sprite/{file}`, `/fonts/{stack}/{range}` | What the style asks for |

**Live stream.** On connect the server sends a `snapshot`. With
`?after=<seq>`, it sends just the events since then (up to 1,000). After that it
sends each event as it happens. The client checks that every event is
`seq + 1`. Otherwise it resyncs from a snapshot. The server also resyncs a
client that falls 1,024 events behind.

**Typed end to end.** Every request, response and message type derives
`ts_rs::TS`. `cargo test` writes the TypeScript to `web/src/lib/types/`, and
the frontend imports nothing else. `Command` and `Event` are discriminated
unions. A new event without a case in `reducer.ts` fails `npm run check`.

**Validation.** `Valid<T>` requires `Content-Type: application/json`. It
deserializes with `serde_path_to_error`, so a type error names its field, and
runs `Validate::check`. Errors are 422s in FastAPI's shape, with every problem
listed:

```json
{"detail": [{"loc": ["body", "assets", "1", "name"], "msg": "must not be blank", "type": "value_error"}]}
```

## The after-action report (`report.rs`)

The report needs more than the exercise's final state. The exercise clock at a
past moment comes from replaying `ClockChanged` events, because the clock can be
paused, set and reset. The count of undone changes and whether the script or
the facilitator wrote a log line also come from the log. `outline()` builds a
list of blocks (headings, facts, tables, text), and two small renderers turn it
into Markdown or HTML, so the two formats cannot disagree. The HTML escapes
every string and is served with a `default-src 'none'` content security policy:
player log text ends up in it.

## Offline map packs (`maps.rs`)

A pack is one MBTiles file: SQLite with the standard `metadata` and `tiles`
tables, plus a `files` table for the dark and positron styles, the sprite and
the glyph ranges. SQLite is already in the binary, so packs need no new tile
library, and one file is the whole map.

A download runs as a background task. It fetches both styles from the tile
service, then the TileJSON, the sprite and the glyphs for every font the
styles use plus the app's label font. Then it fetches the tiles, six at a
time, with retries. Every URL must share the tile service's origin. Tiles are
stored gzipped and served with `Content-Encoding: gzip`. The file is written as
`<id>.part` and renamed only when complete. A request is capped at 40,000
tiles.

At serve time the style is rewritten: one vector source pointing at
`/api/v2/maps/<id>/tiles`, with the pack's bounds and maximum zoom, plus sprite
and glyph URLs on this server. The URLs use the request's `Host`, which the
guard has already checked, so a phone on the LAN gets the LAN address. Layers
from other sources (the online style's hillshade) are dropped. A missing glyph
range returns an empty body, which MapLibre treats as having no glyphs.

## Local-only guard (`guard.rs`)

Binding to loopback keeps other machines out. It does not keep out other
websites open in the user's browser. The guard checks every request:

- `Host` must be `localhost`, `127.0.0.1` or `[::1]`, on any port. This defeats
  DNS rebinding.
- If `Origin` is present, it must be `http://<that host>`. This blocks cross-site
  form posts, `fetch` and websockets.

Clients that send no `Origin` (curl, the seeder, same-origin GETs) pass.

## Frontend (`web/`)

Vite builds the Svelte 5 and TypeScript frontend into `web/dist/`, and
`rust-embed` embeds it. A debug build of the binary reads `web/dist/` from disk instead.

| Path | Role |
|---|---|
| `lib/types/` | Generated from Rust; never edited by hand |
| `lib/api.ts` | Typed `fetch` wrappers |
| `lib/live.svelte.ts` | One exercise kept in sync over the websocket, with resume |
| `lib/reducer.ts` | Applies an event to the local exercise |
| `lib/workspace.svelte.ts` | Console state shared through context: selection, tool mode, layers, filters, the map handle |
| `lib/markers.ts`, `lib/icons.ts` | Draws each marker (fill, ring style, glyph badge, Lucide icon) onto a canvas for MapLibre |
| `lib/sitrep.ts` | Counts, sector health and breached thresholds, all from the taxonomy |
| `lib/exports.ts` | CSV log and HTML briefing |
| `lib/replay.ts` | Rebuilds the exercise at any moment of its run from the event log, and places the review's lane marks |
| `lib/maps.ts` | Tile counts and size estimates for offline packs |
| `components/review/` | The review: replay transport, lanes, the event feed, assessment and the hot wash |
| `lib/geo.ts` | Distance, convex hull, circles |
| `components/map/` | MapView (sources and layers), toolbar, terra-draw drawing, context menu, legend |
| `components/assets/`, `injects/`, `log/`, `setup/` | The sidebar tabs |
| `components/taxonomy/` | The taxonomy editor |

**Map.** MapLibre GL draws everything from GeoJSON sources on the GPU: assets,
links, sector zones (convex hulls), heat, annotations and the measuring line.
Each marker image is named by its inputs (`mk|icon|fill|ring|style|glyph|selected`)
and drawn on demand in `styleimagemissing`. Identical markers share one
image, and any taxonomy works without a sprite sheet. Terra-draw handles
drawing. The client sends a finished shape as an `add_annotation` command and
removes it from terra-draw's own store. The shared annotation layer is the only
copy.

**Graph.** The topology view (`components/graph/`) uses Svelte Flow, loaded
only when someone switches to it. Positions live on each asset (`graph`);
`SetGraphPositions` saves a drag or a whole layout in one undoable change.
Unplaced assets are positioned by dagre (`lib/graph.ts`) until someone moves
them. dagre was chosen over ELK for size: it does the same layered layout in a
fraction of the code.

**Replay.** `buildReplay` walks the event log once. It gives each event an
exercise-clock time that never decreases, saves the state every 200 events and
records the lane marks. Each mark is read against the state just before its
change, which shows a new vote or a status that really changed. Seeking
forward applies events to the shown state in place. Seeking backward starts from
the nearest saved state. The review renders the replayed exercise through a
second `Workspace` built with `connected = false`. The map, graph and SITREP
draw it unchanged, and its `send` does nothing.

**Themes.** `lib/theme.ts` holds four token sets. `BrandFrame` writes the
exercise's tokens onto `:root` and draws the banner. Every component colours
itself from the tokens, so a theme change restyles everything at once. The map
is remounted with a light basemap for Daylight.

## Templates (`templates/`)

`templates/generate.py` writes the four `.coppir` files, and `src/templates.rs`
compiles them in with `include_str!`. A unit test adopts each template and
re-validates every inject, impact rule and branding through `decide`, so a
reference to a missing asset or phase fails the build. CI regenerates the files
and fails if they differ from the committed ones.

## Desktop shell (`src/desktop.rs`)

`tao` creates the window, 1440×900 with a 1100×700 minimum. `wry` hosts the
platform webview: WKWebView on macOS, WebView2 on Windows, WebKitGTK on Linux.
The shell saves blob downloads (the exports) to the user's Downloads folder
under a name that does not overwrite anything. Off-site links open in the default
browser. Devtools are on in debug builds.

On macOS the shell installs a menu bar with `muda`. A webview gets Cut, Copy,
Paste and Select All shortcuts only from an Edit menu, and Cmd+Q only from an
app menu. Undo and Redo are left out, because a menu item would take Cmd+Z from
the page, where it undoes the last change to the exercise.

A release build on Windows uses the GUI subsystem, so it opens no console window.
At startup it attaches to the parent terminal, if there is one, so
`--headless`, `--help` and the logs still print there. `build.rs` compiles the
icon and version details into the `.exe` with `winresource`.

## Packaging

`cargo-packager` builds the installers from `[package.metadata.packager]` in
`Cargo.toml`: a universal `.dmg`, an NSIS setup `.exe`, a `.deb` and an
`.AppImage`. The release workflow (`.github/workflows/release.yml`) runs on a
version tag:

1. It builds the frontend once.
2. On each platform, it builds the binary and packages it. On macOS it builds
   both architectures and joins them with `lipo`.
3. It drafts a release with the installers and their SHA-256 checksums.

The macOS app is signed ad hoc so that its bundle is sealed. An unsealed
bundle that was downloaded is reported as damaged. The icons in
`packaging/icons/` are rendered from SVG by `generate.py` and committed.
`docs/RELEASING.md` has the steps.

## Tests

| Suite | Covers |
|---|---|
| Rust unit tests | Validation, taxonomy checks, undo inverses for every command, store replay with and without snapshots, v1 import, Overpass and Nominatim parsing, host checks |
| `tests/v2.rs` | The API and websocket against a real server on a random port: commands, races, resume, undo, export and import, legacy import |
| `tests/customize.rs` | Every template starting and running to the end, branding validation and undo, graph positions, custom fields following the taxonomy |
| `tests/script.rs` | A scheduled inject firing by itself, chains within a request, undo disarming, impact through the API, players answering polls and the branch firing, what participants see of the script |
| `tests/room.rs` | Joining, roles, per-team snapshots and live patches, player log stamping, moving and kicking, and access over the real LAN address |
| `tests/security.rs` | The origin and host guard, content types, SPA fallback, upstream failures |
| `tests/review.rs` | Expected actions and verdicts, the hot wash and its undo, the report in both formats, escaping |
| `tests/maps.rs` | A pack downloaded from a stand-in tile service, the rewritten style, tiles, sprites and fonts, deletion, bad requests and a failed download |
| `web/src/**/*.test.ts` | The reducer (every event type), replay timing, lanes and seeking, tile counts, geometry, marker ids and icon names, exports |

Outbound URLs in the Rust tests point at a closed local port, so nothing
reaches the internet.
