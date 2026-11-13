//! Room access: who is in the exercise besides the facilitator.
//!
//! The facilitator is whoever is on this machine. Everyone else joins from
//! their own device with a short code and gets a session: a player (optionally
//! on a team) or an observer. Sessions live in memory; after a restart people
//! rejoin with the code.
//!
//! Nothing listens on the network until the facilitator opens the room. Then
//! one listener per LAN address serves the same app, and closes again with
//! the room.

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use serde::Serialize;
use tokio::sync::{Notify, watch};
use ts_rs::TS;

use crate::{error::ApiError, exercise::view::Audience, state::new_id};

pub const COOKIE: &str = "coppir_session";

/// Join attempts allowed per address per minute, right or wrong.
const ATTEMPTS_PER_MINUTE: u32 = 12;
/// No lookalikes: no 0/O, 1/I/L.
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const CODE_LEN: usize = 6;
const MAX_SESSIONS: usize = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Role {
    Facilitator,
    Player,
    Observer,
}

pub struct Session {
    /// The secret in the cookie.
    pub token: String,
    /// What the facilitator's participant list refers to it by.
    pub id: String,
    pub exercise_id: String,
    pub role: Role,
    pub joined_at: String,
    state: Mutex<SessionState>,
    /// Open websockets, so the list can show who is actually connected.
    pub connections: AtomicUsize,
    /// Fired when the session is kicked or moved to another team: its
    /// websockets close, and a moved client reconnects into its new view.
    pub changed: Notify,
}

struct SessionState {
    name: String,
    team: Option<String>,
}

impl Session {
    fn state(&self) -> MutexGuard<'_, SessionState> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
    pub fn name(&self) -> String {
        self.state().name.clone()
    }
    pub fn team(&self) -> Option<String> {
        self.state().team.clone()
    }
    pub fn audience(&self) -> Audience {
        match self.role {
            Role::Facilitator => Audience::Facilitator,
            Role::Observer => Audience::Observer,
            Role::Player => Audience::Player { team: self.team() },
        }
    }
}

/// A participant, as the facilitator sees them.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct Participant {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub team: Option<String>,
    pub connected: bool,
    pub joined_at: String,
}

/// The room as the facilitator sees it.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct RoomStatus {
    pub open: Option<OpenRoom>,
    pub participants: Vec<Participant>,
}

#[derive(Clone, Serialize, TS)]
#[ts(export)]
pub struct OpenRoom {
    pub exercise_id: String,
    pub player_code: String,
    pub observer_code: String,
    /// Addresses other devices can reach, e.g. `http://192.168.1.20:8000`.
    /// Empty when this machine has no network.
    pub urls: Vec<String>,
}

struct Open {
    info: OpenRoom,
    /// Dropping this stops the LAN listeners.
    _stop: watch::Sender<()>,
}

#[derive(Default)]
struct Inner {
    open: Option<Open>,
    sessions: HashMap<String, Arc<Session>>,
    attempts: HashMap<IpAddr, (Instant, u32)>,
}

#[derive(Default)]
pub struct Rooms {
    inner: Mutex<Inner>,
}

fn new_code() -> String {
    // 128 random bits, taken five at a time: plenty for six characters.
    let bytes = uuid::Uuid::new_v4().into_bytes();
    bytes.iter().take(CODE_LEN).map(|b| CODE_ALPHABET[*b as usize % CODE_ALPHABET.len()] as char).collect()
}

/// Uppercase, without the spaces or dashes people add when reading a code out.
pub fn normalize_code(code: &str) -> String {
    code.chars().filter(|c| !c.is_whitespace() && *c != '-').map(|c| c.to_ascii_uppercase()).collect()
}

impl Rooms {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn session(&self, token: &str) -> Option<Arc<Session>> {
        self.lock().sessions.get(token).cloned()
    }

    /// `ip:port` strings the LAN listeners answer on, for the host guard.
    pub fn lan_hosts(&self) -> Vec<String> {
        self.lock()
            .open
            .as_ref()
            .map(|o| o.info.urls.iter().filter_map(|u| u.strip_prefix("http://").map(str::to_owned)).collect())
            .unwrap_or_default()
    }

    pub fn status(&self) -> RoomStatus {
        let inner = self.lock();
        let mut participants: Vec<Participant> = inner
            .sessions
            .values()
            .map(|s| Participant {
                id: s.id.clone(),
                name: s.name(),
                role: s.role,
                team: s.team(),
                connected: s.connections.load(Ordering::Relaxed) > 0,
                joined_at: s.joined_at.clone(),
            })
            .collect();
        participants.sort_by(|a, b| a.joined_at.cmp(&b.joined_at));
        RoomStatus { open: inner.open.as_ref().map(|o| o.info.clone()), participants }
    }

