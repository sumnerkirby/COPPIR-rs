//! Durable storage: one SQLite file holding every exercise's event log.
//!
//! - `events` is append-only. An exercise's state is the fold of its events.
//! - `snapshots` caches that fold every [`SNAPSHOT_EVERY`] events, so loading
//!   a long exercise replays a few hundred events rather than all of them.
//! - `exercises` is an index for listing, kept in the same transaction as the
//!   events it describes.
//!
//! WAL mode with `synchronous=NORMAL`: a committed event survives an app crash;
//! only a power cut can lose the last few milliseconds.

use std::{path::Path, sync::Mutex};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;

use crate::exercise::{Envelope, Exercise};

pub const SNAPSHOT_EVERY: u64 = 250;

const SCHEMA_VERSION: i32 = 1;

const SCHEMA: &str = "
CREATE TABLE exercises (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    head        INTEGER NOT NULL,
    asset_count INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE events (
    exercise_id TEXT NOT NULL REFERENCES exercises(id) ON DELETE CASCADE,
    seq         INTEGER NOT NULL,
    at          TEXT NOT NULL,
    actor       TEXT NOT NULL,
    body        TEXT NOT NULL,
    PRIMARY KEY (exercise_id, seq)
) WITHOUT ROWID;
CREATE TABLE snapshots (
    exercise_id TEXT NOT NULL REFERENCES exercises(id) ON DELETE CASCADE,
    seq         INTEGER NOT NULL,
    state       TEXT NOT NULL,
    PRIMARY KEY (exercise_id, seq)
) WITHOUT ROWID;
";

#[derive(Debug)]
pub struct StoreError(String);

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        Self(format!("database: {e}"))
    }
}
impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        Self(format!("stored data: {e}"))
    }
}

impl From<StoreError> for crate::error::ApiError {
    fn from(e: StoreError) -> Self {
        Self::Internal(e.0)
    }
}

type Result<T> = std::result::Result<T, StoreError>;

