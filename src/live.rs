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
    exercise::{
        Command, Envelope, Event, Exercise, decide, engine,
        undo::{inverse, still_current},
        view::{Audience, diff, project},
    },
    state::now,
    store::Store,
};

/// Clients further behind than this get a fresh snapshot instead of replay.
pub const MAX_CATCH_UP: u64 = 1000;
const CHANNEL_CAPACITY: usize = 1024;
/// The actor recorded for what the script engine does on its own.
pub const ENGINE: &str = "engine";
const ENGINE_ROUNDS: usize = 50;

/// Bring each connected audience's view up to date and send it the
/// difference. Views nobody is listening to are dropped.
fn publish_views(st: &mut RoomState, at: &str) {
    let RoomState { exercise, views, seq, .. } = st;
    views.retain(|_, v| v.tx.receiver_count() > 0);
    for (audience, view) in views.iter_mut() {
        let next = project(exercise, audience);
        let events = diff(&view.exercise, &next);
        view.exercise = next;
        if !events.is_empty() {
            let _ = view.tx.send(ServerMessage::Patch { seq: *seq, at, events: &events }.encode());
        }
    }
}

/// How many changes back undo can reach.
pub const UNDO_DEPTH: usize = 100;

/// One undoable change: the events it produced and the events that reverse it.
struct UndoEntry {
    seqs: Vec<u64>,
    events: Vec<Event>,
    inverse: Vec<Event>,
}

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
    /// For players and observers: what changed in *their* view. Applied in
    /// order; `seq` says how far the exercise has moved on.
    Patch {
        #[ts(type = "number")]
        seq: u64,
        at: &'a str,
        events: &'a [Event],
    },
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
    /// In memory only: after a restart there is nothing to undo, though the
    /// full history is still in the log.
    undo: Vec<UndoEntry>,
    /// One projection per audience that has someone connected.
    views: HashMap<Audience, View>,
}

