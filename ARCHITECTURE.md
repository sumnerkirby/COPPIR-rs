# COPPIR Architecture

## Overview

COPPIR is a single-machine desktop application. It has no cloud component, no
database and no accounts. State lives in memory for the length of a session and
is written to disk only when someone saves a scenario.

One process holds everything:

```
┌───────────────────────────────────────────┐
│  coppir (one executable)                  │
│                                           │
│  main thread: native window (tao + wry)   │
│  ┌─────────────────────────────────────┐  │
│  │  Frontend (HTML / CSS / JS)         │  │
│  │  embedded from static/              │  │
│  └──────────────┬──────────────────────┘  │
│                 │ HTTP + WebSocket         │
│                 │ 127.0.0.1:8000           │
│  ┌──────────────▼──────────────────────┐  │
│  │  Server (axum on tokio workers)     │  │
│  │  REST API, broadcast, OSM, scenarios│  │
│  └─────────────────────────────────────┘  │
└───────────────────────────────────────────┘
```

## Startup (`src/main.rs`)

1. Parse flags (`--port`, `--strict-port`, `--scenarios-dir`, `--headless`).
2. Build a multi-threaded tokio runtime and bind `127.0.0.1:<port>`. If the
   port is taken, bind a free one instead unless `--strict-port` is set.
3. **Headless:** serve on the current thread until Ctrl+C.
   **Desktop:** spawn the server on the runtime's worker threads, then run the
   window's event loop on the main thread, which macOS requires. Closing the
   window ends the process.

The listener is bound before the window opens, so the window loads the app
directly. `index.html` shows its own loading screen while the scripts start.

## Modules

| Module | Role |
|---|---|
| `lib.rs` | `bind()` and `serve()`, the two entry points `main` and the tests share |
| `config.rs` | Runtime `Config`, the default scenarios directory, Overpass mirrors and tag filters |
| `state.rs` | `Session` (the data), `Hub` (lock + broadcast channel), `Event` (the wire protocol), `AppState` |
| `models.rs` | Stored records and request bodies, with their `Validate` impls |
| `validate.rs` | The `Valid<T>` extractor and the `Checks` accumulator |
| `error.rs` | `ApiError`, rendered as `{"detail": ...}` the way FastAPI did |
| `guard.rs` | Middleware that refuses non-loopback `Host` headers and foreign `Origin`s |
| `assets.rs` | Serves the embedded `static/` directory with ETags |
| `routes/` | One module per resource: pins, edges, injects, log, scenarios, thresholds, osm, ws |
| `desktop.rs` | The window (feature `desktop`, on by default) |

## State and broadcasting

`Session` holds the five stores:

| Field | Type | Contents |
|---|---|---|
| `pins` | `IndexMap<String, Pin>` | Map pins, by id, in insertion order |
| `injects` | `IndexMap<String, Inject>` | Queued and fired injects |
| `edges` | `IndexMap<String, Edge>` | Connections between pins |
| `thresholds` | `Vec<Threshold>` | Alert rules |
| `decision_log` | `Vec<LogRecord>` | Every logged action, in order |

`Hub` wraps the session in a `Mutex` next to a `tokio::sync::broadcast`
channel. Handlers mutate state only through `Hub::with`, which passes the
session and a `Publisher`. The broadcast is sent *while the lock is still
held*, which gives two guarantees:

- Every client sees events in exactly the order the state changed.
- `Hub::subscribe` takes the `full_state` snapshot and the channel subscription
  under the same lock, so a client that joins mid-exercise can neither miss an
  event nor receive one twice.

Each event is serialized once and the bytes are shared by every subscriber. A
client more than 1024 events behind gets a fresh `full_state` rather than a gap.
No handler holds the lock across an `.await`. Scenario files are read and
written outside it.

## API

All routes are under `/api/`, except the websocket at `/ws`. Anything else falls
through to the embedded frontend.