#[derive(Clone, Debug, Serialize, TS)]
#[ts(export)]
pub struct ExerciseSummary {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    #[ts(type = "number")]
    pub head: u64,
    #[ts(type = "number")]
    pub asset_count: u64,
}

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: i32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        match version {
            0 => {
                conn.execute_batch(SCHEMA)?;
                conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            }
            SCHEMA_VERSION => {}
            v => return Err(StoreError(format!("database is from a newer COPPIR (schema {v}); upgrade the app"))),
        }
        Ok(Self { conn: Mutex::new(conn) })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Store a new exercise's first events. Fails if the id exists.
    pub fn create(&self, ex: &Exercise, events: &[Envelope]) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO exercises (id, name, created_at, updated_at, head, asset_count) VALUES (?1, ?2, ?3, ?4, 0, 0)",
            params![ex.id, ex.name, ex.created_at, ex.updated_at],
        )?;
        append_tx(&tx, ex, events)?;
        tx.commit()?;
        Ok(())
    }

    /// Append events and update the index, atomically. `ex` is the state
    /// *after* applying them.
    pub fn append(&self, ex: &Exercise, events: &[Envelope]) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        append_tx(&tx, ex, events)?;
        tx.commit()?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<ExerciseSummary>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, created_at, updated_at, head, asset_count FROM exercises ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(ExerciseSummary {
                id: r.get(0)?,
                name: r.get(1)?,
                created_at: r.get(2)?,
                updated_at: r.get(3)?,
                head: r.get::<_, i64>(4)? as u64,
                asset_count: r.get::<_, i64>(5)? as u64,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Rebuild an exercise: the latest snapshot, then every event after it.
    pub fn load(&self, id: &str) -> Result<Option<(Exercise, u64)>> {
        let conn = self.conn();
        let snapshot: Option<(u64, String)> = conn
            .query_row("SELECT seq, state FROM snapshots WHERE exercise_id = ?1 ORDER BY seq DESC LIMIT 1", [id], |r| {
                Ok((r.get::<_, i64>(0)? as u64, r.get(1)?))
            })
            .optional()?;
        let (mut ex, mut seq) = match snapshot {
            Some((seq, state)) => (Some(serde_json::from_str::<Exercise>(&state)?), seq),
            None => (None, 0),
        };
        for env in events_after(&conn, id, seq)? {
            match &mut ex {
                Some(ex) => ex.apply(&env),
                None => ex = Exercise::replay([&env]),
            }
            seq = env.seq;
        }
        Ok(ex.map(|ex| (ex, seq)))
    }

    pub fn events_after(&self, id: &str, after: u64) -> Result<Vec<Envelope>> {
        events_after(&self.conn(), id, after)
    }

    pub fn delete(&self, id: &str) -> Result<bool> {
        Ok(self.conn().execute("DELETE FROM exercises WHERE id = ?1", [id])? > 0)
    }
}

fn append_tx(tx: &rusqlite::Transaction<'_>, ex: &Exercise, events: &[Envelope]) -> Result<()> {
    let Some(last) = events.last() else { return Ok(()) };
    {
        let mut insert =
            tx.prepare_cached("INSERT INTO events (exercise_id, seq, at, actor, body) VALUES (?1, ?2, ?3, ?4, ?5)")?;
        for env in events {
            insert.execute(params![ex.id, env.seq as i64, env.at, env.actor, serde_json::to_string(&env.event)?])?;
        }
    }
    tx.execute(
        "UPDATE exercises SET name = ?2, updated_at = ?3, head = ?4, asset_count = ?5 WHERE id = ?1",
        params![ex.id, ex.name, ex.updated_at, last.seq as i64, ex.assets.len() as i64],
    )?;
    // Snapshot whenever this batch crossed a multiple of SNAPSHOT_EVERY.
    let first = events[0].seq;
    if last.seq / SNAPSHOT_EVERY > (first - 1) / SNAPSHOT_EVERY {
        tx.execute(
            "INSERT OR REPLACE INTO snapshots (exercise_id, seq, state) VALUES (?1, ?2, ?3)",
            params![ex.id, last.seq as i64, serde_json::to_string(ex)?],
        )?;
        // Two are plenty: the newest, and one behind it in case it is damaged.
        tx.execute(
            "DELETE FROM snapshots WHERE exercise_id = ?1 AND seq NOT IN \
             (SELECT seq FROM snapshots WHERE exercise_id = ?1 ORDER BY seq DESC LIMIT 2)",
            [&ex.id],
        )?;
    }
    Ok(())
}

fn events_after(conn: &Connection, id: &str, after: u64) -> Result<Vec<Envelope>> {
    let mut stmt = conn
        .prepare_cached("SELECT seq, at, actor, body FROM events WHERE exercise_id = ?1 AND seq > ?2 ORDER BY seq")?;
    let rows = stmt.query_map(params![id, after as i64], |r| {
        Ok((r.get::<_, i64>(0)? as u64, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (seq, at, actor, body) = row?;
        out.push(Envelope { seq, at, actor, event: serde_json::from_str(&body)? });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exercise::{Command, Event, blank, decide};

    fn envelope(seq: u64, event: Event) -> Envelope {
        Envelope { seq, at: format!("t{seq}"), actor: "test".into(), event }
    }

    /// Create an exercise and run `n` log commands through decide/apply/append.
    fn build(store: &Store, n: usize) -> (Exercise, u64) {
        let ex0 = blank("Drill", "", "t0").unwrap();
        let genesis = envelope(1, Event::Created { exercise: Box::new(ex0.clone()) });
        let mut ex = ex0.clone();
        ex.apply(&genesis);
        store.create(&ex, &[genesis]).unwrap();
        let mut seq = 1;
        for i in 0..n {
            let events =
                decide(&ex, Command::Log { text: format!("entry {i}"), asset: None, notes: None }, "now").unwrap();
            let envs: Vec<_> = events
                .into_iter()
                .map(|e| {
                    seq += 1;
                    envelope(seq, e)
                })
                .collect();
            for e in &envs {
                ex.apply(e);
            }
            store.append(&ex, &envs).unwrap();
        }
        (ex, seq)
    }

    #[test]
    fn load_matches_live_state_with_and_without_snapshots() {
        for n in [0, 3, SNAPSHOT_EVERY as usize + 7, 2 * SNAPSHOT_EVERY as usize] {
            let store = Store::open_in_memory().unwrap();
            let (live, seq) = build(&store, n);
            let (loaded, loaded_seq) = store.load(&live.id).unwrap().unwrap();
            assert_eq!(loaded_seq, seq, "n={n}");
            assert_eq!(loaded, live, "n={n}");
            // A full replay from the log agrees with the snapshot path too.
            let all = store.events_after(&live.id, 0).unwrap();
            assert_eq!(Exercise::replay(&all).unwrap(), live, "n={n}");
        }
    }

    #[test]
    fn keeps_at_most_two_snapshots() {
        let store = Store::open_in_memory().unwrap();
        let (live, _) = build(&store, 5 * SNAPSHOT_EVERY as usize);
        let n: i64 = store
            .conn()
            .query_row("SELECT COUNT(*) FROM snapshots WHERE exercise_id = ?1", [&live.id], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn list_and_delete() {
        let store = Store::open_in_memory().unwrap();
        let (live, seq) = build(&store, 2);
        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!((list[0].name.as_str(), list[0].head), ("Drill", seq));
        assert!(store.delete(&live.id).unwrap());
        assert!(store.load(&live.id).unwrap().is_none());
        assert!(store.events_after(&live.id, 0).unwrap().is_empty());
    }

    #[test]
    fn survives_reopening_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("coppir.db");
        let (live, _) = build(&Store::open(&path).unwrap(), 10);
        let (loaded, _) = Store::open(&path).unwrap().load(&live.id).unwrap().unwrap();
        assert_eq!(loaded, live);
    }
}
