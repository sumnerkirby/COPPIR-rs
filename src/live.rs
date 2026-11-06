//! Live exercises: in-memory state for each open exercise, kept in step with
//! the store, plus the broadcast channel its clients listen on.
//!
//! For every command, under the exercise's lock: decide → number the events →
//! commit them to SQLite → apply → broadcast. Committing before broadcasting
//! means no client ever sees an event that a crash could take back, and doing
//! it all under one lock means every client sees events in `seq` order.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
};

use axum::extract::ws::Utf8Bytes;
use serde::Serialize;
use tokio::sync::broadcast;
use ts_rs::TS;

use crate::{
    error::ApiError,
    exercise::{Command, Envelope, Event, Exercise, decide},
    state::now,
    store::Store,
};

/// Clients further behind than this get a fresh snapshot instead of replay.
pub const MAX_CATCH_UP: u64 = 1000;
const CHANNEL_CAPACITY: usize = 1024;

/// What the server sends down a v2 websocket.
#[derive(Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export, rename = "ServerMessage")]
pub enum ServerMessage<'a> {
    /// The whole exercise as of `seq`.
    Snapshot {
        #[ts(type = "number")]
        seq: u64,
        exercise: &'a Exercise,
    },
    Event(&'a Envelope),
}

impl ServerMessage<'_> {
    pub fn encode(&self) -> Utf8Bytes {
        serde_json::to_string(self).expect("messages always serialize").into()
    }
}

/// The result of a command: the events it produced, already applied.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct Applied {
    #[ts(type = "number")]
    pub seq: u64,
    pub events: Vec<Envelope>,
}

struct RoomState {
    exercise: Exercise,
    seq: u64,
}

pub struct Room {
    state: Mutex<RoomState>,
    tx: broadcast::Sender<Utf8Bytes>,
}

impl Room {
    fn lock(&self) -> MutexGuard<'_, RoomState> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}

pub struct Live {
    store: Store,
    rooms: Mutex<HashMap<String, Arc<Room>>>,
}

impl Live {
    pub fn new(store: Store) -> Self {
        Self { store, rooms: Mutex::default() }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    fn rooms(&self) -> MutexGuard<'_, HashMap<String, Arc<Room>>> {
        self.rooms.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The open room for an exercise, loading it from the store on first use.
    pub fn room(&self, id: &str) -> Result<Arc<Room>, ApiError> {
        let mut rooms = self.rooms();
        if let Some(room) = rooms.get(id) {
            return Ok(room.clone());
        }
        let (exercise, seq) = self.store.load(id)?.ok_or(ApiError::NotFound("Exercise not found"))?;
        let room = Arc::new(Room {
            state: Mutex::new(RoomState { exercise, seq }),
            tx: broadcast::channel(CHANNEL_CAPACITY).0,
        });
        rooms.insert(id.to_owned(), room.clone());
        Ok(room)
    }

    /// Store a new exercise, starting from `exercise` as its genesis state.
    pub fn create(&self, exercise: Exercise, actor: &str) -> Result<(Exercise, u64), ApiError> {
        let genesis = Envelope { seq: 1, at: now(), actor: actor.to_owned(), event: Event::Created { exercise } };
        let Event::Created { exercise } = &genesis.event else { unreachable!() };
        let mut ex = exercise.clone();
        ex.apply(&genesis);
        self.store.create(&ex, std::slice::from_ref(&genesis))?;
        Ok((ex, 1))
    }

    pub fn delete(&self, id: &str) -> Result<(), ApiError> {
        if !self.store.delete(id)? {
            return Err(ApiError::NotFound("Exercise not found"));
        }
        // Dropping the room closes its channel, which disconnects its clients.
        self.rooms().remove(id);
        Ok(())
    }

    pub fn snapshot(&self, id: &str) -> Result<(Exercise, u64), ApiError> {
        let room = self.room(id)?;
        let st = room.lock();
        Ok((st.exercise.clone(), st.seq))
    }

    pub fn execute(&self, id: &str, actor: &str, cmd: Command) -> Result<Applied, ApiError> {
        let room = self.room(id)?;
        let mut st = room.lock();
        let at = now();
        let events = decide(&st.exercise, cmd, &at)?;
        if events.is_empty() {
            return Ok(Applied { seq: st.seq, events: vec![] });
        }
        let envelopes: Vec<Envelope> = events
            .into_iter()
            .zip(st.seq + 1..)
            .map(|(event, seq)| Envelope { seq, at: at.clone(), actor: actor.to_owned(), event })
            .collect();

        // Apply to a copy first: if the commit fails, memory stays as it was.
        let mut next = st.exercise.clone();
        for env in &envelopes {
            next.apply(env);
        }
        self.store.append(&next, &envelopes)?;
        st.exercise = next;
        st.seq = envelopes.last().map_or(st.seq, |e| e.seq);

        for env in &envelopes {
            let _ = room.tx.send(ServerMessage::Event(env).encode());
        }
        Ok(Applied { seq: st.seq, events: envelopes })
    }

    /// Messages that bring a client from `after` to now, and a subscription to
    /// everything after that — both taken under the room lock, so nothing is
    /// missed or repeated. With no `after`, or one too far back, the client
    /// gets a snapshot.
    pub fn subscribe(
        &self,
        id: &str,
        after: Option<u64>,
    ) -> Result<(Vec<Utf8Bytes>, broadcast::Receiver<Utf8Bytes>), ApiError> {
        let room = self.room(id)?;
        let st = room.lock();
        let rx = room.tx.subscribe();
        let catch_up = match after {
            Some(a) if a <= st.seq && st.seq - a <= MAX_CATCH_UP => {
                let events = self.store.events_after(id, a)?;
                Some(events.iter().map(|e| ServerMessage::Event(e).encode()).collect())
            }
            _ => None,
        };
        let messages =
            catch_up.unwrap_or_else(|| vec![ServerMessage::Snapshot { seq: st.seq, exercise: &st.exercise }.encode()]);
        Ok((messages, rx))
    }
}