/// What one audience currently sees, and the channel its clients listen on.
struct View {
    exercise: Exercise,
    tx: broadcast::Sender<Utf8Bytes>,
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
            state: Mutex::new(RoomState { exercise, seq, undo: Vec::new(), views: HashMap::new() }),
            tx: broadcast::channel(CHANNEL_CAPACITY).0,
        });
        rooms.insert(id.to_owned(), room.clone());
        Ok(room)
    }

    /// Store a new exercise, starting from `exercise` as its genesis state.
    pub fn create(&self, exercise: Exercise, actor: &str) -> Result<(Exercise, u64), ApiError> {
        let genesis = Envelope {
            seq: 1,
            at: now(),
            actor: actor.to_owned(),
            event: Event::Created { exercise: Box::new(exercise) },
        };
        let Event::Created { exercise } = &genesis.event else { unreachable!() };
        let mut ex = (**exercise).clone();
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
        self.execute_with(id, actor, cmd, |_| {})
    }

    /// Like [`execute`](Self::execute), with a chance to adjust the events
    /// before they are stored. Used to stamp a player's log entry with who
    /// wrote it, which the player's own request cannot be trusted to say.
    pub fn execute_with(
        &self,
        id: &str,
        actor: &str,
        cmd: Command,
        adjust: impl FnOnce(&mut Vec<Event>),
    ) -> Result<Applied, ApiError> {
        let room = self.room(id)?;
        let mut st = room.lock();
        let at = now();
        let undoing = matches!(cmd, Command::Undo);
        let events = if undoing {
            let entry = st.undo.pop().ok_or(ApiError::Conflict("Nothing to undo"))?;
            if !still_current(&st.exercise, &entry.events) {
                // Put it back: it may become undoable again if the later change is undone first.
                st.undo.push(entry);
                return Err(ApiError::Conflict("Something has changed since; undo that first"));
            }
            let mut events = vec![Event::Undone { seqs: entry.seqs.clone() }];
            events.extend(entry.inverse);
            events
        } else {
            let mut events = decide(&st.exercise, cmd, &at)?;
            adjust(&mut events);
            events
        };
        if events.is_empty() {
            return Ok(Applied { seq: st.seq, events: vec![] });
        }
        let envelopes = self.commit(id, &room, &mut st, actor, events, &at, !undoing)?;
        // Whatever this change set off: conditional injects, a chain of them.
        self.run_engine(id, &room, &mut st);
        publish_views(&mut st, &at);
        Ok(Applied { seq: envelopes.last().map_or(st.seq, |e| e.seq), events: envelopes })
    }

    /// Number, apply, store and broadcast one batch of events, and remember
    /// how to undo it. On a failed write, memory is put back to what the disk
    /// holds and nothing is broadcast.
    #[allow(clippy::too_many_arguments)]
    fn commit(
        &self,
        id: &str,
        room: &Room,
        st: &mut RoomState,
        actor: &str,
        events: Vec<Event>,
        at: &str,
        undoable: bool,
    ) -> Result<Vec<Envelope>, ApiError> {
        let envelopes: Vec<Envelope> = events
            .into_iter()
            .zip(st.seq + 1..)
            .map(|(event, seq)| Envelope { seq, at: at.to_owned(), actor: actor.to_owned(), event })
            .collect();

        // Apply in place, taking each event's inverse from the state just
        // before it. Copying the whole exercise first would make every command
        // cost as much as the exercise is large.
        let mut inverse_events = Vec::new();
        for env in &envelopes {
            if undoable {
                let mut inv = inverse(&st.exercise, &env.event);
                inv.reverse();
                inverse_events.extend(inv);
            }
            st.exercise.apply(env);
        }
        inverse_events.reverse();
        if let Err(e) = self.store.append(&st.exercise, &envelopes) {
            match self.store.load(id) {
                Ok(Some((exercise, seq))) => {
                    st.exercise = exercise;
                    st.seq = seq;
                    st.undo.clear();
                }
                // Cannot read either: forget the room so the next request reloads it.
                _ => {
                    self.rooms().remove(id);
                }
            }
            return Err(e.into());
        }
        st.seq = envelopes.last().map_or(st.seq, |e| e.seq);
        if undoable && !inverse_events.is_empty() {
            if st.undo.len() == UNDO_DEPTH {
                st.undo.remove(0);
            }
            st.undo.push(UndoEntry {
                seqs: envelopes.iter().map(|e| e.seq).collect(),
                events: envelopes.iter().map(|e| e.event.clone()).collect(),
                inverse: inverse_events,
            });
        }
        for env in &envelopes {
            let _ = room.tx.send(ServerMessage::Event(env).encode());
        }
        Ok(envelopes)
    }

    /// Let the script engine act until it has nothing left to do. Each thing
    /// it does is its own change, recorded as `engine`, and undoable.
    fn run_engine(&self, id: &str, room: &Room, st: &mut RoomState) {
        // A chain of conditional injects resolves here; the cap only stops a
        // script that somehow keeps itself busy forever.
        for _ in 0..ENGINE_ROUNDS {
            let at = now();
            let due = engine::due(&st.exercise, &at);
            if due.is_empty() {
                return;
            }
            for cmd in due {
                match decide(&st.exercise, cmd, &at) {
                    Ok(events) if !events.is_empty() => {
                        if let Err(e) = self.commit(id, room, st, ENGINE, events, &at, true) {
                            tracing::error!("engine change not saved: {e}");
                            return;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!("engine skipped a change: {e}"),
                }
            }
        }
        tracing::warn!("the script engine was still busy after {ENGINE_ROUNDS} rounds");
    }

    /// Called once a second: fire what has come due on the clock, close polls
    /// whose time is up. Exercises with nothing timed waiting cost nothing.
    pub fn tick(&self) {
        let rooms: Vec<(String, Arc<Room>)> = self.rooms().iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for (id, room) in rooms {
            let mut st = room.lock();
            if !engine::waiting_on_time(&st.exercise) {
                continue;
            }
            let before = st.seq;
            self.run_engine(&id, &room, &mut st);
            if st.seq != before {
                publish_views(&mut st, &now());
            }
        }
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

    /// A snapshot of what `audience` may see, and a subscription to the
    /// patches that keep it current.
    pub fn subscribe_view(
        &self,
        id: &str,
        audience: &Audience,
    ) -> Result<(Utf8Bytes, broadcast::Receiver<Utf8Bytes>), ApiError> {
        let room = self.room(id)?;
        let mut st = room.lock();
        let RoomState { exercise, views, seq, .. } = &mut *st;
        let view = views.entry(audience.clone()).or_insert_with(|| View {
            exercise: project(exercise, audience),
            tx: broadcast::channel(CHANNEL_CAPACITY).0,
        });
        let rx = view.tx.subscribe();
        Ok((ServerMessage::Snapshot { seq: *seq, exercise: &view.exercise }.encode(), rx))
    }

    pub fn snapshot_for(&self, id: &str, audience: &Audience) -> Result<(Exercise, u64), ApiError> {
        let room = self.room(id)?;
        let st = room.lock();
        Ok((project(&st.exercise, audience), st.seq))
    }
}