    /// Mark the room open for `exercise_id`. The caller starts the listeners
    /// and passes the addresses they are on, with the handle that stops them.
    pub fn open(&self, exercise_id: &str, urls: Vec<String>, stop: watch::Sender<()>) -> OpenRoom {
        let mut inner = self.lock();
        // Reopening for another exercise ends everyone's session in the old one.
        if inner.open.as_ref().is_some_and(|o| o.info.exercise_id != exercise_id) {
            Self::drop_sessions(&mut inner);
        }
        let codes = inner.open.as_ref().filter(|o| o.info.exercise_id == exercise_id).map(|o| o.info.clone());
        let info = OpenRoom {
            exercise_id: exercise_id.to_owned(),
            player_code: codes.as_ref().map_or_else(new_code, |c| c.player_code.clone()),
            observer_code: codes.as_ref().map_or_else(new_code, |c| c.observer_code.clone()),
            urls,
        };
        inner.open = Some(Open { info: info.clone(), _stop: stop });
        info
    }

    /// Close the room: stop listening and end every session.
    pub fn close(&self) {
        let mut inner = self.lock();
        inner.open = None;
        Self::drop_sessions(&mut inner);
    }

    fn drop_sessions(inner: &mut Inner) {
        for (_, s) in inner.sessions.drain() {
            s.changed.notify_waiters();
        }
    }

    /// New codes; people already in stay in.
    pub fn rotate_codes(&self) -> Result<OpenRoom, ApiError> {
        let mut inner = self.lock();
        let open = inner.open.as_mut().ok_or(ApiError::Conflict("The room is not open"))?;
        open.info.player_code = new_code();
        open.info.observer_code = new_code();
        Ok(open.info.clone())
    }

    fn throttle(inner: &mut Inner, ip: IpAddr) -> Result<(), ApiError> {
        let now = Instant::now();
        inner.attempts.retain(|_, (since, _)| now.duration_since(*since) < Duration::from_secs(60));
        let entry = inner.attempts.entry(ip).or_insert((now, 0));
        entry.1 += 1;
        if entry.1 > ATTEMPTS_PER_MINUTE {
            return Err(ApiError::TooManyRequests("Too many attempts; wait a minute and try again"));
        }
        Ok(())
    }

    /// Which exercise and role a code admits to, if any.
    pub fn check_code(&self, code: &str, ip: IpAddr) -> Result<(String, Role), ApiError> {
        let mut inner = self.lock();
        Self::throttle(&mut inner, ip)?;
        let code = normalize_code(code);
        let open = inner.open.as_ref().ok_or(ApiError::NotFound("That code is not valid"))?;
        let role = if code == open.info.player_code {
            Role::Player
        } else if code == open.info.observer_code {
            Role::Observer
        } else {
            return Err(ApiError::NotFound("That code is not valid"));
        };
        Ok((open.info.exercise_id.clone(), role))
    }

    pub fn join(
        &self,
        exercise_id: String,
        role: Role,
        name: String,
        team: Option<String>,
        now: String,
    ) -> Result<Arc<Session>, ApiError> {
        let mut inner = self.lock();
        if inner.sessions.len() >= MAX_SESSIONS {
            return Err(ApiError::Conflict("The room is full"));
        }
        let session = Arc::new(Session {
            token: format!("{}{}", new_id(), new_id()).replace('-', ""),
            id: new_id(),
            exercise_id,
            role,
            joined_at: now,
            state: Mutex::new(SessionState { name, team }),
            connections: AtomicUsize::new(0),
            changed: Notify::new(),
        });
        inner.sessions.insert(session.token.clone(), session.clone());
        Ok(session)
    }

    fn by_id(inner: &Inner, id: &str) -> Result<Arc<Session>, ApiError> {
        inner.sessions.values().find(|s| s.id == id).cloned().ok_or(ApiError::NotFound("Participant not found"))
    }

    /// Rename a participant or move them to another team.
    pub fn update(&self, id: &str, name: Option<String>, team: Option<Option<String>>) -> Result<(), ApiError> {
        let inner = self.lock();
        let s = Self::by_id(&inner, id)?;
        let mut st = s.state();
        if let Some(n) = name {
            st.name = n;
        }
        if let Some(t) = team {
            if st.team != t {
                st.team = t;
                // Their view is now a different one: make them reconnect into it.
                s.changed.notify_waiters();
            }
        }
        Ok(())
    }

    pub fn kick(&self, id: &str) -> Result<(), ApiError> {
        let mut inner = self.lock();
        let s = Self::by_id(&inner, id)?;
        inner.sessions.remove(&s.token);
        s.changed.notify_waiters();
        Ok(())
    }