| Method | Path | |
|---|---|---|
| GET | `/api/health` | Liveness and version |
| GET, POST, DELETE | `/api/pins` | List (a map keyed by id) / create / delete all |
| POST | `/api/pins/bulk` | Create many pins with one shared status |
| POST | `/api/pins/bulk-status` | Set status and/or op status, optionally for one category |
| PUT, DELETE | `/api/pins/{pid}` | Partial update / delete (and its edges) |
| GET, POST | `/api/edges` | List / create |
| DELETE | `/api/edges/{eid}` | Delete |
| GET, POST | `/api/injects` | List / queue |
| DELETE | `/api/injects/{iid}` | Delete |
| POST | `/api/injects/{iid}/trigger` | Fire once: status change, new pin and log entry in one event |
| GET, POST | `/api/log` | List / add a manual entry |
| GET, POST, DELETE | `/api/thresholds`, `/api/thresholds/{tid}` | List / create / delete |
| GET | `/api/scenarios` | Saved scenario names, sorted |
| POST | `/api/scenarios/save`, `/api/scenarios/load` | Save / replace the session |
| DELETE | `/api/scenarios/{name}` | Delete a saved file |
| POST | `/api/state/clear` | Reset the session |
| POST | `/api/geocode` | Nominatim search |
| POST | `/api/search/osm` | Overpass search by category and radius |

Websocket message types: `full_state`, `pin_add`, `bulk_add`, `pin_update`,
`pin_delete`, `pins_cleared`, `bulk_update`, `edge_add`, `edge_delete`,
`inject_queued`, `inject_delete`, `inject_triggered`, `threshold_add`,
`threshold_delete`, `log_entry`. They are the variants of `state::Event`.

## Validation

Request bodies go through `Valid<T>`. It requires `Content-Type:
application/json`, deserializes with `serde_path_to_error` so a type error names
its field, then runs `Validate::check`. Enum-like fields are Rust enums, so an
unknown status cannot be represented. Name-like fields are trimmed and must not
be blank. Errors come back as 422 with FastAPI's shape, which the frontend's
`detailText()` already formats:

```json
{"detail": [{"loc": ["body", "items", "1", "name"], "msg": "must not be blank", "type": "value_error"}]}
```

## Local-only guard

Binding to loopback keeps other machines out but not other websites in the
user's browser. `guard::local_only` runs on every request:

- `Host` must be `localhost`, `127.0.0.1` or `[::1]` (any port). This defeats
  DNS rebinding.
- If `Origin` is present, it must be `http://<that host>`. This blocks
  cross-site form posts to body-less routes, cross-site `fetch`, and cross-site
  websockets.

Clients that send no `Origin`, such as curl, the seeder and same-origin GETs,
pass.

## Scenarios

Names are reduced to `[A-Za-z0-9 _-]`, trimmed and capped at 64 characters.
Names that end up empty or are reserved on Windows (`CON`, `NUL`, `COM1` and so
on) are refused. The file format is the session's stores plus `name`,
`saved_at` and `format_version`. It is the same layout the Python version
wrote, so files move between the two. Saves write a temporary file and rename it
over the target. Loads parse the whole file before touching state, and drop any
edge whose pins are missing.

## Frontend (`static/`)

Unchanged from the Python version: plain ES2020 modules, no build step, with
Leaflet and its plugins vendored under `static/vendor/`. See the original
project's ARCHITECTURE.md for the module-by-module breakdown. Release builds
embed the directory with `rust-embed`. Debug builds read it from disk on each
request.

## Desktop shell (`src/desktop.rs`)

`tao` creates the window, 1440×900 with a 1100×700 minimum. `wry` hosts the
platform webview: WKWebView on macOS, WebView2 on Windows, WebKitGTK on Linux.
Blob downloads (the CSV and briefing exports) are saved to the user's Downloads
folder with a non-clobbering name. Off-site links open in the default browser.
`window.open('')` popups use the webview's default handling. Devtools are on in
debug builds.

## Tests

| Suite | Covers |
|---|---|
| unit tests (`src/**`) | Validation, sanitising, host checks, Overpass query and parsing |
| `tests/*.rs` | The HTTP API and websocket, against a real server on a random port with a temp scenarios directory |

The integration tests are a port of the Python suite, plus coverage for the new
behaviour: concurrent trigger races, event ordering across clients, the origin
guard, damaged and legacy scenario files. The outbound URLs point at a closed
local port, so nothing reaches the internet.
