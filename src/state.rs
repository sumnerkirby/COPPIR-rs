//! In-memory session state, and the channel that pushes changes to clients.
//!
//! There is no database. A session lives in memory and is persisted only when
//! someone saves a scenario.
//!
//! Every mutation happens under one lock, and its broadcast is published while
//! that lock is still held. That gives two guarantees the Python version could
//! not: clients see events in exactly the order the state changed, and a
//! client that connects mid-exercise gets a snapshot and a subscription taken
//! at the same instant, so it can neither miss an event nor see one twice.

use std::sync::{Arc, Mutex, MutexGuard};

use axum::extract::ws::Utf8Bytes;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::models::{Edge, Inject, LogRecord, Pin, Threshold};

/// How many events a slow client may fall behind before it is resynced with a
/// fresh `full_state` instead.
const CHANNEL_CAPACITY: usize = 1024;

/// Everything that makes up an exercise. Maps keep insertion order, so pins
/// list in the order they were placed, as they did in Python.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Session {
    #[serde(default)]
    pub pins: IndexMap<String, Pin>,
    #[serde(default)]
    pub injects: IndexMap<String, Inject>,
    #[serde(default)]
    pub edges: IndexMap<String, Edge>,
    #[serde(default)]
    pub thresholds: Vec<Threshold>,
    #[serde(default)]
    pub decision_log: Vec<LogRecord>,
}

impl Session {
    pub fn full_state(&self) -> Event<'_> {
        Event::FullState {
            pins: &self.pins,
            injects: self.injects.values().collect(),
            edges: self.edges.values().collect(),
            thresholds: &self.thresholds,
            log: &self.decision_log,
        }
    }
}

/// A message pushed to every connected client. The `type` tag and field names
/// are the wire protocol `app.js` dispatches on.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event<'a> {
    FullState {
        pins: &'a IndexMap<String, Pin>,
        injects: Vec<&'a Inject>,
        edges: Vec<&'a Edge>,
        thresholds: &'a [Threshold],
        log: &'a [LogRecord],
    },
    PinAdd {
        pin: &'a Pin,
    },
    BulkAdd {
        pins: Vec<&'a Pin>,
    },
    PinUpdate {
        pin: &'a Pin,
    },
    PinDelete {
        pid: &'a str,
    },
    PinsCleared {
        pids: Vec<String>,
    },
    BulkUpdate {
        pins: Vec<&'a Pin>,
    },
    EdgeAdd {
        edge: &'a Edge,
    },
    EdgeDelete {
        eid: &'a str,
    },
    InjectQueued {
        inject: &'a Inject,
    },
    InjectDelete {
        iid: &'a str,
    },
    InjectTriggered {
        inject: &'a Inject,
        log_entry: &'a LogRecord,
        pins: &'a IndexMap<String, Pin>,
        new_pin: Option<&'a Pin>,
    },
    ThresholdAdd {
        threshold: &'a Threshold,
    },
    ThresholdDelete {
        tid: &'a str,
    },
    LogEntry {
        entry: &'a LogRecord,
    },
}

impl Event<'_> {
    /// Serialize once; every subscriber shares the same bytes.
    pub fn encode(&self) -> Utf8Bytes {
        serde_json::to_string(self).expect("events always serialize").into()
    }
}

/// Publishes events. Only reachable through [`Hub::with`], which is what keeps
/// publishing inside the state lock.
pub struct Publisher<'h>(&'h broadcast::Sender<Utf8Bytes>);

impl Publisher<'_> {
    pub fn send(&self, event: &Event<'_>) {
        // An error only means nobody is connected, which is fine.
        let _ = self.0.send(event.encode());
    }
}

pub struct Hub {
    session: Mutex<Session>,
    tx: broadcast::Sender<Utf8Bytes>,
}

impl Default for Hub {
    fn default() -> Self {
        Self { session: Mutex::default(), tx: broadcast::channel(CHANNEL_CAPACITY).0 }
    }
}

impl Hub {
    fn lock(&self) -> MutexGuard<'_, Session> {
        // A panic mid-mutation leaves a session that is still structurally
        // valid, so carrying on beats taking every later request down with it.
        self.session.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Run `f` with exclusive access to the session and a way to publish.
    /// `f` must not block; it runs with the lock held.
    pub fn with<R>(&self, f: impl FnOnce(&mut Session, &Publisher<'_>) -> R) -> R {
        let mut session = self.lock();
        f(&mut session, &Publisher(&self.tx))
    }

    /// Read-only access, without publishing.
    pub fn read<R>(&self, f: impl FnOnce(&Session) -> R) -> R {
        f(&self.lock())
    }

    /// A `full_state` message and a subscription to everything after it.
    pub fn subscribe(&self) -> (Utf8Bytes, broadcast::Receiver<Utf8Bytes>) {
        let session = self.lock();
        let rx = self.tx.subscribe();
        (session.full_state().encode(), rx)
    }

    pub fn client_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

/// Shared application state handed to every handler.
#[derive(Clone)]
pub struct AppState(Arc<Inner>);

pub struct Inner {
    pub hub: Hub,
    pub config: crate::config::Config,
    pub http: reqwest::Client,
    pub live: crate::live::Live,
}

impl AppState {
    pub fn new(config: crate::config::Config, store: crate::store::Store) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(crate::config::USER_AGENT)
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("the HTTP client builds with static settings");
        Self(Arc::new(Inner { hub: Hub::default(), config, http, live: crate::live::Live::new(store) }))
    }
}

impl std::ops::Deref for AppState {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.0
    }
}

/// The current local time, as `app.js` expects it: ISO 8601 with an offset.
/// The frontend slices characters 11..19 for HH:MM:SS, which this keeps.
pub fn now() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, false)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