    pub fn leave(&self, token: &str) {
        if let Some(s) = self.lock().sessions.remove(token) {
            s.changed.notify_waiters();
        }
    }
}

/// This machine's addresses on the local network: private IPv4 first, since
/// that is what a phone on the same Wi-Fi can reach.
pub fn lan_addresses() -> Vec<IpAddr> {
    let mut ips: Vec<IpAddr> = local_ip_address::list_afinet_netifas()
        .unwrap_or_default()
        .into_iter()
        .map(|(_, ip)| ip)
        .filter(|ip| match ip {
            IpAddr::V4(v4) => !v4.is_loopback() && !v4.is_link_local() && !v4.is_unspecified(),
            IpAddr::V6(_) => false,
        })
        .collect();
    ips.sort_by_key(|ip| match ip {
        IpAddr::V4(v4) if v4.is_private() => 0,
        _ => 1,
    });
    ips.dedup();
    ips
}

pub fn url_for(addr: SocketAddr) -> String {
    format!("http://{addr}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(n: u8) -> IpAddr {
        IpAddr::from([10, 0, 0, n])
    }

    fn open(rooms: &Rooms, exercise: &str) -> OpenRoom {
        rooms.open(exercise, vec!["http://192.168.1.20:8000".into()], watch::channel(()).0)
    }

    #[test]
    fn codes_are_unambiguous_and_forgiving() {
        for _ in 0..200 {
            let code = new_code();
            assert_eq!(code.len(), CODE_LEN);
            assert!(code.bytes().all(|b| CODE_ALPHABET.contains(&b)), "{code}");
        }
        assert_eq!(normalize_code(" abc-d2f "), "ABCD2F");
    }

    #[test]
    fn codes_admit_to_the_right_role() {
        let rooms = Rooms::default();
        assert!(rooms.check_code("ABCDEF", ip(1)).is_err(), "closed room admits nobody");
        let room = open(&rooms, "ex1");
        assert_ne!(room.player_code, room.observer_code);
        assert_eq!(rooms.check_code(&room.player_code.to_lowercase(), ip(1)).unwrap(), ("ex1".into(), Role::Player));
        assert_eq!(rooms.check_code(&room.observer_code, ip(1)).unwrap().1, Role::Observer);
        assert!(rooms.check_code("WRONG1", ip(1)).is_err());
        assert_eq!(rooms.lan_hosts(), ["192.168.1.20:8000"]);
    }

    #[test]
    fn guessing_is_throttled_per_address() {
        let rooms = Rooms::default();
        let room = open(&rooms, "ex1");
        for _ in 0..ATTEMPTS_PER_MINUTE {
            let _ = rooms.check_code("WRONG1", ip(1));
        }
        assert!(matches!(rooms.check_code(&room.player_code, ip(1)), Err(ApiError::TooManyRequests(_))));
        // Someone else is unaffected.
        assert!(rooms.check_code(&room.player_code, ip(2)).is_ok());
    }

    #[test]
    fn sessions_end_when_kicked_or_the_room_closes() {
        let rooms = Rooms::default();
        open(&rooms, "ex1");
        let a = rooms.join("ex1".into(), Role::Player, "Dana".into(), None, "t1".into()).unwrap();
        let b = rooms.join("ex1".into(), Role::Observer, "Screen".into(), None, "t2".into()).unwrap();
        assert_eq!(rooms.status().participants.len(), 2);
        assert!(rooms.session(&a.token).is_some());

        rooms.update(&a.id, Some("Dana K".into()), Some(Some("team1".into()))).unwrap();
        assert_eq!(a.audience(), Audience::Player { team: Some("team1".into()) });
        assert_eq!(rooms.status().participants[0].name, "Dana K");

        rooms.kick(&a.id).unwrap();
        assert!(rooms.session(&a.token).is_none());
        assert!(rooms.kick(&a.id).is_err());

        rooms.close();
        assert!(rooms.session(&b.token).is_none());
        assert!(rooms.status().open.is_none());
    }

    #[test]
    fn reopening_keeps_codes_for_the_same_exercise_only() {
        let rooms = Rooms::default();
        let first = open(&rooms, "ex1");
        let s = rooms.join("ex1".into(), Role::Player, "Dana".into(), None, "t".into()).unwrap();
        let again = open(&rooms, "ex1");
        assert_eq!(first.player_code, again.player_code);
        assert!(rooms.session(&s.token).is_some());
        let other = open(&rooms, "ex2");
        assert_ne!(other.player_code, first.player_code);
        assert!(rooms.session(&s.token).is_none(), "sessions do not carry over to another exercise");
        assert_ne!(rooms.rotate_codes().unwrap().player_code, other.player_code);
    }
}
