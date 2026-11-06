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
| `live.rs` | One room per open exercise: the in-memory state, the undo stack and the broadcast channel |
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

`Live::execute` does all of that under the exercise's lock.

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

### Undo

Before `apply` runs each event, `undo::inverse` computes the events that
reverse it from the state just before it. The room keeps the last 100 changes
with their inverses, in memory. `Command::Undo` pops the newest one. It checks
with `undo::still_current` that nothing later has overwritten what it touched.
Then it appends `Undone { seqs }` followed by the inverse events. History is never
rewritten. The undo stack does not survive a restart; the log does.

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

Every route is under `/api/`. Any other path serves the frontend, with
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
| POST | `/api/v2/exercises/{id}/duplicate` | Copy, with a fresh history |
| POST | `/api/v2/exercises/import` | Create from a `.coppir` file |
| POST | `/api/v2/exercises/import/v1` | Create from a version 1 scenario's JSON |
| GET | `/api/v2/legacy/scenarios` | Version 1 scenario files waiting in the scenarios folder |
| POST | `/api/v2/legacy/scenarios/import` | Import one of them by name |
| POST | `/api/v2/geocode` | Place search |
| GET | `/api/v2/osm/categories` | Infrastructure categories |
| POST | `/api/v2/osm/search` | Infrastructure near a point |

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

## Desktop shell (`src/desktop.rs`)

`tao` creates the window, 1440×900 with a 1100×700 minimum. `wry` hosts the
platform webview: WKWebView on macOS, WebView2 on Windows, WebKitGTK on Linux.
The shell saves blob downloads (the exports) to the user's Downloads folder
under a name that does not overwrite anything. Off-site links open in the default
browser. Devtools are on in debug builds.

## Tests

| Suite | Covers |
|---|---|
| Rust unit tests | Validation, taxonomy checks, undo inverses for every command, store replay with and without snapshots, v1 import, Overpass and Nominatim parsing, host checks |
| `tests/v2.rs` | The API and websocket against a real server on a random port: commands, races, resume, undo, export and import, legacy import |
| `tests/security.rs` | The origin and host guard, content types, SPA fallback, upstream failures |
| `web/src/**/*.test.ts` | The reducer (every event type), geometry, marker ids and icon names, exports |

Outbound URLs in the Rust tests point at a closed local port, so nothing
reaches the internet.
